//! Line icons from the Graphite design (24×24, 1.8 stroke). Colour is applied
//! at draw time, so each SVG is parsed once and tinted per use.

use std::sync::OnceLock;

use iced::widget::svg::{self, Handle, Svg};
use iced::{Color, Length, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Paw,
    PawSolid,
    Laser,
    Text,
    Pin,
    Spotlight,
    Lens,
    Freeze,
    Undo,
    Clear,
    Sound,
    Mute,
    Close,
    ChevronUp,
    Plus,
    Controls,
    Display,
    More,
    Pointer,
    Head,
    Pen,
    WinMinimize,
    WinMaximize,
    WinRestore,
}

const ALL: [Glyph; 24] = [
    Glyph::Paw,
    Glyph::PawSolid,
    Glyph::Laser,
    Glyph::Text,
    Glyph::Pin,
    Glyph::Spotlight,
    Glyph::Lens,
    Glyph::Freeze,
    Glyph::Undo,
    Glyph::Clear,
    Glyph::Sound,
    Glyph::Mute,
    Glyph::Close,
    Glyph::ChevronUp,
    Glyph::Plus,
    Glyph::Controls,
    Glyph::Display,
    Glyph::More,
    Glyph::Pointer,
    Glyph::Head,
    Glyph::Pen,
    Glyph::WinMinimize,
    Glyph::WinMaximize,
    Glyph::WinRestore,
];

fn body(g: Glyph) -> &'static str {
    match g {
        Glyph::Paw => r##"<circle cx="12" cy="15.5" r="4.5"/><circle cx="5.5" cy="10" r="2"/><circle cx="9.5" cy="5.5" r="2"/><circle cx="14.5" cy="5.5" r="2"/><circle cx="18.5" cy="10" r="2"/>"##,
        Glyph::PawSolid => r##"<g fill="#000" stroke="none"><circle cx="12" cy="15.5" r="4.8"/><circle cx="5.5" cy="10" r="2.2"/><circle cx="9.5" cy="5.5" r="2.2"/><circle cx="14.5" cy="5.5" r="2.2"/><circle cx="18.5" cy="10" r="2.2"/></g>"##,
        Glyph::Laser => r##"<circle cx="12" cy="12" r="3"/><circle cx="12" cy="12" r="8" stroke-dasharray="2 3"/>"##,
        Glyph::Text => r##"<path d="M5 6h14M12 6v13M9 19h6"/>"##,
        Glyph::Pin => r##"<path d="M12 21s-6-5.5-6-11a6 6 0 0 1 12 0c0 5.5-6 11-6 11z"/><circle cx="12" cy="10" r="2"/>"##,
        Glyph::Spotlight => r##"<circle cx="12" cy="12" r="4.5"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>"##,
        Glyph::Lens => r##"<circle cx="11" cy="11" r="6.5"/><path d="M20 20l-4.3-4.3M11 8.5v5M8.5 11h5"/>"##,
        Glyph::Freeze => r##"<path d="M12 3v18M4.2 7.5l15.6 9M4.2 16.5l15.6-9M9.5 4.5L12 7l2.5-2.5M9.5 19.5L12 17l2.5 2.5"/>"##,
        Glyph::Undo => r##"<path d="M9 14L4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>"##,
        Glyph::Clear => r##"<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>"##,
        Glyph::Sound => r##"<path d="M4 9v6h4l5 4V5L8 9z"/><path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12"/>"##,
        Glyph::Mute => r##"<path d="M4 9v6h4l5 4V5L8 9z"/><path d="M17 9.5l5 5M22 9.5l-5 5"/>"##,
        Glyph::Close => r##"<path d="M6 6l12 12M18 6L6 18"/>"##,
        Glyph::ChevronUp => r##"<path d="M7 14l5-5 5 5"/>"##,
        Glyph::Plus => r##"<path d="M12 5v14M5 12h14"/>"##,
        Glyph::Controls => r##"<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10"/>"##,
        Glyph::Display => r##"<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>"##,
        Glyph::More => r##"<g fill="#000" stroke="none"><circle cx="5.5" cy="12" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="18.5" cy="12" r="1.7"/></g>"##,
        Glyph::Pointer => r##"<path d="M5 3.5l13 7-5.6 1.6L9.8 18z"/><path d="M12.4 12.1l4.6 5.4"/>"##,
        Glyph::Head => r##"<path d="M5 10.5L4 4l5 3.2a8 8 0 0 1 6 0L20 4l-1 6.5a7.5 7.5 0 1 1-14 0z"/><path d="M9.5 13h.01M14.5 13h.01"/>"##,
        Glyph::WinMinimize => r##"<path d="M5 12h14"/>"##,
        Glyph::WinMaximize => r##"<rect x="5" y="5" width="14" height="14" rx="1.5"/>"##,
        Glyph::WinRestore => r##"<rect x="4.5" y="8.5" width="11" height="11" rx="1.5"/><path d="M8.5 8.5V6a1.5 1.5 0 0 1 1.5-1.5h8A1.5 1.5 0 0 1 19.5 6v8a1.5 1.5 0 0 1-1.5 1.5h-2.5"/>"##,
        Glyph::Pen => r##"<path d="M15.5 4.5l4 4L9 19H5v-4z"/><path d="M13 7l4 4"/>"##,
    }
}

pub fn handle(g: Glyph) -> Handle {
    static HANDLES: OnceLock<Vec<Handle>> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        ALL.iter()
            .map(|&g| {
                let src = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{}</svg>"##,
                    body(g)
                );
                Handle::from_memory(src.into_bytes())
            })
            .collect()
    });
    let i = ALL.iter().position(|&x| x == g).unwrap_or(0);
    handles[i].clone()
}

/// An icon that takes the given colour.
pub fn icon<'a>(g: Glyph, size: f32, color: Color) -> Svg<'a, Theme> {
    iced::widget::svg(handle(g))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_, _| svg::Style { color: Some(color) })
}

