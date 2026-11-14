//! The character stage: native per-pixel-alpha windows for desktop characters,
//! each drawn into a pixel buffer by the shared renderer (`mascot::render`).
//! It runs on its own thread with its own frame clock, so character animation
//! never passes through the Iced dispatcher. The rest of the app talks to it
//! with [`Intent`]s and reads [`Event`]s (plan step 3).
//!
//! This first slice owns one window: placement, drawing, click-through outside
//! the artwork, and carrying with the mouse. Motion, speech and the menu move
//! over in the following slices; until then the Iced desktop character stays
//! in use and the stage is reachable only through `tinypaw --stage-probe`.

#[cfg(windows)]
mod native;

#[cfg(windows)]
pub use native::probe;
