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
        WM_WTSSESSION_CHANGE if matches!(w, 7 | 3 | 4) => super::send(super::External::Interrupted),
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
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), instance, 0);
            KEYBOARD_HOOK.store(!hook.is_null(), Ordering::Relaxed);
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if !hook.is_null() {
                UnhookWindowsHookEx(hook);
            }
        })
        .expect("desktop notification thread");
}
unsafe extern "system" fn notification_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    w: usize,
    l: isize,
) -> isize {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, w, l) }
}

use std::sync::atomic::{AtomicBool, Ordering};
static INTERACT_ARMED: AtomicBool = AtomicBool::new(false);
static ALT_HELD: AtomicBool = AtomicBool::new(false);
static KEYBOARD_HOOK: AtomicBool = AtomicBool::new(false);
pub fn arm_interact(armed: bool) {
    INTERACT_ARMED.store(armed, Ordering::Relaxed);
}
pub fn has_keyboard_hook() -> bool {
    KEYBOARD_HOOK.load(Ordering::Relaxed)
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
                let id = Box::leak(format!("window:{}", hwnd as isize).into_boxed_str());
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
