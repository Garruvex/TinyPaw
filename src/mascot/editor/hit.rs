//! Draw order and hit testing for the preview canvas.

use super::*;

impl Editor {
    pub fn draw_layers(&self) -> Vec<(Selected, &Layer)> {
        let mut out = Vec::new();
        for (i, _) in self
            .parts
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.on_head && p.behind)
        {
            self.append_part(i, &mut out);
        }
        out.push((Selected::Body, &self.body));
        for (i, _) in self
            .parts
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.on_head && !p.behind)
        {
            self.append_part(i, &mut out);
        }
        out
    }

    pub(super) fn append_part<'a>(&'a self, i: usize, out: &mut Vec<(Selected, &'a Layer)>) {
        let p = &self.parts[i];
        let is_head = p.kind == PartType::Head;
        if is_head {
            for (j, c) in self
                .parts
                .iter()
                .enumerate()
                .filter(|(_, c)| c.on_head && c.behind)
            {
                out.push((Selected::Part(j), &c.layer));
            }
        }
        let face = match self.selected {
            Selected::Face(j, s) if i == j => p.faces.get(s).and_then(Option::as_ref),
            _ => None,
        };
        out.push((
            if face.is_some() {
                self.selected
            } else {
                Selected::Part(i)
            },
            face.unwrap_or(&p.layer),
        ));
        if is_head {
            if face.is_none() {
                for eye in &self.eyes {
                    for l in [&eye.white, &eye.pupil].into_iter().flatten() {
                        out.push((Selected::Part(i), l));
                    }
                }
            }
            for (j, c) in self
                .parts
                .iter()
                .enumerate()
                .filter(|(_, c)| c.on_head && !c.behind)
            {
                out.push((Selected::Part(j), &c.layer));
            }
        }
    }

    pub fn hit_target(&self, x: f32, y: f32) -> Option<Selected> {
        if self.hit_selected(x, y) {
            return Some(self.selected);
        }
        self.draw_layers()
            .into_iter()
            .rev()
            .find(|(_, l)| layer_hit(l, x * self.canvas.0 as f32, y * self.canvas.1 as f32))
            .map(|(s, _)| s)
    }

    pub fn hit_selected(&self, x: f32, y: f32) -> bool {
        self.selected_layer()
            .is_some_and(|l| layer_hit(l, x * self.canvas.0 as f32, y * self.canvas.1 as f32))
    }
}
