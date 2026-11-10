//! Physical desktop rectangles stay native; annotations use each window's logical space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    pub id: crate::session::OutputId,
    pub name: &'static str,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    pub dpi: u32,
}
impl std::fmt::Display for Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} — {} × {}{}",
            self.name,
            self.width,
            self.height,
            if self.primary { " (primary)" } else { "" }
        )
    }
}
#[cfg(windows)]
pub fn list() -> Vec<Display> {
    use windows_sys::Win32::Graphics::Gdi::*;
    unsafe extern "system" fn visit(
        m: HMONITOR,
        _: HDC,
        _: *mut windows_sys::Win32::Foundation::RECT,
        data: isize,
    ) -> i32 {
        let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = std::mem::size_of_val(&info) as u32;
        if unsafe { GetMonitorInfoW(m, (&mut info as *mut MONITORINFOEXW).cast()) } != 0 {
            let items = unsafe { &mut *(data as *mut Vec<Display>) };
            let name = String::from_utf16_lossy(
                &info.szDevice[..info.szDevice.iter().position(|c| *c == 0).unwrap_or(32)],
            );
            let rect = info.monitorInfo.rcMonitor;
            let (mut dpi, mut dpi_y) = (96, 96);
            unsafe {
                windows_sys::Win32::UI::HiDpi::GetDpiForMonitor(
                    m,
                    windows_sys::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
                    &mut dpi,
                    &mut dpi_y,
                );
            }
            items.push(Display {
                id: crate::session::OutputId(0),
                name: Box::leak(name.into_boxed_str()),
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
                primary: info.monitorInfo.dwFlags & 1 != 0,
                dpi,
            });
        }
        1
    }
    let mut items: Vec<Display> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(visit),
            &mut items as *mut Vec<Display> as isize,
        );
    }
    items.sort_by_key(|d| d.name);
    for (i, d) in items.iter_mut().enumerate() {
        d.id = crate::session::OutputId(i as u32);
    }
    items
}
#[cfg(not(windows))]
pub fn list() -> Vec<Display> {
    vec![Display {
        id: crate::session::OutputId(0),
        name: "primary",
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        primary: true,
        dpi: 96,
    }]
}

pub fn selected(displays: &[Display], target: &str) -> Vec<Display> {
    displays
        .iter()
        .filter(|d| target == "all" || (target == "primary" && d.primary) || target == d.name)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_origins_and_named_selection_are_preserved() {
        let main = Display {
            id: crate::session::OutputId(0),
            name: "Main",
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
            primary: true,
            dpi: 96,
        };
        let left = Display {
            id: crate::session::OutputId(1),
            name: "Left",
            x: -1920,
            y: -400,
            width: 1920,
            height: 1080,
            primary: false,
            dpi: 144,
        };
        let list = vec![main.clone(), left.clone()];
        assert_eq!(selected(&list, "primary"), vec![main]);
        assert_eq!(selected(&list, "Left"), vec![left]);
        assert_eq!(selected(&list, "all"), list);
        assert!(selected(&list, "Unplugged").is_empty());
    }
}
