//! Win32 helpers for focus handoff and console attachment.

use windows_sys::Win32::System::Console::{FreeConsole, GetConsoleProcessList};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
    SetForegroundWindow,
};

/// The current foreground window, unless it belongs to TinyPaw itself.
pub fn foreground_window() -> Option<isize> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        (pid != GetCurrentProcessId()).then_some(hwnd as isize)
    }
}

pub fn set_foreground(handle: isize) {
    let hwnd = handle as windows_sys::Win32::Foundation::HWND;
    unsafe {
        if IsWindow(hwnd) != 0 {
            SetForegroundWindow(hwnd);
        }
    }
}

/// Lets the resident instance take focus when a CLI command asks it to.
pub fn allow_any_foreground() {
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }
}

/// The binary is a console program so shells wait for CLI replies. When the
/// resident instance was started from Explorer it owns a console nobody reads;
/// release it so no window lingers. A shared terminal console is kept.
pub fn release_own_console() {
    unsafe {
        let mut pids = [0u32; 2];
        if GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32) == 1 {
            FreeConsole();
        }
    }
}

pub fn reduced_motion() -> bool {
    let mut enabled = 1i32;
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        );
    }
    enabled == 0
}

/// Observe broadcasts on each active top-level surface; never poll while Off.
pub fn observe(hwnd: isize) {
    unsafe {
        windows_sys::Win32::UI::Shell::SetWindowSubclass(hwnd as _, Some(events), 0x5450, 0);
        windows_sys::Win32::System::RemoteDesktop::WTSRegisterSessionNotification(hwnd as _, 0);
    }
}

unsafe extern "system" fn events(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    w: usize,
    l: isize,
    _: usize,
    _: usize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match msg {
        WM_WTSSESSION_CHANGE if matches!(w, 2 | 4 | 7) => super::send(super::External::Interrupted),
        WM_POWERBROADCAST if w == 4 => super::send(super::External::Interrupted),
        WM_DPICHANGED | WM_DISPLAYCHANGE => super::send(super::External::DisplaysChanged),
        WM_SETTINGCHANGE => super::send(super::External::MotionChanged),
        WM_NCDESTROY => unsafe {
            windows_sys::Win32::System::RemoteDesktop::WTSUnRegisterSessionNotification(hwnd);
        },
        _ => {}
    }
    unsafe { windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, msg, w, l) }
}

pub fn key_down(vk: i32) -> bool {
    unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk) < 0 }
}
pub fn buttons_up() -> bool {
    ![1, 2, 4, 5, 6].into_iter().any(key_down)
}

pub fn place(hwnd: isize, d: &super::displays::Display) {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos(
            hwnd as _,
            windows_sys::Win32::UI::WindowsAndMessaging::HWND_TOPMOST,
            d.x,
            d.y,
            d.width,
            d.height,
            windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}

/// A live GPU-composed crop of one external window. Never captures the desktop,
/// so neither the pointer, ink nor lens can feed back into the source.
pub struct Lens {
    thumbnail: isize,
    source: isize,
    destination: isize,
}
impl Lens {
    pub fn new(destination: isize, source: isize) -> Result<Self, String> {
        let mut thumbnail = 0;
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(source as _, &mut pid);
            if pid == GetCurrentProcessId() || pid == 0 {
                return Err("Select the game window before turning on TinyPaw".into());
            }
            let hr = windows_sys::Win32::Graphics::Dwm::DwmRegisterThumbnail(
                destination as _,
                source as _,
                &mut thumbnail,
            );
            if hr < 0 {
                return Err(format!(
                    "Live window capture unavailable ({hr:#x}); use a borderless window"
                ));
            }
        }
        Ok(Self {
            thumbnail,
            source,
            destination,
        })
    }
    pub fn destination(&self) -> isize {
        self.destination
    }
    pub fn update(
        &self,
        display: &super::displays::Display,
        size: f32,
        zoom: f32,
    ) -> Result<(), String> {
        use windows_sys::Win32::Foundation::{POINT, RECT};
        use windows_sys::Win32::Graphics::Dwm::*;
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            if IsWindow(self.source as _) == 0 || IsIconic(self.source as _) != 0 {
                return Err("Lens source closed or minimised".into());
            }
            let mut source: RECT = std::mem::zeroed();
            let mut client: RECT = std::mem::zeroed();
            let mut cursor: POINT = std::mem::zeroed();
            if GetWindowRect(self.source as _, &mut source) == 0
                || GetClientRect(self.destination as _, &mut client) == 0
                || GetCursorPos(&mut cursor) == 0
            {
                return Err("Lens source unavailable".into());
            }
            let sw = source.right - source.left;
            let sh = source.bottom - source.top;
            if sw <= 0 || sh <= 0 {
                return Err("Lens source has no drawable area".into());
            }
            let dpi =
                windows_sys::Win32::UI::HiDpi::GetDpiForWindow(self.destination as _) as f32 / 96.0;
            let side = (size * dpi).round().max(1.0) as i32;
            let crop = ((side as f32 / zoom) as i32).min(sw).min(sh).max(1);
            let sx = (cursor.x - source.left - crop / 2).clamp(0, sw - crop);
            let sy = (cursor.y - source.top - crop / 2).clamp(0, sh - crop);
            let x = cursor.x - display.x;
            let y = cursor.y - display.y;
            let gap = (24.0 * dpi) as i32;
            let dx = if x + gap + side <= client.right {
                x + gap
            } else {
                x - gap - side
            }
            .clamp(0, (client.right - side).max(0));
            let dy = if y + gap + side <= client.bottom {
                y + gap
            } else {
                y - gap - side
            }
            .clamp(0, (client.bottom - side).max(0));
            let props = DWM_THUMBNAIL_PROPERTIES {
                dwFlags: DWM_TNP_RECTDESTINATION
                    | DWM_TNP_RECTSOURCE
                    | DWM_TNP_VISIBLE
                    | DWM_TNP_OPACITY
                    | DWM_TNP_SOURCECLIENTAREAONLY,
                rcDestination: RECT {
                    left: dx,
                    top: dy,
                    right: dx + side,
                    bottom: dy + side,
                },
                rcSource: RECT {
                    left: sx,
                    top: sy,
                    right: sx + crop,
                    bottom: sy + crop,
                },
                opacity: 255,
                fVisible: 1,
                fSourceClientAreaOnly: 0,
            };
            if DwmUpdateThumbnailProperties(self.thumbnail, &props) < 0 {
                return Err("Lens capture stopped".into());
            }
        }
        Ok(())
    }
}
impl Drop for Lens {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Graphics::Dwm::DwmUnregisterThumbnail(self.thumbnail);
        }
    }
}

/// Magnifies the desktop under the cursor, whichever app is there. It is a
/// small round click-through window that never activates, holding a Windows
/// Magnifier control. It follows the cursor across monitors on its own timer,
/// so it works with annotation on or off. TinyPaw's own surfaces are filtered
/// out of the magnified image only, so a monitor share still shows the paw
/// and ink.
pub struct DesktopLens {
    host: isize,
    state: Box<LensState>,
    excluded: Vec<isize>,
}

/// What the lens window's timer reads each frame.
struct LensState {
    magnifier: isize,
    /// Logical diameter and zoom, set by the app.
    size: f32,
    zoom: f32,
    /// What the native control currently has, to skip redundant calls.
    applied_zoom: f32,
    shaped: i32,
}

/// Lens frame thickness in logical pixels, drawn by the host's background.
const LENS_BORDER: f32 = 2.0;
/// Gap between the cursor and the lens, in logical pixels.
const LENS_GAP: f32 = 24.0;
const LENS_TIMER: usize = 1;
/// Refresh period: the magnifier only repaints when invalidated.
const LENS_FRAME_MS: u32 = 16;

impl DesktopLens {
    pub fn new(size: f32, zoom: f32) -> Result<Self, String> {
        use std::sync::OnceLock;
        use windows_sys::Win32::Graphics::Gdi::CreateSolidBrush;
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::UI::Magnification::{MagInitialize, WC_MAGNIFIER};
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        const CLASS: windows_sys::core::PCWSTR = windows_sys::core::w!("TinyPawDesktopLens");
        // Initialised once per process; the windows below are per activation.
        static READY: OnceLock<Result<(), String>> = OnceLock::new();
        READY
            .get_or_init(|| unsafe {
                if MagInitialize() == 0 {
                    return Err("the Windows magnifier could not start".into());
                }
                let class = WNDCLASSW {
                    lpfnWndProc: Some(lens_proc),
                    hInstance: GetModuleHandleW(std::ptr::null()),
                    lpszClassName: CLASS,
                    // The frame colour shows as a ring around the magnifier.
                    hbrBackground: CreateSolidBrush(0x0050_4A46),
                    ..std::mem::zeroed()
                };
                if RegisterClassW(&class) == 0 {
                    return Err("the lens window class could not be registered".into());
                }
                Ok(())
            })
            .clone()?;
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let host = CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                CLASS,
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if host.is_null() {
                return Err("the lens window could not be created".into());
            }
            // The magnifier needs a layered host; fully opaque.
            SetLayeredWindowAttributes(host, 0, 255, LWA_ALPHA);
            let magnifier = CreateWindowExW(
                0,
                WC_MAGNIFIER,
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                1,
                1,
                host,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if magnifier.is_null() {
                DestroyWindow(host);
                return Err("the magnifier control could not be created".into());
            }
            let mut state = Box::new(LensState {
                magnifier: magnifier as isize,
                size,
                zoom,
                applied_zoom: 0.0,
                shaped: 0,
            });
            SetWindowLongPtrW(host, GWLP_USERDATA, &mut *state as *mut LensState as isize);
            let mut lens = Self {
                host: host as isize,
                state,
                excluded: Vec::new(),
            };
            lens.set_excluded(&[])?;
            lens_tick(host, &mut lens.state);
            SetTimer(host, LENS_TIMER, LENS_FRAME_MS, None);
            Ok(lens)
        }
    }

    /// Updates size and zoom, and the TinyPaw windows to leave out of the
    /// magnified image. Placement follows the cursor by itself.
    pub fn configure(&mut self, size: f32, zoom: f32, exclude: &[isize]) -> Result<(), String> {
        self.state.size = size;
        self.state.zoom = zoom;
        if exclude != self.excluded.as_slice() {
            self.set_excluded(exclude)?;
        }
        Ok(())
    }

    fn set_excluded(&mut self, exclude: &[isize]) -> Result<(), String> {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::Magnification::*;
        let mut list: Vec<HWND> = exclude
            .iter()
            .chain(std::iter::once(&self.host))
            .map(|&h| h as HWND)
            .collect();
        let ok = unsafe {
            MagSetWindowFilterList(
                self.state.magnifier as HWND,
                MW_FILTERMODE_EXCLUDE,
                list.len() as i32,
                list.as_mut_ptr(),
            )
        };
        if ok == 0 {
            return Err("TinyPaw could not keep itself out of the lens".into());
        }
        self.excluded = exclude.to_vec();
        Ok(())
    }
}

impl Drop for DesktopLens {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            KillTimer(self.host as _, LENS_TIMER);
            // The timer no longer reads the state once this is cleared.
            SetWindowLongPtrW(self.host as _, GWLP_USERDATA, 0);
            // Destroys the magnifier child with it.
            DestroyWindow(self.host as _);
        }
    }
}

/// One frame: centre the source on the cursor, place the round lens beside
/// it on the cursor's monitor, and repaint.
fn lens_tick(host: windows_sys::Win32::Foundation::HWND, state: &mut LensState) {
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::Magnification::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let mut cursor: POINT = std::mem::zeroed();
        if GetCursorPos(&mut cursor) == 0 {
            return;
        }
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return;
        }
        let (mut dpi, mut dpi_y) = (96u32, 96u32);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        let k = dpi.max(96) as f32 / 96.0;
        let side = (state.size * k).round().max(8.0) as i32;
        let border = (LENS_BORDER * k).round() as i32;
        let inner = (side - 2 * border).max(1);
        let area = info.rcMonitor;

        // Beside the cursor, flipping to stay on its monitor.
        let gap = (LENS_GAP * k) as i32;
        let x = if cursor.x + gap + side <= area.right {
            cursor.x + gap
        } else {
            cursor.x - gap - side
        }
        .clamp(area.left, (area.right - side).max(area.left));
        let y = if cursor.y + gap + side <= area.bottom {
            cursor.y + gap
        } else {
            cursor.y - gap - side
        }
        .clamp(area.top, (area.bottom - side).max(area.top));
        let magnifier = state.magnifier as HWND;
        SetWindowPos(host, HWND_TOPMOST, x, y, side, side, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        SetWindowPos(
            magnifier,
            std::ptr::null_mut(),
            border,
            border,
            inner,
            inner,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
        // Round: the host is clipped to a circle and the magnifier to a
        // smaller one, so the host's background shows as a ring. Each window
        // owns the region it is given.
        if side != state.shaped {
            SetWindowRgn(host, CreateEllipticRgn(0, 0, side + 1, side + 1), 1);
            SetWindowRgn(magnifier, CreateEllipticRgn(0, 0, inner + 1, inner + 1), 1);
            state.shaped = side;
        }
        if (state.zoom - state.applied_zoom).abs() > f32::EPSILON {
            let mut transform = MAGTRANSFORM {
                v: [state.zoom, 0.0, 0.0, 0.0, state.zoom, 0.0, 0.0, 0.0, 1.0],
            };
            if MagSetWindowTransform(magnifier, &mut transform) != 0 {
                state.applied_zoom = state.zoom;
            }
        }
        // Source: the area under the cursor, scaled into the inner circle.
        let crop = ((inner as f32 / state.zoom.max(1.0)).round() as i32).max(1);
        let left = cursor.x - crop / 2;
        let top = cursor.y - crop / 2;
        MagSetWindowSource(
            magnifier,
            RECT {
                left,
                top,
                right: left + crop,
                bottom: top + crop,
            },
        );
        InvalidateRect(magnifier, std::ptr::null(), 0);
    }
}

unsafe extern "system" fn lens_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    w: usize,
    l: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        match msg {
            WM_TIMER if w == LENS_TIMER => {
                let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LensState;
                if let Some(state) = state.as_mut() {
                    lens_tick(hwnd, state);
                }
                0
            }
            WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
            WM_NCHITTEST => HTTRANSPARENT as isize,
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

/// A hidden top-level notification window survives click-through sessions with
/// no marks. Message-only windows do not receive power/display broadcasts.
pub fn start_notifications() {
    std::thread::Builder::new()
        .name("tinypaw-desktop".into())
        .spawn(|| unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let name: Vec<u16> = "TinyPawNotifications\0".encode_utf16().collect();
            let instance =
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(notification_proc),
                hInstance: instance,
                lpszClassName: name.as_ptr(),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&class) == 0 {
                return;
            }
            let hwnd = CreateWindowExW(
                0,
                name.as_ptr(),
                name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            observe(hwnd as isize);
            NOTIFY_HWND.store(hwnd as isize, Ordering::Release);
            // Honour an arm request that arrived before the window existed.
            if INTERACT_ARMED.load(Ordering::Acquire) {
                set_hook(true);
            }
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            set_hook(false);
        })
        .expect("desktop notification thread");
}
/// Posted to the notification window to install (w = 1) or remove (w = 0)
/// the keyboard hook on its own thread.
const WM_TINYPAW_HOOK: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;

unsafe extern "system" fn notification_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    w: usize,
    l: isize,
) -> isize {
    if msg == WM_TINYPAW_HOOK {
        set_hook(w != 0);
        return 0;
    }
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, w, l) }
}

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
static INTERACT_ARMED: AtomicBool = AtomicBool::new(false);
static ALT_HELD: AtomicBool = AtomicBool::new(false);
static KEYBOARD_HOOK: AtomicIsize = AtomicIsize::new(0);
static NOTIFY_HWND: AtomicIsize = AtomicIsize::new(0);

/// Installs or removes the low-level keyboard hook. Runs on the notification
/// thread only, which pumps the messages the hook is delivered through.
fn set_hook(on: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let current = KEYBOARD_HOOK.load(Ordering::Acquire);
    unsafe {
        match (on, current) {
            (true, 0) => {
                let instance =
                    windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), instance, 0);
                KEYBOARD_HOOK.store(hook as isize, Ordering::Release);
            }
            (false, h) if h != 0 => {
                UnhookWindowsHookEx(h as _);
                KEYBOARD_HOOK.store(0, Ordering::Release);
                ALT_HELD.store(false, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

/// Arms Hold to interact while TinyPaw is On. The system-wide keyboard hook
/// exists only while armed, so an Off TinyPaw sees no keystrokes at all.
pub fn arm_interact(armed: bool) {
    if INTERACT_ARMED.swap(armed, Ordering::AcqRel) == armed {
        return;
    }
    let hwnd = NOTIFY_HWND.load(Ordering::Acquire);
    if hwnd != 0 {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                hwnd as _,
                WM_TINYPAW_HOOK,
                armed as usize,
                0,
            );
        }
    }
}
pub fn has_keyboard_hook() -> bool {
    KEYBOARD_HOOK.load(Ordering::Acquire) != 0
}
unsafe extern "system" fn keyboard_hook(code: i32, w: usize, l: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    if code >= 0 {
        let key = unsafe { &*(l as *const KBDLLHOOKSTRUCT) };
        if matches!(key.vkCode, 0x12 | 0xA4 | 0xA5) {
            let down = matches!(w as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let was = ALT_HELD.swap(down, Ordering::Relaxed);
            if was != down && INTERACT_ARMED.load(Ordering::Relaxed) && (!down || !key_down(0x11)) {
                super::send(super::External::InteractKey(down));
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w, l) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSource {
    pub id: &'static str,
    pub handle: isize,
    pub title: String,
}
impl std::fmt::Display for WindowSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}
pub fn window_sources() -> Vec<WindowSource> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe extern "system" fn visit(
        hwnd: windows_sys::Win32::Foundation::HWND,
        data: isize,
    ) -> i32 {
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == GetCurrentProcessId() || IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
                return 1;
            }
            let mut title = [0u16; 512];
            let len = GetWindowTextW(hwnd, title.as_mut_ptr(), 512);
            if len > 0 {
                let id = super::intern(&format!("window:{}", hwnd as isize));
                (&mut *(data as *mut Vec<WindowSource>)).push(WindowSource {
                    id,
                    handle: hwnd as isize,
                    title: String::from_utf16_lossy(&title[..len as usize]),
                });
            }
        }
        1
    }
    let mut sources: Vec<WindowSource> = Vec::new();
    unsafe {
        EnumWindows(Some(visit), &mut sources as *mut Vec<WindowSource> as isize);
    }
    sources.sort_by(|a, b| a.title.cmp(&b.title));
    sources
}

/// Physical cursor position on the virtual desktop.
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    let ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) };
    (ok != 0).then_some((p.x, p.y))
}

/// Physical outer rectangle of a window: left, top, right, bottom.
pub fn window_rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    let mut r = windows_sys::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd as _, &mut r) };
    (ok != 0).then_some((r.left, r.top, r.right, r.bottom))
}

/// Work area (desktop minus taskbar) and effective DPI of the monitor that
/// contains a physical point, or the nearest one when no monitor does.
/// `primary` ignores the point and uses the primary monitor.
pub fn monitor_at(x: i32, y: i32, primary: bool) -> ((i32, i32, i32, i32), u32) {
    use windows_sys::Win32::Graphics::Gdi::*;
    unsafe {
        let point = windows_sys::Win32::Foundation::POINT {
            x: if primary { 0 } else { x },
            y: if primary { 0 } else { y },
        };
        let flags = if primary {
            MONITOR_DEFAULTTOPRIMARY
        } else {
            MONITOR_DEFAULTTONEAREST
        };
        let monitor = MonitorFromPoint(point, flags);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let work = if GetMonitorInfoW(monitor, &mut info) != 0 {
            let r = info.rcWork;
            (r.left, r.top, r.right, r.bottom)
        } else {
            (0, 0, 1920, 1080)
        };
        let (mut dpi, mut dpi_y) = (96, 96);
        windows_sys::Win32::UI::HiDpi::GetDpiForMonitor(
            monitor,
            windows_sys::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
            &mut dpi,
            &mut dpi_y,
        );
        (work, dpi.max(48))
    }
}

/// Moves (and optionally resizes) a topmost window without activating it.
pub fn move_window(hwnd: isize, x: i32, y: i32, size: Option<(i32, i32)>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let (w, h, flag) = match size {
        Some((w, h)) => (w, h, 0),
        None => (0, 0, SWP_NOSIZE),
    };
    unsafe {
        SetWindowPos(hwnd as _, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | flag);
    }
}

/// Clicks on this window never activate it, so the app the user was in keeps
/// keyboard focus while they use the desktop character.
pub fn never_activate(hwnd: isize) {
    unsafe {
        windows_sys::Win32::UI::Shell::SetWindowSubclass(hwnd as _, Some(no_activate), 0x4D41, 0);
    }
}

unsafe extern "system" fn no_activate(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    w: usize,
    l: isize,
    _: usize,
    _: usize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    if msg == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as isize;
    }
    unsafe { windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, msg, w, l) }
}

/// Keeps a full-screen overlay that is taking input from counting as an
/// opaque window. Chromium, Electron (Discord, browsers) and other apps with
/// occlusion tracking stop rendering when a visible, non-layered window covers
/// them, so the screen froze under TinyPaw while annotating. A layered window
/// at 254/255 opacity is treated as see-through, still receives mouse input,
/// and keeps per-pixel transparency.
pub fn unoccluding(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let ex = GetWindowLongPtrW(hwnd as _, GWL_EXSTYLE);
        let layered = ex | WS_EX_LAYERED as isize;
        if layered != ex {
            SetWindowLongPtrW(hwnd as _, GWL_EXSTYLE, layered);
        }
        SetLayeredWindowAttributes(hwnd as _, 0, 254, LWA_ALPHA);
        // Occlusion trackers re-check on window events, not style changes.
        SetWindowPos(
            hwnd as _,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// The Settings window's merged header: TinyPaw draws one row in place of the
/// system title bar, and this subclass tells Windows which parts of that row
/// drag, which are caption buttons and which are ordinary controls. The real
/// frame stays, so resizing, Snap Layouts, the system menu and Alt+F4 keep
/// their native behaviour.
pub mod frame {
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
    };
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    // The header's drawn layout and this hit test share one set of sizes.
    pub use crate::view::settings::caption::{BUTTON_W, Button, CONTROLS_W, HEADER};

    static HOVER: AtomicU8 = AtomicU8::new(0);
    static PRESSED: AtomicU8 = AtomicU8::new(0);
    static MAXIMIZED: AtomicBool = AtomicBool::new(false);

    fn button(code: u8) -> Option<Button> {
        match code as u32 {
            HTMINBUTTON => Some(Button::Minimize),
            HTMAXBUTTON => Some(Button::Maximize),
            HTCLOSE => Some(Button::Close),
            _ => None,
        }
    }

    /// The caption button under the pointer, for hover drawing.
    pub fn hover() -> Option<Button> {
        button(HOVER.load(Ordering::Relaxed))
    }

    pub fn maximized() -> bool {
        MAXIMIZED.load(Ordering::Relaxed)
    }

    /// Installs the merged header on `hwnd`. Returns false if Windows refused,
    /// in which case the window keeps its normal title bar.
    pub fn install(hwnd: isize) -> bool {
        let hwnd = hwnd as HWND;
        unsafe {
            if SetWindowSubclass(hwnd, Some(proc), 0x4652, 0) == 0 {
                return false;
            }
            MAXIMIZED.store(IsZoomed(hwnd) != 0, Ordering::Relaxed);
            HOVER.store(0, Ordering::Relaxed);
            // Recompute the frame now that WM_NCCALCSIZE drops the caption.
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        true
    }

    /// Resize-border thickness and scale at the window's current DPI.
    fn border(hwnd: HWND) -> (i32, i32, f32) {
        unsafe {
            let dpi = GetDpiForWindow(hwnd).max(96);
            let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
            (
                GetSystemMetricsForDpi(SM_CXFRAME, dpi) + pad,
                GetSystemMetricsForDpi(SM_CYFRAME, dpi) + pad,
                dpi as f32 / 96.0,
            )
        }
    }

    fn set_hover(code: u8) {
        if HOVER.swap(code, Ordering::Relaxed) != code {
            crate::platform::send(crate::platform::External::FrameChanged);
        }
    }

    /// Where a client-space point falls in the header.
    fn hit(hwnd: HWND, x: i32, y: i32) -> u32 {
        let (frame_x, frame_y, k) = border(hwnd);
        let mut rc = RECT::default();
        unsafe { GetClientRect(hwnd, &mut rc) };
        let width = rc.right - rc.left;
        // The top resize band replaces the caption's own top border.
        if !maximized() && y < frame_y {
            return if x < frame_x {
                HTTOPLEFT
            } else if x >= width - frame_x {
                HTTOPRIGHT
            } else {
                HTTOP
            };
        }
        if (y as f32) >= HEADER * k {
            return HTCLIENT;
        }
        let from_right = (width - x) as f32 / k;
        if from_right <= BUTTON_W {
            HTCLOSE
        } else if from_right <= BUTTON_W * 2.0 {
            HTMAXBUTTON
        } else if from_right <= BUTTON_W * 3.0 {
            HTMINBUTTON
        } else if from_right <= BUTTON_W * 3.0 + CONTROLS_W {
            HTCLIENT
        } else {
            HTCAPTION
        }
    }

    unsafe extern "system" fn proc(
        hwnd: HWND,
        msg: u32,
        w: usize,
        l: isize,
        _: usize,
        _: usize,
    ) -> isize {
        unsafe {
            match msg {
                // Keep the side and bottom borders; give the caption to the
                // client. A maximized window overhangs the monitor by its
                // border, so inset the top by that much.
                WM_NCCALCSIZE if w != 0 => {
                    let params = &mut *(l as *mut NCCALCSIZE_PARAMS);
                    let top = params.rgrc[0].top;
                    let result = DefSubclassProc(hwnd, msg, w, l);
                    let zoomed = IsZoomed(hwnd) != 0;
                    params.rgrc[0].top = top + if zoomed { border(hwnd).1 } else { 0 };
                    if MAXIMIZED.swap(zoomed, Ordering::Relaxed) != zoomed {
                        crate::platform::send(crate::platform::External::FrameChanged);
                    }
                    return result;
                }
                WM_NCHITTEST => {
                    let native = DefSubclassProc(hwnd, msg, w, l);
                    if native as u32 != HTCLIENT {
                        return native; // side and bottom borders
                    }
                    let mut p = POINT {
                        x: (l & 0xFFFF) as i16 as i32,
                        y: ((l >> 16) & 0xFFFF) as i16 as i32,
                    };
                    ScreenToClient(hwnd, &mut p);
                    return hit(hwnd, p.x, p.y) as isize;
                }
                WM_NCMOUSEMOVE => {
                    let code = if button(w as u8).is_some() { w as u8 } else { 0 };
                    if code != 0 && HOVER.load(Ordering::Relaxed) == 0 {
                        let mut track = TRACKMOUSEEVENT {
                            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE | TME_NONCLIENT,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        TrackMouseEvent(&mut track);
                    }
                    set_hover(code);
                }
                WM_NCMOUSELEAVE | WM_MOUSEMOVE => {
                    set_hover(0);
                    PRESSED.store(0, Ordering::Relaxed);
                }
                // TinyPaw draws the buttons: keep Windows from drawing classic
                // ones on press, and act on release over the same button.
                WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if button(w as u8).is_some() => {
                    PRESSED.store(w as u8, Ordering::Relaxed);
                    return 0;
                }
                WM_NCLBUTTONUP if button(w as u8).is_some() => {
                    let pressed = PRESSED.swap(0, Ordering::Relaxed);
                    if pressed == w as u8 {
                        let command = match button(pressed) {
                            Some(Button::Minimize) => SC_MINIMIZE,
                            Some(Button::Maximize) if IsZoomed(hwnd) != 0 => SC_RESTORE,
                            Some(Button::Maximize) => SC_MAXIMIZE,
                            _ => SC_CLOSE,
                        };
                        set_hover(0);
                        PostMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0);
                    }
                    return 0;
                }
                _ => {}
            }
            DefSubclassProc(hwnd, msg, w, l)
        }
    }
}
