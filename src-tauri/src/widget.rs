//! The taskbar widget: a small strip on the Windows taskbar, just left of the clock, showing the next
//! reset of the two soonest games ("BA 02:15:07"). Click it for the popup, right-click for the menu.
//!
//! Same approach as Wireless Battery (github.com/krisgfx9/Wireless-Device-Battery-Monitor): a layered child
//! window of the taskbar, repainted and re-placed once a second, recreated when Explorer restarts, and on
//! Windows 10 the task buttons are narrowed so they never slide under it. When it can't be placed
//! (vertical or auto-hide taskbar, no taskbar) the tray icon is shown instead.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::games::{ordered, Game};
use crate::time::{fmt_clock, Ts};

// How the widget is shown (for Settings' "Testing" section), and the "Show as a tray icon" setting.
static STRIPS: AtomicUsize = AtomicUsize::new(0);
static TRAY_SHOWN: AtomicBool = AtomicBool::new(false);
static TRAY_ONLY: AtomicBool = AtomicBool::new(false);

/// How the widget is shown right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub strips: usize,     // taskbars with a strip
    pub tray_shown: bool,  // the tray icon is in use
    pub tray_only: bool,   // the "Show as a tray icon" setting
}

pub fn status() -> Status {
    Status { strips: STRIPS.load(Ordering::SeqCst), tray_shown: TRAY_SHOWN.load(Ordering::SeqCst),
             tray_only: TRAY_ONLY.load(Ordering::SeqCst) }
}

/// The widget's lines: (short game name, time to its next reset) for the two soonest games.
pub fn widget_lines(games: &[Game], now: Ts) -> Vec<(String, String)> {
    ordered(games, now).into_iter().take(2).map(|(g, at)| (label(&g.short), fmt_clock(at - now))).collect()
}

/// A name short enough for the taskbar: kept if up to 4 characters, otherwise the initials of its words
/// ("Honkai Star Rail" is "HSR"), or the first three letters of a single word ("Arknights" is "ARK").
pub fn label(name: &str) -> String {
    if name.chars().count() <= 4 {
        return name.to_string();
    }
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let short: String = if words.len() > 1 {
        words.iter().filter_map(|w| w.chars().next()).take(3).collect()
    } else {
        name.chars().filter(|c| c.is_alphanumeric()).take(3).collect()
    };
    short.to_uppercase()
}

/// How far apart the taskbar clock's time and date lines are at 100% scale (measured on Windows 10's
/// 40px taskbar: their digits start 18px apart).
pub const CLOCK_LINE_PITCH: i32 = 18;

/// Where each of `count` lines (`line_h` tall) starts in a strip `height` tall: `pitch` apart like the
/// clock's time and date, closer if the taskbar is too short for that, and centred as a block.
pub fn line_tops(height: i32, line_h: i32, count: usize, pitch: i32) -> Vec<i32> {
    let n = count as i32;
    let pitch = if n > 1 { pitch.min((height - line_h) / (n - 1)).max(line_h) } else { 0 };
    let top = (height - line_h - pitch * (n - 1).max(0)) / 2;
    (0..n).map(|i| top + pitch * i).collect()
}

/// Left edge (in taskbar client pixels) for a strip `width` wide ending `gap` pixels before `edge`.
pub fn strip_left(edge: i32, width: i32, gap: i32) -> i32 {
    edge - width - gap
}

/// Windows 10's task-button area, narrowed to end where the strip starts, if it currently reaches under it.
pub fn rebar_width(rebar_left: i32, rebar_width: i32, strip_left: i32) -> Option<i32> {
    (rebar_left < strip_left && rebar_left + rebar_width > strip_left).then_some(strip_left - rebar_left)
}

/// Text colour as (red, green, blue): dark on a light taskbar, white on a dark one.
pub fn text_colour(light_taskbar: bool) -> (u8, u8, u8) {
    if light_taskbar { (0x1C, 0x1C, 0x1C) } else { (0xFF, 0xFF, 0xFF) }
}

/// The strip's background, as premultiplied BGRA. Never fully transparent: Windows passes clicks on
/// alpha-0 pixels through to the taskbar, so alpha 1 (invisible) keeps the whole strip clickable. While
/// the mouse is over it, a faint wash shows it can be clicked.
pub fn background(light_taskbar: bool, hovered: bool) -> [u8; 4] {
    match (hovered, light_taskbar) {
        (false, _) => [0, 0, 0, 1],
        (true, false) => [26, 26, 26, 26], // white at about 10%
        (true, true) => [0, 0, 0, 20],     // black at about 8%
    }
}

/// Turn white-on-black text (BGRA pixels, as GDI draws it) into `colour` text over `background`, with
/// premultiplied alpha, so the layered window shows it smoothly over the taskbar.
pub fn premultiply(bgra: &mut [u8], (r, g, b): (u8, u8, u8), background: [u8; 4]) {
    for px in bgra.as_chunks_mut::<4>().0 {
        let a = px[0].max(px[1]).max(px[2]) as u32;
        let over = |text: u8, under: u8| ((text as u32 * a + under as u32 * (255 - a)) / 255) as u8;
        *px = [over(b, background[0]), over(g, background[1]), over(r, background[2]), over(255, background[3])];
    }
}

/// A keyboard shortcut: Windows' modifier flags and virtual-key code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub mods: u32,
    pub vk: u32,
}

const MOD_ALT: u32 = 1;
const MOD_CONTROL: u32 = 2;
const MOD_SHIFT: u32 = 4;
const MOD_WIN: u32 = 8;
const NAMED_KEYS: [(&str, u32); 10] = [("Space", 0x20), ("PageUp", 0x21), ("PageDown", 0x22), ("End", 0x23),
    ("Home", 0x24), ("Left", 0x25), ("Up", 0x26), ("Right", 0x27), ("Down", 0x28), ("Insert", 0x2D)];

/// Read a shortcut as Settings writes it ("Ctrl+Alt+G", "Ctrl+Shift+F9"); "" means none. It needs Ctrl, Alt
/// or Win (Shift alone isn't enough), except with a function key.
pub fn parse_hotkey(text: &str) -> Result<Option<Hotkey>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let (key, modifiers) = parts.split_last().ok_or("No key given.")?;
    let mut mods = 0;
    for m in modifiers {
        mods |= match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => MOD_CONTROL,
            "alt" => MOD_ALT,
            "shift" => MOD_SHIFT,
            "win" => MOD_WIN,
            _ => return Err(format!("\"{m}\" isn't Ctrl, Alt, Shift or Win.")),
        };
    }
    let upper = key.to_ascii_uppercase();
    let function = upper.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=24).contains(n));
    let vk = if let Some(n) = function {
        0x6F + n // F1 is 0x70
    } else if upper.len() == 1 && upper.chars().all(|c| c.is_ascii_alphanumeric()) {
        upper.chars().next().unwrap() as u32 // 'A'..'Z' and '0'..'9' are their own codes
    } else if let Some((_, vk)) = NAMED_KEYS.iter().find(|(name, _)| name.eq_ignore_ascii_case(key)) {
        *vk
    } else {
        return Err(format!("{key} can't be used in a shortcut."));
    };
    if function.is_none() && mods & (MOD_CONTROL | MOD_ALT | MOD_WIN) == 0 {
        return Err("Use Ctrl, Alt or Win in the shortcut, so it doesn't get in the way of typing.".into());
    }
    Ok(Some(Hotkey { mods, vk }))
}

#[cfg(windows)]
pub use win::{set_hotkey, start, stop, suspend_hotkey};

#[cfg(not(windows))]
pub fn set_hotkey(_text: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn suspend_hotkey(_suspended: bool) {}

/// Take the strips off every taskbar and show the tray icon instead, as when the taskbar auto-hides (or
/// put them back): the "Show as a tray icon" setting.
pub fn set_tray_only(on: bool) {
    TRAY_ONLY.store(on, Ordering::SeqCst);
    #[cfg(windows)]
    win::redraw();
}

#[cfg(not(windows))]
pub fn start(app: &tauri::AppHandle) {
    TRAY_SHOWN.store(true, Ordering::SeqCst);
    crate::tray::show_tray(app, true);
}

#[cfg(not(windows))]
pub fn stop() {}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::sync::{Arc, OnceLock};

    use tauri::{AppHandle, Manager};
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, TrackMouseEvent, UnregisterHotKey, MOD_NOREPEAT, TME_LEAVE, TRACKMOUSEEVENT,
    };
    use windows_sys::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABS_AUTOHIDE, APPBARDATA};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::{background, line_tops, parse_hotkey, premultiply, rebar_width, strip_left, text_colour, widget_lines,
                Hotkey, CLOCK_LINE_PITCH, STRIPS, TRAY_ONLY, TRAY_SHOWN};
    use crate::store::Store;
    use crate::time::now;
    use crate::tray::{self, Area};

    const OWNER_CLASS: &str = "GachaSlopWidgetOwner";
    const STRIP_CLASS: &str = "GachaSlopWidget";
    const TIMER_ID: usize = 1;
    // messages the strip posts to the owner window, so the work happens outside the taskbar's own window
    const SHOW_MENU: u32 = WM_APP + 1;
    const TOGGLE_POPUP: u32 = WM_APP + 2;
    const REPAINT: u32 = WM_APP + 3;
    /// Sent after TrackMouseEvent(TME_LEAVE) when the mouse leaves (not in the windows-sys modules used here).
    const WM_MOUSELEAVE: u32 = 0x02A3;
    const MENU_ITEMS: [(usize, &str, &str); 5] =
        [(1, "open", "Open"), (2, "refresh", "Refresh banners"), (3, "offline", "Offline mode"),
         (4, "settings", "Settings"), (5, "quit", "Quit")];

    /// Which taskbar a strip is on: the main one, or another screen's.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Bar {
        Main,
        Secondary,
    }

    /// One strip, on one taskbar. HWNDs are kept as integers: windows-sys handles are raw pointers.
    struct Strip {
        bar: usize,
        wnd: usize,
        kind: Bar,
        placed: (i32, i32, i32), // last left, width, height, to skip moves that change nothing
        hovered: bool,           // the mouse is over it
    }

    #[derive(Default)]
    struct Widget {
        owner: usize,
        strips: Vec<Strip>,
        tray_shown: Option<bool>,
    }

    thread_local! {
        static STATE: RefCell<Widget> = RefCell::new(Widget::default());
    }
    static APP: OnceLock<AppHandle> = OnceLock::new();
    static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
    static OWNER: AtomicUsize = AtomicUsize::new(0); // for redraw(), which may be called from any thread

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn hwnd(raw: usize) -> HWND {
        raw as HWND
    }

    fn find(parent: HWND, class: &str) -> HWND {
        unsafe { FindWindowExW(parent, null_mut(), wide(class).as_ptr(), null()) }
    }

    fn rect(window: HWND) -> RECT {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(window, &mut r) };
        r
    }

    fn dpi(bar: HWND) -> i32 {
        match unsafe { GetDpiForWindow(bar) } { 0 => 96, d => d as i32 }
    }

    /// Windows 11's taskbar is a XAML island: its clock and buttons aren't windows we can measure.
    fn is_xaml(bar: HWND) -> bool {
        !find(bar, "Windows.UI.Composition.DesktopWindowContentBridge").is_null()
    }

    fn horizontal(bar: HWND) -> bool {
        let r = rect(bar);
        r.right - r.left > r.bottom - r.top
    }

    /// Every taskbar the widget can sit on: the main one (with its notification area) and, on Windows 10,
    /// each other screen's. None when the taskbar auto-hides (a setting shared by all of them) or is vertical.
    fn taskbars() -> Vec<(HWND, Bar)> {
        if TRAY_ONLY.load(Ordering::SeqCst) {
            return vec![];
        }
        let mut data: APPBARDATA = unsafe { std::mem::zeroed() };
        data.cbSize = size_of::<APPBARDATA>() as u32;
        if unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) } as u32 & ABS_AUTOHIDE != 0 {
            return vec![];
        }
        let mut bars = Vec::new();
        let main = unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), null()) };
        if !main.is_null() && !find(main, "TrayNotifyWnd").is_null() && horizontal(main) {
            bars.push((main, Bar::Main));
        }
        let class = wide("Shell_SecondaryTrayWnd");
        let mut bar = unsafe { FindWindowExW(null_mut(), null_mut(), class.as_ptr(), null()) };
        while !bar.is_null() {
            // Windows 11's secondary clocks can't be located, so no guessing there: never cover something
            if unsafe { IsWindowVisible(bar) } != 0 && horizontal(bar) && !is_xaml(bar) {
                bars.push((bar, Bar::Secondary));
            }
            bar = unsafe { FindWindowExW(null_mut(), bar, class.as_ptr(), null()) };
        }
        bars
    }

    /// Where a strip ends (screen x): left of the notification area (or Windows 10's weather) on the main
    /// taskbar, left of the clock on another screen's (or near its right edge if its clock is off).
    fn edge(bar: HWND, kind: Bar) -> i32 {
        match kind {
            Bar::Main => {
                let mut edge = rect(find(bar, "TrayNotifyWnd")).left;
                for class in ["ShellFeedsTaskbarViewWindow", "DynamicContent2"] {
                    let feeds = find(bar, class);
                    if !feeds.is_null() && unsafe { IsWindowVisible(feeds) } != 0 {
                        let r = rect(feeds);
                        if r.left < edge && r.right > r.left {
                            edge = r.left;
                        }
                    }
                }
                edge
            }
            Bar::Secondary => {
                let clock = find(bar, "ClockButton");
                if !clock.is_null() && unsafe { IsWindowVisible(clock) } != 0 {
                    rect(clock).left
                } else {
                    rect(bar).right - (8 * dpi(bar) + 48) / 96
                }
            }
        }
    }

    /// The window holding a taskbar's task buttons (Windows 10), which is narrowed to make room.
    fn task_buttons(bar: HWND, kind: Bar) -> HWND {
        if is_xaml(bar) {
            return null_mut();
        }
        find(bar, if kind == Bar::Main { "ReBarWindow32" } else { "WorkerW" })
    }

    /// Resize the task buttons to end at `end` (screen x), if they currently reach past it (or, with
    /// `restore`, to reach it again).
    fn fit_task_buttons(bar: HWND, kind: Bar, end: i32, restore: bool) {
        let tasks = task_buttons(bar, kind);
        if tasks.is_null() {
            return;
        }
        let r = rect(tasks);
        let (mut top_left, mut end_point) = (POINT { x: r.left, y: r.top }, POINT { x: end, y: r.top });
        unsafe {
            ScreenToClient(bar, &mut top_left);
            ScreenToClient(bar, &mut end_point);
        }
        let width = if restore {
            Some(end_point.x - top_left.x).filter(|w| *w > r.right - r.left)
        } else {
            rebar_width(top_left.x, r.right - r.left, end_point.x)
        };
        if let Some(width) = width {
            unsafe { MoveWindow(tasks, top_left.x, top_left.y, width, r.bottom - r.top, 1) };
        }
    }

    /// The taskbar's theme as last read: 0 not read yet, 1 light, 2 dark. Read again after WM_SETTINGCHANGE,
    /// rather than from the registry on every once-a-second repaint.
    static THEME: AtomicU32 = AtomicU32::new(0);

    /// Whether the taskbar uses the light theme.
    fn light_taskbar() -> bool {
        match THEME.load(Ordering::Relaxed) {
            1 => return true,
            2 => return false,
            _ => {}
        }
        let light = read_light_taskbar();
        THEME.store(if light { 1 } else { 2 }, Ordering::Relaxed);
        light
    }

    fn read_light_taskbar() -> bool {
        let (mut value, mut size) = (0u32, size_of::<u32>() as u32);
        let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let name = wide("SystemUsesLightTheme");
        let result = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD, null_mut(),
                         &mut value as *mut u32 as *mut c_void, &mut size)
        };
        result == ERROR_SUCCESS && value == 1
    }

    pub fn start(app: &AppHandle) {
        let _ = APP.set(app.clone());
        unsafe {
            let instance = GetModuleHandleW(null());
            let owner_class = wide(OWNER_CLASS);
            let strip_class = wide(STRIP_CLASS);
            let mut class: WNDCLASSEXW = std::mem::zeroed();
            class.cbSize = size_of::<WNDCLASSEXW>() as u32;
            class.hInstance = instance;
            class.lpfnWndProc = Some(owner_proc);
            class.lpszClassName = owner_class.as_ptr();
            RegisterClassExW(&class);
            class.lpfnWndProc = Some(strip_proc);
            class.lpszClassName = strip_class.as_ptr();
            class.hCursor = LoadCursorW(null_mut(), IDC_HAND);
            RegisterClassExW(&class);
            let owner = CreateWindowExW(WS_EX_TOOLWINDOW, owner_class.as_ptr(), null(), WS_POPUP, 0, 0, 0, 0,
                                        null_mut(), null_mut(), instance, null());
            if owner.is_null() {
                crate::logs::warn("Could not create the taskbar widget; using the tray icon");
                tray::show_tray(app, true);
                return;
            }
            TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), Ordering::SeqCst);
            SetTimer(owner, TIMER_ID, 1000, None);
            STATE.with_borrow_mut(|w| w.owner = owner as usize);
            OWNER.store(owner as usize, Ordering::SeqCst);
        }
        tick();
    }

    pub fn stop() {
        STATE.with_borrow_mut(|w| unsafe {
            for strip in &w.strips {
                let bar = hwnd(strip.bar);
                if IsWindow(bar) != 0 {
                    fit_task_buttons(bar, strip.kind, edge(bar, strip.kind), true); // give the room back
                }
                DestroyWindow(hwnd(strip.wnd));
            }
            if w.owner != 0 {
                KillTimer(hwnd(w.owner), TIMER_ID);
                DestroyWindow(hwnd(w.owner));
            }
            *w = Widget::default();
        });
        OWNER.store(0, Ordering::SeqCst);
    }

    const HOTKEY_ID: i32 = 1;
    /// The shortcut registered now (as written in Settings), to put back after a failed change or a pause.
    static HOTKEY: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

    fn register(hotkey: Option<Hotkey>) -> bool {
        let owner = hwnd(OWNER.load(Ordering::SeqCst));
        unsafe { UnregisterHotKey(owner, HOTKEY_ID) };
        hotkey.is_none_or(|k| unsafe { RegisterHotKey(owner, HOTKEY_ID, k.mods | MOD_NOREPEAT, k.vk) } != 0)
    }

    /// Use this keyboard shortcut ("" for none) to open and close the popup from anywhere. If Windows won't
    /// have it (another app uses it), the previous one is kept. Main thread only.
    pub fn set_hotkey(text: &str) -> Result<(), String> {
        let hotkey = parse_hotkey(text)?;
        if OWNER.load(Ordering::SeqCst) == 0 {
            return Err("The shortcut can't be used: the app's hidden window is missing.".into());
        }
        let mut current = HOTKEY.lock().unwrap();
        if register(hotkey) {
            *current = text.trim().to_string();
            return Ok(());
        }
        register(parse_hotkey(&current).ok().flatten()); // keep the one that worked
        Err(format!("{} is already used by another app. Try a different one.", text.trim()))
    }

    /// While Settings records a new shortcut, the current one is let go, so pressing it again reaches the popup.
    pub fn suspend_hotkey(suspended: bool) {
        let current = HOTKEY.lock().unwrap().clone();
        register(if suspended { None } else { parse_hotkey(&current).ok().flatten() });
    }

    /// Re-place and repaint the strips now rather than on the next tick.
    pub fn redraw() {
        let owner = hwnd(OWNER.load(Ordering::SeqCst));
        if !owner.is_null() {
            unsafe { PostMessageW(owner, REPAINT, 0, 0) };
        }
    }

    /// Once a second: keep one strip on every usable taskbar (adding them for new screens, dropping them
    /// for gone ones or after Explorer restarts), repaint and place each, and use the tray icon instead
    /// when the main taskbar has none.
    fn tick() {
        let Some(app) = APP.get() else { return };
        let bars = taskbars();
        let show_tray = STATE.with_borrow_mut(|w| unsafe {
            w.strips.retain(|s| {
                let alive = IsWindow(hwnd(s.wnd)) != 0 && GetParent(hwnd(s.wnd)) == hwnd(s.bar);
                let wanted = bars.iter().any(|(bar, _)| *bar as usize == s.bar);
                if alive && !wanted {
                    let bar = hwnd(s.bar);
                    fit_task_buttons(bar, s.kind, edge(bar, s.kind), true); // give the room back
                    DestroyWindow(hwnd(s.wnd));
                }
                alive && wanted
            });
            for (bar, kind) in &bars {
                if !w.strips.iter().any(|s| s.bar == *bar as usize) {
                    let created = CreateWindowExW(WS_EX_LAYERED | WS_EX_NOACTIVATE, wide(STRIP_CLASS).as_ptr(), null(),
                                                  WS_CHILD | WS_CLIPSIBLINGS, 0, 0, 1, 1, *bar, null_mut(),
                                                  GetModuleHandleW(null()), null());
                    if !created.is_null() {
                        w.strips.push(Strip { bar: *bar as usize, wnd: created as usize, kind: *kind, placed: (0, 0, 0),
                                              hovered: false });
                    }
                }
            }
            let mut main_shown = false;
            for strip in &mut w.strips {
                if let Some(size) = paint(app, hwnd(strip.wnd), hwnd(strip.bar), strip.hovered) {
                    place(strip, size);
                    main_shown |= strip.kind == Bar::Main;
                }
            }
            STRIPS.store(w.strips.len(), Ordering::SeqCst);
            !main_shown
        });
        TRAY_SHOWN.store(show_tray, Ordering::SeqCst);
        STATE.with_borrow_mut(|w| {
            if w.tray_shown != Some(show_tray) {
                w.tray_shown = Some(show_tray);
                tray::show_tray(app, show_tray);
            }
        });
    }

    /// Draw the lines into the strip. Returns its size, or None if drawing failed.
    unsafe fn paint(app: &AppHandle, strip: HWND, bar: HWND, hovered: bool) -> Option<(i32, i32)> {
        let (lines, unseen) = {
            let store = app.state::<Arc<Store>>();
            let inner = store.lock();
            (widget_lines(&inner.enabled_games(), now()), inner.state.unseen)
        };
        let dpi = match unsafe { GetDpiForWindow(bar) } { 0 => 96, d => d } as i32;
        let px = |n: i32| (n * dpi + 48) / 96;
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetClientRect(bar, &mut client) };
        let height = (client.bottom - client.top).max(px(30));

        unsafe {
            let screen = GetDC(null_mut());
            let dc = CreateCompatibleDC(screen);
            let font = CreateFontW(-px(12), 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET as _, OUT_DEFAULT_PRECIS as _,
                                   CLIP_DEFAULT_PRECIS as _, ANTIALIASED_QUALITY as _, 0, wide("Segoe UI").as_ptr());
            let old_font = SelectObject(dc, font as HGDIOBJ);
            let measure = |text: &str| {
                let w: Vec<u16> = text.encode_utf16().collect();
                let mut size = SIZE { cx: 0, cy: 0 };
                GetTextExtentPoint32W(dc, w.as_ptr(), w.len() as i32, &mut size);
                size
            };
            let line_h = measure("0").cy;
            let label_w = lines.iter().map(|(l, _)| measure(l).cx).max().unwrap_or(0);
            let time_w = lines.iter().map(|(_, t)| measure(t).cx).max().unwrap_or(measure("00:00:00").cx);
            let (pad, gap) = (px(6), px(7));
            let width = pad + label_w + gap + time_w + pad;

            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height; // top-down rows
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            let drawn = !bitmap.is_null() && !bits.is_null();
            if drawn {
                let old_bitmap = SelectObject(dc, bitmap as HGDIOBJ);
                SetBkMode(dc, TRANSPARENT as _);
                SetTextColor(dc, 0x00FF_FFFF);
                let tops = line_tops(height, line_h, lines.len(), px(CLOCK_LINE_PITCH));
                for ((label, time), y) in lines.iter().zip(tops) {
                    let (l, t): (Vec<u16>, Vec<u16>) = (label.encode_utf16().collect(), time.encode_utf16().collect());
                    TextOutW(dc, pad, y, l.as_ptr(), l.len() as i32);
                    TextOutW(dc, width - pad - measure(time).cx, y, t.as_ptr(), t.len() as i32);
                }
                GdiFlush();
                let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize);
                let light = light_taskbar();
                premultiply(pixels, text_colour(light), background(light, hovered));
                if unseen {
                    draw_dot(pixels, width, px(3));
                }
                let size = SIZE { cx: width, cy: height };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255,
                                            AlphaFormat: AC_SRC_ALPHA as u8 };
                UpdateLayeredWindow(strip, screen, null(), &size, dc, &origin, 0, &blend, ULW_ALPHA);
                SelectObject(dc, old_bitmap);
                DeleteObject(bitmap as HGDIOBJ);
            }
            SelectObject(dc, old_font);
            DeleteObject(font as HGDIOBJ);
            DeleteDC(dc);
            ReleaseDC(null_mut(), screen);
            drawn.then_some((width, height))
        }
    }

    /// A small red dot in the top-right corner: missed items not seen yet.
    fn draw_dot(pixels: &mut [u8], width: i32, radius: i32) {
        let (cx, cy) = (width - radius - 2, radius + 2);
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                if (x - cx).pow(2) + (y - cy).pow(2) <= radius * radius && x >= 0 && y >= 0 {
                    let i = ((y * width + x) * 4) as usize;
                    if let Some(px) = pixels.get_mut(i..i + 4) {
                        px.copy_from_slice(&[0x24, 0x24, 0xE0, 0xFF]); // BGRA
                    }
                }
            }
        }
    }

    /// Put a strip just before its taskbar's edge (see edge()) and keep the task buttons out from under it.
    fn place(strip: &mut Strip, (width, height): (i32, i32)) {
        let (bar, wnd) = (hwnd(strip.bar), hwnd(strip.wnd));
        let end = edge(bar, strip.kind);
        let mut point = POINT { x: end, y: rect(bar).top };
        unsafe { ScreenToClient(bar, &mut point) };
        let left = strip_left(point.x, width, (2 * dpi(bar) + 48) / 96);
        let mut screen_left = POINT { x: left, y: 0 };
        unsafe { ClientToScreen(bar, &mut screen_left) };
        fit_task_buttons(bar, strip.kind, screen_left.x, false);

        let covered = unsafe { !GetWindow(wnd, GW_HWNDPREV).is_null() };
        if covered || strip.placed != (left, width, height) {
            unsafe { SetWindowPos(wnd, HWND_TOP, left, 0, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
            strip.placed = (left, width, height);
        }
    }

    fn show_menu() {
        let Some(app) = APP.get() else { return };
        let owner = hwnd(STATE.with_borrow(|w| w.owner));
        let offline = app.state::<Arc<Store>>().lock().settings.offline;
        let chosen = unsafe {
            let menu = CreatePopupMenu();
            for (id, key, text) in MENU_ITEMS {
                if key == "quit" {
                    AppendMenuW(menu, MF_SEPARATOR, 0, null());
                }
                let flags = if key == "offline" && offline { MF_STRING | MF_CHECKED } else { MF_STRING };
                AppendMenuW(menu, flags, id, wide(text).as_ptr());
            }
            let mut at = POINT { x: 0, y: 0 };
            GetCursorPos(&mut at);
            SetForegroundWindow(owner); // so the menu closes when clicking elsewhere
            let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_NONOTIFY,
                                        at.x, at.y, 0, owner, null());
            PostMessageW(owner, WM_NULL, 0, 0);
            DestroyMenu(menu);
            chosen as usize
        };
        if let Some((_, key, _)) = MENU_ITEMS.iter().find(|(id, ..)| *id == chosen) {
            tray::menu_action(app, key);
        }
    }

    unsafe extern "system" fn owner_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let taskbar_created = TASKBAR_CREATED.load(Ordering::SeqCst);
        if message == WM_TIMER || (taskbar_created != 0 && message == taskbar_created) {
            tick();
            return 0;
        }
        if message == WM_HOTKEY {
            if let Some(app) = APP.get() {
                tray::toggle_popup(app);
            }
            return 0;
        }
        if message == WM_SETTINGCHANGE {
            THEME.store(0, Ordering::Relaxed); // light or dark may have changed: read it on the next paint
            tick();
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        match message {
            SHOW_MENU => show_menu(),
            TOGGLE_POPUP => {
                if let Some(app) = APP.get() {
                    tray::toggle_popup(app);
                }
            }
            REPAINT => tick(),
            _ => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
        0
    }

    /// Hand a strip event to the owner window, outside the taskbar's own window.
    fn to_owner(message: u32) {
        let owner = hwnd(STATE.with(|s| s.try_borrow().map_or(0, |w| w.owner)));
        if !owner.is_null() {
            unsafe { PostMessageW(owner, message, 0, 0) };
        }
    }

    /// Note whether the mouse is over this strip; repaint (with or without the hover wash) if it changed.
    fn set_hovered(window: HWND, hovered: bool) {
        let changed = STATE.with(|s| {
            s.try_borrow_mut().is_ok_and(|mut w| {
                w.strips.iter_mut().find(|s| s.wnd == window as usize)
                    .is_some_and(|s| std::mem::replace(&mut s.hovered, hovered) != hovered)
            })
        });
        if changed {
            if hovered {
                let mut track = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE,
                                                  hwndTrack: window, dwHoverTime: 0 };
                unsafe { TrackMouseEvent(&mut track) }; // so WM_MOUSELEAVE arrives
            }
            to_owner(REPAINT);
        }
    }

    unsafe extern "system" fn strip_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            WM_LBUTTONUP => {
                // the popup opens above the strip that was clicked, on that screen
                let r = rect(window);
                tray::set_anchor(Area { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top });
                to_owner(TOGGLE_POPUP)
            }
            WM_RBUTTONUP => to_owner(SHOW_MENU),
            WM_MOUSEMOVE => set_hovered(window, true),
            WM_MOUSELEAVE => set_hovered(window, false),
            WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
            _ => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
        0
    }
}
