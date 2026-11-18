//! Notification cards, drawn by the app instead of using Windows notifications. A card (game icon, title,
//! a line or two) appears above the taskbar widget, or beside the popup while that's open, and fades out
//! after a few seconds. Hovering keeps it; clicking opens the popup; × or a right-click closes it. While a
//! full-screen game or presentation is in front, cards wait and are then shown together as one.
//!
//! Drawn like the widget: a layered window painted with GDI (text) and GDI+ (icons), both part of Windows.
use std::time::Duration;

use crate::config::{TOAST_BASE, TOAST_MAX, TOAST_MERGED_LINES, TOAST_PER_LINE};
use crate::tray::{place, Area};

/// One notification. `game` is the key of the game whose icon it shows (None for a summary: the app's icon).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub game: Option<String>,
    pub title: String,
    pub body: String,
}

impl Card {
    pub fn new(game: Option<&str>, title: impl Into<String>, body: impl Into<String>) -> Card {
        Card { game: game.map(str::to_string), title: title.into(), body: body.into() }
    }
}

/// A card's text listing items one per line, the last line "and N more" if there are too many to read.
pub fn list<S: AsRef<str>>(items: &[S]) -> String {
    let n = items.len();
    let keep = if n > TOAST_MERGED_LINES { TOAST_MERGED_LINES - 1 } else { n };
    let mut lines: Vec<String> = items.iter().take(keep).map(|s| s.as_ref().to_string()).collect();
    if n > keep {
        lines.push(format!("and {} more", n - keep));
    }
    lines.join("\n")
}

/// Cards waiting together become one that lists their titles ("3 notifications").
pub fn merge(cards: &[Card]) -> Option<Card> {
    let (first, rest) = cards.split_first()?;
    if rest.is_empty() {
        return Some(first.clone());
    }
    let titles: Vec<&str> = cards.iter().map(|c| c.title.as_str()).collect();
    let game = first.game.clone().filter(|g| cards.iter().all(|c| c.game.as_ref() == Some(g)));
    Some(Card { game, title: format!("{} notifications", cards.len()), body: list(&titles) })
}

/// How long a card stays: longer for more to read.
pub fn display_time(card: &Card) -> Duration {
    (TOAST_BASE + TOAST_PER_LINE * card.body.lines().count() as u32).min(TOAST_MAX)
}

/// Where a card of `size` goes: beside the open popup (to its left, or its right if there's no room),
/// otherwise above the taskbar widget as the popup would be, otherwise the work area's bottom-right.
pub fn position(popup: Option<Area>, anchor: Option<Area>, work: Area, size: (i32, i32)) -> (i32, i32) {
    let Some(p) = popup else { return place(anchor, work, size) };
    let (w, h) = size;
    let max_x = (work.x + work.w - w).max(work.x);
    let x = if p.x - w >= work.x { p.x - w } else { (p.x + p.w).min(max_x) };
    (x, (p.y + p.h - h).clamp(work.y, (work.y + work.h - h).max(work.y)))
}

/// Signed distance from (x, y) to the edge of a w×h rectangle at the origin with corners of radius r:
/// negative inside.
pub fn rounded_distance(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let qx = (x - w / 2.0).abs() - (w / 2.0 - r);
    let qy = (y - h / 2.0).abs() - (h / 2.0 - r);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

/// How much of the pixel centred on (x, y) the rounded rectangle covers (0 to 1), for smooth corners.
pub fn coverage(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    (0.5 - rounded_distance(x, y, w, h, r)).clamp(0.0, 1.0)
}

/// "#3498db" as (red, green, blue).
pub fn hex(colour: &str) -> Option<(u8, u8, u8)> {
    let digits = colour.strip_prefix('#').filter(|d| d.len() == 6)?;
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some((byte(0)?, byte(2)?, byte(4)?))
}

/// Dark text on a light colour, white on a dark one (for a badge's letters).
pub fn text_on(colour: (u8, u8, u8)) -> (u8, u8, u8) {
    let luminance = 0.299 * colour.0 as f32 + 0.587 * colour.1 as f32 + 0.114 * colour.2 as f32;
    if luminance > 150.0 { (0x17, 0x16, 0x1C) } else { (0xFF, 0xFF, 0xFF) }
}

/// Windows' notification state (SHQueryUserNotificationState): wait while the user is away (locked, screen
/// saver), a full-screen app or game is in front, or a presentation is on.
pub fn is_busy(state: i32) -> bool {
    matches!(state, 1..=4) // QUNS_NOT_PRESENT, QUNS_BUSY, QUNS_RUNNING_D3D_FULL_SCREEN, QUNS_PRESENTATION_MODE
}

#[cfg(windows)]
pub use win::show;

#[cfg(not(windows))]
pub fn show(_app: &tauri::AppHandle, card: Card) {
    crate::logs::info(format!("Notification: {} · {}", card.title, card.body));
}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::ptr::{null, null_mut};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use tauri::{AppHandle, Manager};
    use windows_sys::Win32::Foundation::{COLORREF, ERROR_SUCCESS, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::Graphics::GdiPlus::{
        GdipCreateBitmapFromFile, GdipCreateBitmapFromScan0, GdipCreateBitmapFromStream, GdipCreateFromHDC,
        GdipDeleteGraphics, GdipDisposeImage, GdipDrawImageRectI, GdipSetInterpolationMode, GdipSetPixelOffsetMode,
        GdiplusStartup, GdiplusStartupInput, GpBitmap, GpGraphics, GpImage, InterpolationModeHighQualityBicubic,
        PixelOffsetModeHighQuality,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
    use windows_sys::Win32::UI::Shell::{IUnknown_AtomicRelease, SHCreateMemStream, SHQueryUserNotificationState};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::{coverage, display_time, hex, is_busy, merge, position, rounded_distance, text_on, Card};
    use crate::config::{TOAST_AFTER_HOVER, TOAST_FADE_IN, TOAST_FADE_OUT};
    use crate::store::Store;
    use crate::tray::{self, Area};
    use crate::widget::label;

    const CLASS: &str = "GachaSlopCard";
    const TIMER_ID: usize = 1;
    const FRAME_MS: u32 = 16; // while fading
    const STEADY_MS: u32 = 200; // while shown, to notice the popup opening (the card then moves beside it)
    const BUSY_POLL_MS: u32 = 1000; // while waiting for a full-screen app to close
    const WM_MOUSELEAVE: u32 = 0x02A3; // not in the windows-sys modules used here
    const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026_200A; // GDI+ PixelFormat32bppARGB
    const BA_ICON: &[u8] = include_bytes!("../../public/games/ba.png");
    const ZZZ_ICON: &[u8] = include_bytes!("../../public/games/zzz.png");

    /// Cards from any thread, waiting for the main thread to show them.
    static PENDING: Mutex<Vec<Card>> = Mutex::new(Vec::new());
    static APP: OnceLock<AppHandle> = OnceLock::new();
    static GDIPLUS: OnceLock<bool> = OnceLock::new();

    #[derive(Clone, Copy)]
    enum Phase {
        Idle,
        Waiting, // cards pending, held back while a full-screen app is in front
        Showing { since: Instant, until: Duration },
        Hiding { since: Instant },
    }

    /// The drawn card: premultiplied BGRA pixels, including a soft shadow around it.
    struct Image {
        pixels: Vec<u8>,
        width: i32,
        height: i32,
        close: RECT, // the × button, in window pixels
        slide: i32,  // how far it rises while fading in
    }

    struct Toasts {
        wnd: usize,
        phase: Phase,
        cards: Vec<Card>, // on screen now (several are merged into one card)
        image: Option<Image>,
        hovered: bool,
    }

    thread_local! {
        static STATE: RefCell<Toasts> =
            const { RefCell::new(Toasts { wnd: 0, phase: Phase::Idle, cards: Vec::new(), image: None, hovered: false }) };
    }

    /// Theme colours as (red, green, blue), matching the popup's (src/app.css).
    struct Theme {
        surface: (u8, u8, u8),
        text: (u8, u8, u8),
        secondary: (u8, u8, u8),
        border: (u8, u8, u8),
        shadow: f32,
    }

    const LIGHT: Theme = Theme { surface: (0xFF, 0xFF, 0xFF), text: (0x17, 0x16, 0x1C), secondary: (0x48, 0x45, 0x52),
                                 border: (0xD6, 0xD4, 0xDE), shadow: 0.22 };
    const DARK: Theme = Theme { surface: (0x26, 0x25, 0x2D), text: (0xF4, 0xF3, 0xF8), secondary: (0xCD, 0xCA, 0xD9),
                                border: (0x45, 0x43, 0x50), shadow: 0.45 };

    /// What a card's icon is drawn from.
    enum Icon {
        Png(&'static [u8]),
        File(PathBuf),
        Rgba(Vec<u8>, u32, u32),
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn hwnd(raw: usize) -> HWND {
        raw as HWND
    }

    fn colorref((r, g, b): (u8, u8, u8)) -> COLORREF {
        r as u32 | (g as u32) << 8 | (b as u32) << 16
    }

    /// Show a card. Callable from any thread: it's drawn on the main thread.
    pub fn show(app: &AppHandle, card: Card) {
        let _ = APP.set(app.clone());
        PENDING.lock().unwrap().push(card);
        let _ = app.run_on_main_thread(pump);
    }

    fn pump() {
        if !ensure_window() {
            for card in std::mem::take(&mut *PENDING.lock().unwrap()) {
                crate::logs::warn(format!("Could not show a notification: {}", card.title));
            }
            return;
        }
        // a card arriving while another is on screen joins it, rather than queueing behind it
        let joined = STATE.with_borrow_mut(|t| match t.phase {
            Phase::Showing { .. } | Phase::Hiding { .. } => {
                t.cards.append(&mut PENDING.lock().unwrap());
                true
            }
            Phase::Idle => {
                t.phase = Phase::Waiting;
                false
            }
            Phase::Waiting => false,
        });
        if joined {
            begin(true);
        }
        tick();
    }

    fn ensure_window() -> bool {
        if STATE.with_borrow(|t| t.wnd != 0) {
            return true;
        }
        unsafe {
            let instance = GetModuleHandleW(null());
            let class_name = wide(CLASS);
            let mut class: WNDCLASSEXW = std::mem::zeroed();
            class.cbSize = size_of::<WNDCLASSEXW>() as u32;
            class.hInstance = instance;
            class.lpfnWndProc = Some(card_proc);
            class.lpszClassName = class_name.as_ptr();
            class.hCursor = LoadCursorW(null_mut(), IDC_HAND);
            RegisterClassExW(&class);
            let wnd = CreateWindowExW(WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                                      class_name.as_ptr(), wide("GachaSlop").as_ptr(), WS_POPUP, 0, 0, 1, 1,
                                      null_mut(), null_mut(), instance, null());
            if wnd.is_null() {
                return false;
            }
            STATE.with_borrow_mut(|t| t.wnd = wnd as usize);
        }
        true
    }

    fn set_timer(ms: u32) {
        let wnd = hwnd(STATE.with_borrow(|t| t.wnd));
        unsafe { SetTimer(wnd, TIMER_ID, ms, None) };
    }

    fn busy() -> bool {
        let mut state = 0;
        unsafe { SHQueryUserNotificationState(&mut state) == 0 && is_busy(state) }
    }

    /// Draw the cards on screen now and start (or restart) their time. `visible`: one is already showing,
    /// so skip the fade-in.
    fn begin(visible: bool) {
        let Some(card) = STATE.with_borrow(|t| merge(&t.cards)) else { return };
        let Some(app) = APP.get() else { return };
        let image = render(app, &card);
        let now = Instant::now();
        let since = if visible { now.checked_sub(TOAST_FADE_IN).unwrap_or(now) } else { now };
        STATE.with_borrow_mut(|t| {
            t.image = image;
            t.phase = Phase::Showing { since, until: display_time(&card) + TOAST_FADE_IN };
        });
        set_timer(FRAME_MS);
    }

    /// One step: start showing waiting cards once nothing full-screen is in front, fade in, wait, fade out.
    fn tick() {
        enum Step {
            Nothing,
            Begin,
            Present(f32),
            Hide,
        }
        let now = Instant::now();
        let step = STATE.with_borrow_mut(|t| match t.phase {
            Phase::Idle => Step::Nothing,
            Phase::Waiting if busy() => Step::Nothing,
            Phase::Waiting => {
                t.cards = std::mem::take(&mut *PENDING.lock().unwrap());
                if t.cards.is_empty() {
                    t.phase = Phase::Idle;
                    Step::Nothing
                } else {
                    Step::Begin
                }
            }
            Phase::Showing { since, until } => {
                let shown = now.saturating_duration_since(since);
                if !t.hovered && shown >= until {
                    t.phase = Phase::Hiding { since: now };
                }
                Step::Present((shown.as_secs_f32() / TOAST_FADE_IN.as_secs_f32()).min(1.0))
            }
            Phase::Hiding { since } => {
                let left = 1.0 - now.saturating_duration_since(since).as_secs_f32() / TOAST_FADE_OUT.as_secs_f32();
                if left > 0.0 {
                    Step::Present(left)
                } else {
                    t.cards.clear();
                    t.image = None;
                    t.phase = if PENDING.lock().unwrap().is_empty() { Phase::Idle } else { Phase::Waiting };
                    Step::Hide
                }
            }
        });
        match step {
            Step::Nothing => {}
            Step::Begin => begin(false),
            Step::Present(alpha) => present(alpha),
            Step::Hide => unsafe {
                ShowWindow(hwnd(STATE.with_borrow(|t| t.wnd)), SW_HIDE);
            },
        }
        let timer = STATE.with_borrow(|t| match t.phase {
            Phase::Idle => None,
            Phase::Waiting => Some(BUSY_POLL_MS),
            Phase::Showing { since, .. } if now.saturating_duration_since(since) >= TOAST_FADE_IN => Some(STEADY_MS),
            Phase::Showing { .. } | Phase::Hiding { .. } => Some(FRAME_MS),
        });
        match timer {
            Some(ms) => set_timer(ms),
            None => unsafe {
                KillTimer(hwnd(STATE.with_borrow(|t| t.wnd)), TIMER_ID);
            },
        }
    }

    /// Where things are: the open popup, the main taskbar's widget (or the tray icon), and the work area
    /// and DPI of the screen they're on.
    struct Spot {
        popup: Option<Area>,
        anchor: Option<Area>,
        work: Area,
        dpi: i32,
    }

    fn spot(app: &AppHandle) -> Spot {
        let popup = tray::popup_area(app);
        let anchor = unsafe {
            let bar = FindWindowW(wide("Shell_TrayWnd").as_ptr(), null());
            let strip = FindWindowExW(bar, null_mut(), wide("GachaSlopWidget").as_ptr(), null());
            let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            (!strip.is_null() && IsWindowVisible(strip) != 0 && GetWindowRect(strip, &mut r) != 0)
                .then(|| Area { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top })
        }.or_else(tray::anchor);
        let at = popup.or(anchor).map_or(POINT { x: 0, y: 0 }, |a| POINT { x: a.x + a.w / 2, y: a.y + a.h / 2 });
        unsafe {
            let monitor = MonitorFromPoint(at, MONITOR_DEFAULTTOPRIMARY);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            GetMonitorInfoW(monitor, &mut info);
            let r = info.rcWork;
            let (mut dpi, mut unused) = (96, 96);
            GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut unused);
            Spot { popup, anchor, work: Area { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top },
                   dpi: dpi.max(96) as i32 }
        }
    }

    fn light_theme() -> bool {
        let (mut value, mut size) = (0u32, size_of::<u32>() as u32);
        let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let name = wide("AppsUseLightTheme");
        let result = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD, null_mut(),
                         &mut value as *mut u32 as *mut c_void, &mut size)
        };
        result != ERROR_SUCCESS || value == 1 // light unless Windows says dark
    }

    /// The card's game: its badge letters, colour and icon.
    fn game_look(app: &AppHandle, key: Option<&str>) -> (String, (u8, u8, u8), Option<Icon>) {
        let store = app.state::<Arc<Store>>();
        let game = key.and_then(|k| store.lock().games().into_iter().find(|g| g.key == k));
        let Some(game) = game else {
            let icon = app.default_window_icon().map(|i| Icon::Rgba(i.rgba().to_vec(), i.width(), i.height()));
            return ("GS".into(), (0x6A, 0x3D, 0xF0), icon);
        };
        let icon = match (game.builtin, game.key.as_str()) {
            (true, "ba") => Some(Icon::Png(BA_ICON)),
            (true, "zzz") => Some(Icon::Png(ZZZ_ICON)),
            _ => game.icon.as_ref().map(|f| Icon::File(store.paths.game_icons().join(f))),
        };
        (label(&game.short), hex(&game.color).unwrap_or((0x8E, 0x8E, 0x93)), icon)
    }

    unsafe fn font(size: i32, weight: u32, face: &str) -> HFONT {
        unsafe {
            CreateFontW(-size, 0, 0, 0, weight as i32, 0, 0, 0, DEFAULT_CHARSET as _, OUT_DEFAULT_PRECIS as _,
                        CLIP_DEFAULT_PRECIS as _, CLEARTYPE_QUALITY as _, 0, wide(face).as_ptr())
        }
    }

    /// Draw an icon with GDI+ (PNG and JPG; it can't read WebP). False if it couldn't be drawn.
    unsafe fn draw_icon(dc: HDC, icon: &Icon, x: i32, y: i32, size: i32) -> bool {
        let started = *GDIPLUS.get_or_init(|| {
            let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
            let mut token = 0;
            unsafe { GdiplusStartup(&mut token, &input, null_mut()) == 0 } // runs until the app exits
        });
        if !started {
            return false;
        }
        unsafe {
            let mut bitmap: *mut GpBitmap = null_mut();
            let mut stream: *mut c_void = null_mut();
            let mut bgra = Vec::new(); // must outlive the bitmap made from it
            let status = match icon {
                Icon::Png(bytes) => {
                    stream = SHCreateMemStream(bytes.as_ptr(), bytes.len() as u32);
                    if stream.is_null() { -1 } else { GdipCreateBitmapFromStream(stream, &mut bitmap) }
                }
                Icon::File(path) => GdipCreateBitmapFromFile(wide(&path.to_string_lossy()).as_ptr(), &mut bitmap),
                Icon::Rgba(rgba, w, h) => {
                    bgra = rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
                    GdipCreateBitmapFromScan0(*w as i32, *h as i32, *w as i32 * 4, PIXEL_FORMAT_32BPP_ARGB,
                                              bgra.as_ptr(), &mut bitmap)
                }
            };
            let mut drawn = false;
            if status == 0 && !bitmap.is_null() {
                let mut graphics: *mut GpGraphics = null_mut();
                if GdipCreateFromHDC(dc, &mut graphics) == 0 {
                    GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic);
                    GdipSetPixelOffsetMode(graphics, PixelOffsetModeHighQuality);
                    drawn = GdipDrawImageRectI(graphics, bitmap as *mut GpImage, x, y, size, size) == 0;
                    GdipDeleteGraphics(graphics);
                }
                GdipDisposeImage(bitmap as *mut GpImage); // also lets go of a file
            }
            if !stream.is_null() {
                IUnknown_AtomicRelease(&mut stream);
            }
            drop(bgra);
            drawn
        }
    }

    /// Draw a card: icon (or badge) on the left, title and text beside it, × at the top right, with
    /// rounded corners, a 1px border and a soft shadow.
    fn render(app: &AppHandle, card: &Card) -> Option<Image> {
        let spot = spot(app);
        let (badge, colour, icon) = game_look(app, card.game.as_deref());
        let theme = if light_theme() { LIGHT } else { DARK };
        let s = |n: i32| (n * spot.dpi + 48) / 96;
        let (margin, card_w, pad, icon_size, gap, close_size) = (s(16), s(360), s(14), s(40), s(12), s(24));
        let text_x = margin + pad + icon_size + gap;
        let text_w = card_w - pad - icon_size - gap - pad;

        unsafe {
            let screen = GetDC(null_mut());
            let dc = CreateCompatibleDC(screen);
            let fonts = [font(s(15), FW_NORMAL, "Segoe UI Semibold"), font(s(14), FW_NORMAL, "Segoe UI"),
                         font(s(14), FW_BOLD, "Segoe UI"), font(s(18), FW_NORMAL, "Segoe UI")];
            let [title_font, body_font, badge_font, close_font] = fonts;
            let text = |dc: HDC, f: HFONT, s: &str, r: &mut RECT, flags: u32| {
                let w: Vec<u16> = s.encode_utf16().collect();
                SelectObject(dc, f as HGDIOBJ);
                DrawTextW(dc, w.as_ptr(), w.len() as i32, r, flags | DT_NOPREFIX);
                r.bottom - r.top
            };
            let measure = |f, s: &str, width| text(dc, f, s, &mut RECT { left: 0, top: 0, right: width, bottom: 0 },
                                                   DT_CALCRECT | DT_WORDBREAK);
            let title_h = measure(title_font, &card.title, text_w - close_size + s(4));
            let body_h = if card.body.is_empty() { 0 } else { measure(body_font, &card.body, text_w) };
            let content_h = title_h + if body_h > 0 { s(3) + body_h } else { 0 };
            let card_h = pad * 2 + content_h.max(icon_size);
            let (width, height) = (card_w + 2 * margin, card_h + 2 * margin);

            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height; // top-down rows
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            let mut image = None;
            if !bitmap.is_null() && !bits.is_null() {
                let old_bitmap = SelectObject(dc, bitmap as HGDIOBJ);
                let fill = |r: &RECT, c| {
                    let brush = CreateSolidBrush(colorref(c));
                    FillRect(dc, r, brush);
                    DeleteObject(brush as HGDIOBJ);
                };
                fill(&RECT { left: 0, top: 0, right: width, bottom: height }, theme.surface);
                SetBkMode(dc, TRANSPARENT as _);

                let (icon_x, icon_y) = (margin + pad, margin + pad);
                let icon_rect = RECT { left: icon_x, top: icon_y, right: icon_x + icon_size, bottom: icon_y + icon_size };
                if !icon.is_some_and(|i| draw_icon(dc, &i, icon_x, icon_y, icon_size)) {
                    fill(&icon_rect, colour);
                    SetTextColor(dc, colorref(text_on(colour)));
                    text(dc, badge_font, &badge, &mut icon_rect.clone(), DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                }
                let top = margin + pad + (card_h - 2 * pad - content_h).max(0) / 2; // centred beside the icon
                SetTextColor(dc, colorref(theme.text));
                let mut r = RECT { left: text_x, top, right: text_x + text_w - close_size + s(4), bottom: top + title_h };
                text(dc, title_font, &card.title, &mut r, DT_WORDBREAK);
                if body_h > 0 {
                    SetTextColor(dc, colorref(theme.secondary));
                    let body_top = top + title_h + s(3);
                    let mut r = RECT { left: text_x, top: body_top, right: text_x + text_w, bottom: body_top + body_h };
                    text(dc, body_font, &card.body, &mut r, DT_WORDBREAK);
                }
                let close = RECT { left: margin + card_w - s(6) - close_size, top: margin + s(6),
                                   right: margin + card_w - s(6), bottom: margin + s(6) + close_size };
                SetTextColor(dc, colorref(theme.secondary));
                text(dc, close_font, "×", &mut close.clone(), DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                GdiFlush();

                let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize);
                finish(pixels, width, &theme, Shape { margin, card: (card_w, card_h), radius: s(10),
                                                      icon: (pad, icon_size, s(8)), shadow_drop: s(3) });
                image = Some(Image { pixels: pixels.to_vec(), width, height, close, slide: s(10) });
                SelectObject(dc, old_bitmap);
                DeleteObject(bitmap as HGDIOBJ);
            }
            for f in fonts {
                DeleteObject(f as HGDIOBJ);
            }
            DeleteDC(dc);
            ReleaseDC(null_mut(), screen);
            image
        }
    }

    struct Shape {
        margin: i32,
        card: (i32, i32),
        radius: i32,
        icon: (i32, i32, i32), // offset from the card's corner, size, corner radius
        shadow_drop: i32,
    }

    /// Turn GDI's drawing (opaque, alpha 0) into the final card: round the icon's and the card's corners,
    /// add the border and the shadow, and premultiply.
    fn finish(pixels: &mut [u8], width: i32, theme: &Theme, shape: Shape) {
        let (m, (cw, ch)) = (shape.margin as f32, (shape.card.0 as f32, shape.card.1 as f32));
        let (icon_at, icon_size, icon_r) = (shape.icon.0 as f32, shape.icon.1 as f32, shape.icon.2 as f32);
        let mix = |under: (u8, u8, u8), over: [f32; 3], t: f32| {
            [under.2 as f32 + (over[0] - under.2 as f32) * t, under.1 as f32 + (over[1] - under.1 as f32) * t,
             under.0 as f32 + (over[2] - under.0 as f32) * t]
        };
        for (i, px) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = ((i as i32 % width) as f32 + 0.5 - m, (i as i32 / width) as f32 + 0.5 - m);
            let d = rounded_distance(x, y, cw, ch, shape.radius as f32);
            let mut c = [px[0] as f32, px[1] as f32, px[2] as f32]; // BGR
            let (ix, iy) = (x - icon_at, y - icon_at);
            if (-1.0..icon_size + 1.0).contains(&ix) && (-1.0..icon_size + 1.0).contains(&iy) {
                c = mix(theme.surface, c, coverage(ix, iy, icon_size, icon_size, icon_r));
            }
            c = mix(theme.border, c, (0.5 - (d + 1.0)).clamp(0.0, 1.0)); // 1px border inside the edge
            let inside = (0.5 - d).clamp(0.0, 1.0);
            let ds = rounded_distance(x, y - shape.shadow_drop as f32, cw, ch, shape.radius as f32);
            let shadow = theme.shadow * if ds <= 0.0 { 1.0 } else { (1.0 - ds / m).max(0.0).powi(2) };
            let alpha = inside + shadow * (1.0 - inside);
            *px =
                [(c[0] * inside) as u8, (c[1] * inside) as u8, (c[2] * inside) as u8, (alpha * 255.0) as u8];
        }
    }

    /// Put the drawn card on screen at this opacity, where it belongs right now (it moves beside the popup
    /// if that opens).
    fn present(alpha: f32) {
        let Some(app) = APP.get() else { return };
        let spot = spot(app);
        STATE.with_borrow(|t| {
            let Some(image) = &t.image else { return };
            let (x, y) = position(spot.popup, spot.anchor, spot.work, (image.width, image.height));
            let y = y + ((1.0 - alpha) * image.slide as f32) as i32;
            let wnd = hwnd(t.wnd);
            unsafe {
                let screen = GetDC(null_mut());
                let dc = CreateCompatibleDC(screen);
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = image.width;
                info.bmiHeader.biHeight = -image.height;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let mut bits: *mut c_void = null_mut();
                let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
                if !bitmap.is_null() && !bits.is_null() {
                    std::ptr::copy_nonoverlapping(image.pixels.as_ptr(), bits as *mut u8, image.pixels.len());
                    let old = SelectObject(dc, bitmap as HGDIOBJ);
                    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0,
                                                SourceConstantAlpha: (alpha.clamp(0.0, 1.0) * 255.0) as u8,
                                                AlphaFormat: AC_SRC_ALPHA as u8 };
                    let size = SIZE { cx: image.width, cy: image.height };
                    UpdateLayeredWindow(wnd, screen, &POINT { x, y }, &size, dc, &POINT { x: 0, y: 0 }, 0, &blend,
                                        ULW_ALPHA);
                    SelectObject(dc, old);
                    DeleteObject(bitmap as HGDIOBJ);
                }
                DeleteDC(dc);
                ReleaseDC(null_mut(), screen);
                if IsWindowVisible(wnd) == 0 {
                    ShowWindow(wnd, SW_SHOWNOACTIVATE);
                }
                SetWindowPos(wnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        });
    }

    /// Start fading out (a click, or ×).
    fn dismiss() {
        STATE.with(|s| {
            if let Ok(mut t) = s.try_borrow_mut()
                && let Phase::Showing { .. } = t.phase
            {
                t.phase = Phase::Hiding { since: Instant::now() };
            }
        });
        set_timer(FRAME_MS);
    }

    fn set_hovered(wnd: HWND, hovered: bool) {
        STATE.with(|s| {
            let Ok(mut t) = s.try_borrow_mut() else { return };
            if t.hovered == hovered {
                return;
            }
            t.hovered = hovered;
            if hovered {
                let mut track = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE,
                                                  hwndTrack: wnd, dwHoverTime: 0 };
                unsafe { TrackMouseEvent(&mut track) }; // so WM_MOUSELEAVE arrives
            } else if let Phase::Showing { .. } = t.phase {
                // read it, moved away: a little longer, then go
                let now = Instant::now();
                t.phase = Phase::Showing { since: now.checked_sub(TOAST_FADE_IN).unwrap_or(now),
                                           until: TOAST_FADE_IN + TOAST_AFTER_HOVER };
            }
        });
    }

    unsafe extern "system" fn card_proc(wnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            WM_TIMER => tick(),
            WM_MOUSEMOVE => set_hovered(wnd, true),
            WM_MOUSELEAVE => set_hovered(wnd, false),
            WM_LBUTTONUP => {
                let at = POINT { x: (lparam & 0xFFFF) as i16 as i32, y: ((lparam >> 16) & 0xFFFF) as i16 as i32 };
                let on_close = STATE.with(|s| s.try_borrow().is_ok_and(|t| {
                    t.image.as_ref().is_some_and(|i| unsafe { PtInRect(&i.close, at) } != 0)
                }));
                dismiss();
                if !on_close && let Some(app) = APP.get() {
                    tray::show_popup(app);
                }
            }
            WM_RBUTTONUP => dismiss(),
            WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
            _ => return unsafe { DefWindowProcW(wnd, message, wparam, lparam) },
        }
        0
    }
}
