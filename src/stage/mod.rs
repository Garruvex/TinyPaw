//! The character stage: native per-pixel-alpha windows for desktop characters,
//! each drawn into a pixel buffer by the shared renderer (`mascot::render`).
//! It runs on its own thread with its own frame clock, so character animation
//! never passes through the Iced dispatcher. The rest of the app talks to it
//! with [`Intent`]s and reads [`Event`]s (plan step 3).
//!
//! The stage thread owns the character's whole behaviour (`mascot::desktop`):
//! idle motion, gaze, press, carry, throw and fall, petting and sleep, music
//! response. Speech and the menu move over in slice 3c; until the cutover in
//! 3d the Iced desktop character stays in use and the stage is reachable
//! only through `tinypaw --stage-probe`.

#[cfg(windows)]
mod native;

#[cfg(windows)]
pub use native::{Stage, probe};

use crate::mascot::Msg;
use crate::mascot::companionship::Audio;
use crate::mascot::desktop::{Item, Options, Pictures};
use crate::mascot::lines::{Lines, SoundRef};
use crate::mascot::reactions::Event as Gesture;
use crate::mascot::rig::Rig;

/// A character to show: its art, lines, reaction pictures and flags.
pub struct Character {
    pub id: &'static str,
    pub rig: Rig,
    pub lines: Lines,
    /// A click plays Compress.
    pub click_compress: bool,
    pub pictures: Pictures,
    /// Falls to the bottom of the screen after being let go.
    pub fall: bool,
}

/// Messages to the stage.
pub enum Intent {
    /// Shows the character at `anchor` (physical feet position; the unplaced
    /// value picks the default spot), `size` logical pixels tall.
    Open {
        character: Box<Character>,
        size: f32,
        anchor: (f64, f64),
        rows: usize,
        hidden: bool,
        options: Options,
    },
    /// Switches to another character, keeping the feet where they were.
    SetCharacter(Box<Character>),
    /// New behaviour settings, read on every step.
    Options(Options),
    /// The menu's rows, as the app wants them shown.
    Menu(Vec<Item>),
    /// What is playing and whether a voice line is busy.
    Audio {
        audio: Audio,
        voice_busy: bool,
    },
    Resize {
        size: f32,
        rows: usize,
    },
    ResetPosition,
    DisplaysChanged,
    /// Steps aside while annotation owns input.
    Hidden(bool),
    Quit,
}

/// Messages from the stage.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Ready,
    /// A gesture to react to, each raised once.
    Gesture(Gesture),
    /// A line's sound to play.
    Sound(SoundRef),
    /// A menu row was chosen: a registered command or the character page.
    Menu(Msg),
    /// The character came to rest here: the feet's physical position, to save.
    Placed(f64, f64),
}
