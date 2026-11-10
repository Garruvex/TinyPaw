//! "Start TinyPaw when I sign in": one value under the per-user
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key. No admin rights
//! are needed and uninstalling removes the value.

use std::io;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The value name. The installer's uninstaller deletes the same name.
const VALUE: &str = "TinyPaw";
/// The GUI-subsystem launcher, so sign-in never flashes a console window.
const LAUNCHER: &str = "tinypawd.exe";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The command stored in the Run key: the launcher next to `exe`, or `exe`
/// itself when the launcher is missing.
fn command_for(exe: &Path, launcher_exists: impl Fn(&Path) -> bool) -> String {
    let launcher: PathBuf = exe.with_file_name(LAUNCHER);
    let target = if launcher_exists(&launcher) {
        launcher
    } else {
        exe.to_path_buf()
    };
    format!("\"{}\"", target.display())
}

/// Whether TinyPaw is registered to start at sign-in.
pub fn enabled() -> bool {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    status == ERROR_SUCCESS
}

/// Adds or removes the Run value.
pub fn set(on: bool) -> io::Result<()> {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    let status = if on {
        let exe = std::env::current_exe()?;
        let data = wide(&command_for(&exe, |p| p.is_file()));
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        }
    } else {
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) } {
            ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
            other => other,
        }
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_the_gui_launcher_beside_the_exe() {
        let exe = Path::new(r"C:\Users\a b\AppData\Local\Programs\TinyPaw\tinypaw.exe");
        assert_eq!(
            command_for(exe, |_| true),
            r#""C:\Users\a b\AppData\Local\Programs\TinyPaw\tinypawd.exe""#
        );
    }

    #[test]
    fn falls_back_to_the_exe_without_a_launcher() {
        let exe = Path::new(r"C:\dev\tinypaw.exe");
        assert_eq!(command_for(exe, |_| false), r#""C:\dev\tinypaw.exe""#);
    }
}
