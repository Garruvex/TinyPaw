use crate::core::Rectangle;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    /// A path filled with some paint.
    Fill {
        /// The path to fill.
        path: tiny_skia::Path,
        /// The paint to use.
        paint: tiny_skia::Paint<'static>,
        /// The fill rule to follow.
        rule: tiny_skia::FillRule,
    },
    /// A path stroked with some paint.
    Stroke {
        /// The path to stroke.
        path: tiny_skia::Path,
        /// The paint to use.
        paint: tiny_skia::Paint<'static>,
        /// The stroke settings.
        stroke: tiny_skia::Stroke,
    },
}

impl Primitive {
    /// Returns the visible bounds of the [`Primitive`].
    pub fn visible_bounds(&self) -> Rectangle {
        // TinyPaw patch: a stroke paints up to half its width (more at miter
        // joins) outside the path, plus a pixel of anti-aliasing. Using the
        // bare path bounds left stroke edges behind as damage-tracking ghosts.
        let (bounds, outset) = match self {
            Primitive::Fill { path, .. } => (path.bounds(), 1.0),
            Primitive::Stroke { path, stroke, .. } => {
                let half = stroke.width * 0.5;
                let join = match stroke.line_join {
                    tiny_skia::LineJoin::Miter | tiny_skia::LineJoin::MiterClip => {
                        stroke.miter_limit.max(1.0)
                    }
                    _ => 1.0,
                };
                (path.bounds(), half * join + 1.0)
            }
        };

        Rectangle {
            x: bounds.x() - outset,
            y: bounds.y() - outset,
            width: bounds.width() + outset * 2.0,
            height: bounds.height() + outset * 2.0,
        }
    }
}
