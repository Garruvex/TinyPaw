//! `tinypawd`: the GUI-subsystem launcher used by the Start-menu shortcut and
//! the sign-in Run key. Windows opens no console for it, and it starts the
//! console `tinypaw` with `CREATE_NO_WINDOW`, so nothing flashes on screen.
//! `tinypaw` stays the CLI and the resident instance.
//!
//! When TinyPaw is already running, launching again opens Settings instead
//! of doing nothing visible.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::{Command, ExitCode};

#[cfg(windows)]
const CLI: &str = "tinypaw.exe";
#[cfg(not(windows))]
const CLI: &str = "tinypaw";

/// `tinypaw <command>` exits with 2 when no resident instance answers.
const NOT_RUNNING: i32 = 2;

fn cli() -> PathBuf {
    std::env::current_exe()
        .map(|exe| exe.with_file_name(CLI))
        .unwrap_or_else(|_| PathBuf::from(CLI))
}

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(cli());
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn main() -> ExitCode {
    // Pass our foreground right on, so Settings can come to the front.
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(
            windows_sys::Win32::UI::WindowsAndMessaging::ASFW_ANY,
        );
    }
    match command(&["settings"]).status() {
        Ok(status) if status.code() == Some(NOT_RUNNING) => {}
        Ok(_) => return ExitCode::SUCCESS,
        Err(_) => return ExitCode::FAILURE,
    }
    // Start the resident instance and leave it running on its own.
    match command(&[]).spawn() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
