//! The popup: showing it above the taskbar widget (or the tray icon), and the menu actions both share.
//! The tray icon itself is only shown when the taskbar widget can't be (see widget.rs).
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, WindowEvent};

use crate::games::ordered;
use crate::store::Store;
use crate::time::{fmt_duration, now};

const TRAY_ID: &str = "main";
const POPUP: &str = "main";
/// A click on the tray icon while the popup is open first takes focus away (hiding it): don't then
/// show it again straight away.
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// True while the icon file picker is open, so the popup doesn't hide when it loses focus to it.
pub static DIALOG_OPEN: AtomicBool = AtomicBool::new(false);
static LAST_BLUR_HIDE: Mutex<Option<Instant>> = Mutex::new(None);
/// What the popup opens above: the taskbar widget, or the tray icon when that's in use.
static ANCHOR: Mutex<Option<Area>> = Mutex::new(None);
static OFFLINE_ITEM: OnceLock<CheckMenuItem<tauri::Wry>> = OnceLock::new();

/// The tray icon, and the same icon with a dot for "missed while you were away".
struct Icons {
    normal: Image<'static>,
    dot: Image<'static>,
}

static ICONS: OnceLock<Icons> = OnceLock::new();

/// A rectangle on screen, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Where the popup goes: centred above the tray icon, kept inside the work area (clear of the taskbar)
/// with a margin. If the tray icon's position isn't known yet, the work area's bottom-right corner.
pub fn place(tray: Option<Area>, work: Area, popup: (i32, i32)) -> (i32, i32) {
    const MARGIN: i32 = 12;
    let (w, h) = popup;
    let (left, top) = (work.x + MARGIN, work.y + MARGIN);
    let right = (work.x + work.w - w - MARGIN).max(left);
    let bottom = (work.y + work.h - h - MARGIN).max(top);
    match tray {
        Some(t) => ((t.x + t.w / 2 - w / 2).clamp(left, right), (t.y - h - MARGIN).clamp(top, bottom)),
        None => (right, bottom),
    }
}

fn with_dot(icon: &Image<'_>) -> Image<'static> {
    let (w, h) = (icon.width() as i64, icon.height() as i64);
    let mut rgba = icon.rgba().to_vec();
    let r = w.min(h) as f64 * 0.22;
    let (cx, cy) = (w as f64 - r - 1.0, h as f64 - r - 1.0);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt();
            let px = ((y * w + x) * 4) as usize;
            if d <= r {
                rgba[px..px + 4].copy_from_slice(&[0xE0, 0x24, 0x24, 0xFF]);
            } else if d <= r + 1.5 {
                rgba[px..px + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // outline, visible on any taskbar
            }
        }
    }
    Image::new_owned(rgba, w as u32, h as u32)
}

pub fn build(app: &AppHandle, store: &Arc<Store>) -> tauri::Result<()> {
    let base = app.default_window_icon().cloned().expect("bundle has an icon");
    let icons = ICONS.get_or_init(|| Icons { dot: with_dot(&base), normal: base.to_owned() });

    let offline = store.lock().settings.offline;
    let offline_item = CheckMenuItem::with_id(app, "offline", "Offline mode", true, offline, None::<&str>)?;
    let menu = Menu::with_items(app, &[
        &MenuItem::with_id(app, "open", "Open", true, None::<&str>)?,
        &MenuItem::with_id(app, "refresh", "Refresh banners", true, None::<&str>)?,
        &offline_item,
        &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
    ])?;
    let _ = OFFLINE_ITEM.set(offline_item);

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icons.normal.clone())
        .tooltip("GachaSlop")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { rect, button, button_state, .. } = event {
                let scale = tray.app_handle().primary_monitor().ok().flatten().map_or(1.0, |m| m.scale_factor());
                let (pos, size) = (rect.position.to_physical::<i32>(scale), rect.size.to_physical::<i32>(scale));
                set_anchor(Area { x: pos.x, y: pos.y, w: size.width, h: size.height });
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    toggle_popup(tray.app_handle());
                }
            }
        })
        .on_menu_event(|app, event| menu_action(app, event.id().as_ref()))
        .build(app)?;
    let _ = tray.set_visible(false); // the taskbar widget is used instead, when it can be

    if let Some(window) = app.get_webview_window(POPUP) {
        let popup = window.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::Focused(true) => DIALOG_OPEN.store(false, Ordering::SeqCst), // back from the picker
            WindowEvent::Focused(false) if !DIALOG_OPEN.load(Ordering::SeqCst) => {
                *LAST_BLUR_HIDE.lock().unwrap() = Some(Instant::now());
                let _ = popup.hide();
            }
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = popup.hide();
            }
            _ => {}
        });
    }
    Ok(())
}

/// The tray and widget menus: "open", "refresh", "offline", "settings" or "quit".
pub fn menu_action(app: &AppHandle, id: &str) {
    let store = app.state::<Arc<Store>>();
    match id {
        "open" => show_popup(app),
        "settings" => {
            show_popup(app);
            let _ = app.emit("open-settings", ());
        }
        "refresh" => {
            store.request_refresh();
            show_popup(app);
        }
        "offline" => {
            let mut inner = store.lock();
            inner.settings.offline = !inner.settings.offline;
            store.save_settings(&inner);
            drop(inner);
            store.wake();
            sync(app, &store);
            let _ = app.emit("state-changed", ());
        }
        "quit" => quit(app, &store),
        _ => {}
    }
}

/// Show the tray icon (when the taskbar widget can't be placed) or hide it.
pub fn show_tray(app: &AppHandle, visible: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(visible);
    }
}

/// What the popup opens above (the widget or tray icon last clicked), if known.
pub fn anchor() -> Option<Area> {
    *ANCHOR.lock().unwrap()
}

/// Where the popup is while it's open, in physical screen pixels.
pub fn popup_area(app: &AppHandle) -> Option<Area> {
    let window = app.get_webview_window(POPUP)?;
    if !window.is_visible().unwrap_or(false) {
        return None;
    }
    let (at, size) = (window.outer_position().ok()?, window.outer_size().ok()?);
    Some(Area { x: at.x, y: at.y, w: size.width as i32, h: size.height as i32 })
}

/// Set what the popup opens above, in physical screen pixels.
pub fn set_anchor(area: Area) {
    *ANCHOR.lock().unwrap() = Some(area);
}

pub fn toggle_popup(app: &AppHandle) {
    let Some(window) = app.get_webview_window(POPUP) else { return };
    let just_hidden = LAST_BLUR_HIDE.lock().unwrap().is_some_and(|t| t.elapsed() < REOPEN_GUARD);
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else if !just_hidden {
        show_popup(app);
    }
}

fn position(window: &WebviewWindow) {
    let tray = *ANCHOR.lock().unwrap();
    let monitor = match tray {
        Some(t) => window.monitor_from_point(t.x as f64, t.y as f64),
        None => window.primary_monitor(),
    };
    let (Ok(Some(monitor)), Ok(size)) = (monitor, window.outer_size()) else { return };
    let work = monitor.work_area();
    let area = Area { x: work.position.x, y: work.position.y, w: work.size.width as i32, h: work.size.height as i32 };
    let (x, y) = place(tray, area, (size.width as i32, size.height as i32));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

pub fn show_popup(app: &AppHandle) {
    let Some(window) = app.get_webview_window(POPUP) else { return };
    position(&window);
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    let store = app.state::<Arc<Store>>();
    let was_unseen = std::mem::replace(&mut store.lock().state.unseen, false);
    if was_unseen {
        store.save_state(&store.lock());
        sync(app, &store);
    }
    let _ = app.emit("state-changed", ());
}

pub fn hide_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(POPUP) {
        let _ = window.hide();
    }
}

/// Bring the tray up to date: tooltip (next reset), dot (missed items not yet seen), offline tick.
/// The store is unlocked before touching the tray: tray calls wait for the main thread, which may itself
/// be waiting for the store (commands run there).
pub fn sync(app: &AppHandle, store: &Store) {
    let Some(tray): Option<TrayIcon> = app.tray_by_id(TRAY_ID) else { return };
    let now = now();
    let (mut tip, unseen, offline) = {
        let inner = store.lock();
        let games = inner.enabled_games();
        let tip = match ordered(&games, now).first() {
            Some((g, at)) => format!("GachaSlop\nNext reset: {} in {}", g.short, fmt_duration(at - now)),
            None => "GachaSlop".to_string(),
        };
        (tip, inner.state.unseen, inner.settings.offline)
    };
    if unseen {
        tip.push_str("\nMissed while you were away");
    }
    if offline {
        tip.push_str("\nOffline mode");
    }
    let _ = tray.set_tooltip(Some(tip.chars().take(127).collect::<String>()));
    if let Some(icons) = ICONS.get() {
        let _ = tray.set_icon(Some(if unseen { icons.dot.clone() } else { icons.normal.clone() }));
    }
    if let Some(item) = OFFLINE_ITEM.get() {
        let _ = item.set_checked(offline);
    }
}

/// Save when the app was last running, then exit.
pub fn quit(app: &AppHandle, store: &Store) {
    let mut inner = store.lock();
    inner.state.last_seen = Some(now());
    store.save_state(&inner);
    drop(inner);
    app.exit(0);
}
