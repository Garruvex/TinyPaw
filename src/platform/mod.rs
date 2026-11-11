//! Native integration: shortcuts, tray, focus and the local command endpoint.
//! Native callbacks run on other threads or inside the Win32 message loop;
//! they forward into one channel that the Iced subscription drains, so the UI
//! thread stays the only writer of session state and idles without polling.

#[cfg(windows)]
pub mod beat;
pub mod capture;
pub mod displays;
pub mod hotkeys;
pub mod ipc;
#[cfg(windows)]
pub mod lens;
#[cfg(windows)]
pub mod music;
pub mod tray;
#[cfg(windows)]
pub mod windows;

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Returns a `'static` copy of `s`, storing each distinct string once. Window
/// and display identifiers are refreshed often and repeat, so interning keeps
/// the settings-friendly `&'static str` without growing on every refresh.
pub fn intern(s: &str) -> &'static str {
    static SEEN: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let set = seen.get_or_insert_with(HashSet::new);
    if let Some(existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    set.insert(leaked);
    leaked
}

use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::stream::{self, BoxStream, StreamExt};

#[derive(Debug, Clone)]
pub enum External {
    PreferencesSaved(Result<(), String>),
    Hotkey {
        id: u32,
        pressed: bool,
    },
    /// A tray menu entry, identified by its registered command ID.
    Menu(String),
    /// Left click on the tray icon.
    TrayClick,
    Interrupted,
    DisplaysChanged,
    MotionChanged,
    /// Settings caption-button hover or maximized state changed.
    FrameChanged,
    InteractKey(bool),
    Cli {
        line: String,
        reply: std::sync::mpsc::Sender<String>,
    },
}

static TX: OnceLock<UnboundedSender<External>> = OnceLock::new();
static RX: Mutex<Option<UnboundedReceiver<External>>> = Mutex::new(None);

/// Creates the channel. Call once before starting native integrations.
pub fn init_bridge() {
    let (tx, rx) = unbounded();
    if TX.set(tx).is_ok() {
        *RX.lock().expect("bridge lock") = Some(rx);
    }
}

pub fn send(event: External) {
    if let Some(tx) = TX.get() {
        let _ = tx.unbounded_send(event);
    }
}

/// Subscription builder: yields every forwarded native event.
pub fn external_stream() -> BoxStream<'static, External> {
    match RX.lock().expect("bridge lock").take() {
        Some(rx) => rx.boxed(),
        None => stream::pending().boxed(),
    }
}

/// One item per composed display frame (DWM's refresh on Windows), for
/// following the cursor smoothly. A 16 ms `iced::time::every` fires about
/// every 31 ms on Windows, whose timers round up to the 15.6 ms tick. The
/// pacing thread ends when the subscription is dropped.
pub fn refresh_ticks() -> BoxStream<'static, ()> {
    let (tx, rx) = iced::futures::channel::mpsc::channel::<()>(1);
    let spawned = std::thread::Builder::new()
        .name("tinypaw-refresh".into())
        .spawn(move || {
            let mut tx = tx;
            loop {
                #[cfg(windows)]
                let waited = unsafe { windows_sys::Win32::Graphics::Dwm::DwmFlush() } >= 0;
                #[cfg(not(windows))]
                let waited = false;
                if !waited {
                    std::thread::sleep(std::time::Duration::from_millis(8));
                }
                match tx.try_send(()) {
                    // Full: the UI has not taken the last tick yet.
                    Ok(()) => {}
                    Err(e) if e.is_full() => {}
                    Err(_) => return,
                }
            }
        });
    match spawned {
        Ok(_) => rx.boxed(),
        Err(_) => stream::pending().boxed(),
    }
}

/// Remembers which window had focus before TinyPaw took input.
pub fn foreground_window() -> Option<isize> {
    #[cfg(windows)]
    {
        windows::foreground_window()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn restore_foreground(_handle: Option<isize>) {
    #[cfg(windows)]
    if let Some(h) = _handle {
        windows::set_foreground(h);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn interning_stores_each_string_once() {
        let a = super::intern("window:1234");
        let b = super::intern(&String::from("window:1234"));
        assert!(
            std::ptr::eq(a, b),
            "repeat refreshes reuse the same allocation"
        );
        assert_ne!(super::intern("window:5678"), a);
    }
}
