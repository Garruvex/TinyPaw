//! Scene data: committed marks in output-local logical coordinates.

/// Identifies one target display. This alpha uses a single output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputId(pub u32);

/// A point in output-local logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Pt) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// An RGBA color with 8-bit channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    #[cfg(test)]
    pub const CYAN: Rgba = Rgba([0x22, 0xD3, 0xEE, 0xFF]);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub output: OutputId,
    pub color: Rgba,
    pub width: f32,
    pub points: Vec<Pt>,
    /// When the stroke was committed; drives optional fading.
    pub born: u64,
}

/// A one-line text label; `at` is its top-left corner.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub output: OutputId,
    pub at: Pt,
    pub text: String,
    pub size: f32,
    pub color: Rgba,
    pub backing: bool,
}

/// A numbered marker centred on `at`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pin {
    pub output: OutputId,
    pub at: Pt,
    pub number: u32,
    pub color: Rgba,
}

/// Anything that undo and clear operate on.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Stroke(Stroke),
    Label(Label),
    Pin(Pin),
}

impl Mark {
    pub fn output(&self) -> OutputId {
        match self {
            Mark::Stroke(s) => s.output,
            Mark::Label(l) => l.output,
            Mark::Pin(p) => p.output,
        }
    }

    /// Commit time for strokes; labels and pins never fade.
    pub fn born(&self) -> Option<u64> {
        match self {
            Mark::Stroke(s) => Some(s.born),
            _ => None,
        }
    }

    pub fn set_born(&mut self, at: u64) {
        if let Mark::Stroke(s) = self {
            s.born = at;
        }
    }
}

/// One scene (live or frozen) with a single chronological undo order across
/// outputs.
#[derive(Debug, Default)]
pub struct Scene {
    marks: Vec<Mark>,
    /// Bumped on every change so views can invalidate cached paths.
    revision: u64,
}

impl Scene {
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    pub fn push(&mut self, mark: Mark) {
        self.marks.push(mark);
        self.revision += 1;
    }

    pub fn undo(&mut self) -> Option<Mark> {
        let mark = self.marks.pop();
        if mark.is_some() {
            self.revision += 1;
        }
        mark
    }

    /// The number the next pin gets. Undoing the last pin frees its number;
    /// clearing starts again at 1.
    pub fn next_pin(&self) -> u32 {
        self.marks
            .iter()
            .filter_map(|m| match m {
                Mark::Pin(p) => Some(p.number),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Removes strokes committed before `cutoff`, also dropping them from undo.
    /// Labels and pins persist independently of ink fading.
    pub fn expire(&mut self, cutoff: u64) {
        let before = self.marks.len();
        self.marks.retain(|m| m.born().is_none_or(|b| b >= cutoff));
        if self.marks.len() != before {
            self.revision += 1;
        }
    }

    pub fn clear(&mut self) {
        if !self.marks.is_empty() {
            self.marks.clear();
            self.revision += 1;
        }
    }
}
