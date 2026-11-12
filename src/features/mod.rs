//! Built-in feature modules. Adding a feature means adding its module and one
//! line to [`BUILTINS`].

pub mod anywhere;
pub mod display;
pub mod freeze;
pub mod mascot;
pub mod pin;
pub mod point;
pub mod sound;
pub mod spotlight;
pub mod system;
pub mod text;
pub mod upcoming;
pub mod wheel;

use crate::registry::RegisterFn;

/// The first registered tool is the default tool.
pub const BUILTINS: &[RegisterFn] = &[
    point::register,
    text::register,
    pin::register,
    spotlight::register,
    freeze::register,
    system::register,
    sound::register,
    display::register,
    wheel::register,
    mascot::register,
    upcoming::register,
];
