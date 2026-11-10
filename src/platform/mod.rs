//! Native integration: shortcuts, tray, focus and the local command endpoint.
//! Native callbacks run on other threads or inside the Win32 message loop;
//! they forward into one channel that the Iced subscription drains, so the UI
//! thread stays the only writer of session state and idles without polling.

pub mod capture;
pub mod hotkeys;
pub mod ipc;
pub mod tray;
#[cfg(windows)]
pub mod windows;

use std::sync::{Mutex, OnceLock};

use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::stream::{self, BoxStream, StreamExt};

#[derive(Debug, Clone)]
pub enum External {
    Hotkey { id: u32, pressed: bool },
    /// A tray menu entry, identified by its registered command ID.
    Menu(String),
    /// Left click on the tray icon.
    TrayClick,
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
