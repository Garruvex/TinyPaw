//! Graphite design tokens and shared widget styles.

use iced::widget::{button, container, pick_list, slider, toggler};
use iced::{Background, Border, Color, Font, Shadow, Theme, font};

pub const SURFACE: Color = Color::from_rgb8(0x17, 0x18, 0x1C);
pub const SURFACE_A: Color = Color::from_rgba8(0x17, 0x18, 0x1C, 0.94);
pub const HEADER: Color = Color::from_rgb8(0x1C, 0x1D, 0x22);
pub const NAV: Color = Color::from_rgb8(0x1A, 0x1B, 0x20);
pub const RAISED: Color = Color::from_rgb8(0x20, 0x22, 0x27);
pub const CONTROL: Color = Color::from_rgb8(0x23, 0x25, 0x2B);
pub const HOVER: Color = Color::from_rgb8(0x2C, 0x2F, 0x36);
pub const WELL: Color = Color::from_rgb8(0x11, 0x12, 0x16);
/// Label backing: white text on near-black, readable over any game.
pub const LABEL_BG: Color = Color::from_rgba8(0x11, 0x12, 0x16, 0.88);
pub const LINE: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.08);
pub const LINE_STRONG: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.12);

pub const TEXT: Color = Color::from_rgb8(0xF3, 0xF4, 0xF6);
pub const TEXT_SOFT: Color = Color::from_rgb8(0xE5, 0xE7, 0xEB);
pub const SUBTLE: Color = Color::from_rgb8(0xC9, 0xCD, 0xD4);
pub const MUTED: Color = Color::from_rgb8(0xA1, 0xA6, 0xB0);
pub const FAINT: Color = Color::from_rgb8(0x85, 0x8A, 0x94);
pub const DISABLED: Color = Color::from_rgb8(0x5E, 0x63, 0x6C);

pub const CYAN: Color = Color::from_rgb8(0x22, 0xD3, 0xEE);
pub const CYAN_TEXT: Color = Color::from_rgb8(0x67, 0xE8, 0xF9);
pub const CYAN_BG: Color = Color::from_rgba8(0x22, 0xD3, 0xEE, 0.16);
pub const CYAN_RING: Color = Color::from_rgba8(0x67, 0xE8, 0xF9, 0.7);
pub const AMBER: Color = Color::from_rgb8(0xF5, 0xA5, 0x24);
pub const AMBER_TEXT: Color = Color::from_rgb8(0xFB, 0xBF, 0x24);
pub const AMBER_BG: Color = Color::from_rgba8(0xF5, 0xA5, 0x24, 0.16);
pub const ON_AMBER: Color = Color::from_rgb8(0x1A, 0x12, 0x06);
pub const DANGER: Color = Color::from_rgb8(0xFC, 0xA5, 0xA5);
pub const DANGER_BG: Color = Color::from_rgba8(0xF8, 0x71, 0x71, 0.16);

/// Ink colours offered in the quick bar and settings.
pub const INKS: &[(&str, [u8; 4])] = &[
    ("Cyan", [0x22, 0xD3, 0xEE, 0xFF]),
    ("Amber", [0xF5, 0xA5, 0x24, 0xFF]),
    ("Pink", [0xF4, 0x72, 0xB6, 0xFF]),
    ("Violet", [0xA7, 0x8B, 0xFA, 0xFF]),
    ("Lime", [0xA3, 0xE6, 0x35, 0xFF]),
    ("White", [0xF9, 0xFA, 0xFB, 0xFF]),
];
pub const WIDTHS: &[f64] = &[2.0, 4.0, 6.0, 8.0];

pub const BODY: Font = Font::with_name("Segoe UI");
pub const MEDIUM: Font = Font {
    weight: font::Weight::Semibold,
    ..Font::with_name("Segoe UI")
};
pub const HEADING: Font = Font {
    weight: font::Weight::Bold,
    ..Font::with_name("Segoe UI")
};

pub fn rgba(c: [u8; 4]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3] as f32 / 255.0)
}

fn border(radius: f32, width: f32, color: Color) -> Border {
    Border {
        radius: radius.into(),
        width,
        color,
    }
}

/// Floating panels over the game: quick bar, chip, popovers, wheel.
pub fn panel(radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(SURFACE_A.into()),
        border: border(radius, 1.0, LINE_STRONG),
        // No drop shadow. On the transparent overlay, tiny-skia's partial
        // repaints stacked the 35 % black shadow onto itself whenever the
        // cursor moved along the bar's edge, until the bar became a solid
        // black block covering its icons.
        shadow: Shadow::default(),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn fill(color: Color, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: border(radius, 0.0, Color::TRANSPARENT),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn outlined(color: Color, radius: f32, line: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: border(radius, 1.0, line),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn keycap(_: &Theme) -> container::Style {
    container::Style {
        background: Some(CONTROL.into()),
        border: border(6.0, 1.0, LINE_STRONG),
        text_color: Some(SUBTLE),
        ..container::Style::default()
    }
}

/// Icon and tool buttons. `selected` uses cyan; `warm` uses amber (mute on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Selected,
    Warm,
    Exit,
}

pub fn tool_button(tone: Tone) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg) = match (tone, status) {
            (_, button::Status::Disabled) => (Color::TRANSPARENT, DISABLED),
            (Tone::Selected, _) => (CYAN_BG, CYAN_TEXT),
            (Tone::Warm, _) => (AMBER_BG, AMBER_TEXT),
            (Tone::Exit, button::Status::Hovered | button::Status::Pressed) => (DANGER_BG, DANGER),
            (_, button::Status::Hovered) => (Color::from_rgba8(255, 255, 255, 0.08), TEXT_SOFT),
            (_, button::Status::Pressed) => (Color::from_rgba8(255, 255, 255, 0.12), TEXT_SOFT),
            _ => (Color::TRANSPARENT, TEXT_SOFT),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: fg,
            border: border(10.0, 0.0, Color::TRANSPARENT),
            ..button::Style::default()
        }
    }
}

/// Settings navigation rows.
pub fn nav_button(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg) = match (selected, status) {
            (true, _) => (CYAN_BG, CYAN_TEXT),
            (false, button::Status::Hovered) => (Color::from_rgba8(255, 255, 255, 0.05), SUBTLE),
            _ => (Color::TRANSPARENT, SUBTLE),
        };
        button::Style {
            background: Some(bg.into()),
            text_color: fg,
            border: border(8.0, 0.0, Color::TRANSPARENT),
            ..button::Style::default()
        }
    }
}

/// Selectable cards (pointer, activation style).
pub fn card(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Disabled => RAISED,
            _ if selected => CONTROL,
            button::Status::Hovered => CONTROL,
            _ => RAISED,
        };
        button::Style {
            background: Some(bg.into()),
            text_color: if status == button::Status::Disabled {
                FAINT
            } else {
                TEXT
            },
            border: border(12.0, if selected { 1.5 } else { 0.0 }, CYAN_RING),
            ..button::Style::default()
        }
    }
}

/// Segment inside a dark well (presentation, widths).
pub fn segment(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg) = match (selected, status) {
            (_, button::Status::Disabled) => (Color::TRANSPARENT, DISABLED),
            (true, _) => (HOVER, TEXT),
            (false, button::Status::Hovered) => (Color::from_rgba8(255, 255, 255, 0.05), SUBTLE),
            _ => (Color::TRANSPARENT, MUTED),
        };
        button::Style {
            background: Some(bg.into()),
            text_color: fg,
            border: border(7.0, 0.0, Color::TRANSPARENT),
            ..button::Style::default()
        }
    }
}

pub fn secondary(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(
            if status == button::Status::Hovered {
                HOVER
            } else {
                CONTROL
            }
            .into(),
        ),
        text_color: if status == button::Status::Disabled {
            DISABLED
        } else {
            TEXT
        },
        border: border(10.0, 1.0, LINE_STRONG),
        ..button::Style::default()
    }
}

pub fn primary(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgb8(0xF7, 0xB5, 0x45),
        button::Status::Pressed => Color::from_rgb8(0xE0, 0x93, 0x18),
        _ => AMBER,
    };
    button::Style {
        background: Some(bg.into()),
        text_color: ON_AMBER,
        border: border(10.0, 0.0, Color::TRANSPARENT),
        ..button::Style::default()
    }
}

pub fn swatch(color: Color, selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: Some(color.into()),
        text_color: TEXT,
        border: border(
            14.0,
            if selected { 2.5 } else { 1.5 },
            if selected { TEXT } else { LINE_STRONG },
        ),
        ..button::Style::default()
    }
}

pub fn master_toggler(_: &Theme, status: toggler::Status) -> toggler::Style {
    let on = matches!(
        status,
        toggler::Status::Active { is_toggled: true }
            | toggler::Status::Hovered { is_toggled: true }
    );
    toggler::Style {
        background: (if on {
            AMBER
        } else {
            Color::from_rgb8(0x3A, 0x3D, 0x45)
        })
        .into(),
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Color::WHITE.into(),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        text_color: Some(TEXT),
        border_radius: None,
        padding_ratio: 0.1,
    }
}

pub fn range(_: &Theme, status: slider::Status) -> slider::Style {
    let handle = if matches!(status, slider::Status::Hovered | slider::Status::Dragged) {
        CYAN_TEXT
    } else {
        CYAN
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (CYAN.into(), Color::from_rgb8(0x3A, 0x3D, 0x45).into()),
            width: 4.0,
            border: border(2.0, 0.0, Color::TRANSPARENT),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 8.0 },
            background: handle.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

pub fn select(_: &Theme, status: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: TEXT,
        placeholder_color: MUTED,
        handle_color: MUTED,
        background: (if matches!(status, pick_list::Status::Hovered) {
            HOVER
        } else {
            CONTROL
        })
        .into(),
        border: border(8.0, 1.0, LINE_STRONG),
    }
}
