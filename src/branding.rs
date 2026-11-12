//! Shared app artwork for windows, the tray and branded headers.

use std::sync::OnceLock;

fn pixels() -> &'static image::RgbaImage {
    static PIXELS: OnceLock<image::RgbaImage> = OnceLock::new();
    PIXELS.get_or_init(|| {
        image::load_from_memory(include_bytes!("../assets/app/icon.png"))
            .expect("bundled app icon must be a valid PNG")
            .into_rgba8()
    })
}

pub fn window_icon() -> iced::window::Icon {
    let image = pixels();
    iced::window::icon::from_rgba(image.as_raw().clone(), image.width(), image.height())
        .expect("bundled app icon must have valid RGBA dimensions")
}

pub fn logo() -> iced::widget::image::Handle {
    static HANDLE: OnceLock<iced::widget::image::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            let image = pixels();
            iced::widget::image::Handle::from_rgba(
                image.width(),
                image.height(),
                image.as_raw().clone(),
            )
        })
        .clone()
}

pub fn tray_icon(on: bool) -> Result<tray_icon::Icon, String> {
    let mut image =
        image::imageops::resize(pixels(), 32, 32, image::imageops::FilterType::Lanczos3);
    // Keep the familiar grey Off state; full colour signals annotation On.
    if !on {
        for pixel in image.pixels_mut() {
            let grey =
                (u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114)
                    / 1000;
            pixel[0] = grey as u8;
            pixel[1] = grey as u8;
            pixel[2] = grey as u8;
        }
    }
    tray_icon::Icon::from_rgba(image.into_raw(), 32, 32).map_err(|e| e.to_string())
}
