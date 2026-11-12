//! Stand-in for `platform::windows`, mounted under the same path so the app
//! compiles on macOS before its native port lands. Linux uses `wayland` instead. Every
//! item mirrors the Win32 signature and does nothing: no focus handoff, no
//! Lens, no window sources, no keyboard hook.

#![allow(dead_code)]

pub fn foreground_window() -> Option<isize> {
    None
}

pub fn set_foreground(_handle: isize) {}

pub fn allow_any_foreground() {}

pub fn release_own_console() {}

pub fn reduced_motion() -> bool {
    false
}

pub fn observe(_hwnd: isize) {}

pub fn key_down(_vk: i32) -> bool {
    false
}

pub fn buttons_up() -> bool {
    true
}

pub fn place(_hwnd: isize, _d: &super::displays::Display) {}

/// Live window crops need a native compositor API; unavailable here.
pub struct Lens {
    destination: isize,
}
impl Lens {
    pub fn new(_destination: isize, _source: isize) -> Result<Self, String> {
        Err("Lens is not available on this platform yet".into())
    }
    pub fn destination(&self) -> isize {
        self.destination
    }
    pub fn update(
        &self,
        _display: &super::displays::Display,
        _size: f32,
        _zoom: f32,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub fn start_notifications() {}

pub fn arm_interact(_armed: bool) {}

pub fn has_keyboard_hook() -> bool {
    false
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
    Vec::new()
}
