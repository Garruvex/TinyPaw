//! One-shot display stills for Freeze. Windows copies the composed primary
//! display with GDI, which shows no capture border and works for borderless
//! and windowed games; exclusive fullscreen is out of scope. Desktop
//! Duplication (faster, HDR-aware) can replace this behind the same function.

/// A still in RGBA8, physical pixels.
pub struct Still {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[cfg(windows)]
pub fn display(d: &super::displays::Display) -> Result<Still, String> {
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
    };

    unsafe {
        // The process is per-monitor DPI aware, so these are physical pixels.
        let (w, h) = (d.width, d.height);
        if w <= 0 || h <= 0 {
            return Err("no primary display".into());
        }
        let screen = GetDC(null_mut());
        if screen.is_null() {
            return Err("cannot open the screen".into());
        }
        let mem = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bitmap);
        let copied = BitBlt(mem, 0, 0, w, h, screen, d.x, d.y, SRCCOPY) != 0;

        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top-down rows
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        let rows = GetDIBits(
            mem,
            bitmap,
            0,
            h as u32,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        );

        SelectObject(mem, old);
        DeleteObject(bitmap);
        DeleteDC(mem);
        ReleaseDC(null_mut(), screen);

        if !copied || rows != h {
            return Err("the screen copy failed".into());
        }
        // BGRA → RGBA, fully opaque.
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 0xFF;
        }
        Ok(Still {
            width: w as u32,
            height: h as u32,
            rgba: pixels,
        })
    }
}

#[cfg(not(windows))]
pub fn display(d: &super::displays::Display) -> Result<Still, String> {
    Err("Freeze capture is not available on this desktop yet".into())
}
