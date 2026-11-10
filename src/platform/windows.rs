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
