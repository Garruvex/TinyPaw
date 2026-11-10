//! Scene data: committed marks in output-local logical coordinates.

/// Identifies one target display. Phase 1 uses a single output.
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
    pub const CYAN: Rgba = Rgba([0x00, 0xE5, 0xFF, 0xFF]);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub output: OutputId,
    pub color: Rgba,
    pub width: f32,
    pub points: Vec<Pt>,
}

/// Anything that undo and clear operate on. Labels and pins join in later phases.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Stroke(Stroke),
}

impl Mark {
    pub fn output(&self) -> OutputId {
        match self {
            Mark::Stroke(s) => s.output,
        }
    }
}

/// One scene (live, later also frozen) with a single chronological undo order
/// across outputs.
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

    pub fn clear(&mut self) {
        if !self.marks.is_empty() {
            self.marks.clear();
            self.revision += 1;
        }
    }
}
