//! Undo and redo snapshots.

use super::*;

impl Editor {
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            body: self.body.clone(),
            parts: self.parts.clone(),
            eyes: self.eyes.clone(),
            ground: self.ground,
            rest: self.rest,
            selected: self.selected,
            canvas: self.canvas,
        }
    }

    pub(super) fn restore(&mut self, s: Snapshot) {
        self.body = s.body;
        self.parts = s.parts;
        self.eyes = s.eyes;
        self.ground = s.ground;
        self.rest = s.rest;
        self.canvas = s.canvas;
        self.canvas_fields = [s.canvas.0.to_string(), s.canvas.1.to_string()];
        self.select(s.selected);
    }

    /// One undo step per gesture: repeated messages of the same gesture
    /// share the checkpoint the first one made.
    pub fn checkpoint_gesture(&mut self, g: Gesture) {
        if self.gesture != Some(g) {
            self.checkpoint();
            self.gesture = Some(g);
        }
    }

    pub fn checkpoint(&mut self) {
        self.gesture = None;
        if self.undo.len() >= 30 {
            self.undo.remove(0);
        }
        self.undo.push(self.snapshot());
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(s);
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(s);
        }
    }
}
