//! The Art tab's model: an installed character opened for editing. Images are
//! held decoded and canvas-sized while editing; Save writes a fresh pack
//! through the same staged install as an import, so a failed save leaves the
//! installed character as it was.

use super::placement::{self, Placement, SourceRecord};
use std::path::Path;
use std::sync::Arc;

use iced::advanced::image::Handle;
use image::RgbaImage;

use super::motion::{MotionSpec, PartType};
use super::pack::{
    self, Character, EyeDef, Manifest, PartDef, Pose, Source, fit_png, installed_folder,
    read_manifest, read_png, stage_and_install,
};
use super::rest::RestMode;

/// A decoded, canvas-sized image and its handle for drawing.
#[derive(Debug, Clone)]
pub struct Layer {
    pub image: Arc<RgbaImage>,
    pub handle: Handle,
    pub placement: Placement,
}

impl Layer {
    fn new(image: RgbaImage) -> Self {
        let handle = Handle::from_rgba(image.width(), image.height(), image.as_raw().clone());
        Self {
            image: Arc::new(image),
            handle,
            placement: Placement::default(),
        }
    }

    /// Retain every visible pixel while removing unused source padding.
    fn compact_source(&self) -> Self {
        let mut bounds = None;
        for (x, y, pixel) in self.image.enumerate_pixels() {
            if pixel[3] != 0 {
                let (x0, y0, x1, y1) = bounds.get_or_insert((x, y, x, y));
                *x0 = (*x0).min(x);
                *y0 = (*y0).min(y);
                *x1 = (*x1).max(x);
                *y1 = (*y1).max(y);
            }
        }
        let Some((x0, y0, x1, y1)) = bounds else {
            return self.clone();
        };
        let (width, height) = (x1 - x0 + 1, y1 - y0 + 1);
        let centre = self.placement.map(
            (
                x0 as f32 + width as f32 / 2.0,
                y0 as f32 + height as f32 / 2.0,
            ),
            self.image.dimensions(),
        );
        let mut source = Self::new(
            image::imageops::crop_imm(self.image.as_ref(), x0, y0, width, height).to_image(),
        );
        source.placement = Placement {
            x: centre.0 - width as f32 * self.placement.scale / 2.0,
            y: centre.1 - height as f32 * self.placement.scale / 2.0,
            ..self.placement
        };
        source
    }

    /// Centre and bottom of the visible pixels, as canvas fractions.
    fn visible_centre(&self, canvas: (u32, u32)) -> Option<(f32, f32, f32)> {
        let img = &self.image;
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for (x, y, p) in img.enumerate_pixels() {
            if p[3] >= pack::SOLID {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
        (x0 <= x1).then(|| {
            let centre = self.placement.map(
                ((x0 + x1 + 1) as f32 / 2.0, (y0 + y1 + 1) as f32 / 2.0),
                img.dimensions(),
            );
            let bottom = self.placement.map(
                ((x0 + x1 + 1) as f32 / 2.0, (y1 + 1) as f32),
                img.dimensions(),
            );
            (
                centre.0 / canvas.0 as f32,
                centre.1 / canvas.1 as f32,
                bottom.1 / canvas.1 as f32,
            )
        })
    }
}

#[derive(Debug, Clone)]
pub struct EditPart {
    /// What the layer list calls it, such as "Left eye". Starts from the
    /// imported file's name; the runtime type stays separate.
    pub name: String,
    pub kind: PartType,
    pub layer: Layer,
    pub pivot: (f32, f32),
    /// Rides on the head (only when there is one, and this isn't it).
    pub on_head: bool,
    pub behind: bool,
    pub motion: MotionSpec,
    /// Head faces: blink, pressed, dragging.
    pub faces: [Option<Layer>; 4],
}

#[derive(Debug, Clone, Default)]
pub struct EditEye {
    pub white: Option<Layer>,
    pub pupil: Option<Layer>,
    pub mask: Option<Layer>,
    pub blink: Option<Layer>,
    /// Pupil travel, as a fraction of the canvas width (height uses 3/4).
    pub travel: f32,
}

impl EditEye {
    fn slot_mut(&mut self, slot: EyeSlot) -> &mut Option<Layer> {
        match slot {
            EyeSlot::White => &mut self.white,
            EyeSlot::Pupil => &mut self.pupil,
            EyeSlot::Mask => &mut self.mask,
            EyeSlot::Blink => &mut self.blink,
        }
    }
}

/// The eye images, in the order the Art tab shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EyeSlot {
    White,
    Pupil,
    Mask,
    Blink,
}

/// "How are the eyes drawn?" in the Eyes section. A starting workflow,
/// not a conversion: static and tracking eyes can both exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EyeStyle {
    /// Painted into the head (or body) art: nothing to set up.
    InArt,
    /// Separate complete eye and eyebrow images that ride on the head.
    Separate,
    /// Eyes that follow the cursor: background, moving pupil and mask.
    Tracking,
}

/// Display names for head faces, by slot. File names keep `FACE_NAMES`.
pub const FACE_LABELS: [&str; 4] = ["Eyes closed", "Pressed", "Being carried", "Headpat"];

/// A readable layer name from a file name: `eye_screen_left.png` becomes
/// "Eye screen left".
pub fn display_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let words: Vec<&str> = stem
        .split(['_', '-', ' ', '.'])
        .filter(|w| !w.is_empty())
        .collect();
    let mut name = words.join(" ").to_lowercase();
    if let Some(first) = name.get(0..1) {
        name = first.to_uppercase() + &name[1..];
    }
    name.chars().take(placement::MAX_NAME).collect()
}

/// What the preview's clicks move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selected {
    Body,
    Face(usize, usize),
    Part(usize),
}

/// Which image a picked PNG becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Body,
    NewPart,
    /// A complete eye or eyebrow image: a still part riding on the head.
    EyePart,
    /// The whole character in a carried pose, for Pick up's held pose.
    Held,
    /// A head face: part index and face slot (blink, pressed, dragging).
    Face(usize, usize),
    Eye(usize, EyeSlot),
}

/// A continuous edit that undo restores as one step, however many
/// messages it took: holding an arrow key, typing a number, dragging a
/// slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Nudge,
    Field(usize),
    Travel(usize),
    EyeSize(usize),
    EyeLayerSize(usize, EyeSlot),
    Rename(usize),
}

/// Advanced → Resting point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestAction {
    Adjust,
    Done,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Arrange,
    Pivot,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Light,
    Dark,
    Checker,
}
#[derive(Debug, Clone)]
struct Snapshot {
    body: Layer,
    parts: Vec<EditPart>,
    eyes: Vec<EditEye>,
    ground: (f32, f32),
    rest: RestMode,
    selected: Selected,
    canvas: (u32, u32),
}

/// Where a held pose made in the editor is written in the pack.
const HELD_FILE: &str = "reactions/held.png";

pub const FACE_NAMES: [&str; 4] = ["Blink", "Pressed", "Dragging", "Petted"];
const DEFAULT_TRAVEL: f32 = 0.012;

#[derive(Debug, Clone)]
pub struct Editor {
    pub id: &'static str,
    pub name: String,
    pub canvas: (u32, u32),
    pub ground: (f32, f32),
    /// Whether `ground` follows the body art or was set by hand.
    pub rest: RestMode,
    /// The temporary Adjust manually tool: preview clicks move the
    /// resting point until Done.
    pub adjusting_rest: bool,
    pub body: Layer,
    pub parts: Vec<EditPart>,
    pub eyes: Vec<EditEye>,
    pub selected: Selected,
    pub mode: Mode,
    pub zoom: f32,
    pub background: Background,
    /// The Eyes section's chosen workflow; `None` follows the art.
    pub eye_style: Option<EyeStyle>,
    /// The gesture the last undo checkpoint belongs to.
    gesture: Option<Gesture>,
    /// A held pose added here, written as the pack's `reactions/held.png`.
    pub held: Option<Layer>,
    reference_art: Vec<(super::reactions::Event, Layer)>,
    pub fields: [String; 4],
    pub canvas_fields: [String; 2],
    pub new_install: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    other_sources: Vec<SourceRecord>,
    source: Option<Source>,
    author: Option<String>,
    /// The pack's lines, bop and reactions, kept as they are by an art save.
    lines: super::lines::Lines,
    bop: Option<super::motion::Bop>,
    music_art: Option<super::pack::MusicArtwork>,
    reactions: super::reactions::Config,
    /// Poses this editor doesn't show, kept as they are.
    others: Vec<Pose>,
    pose: String,
}

impl Editor {
    /// Opens an installed character's default pose.
    pub fn open(c: &Character) -> Result<Self, String> {
        Self::read(&installed_folder(c)?, c.id)
    }

    fn read(folder: &Path, id: &'static str) -> Result<Self, String> {
        let folder = folder.to_path_buf();
        let m = read_manifest(&folder)?;
        let pose = m.default_pose().clone();
        let png = |asset: &str| {
            if let Some(r) = m.editor.iter().find(|r| r.target == asset) {
                let mut layer = Layer::new(read_png(&folder, &r.source, r.size)?);
                layer.placement = r.placement;
                Ok(layer)
            } else {
                read_png(&folder, asset, m.canvas).map(Layer::new)
            }
        };
        let mut parts = Vec::new();
        for p in &pose.parts {
            let mut faces = [None, None, None, None];
            for (slot, face) in faces.iter_mut().zip(&p.faces) {
                if let Some(asset) = face {
                    *slot = Some(png(asset)?);
                }
            }
            let name = m
                .editor
                .iter()
                .find(|r| r.target == p.image)
                .and_then(|r| r.name.clone())
                .unwrap_or_else(|| format!("{} {}", p.kind, parts.len() + 1));
            parts.push(EditPart {
                name,
                kind: p.kind,
                layer: png(&p.image)?,
                pivot: p.pivot,
                on_head: p.parent.is_some(),
                behind: p.behind,
                motion: p.motion,
                faces,
            });
        }
        let mut eyes = Vec::new();
        for e in &pose.eyes {
            eyes.push(EditEye {
                white: Some(png(&e.white)?),
                pupil: Some(png(&e.pupil)?),
                mask: Some(png(&e.mask)?),
                blink: e.blink.as_deref().map(png).transpose()?,
                travel: e.max_offset.0,
            });
        }
        let mut editor = Self {
            id,
            name: m.name.clone(),
            canvas: m.canvas,
            ground: pose.ground,
            rest: m.rest,
            adjusting_rest: false,
            body: png(&pose.body)?,
            parts,
            eyes,
            selected: Selected::Body,
            mode: Mode::Arrange,
            zoom: 1.0,
            background: Background::Checker,
            eye_style: None,
            gesture: None,
            held: None,
            reference_art: Vec::new(),
            fields: Default::default(),
            canvas_fields: [m.canvas.0.to_string(), m.canvas.1.to_string()],
            new_install: false,
            undo: Vec::new(),
            redo: Vec::new(),
            other_sources: m
                .editor
                .iter()
                .filter(|r| !pose.assets().contains(&r.target.as_str()))
                .cloned()
                .collect(),
            source: m.source,
            author: m.author.clone(),
            lines: m.lines.clone(),
            bop: m.bop,
            music_art: m.music_art.clone(),
            reactions: m.reactions.clone(),
            others: m
                .poses
                .iter()
                .filter(|p| p.name != m.default_pose)
                .cloned()
                .collect(),
            pose: m.default_pose.clone(),
        };
        // The body starts selected, so its fields show its placement.
        editor.sync_fields();
        Ok(editor)
    }

    pub fn canvas_hint(&self) -> String {
        format!(
            "Stage: {} × {}. Cropped PNGs keep their proportions; matching canvases retain their alignment.",
            self.canvas.0, self.canvas.1
        )
    }

    pub fn head(&self) -> Option<usize> {
        self.parts.iter().position(|p| p.kind == PartType::Head)
    }

    /// Resolve the head before static eye/eyebrow layers regardless of picker order.
    /// Every source reports its own error and valid earlier layers remain usable.
    pub fn set_images(
        &mut self,
        target: Target,
        mut paths: Vec<std::path::PathBuf>,
    ) -> Vec<String> {
        if matches!(target, Target::NewPart | Target::EyePart) && self.head().is_none() {
            paths.sort_by_key(|p| {
                !p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains("head")
            });
        }
        let mut errors = Vec::new();
        for path in paths {
            if let Err(why) = self.set_image(target, &path) {
                errors.push(format!(
                    "{}: {why}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
        }
        errors
    }

    /// Puts a picked PNG where `target` says, fitted to the canvas.
    pub fn set_image(&mut self, target: Target, path: &Path) -> Result<(), String> {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > pack::MAX_FILE {
            return Err("the PNG is larger than 64 MiB".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let dimensions =
            image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
                .into_dimensions()
                .map_err(|e| format!("not a readable PNG ({e})"))?;
        if dimensions.0 > pack::MAX_SOURCE_SIDE || dimensions.1 > pack::MAX_SOURCE_SIDE {
            return Err("source PNG dimensions exceed 8192 pixels".into());
        }
        if self.decoded_bytes() + u64::from(dimensions.0) * u64::from(dimensions.1) * 8
            > 128 * 1024 * 1024
        {
            return Err("editor sources exceed the 128 MiB decoded memory budget; remove unused layers or start a new edit".into());
        }
        let original = self.source.is_some_and(|s| s.size == dimensions);
        let previous_canvas =
            dimensions == self.body.image.dimensions() && dimensions != self.canvas;
        let aligned = dimensions == self.canvas || original || previous_canvas;
        let fit_canvas = if original || previous_canvas {
            self.body.image.dimensions()
        } else {
            self.canvas
        };
        let art = if aligned {
            fit_png(path, fit_canvas, self.source.as_ref())?
        } else {
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?
                .to_rgba8()
        };
        if !art.pixels().any(|p| p[3] >= pack::SOLID) {
            return Err("that PNG is empty".into());
        }
        let mut layer = Layer::new(art);
        if !aligned {
            layer.placement = Placement::centred(layer.image.dimensions(), self.canvas);
        } else if original || previous_canvas {
            layer.placement = self.body.placement;
        }
        if let Target::Face(part, _) = target
            && aligned
            && let Some(base) = self.parts.get(part)
            && base.layer.image.dimensions() == layer.image.dimensions()
        {
            layer.placement = base.layer.placement;
        }
        if let Target::Face(part, _) = target
            && !aligned
            && let Some(base) = self.parts.get(part)
        {
            let size = base.layer.image.dimensions();
            let centre = base
                .layer
                .placement
                .map((size.0 as f32 / 2.0, size.1 as f32 / 2.0), size);
            let scale = (size.0 as f32 * base.layer.placement.scale / layer.image.width() as f32)
                .min(size.1 as f32 * base.layer.placement.scale / layer.image.height() as f32)
                .clamp(0.01, 8.0);
            layer.placement = Placement {
                x: centre.0 - layer.image.width() as f32 * scale / 2.0,
                y: centre.1 - layer.image.height() as f32 * scale / 2.0,
                scale,
                angle: base.layer.placement.angle,
            };
        }
        // Invalid slots must not create undo entries or discard valid draft work.
        match target {
            Target::NewPart | Target::EyePart if self.parts.len() >= 16 => {
                return Err("a character can have at most 16 parts".into());
            }
            Target::Face(i, _) if self.parts.get(i).is_none_or(|p| p.kind != PartType::Head) => {
                return Err("only the head has faces".into());
            }
            Target::Eye(i, _) if i >= self.eyes.len() => return Err("that eye is gone".into()),
            _ => {}
        }
        self.checkpoint();
        match target {
            Target::Body => {
                self.body = layer;
                self.select(Selected::Body);
                self.settle_rest();
            }
            Target::NewPart => {
                if self.parts.len() >= 16 {
                    return Err("a character can have at most 16 parts".into());
                }
                let kind = guess_kind(path, self.head().is_some());
                let name = display_name(path);
                let (cx, cy, bottom) = layer.visible_centre(self.canvas).unwrap_or((0.5, 0.5, 0.5));
                // A head turns at the neck; anything else from its middle.
                let pivot = if kind == PartType::Head {
                    (cx, bottom)
                } else {
                    (cx, cy)
                };
                self.parts.push(EditPart {
                    name,
                    kind,
                    on_head: matches!(kind, PartType::Ear | PartType::Hair | PartType::Other)
                        && self.head().is_some(),
                    behind: kind.behind_by_default(),
                    motion: MotionSpec::default_for(kind),
                    layer,
                    pivot,
                    faces: [None, None, None, None],
                });
                self.select(Selected::Part(self.parts.len() - 1));
            }
            Target::Held => {
                self.held = Some(layer);
            }
            Target::EyePart => {
                let (cx, cy, _) = layer.visible_centre(self.canvas).unwrap_or((0.5, 0.5, 0.5));
                self.parts.push(EditPart {
                    name: display_name(path),
                    kind: PartType::Other,
                    on_head: self.head().is_some(),
                    behind: false,
                    motion: MotionSpec::of(super::motion::Preset::Still),
                    layer,
                    pivot: (cx, cy),
                    faces: [None, None, None, None],
                });
                self.select(Selected::Part(self.parts.len() - 1));
            }
            Target::Face(part, slot) => {
                let p = self.parts.get_mut(part).ok_or("that part is gone")?;
                if p.kind != PartType::Head {
                    return Err("only the head has faces".into());
                }
                *p.faces.get_mut(slot).ok_or("Unknown head expression.")? = Some(layer);
                self.select(Selected::Face(part, slot));
            }
            Target::Eye(eye, slot) => {
                let e = self.eyes.get_mut(eye).ok_or("that eye is gone")?;
                *e.slot_mut(slot) = Some(layer);
            }
        }
        Ok(())
    }

    /// The Eyes section's workflow: the creator's choice, else what the
    /// artwork already has.
    pub fn eye_style(&self) -> EyeStyle {
        self.eye_style.unwrap_or(if !self.eyes.is_empty() {
            EyeStyle::Tracking
        } else if self
            .parts
            .iter()
            .any(|p| p.kind == PartType::Other && p.name.to_lowercase().contains("eye"))
        {
            EyeStyle::Separate
        } else {
            EyeStyle::InArt
        })
    }

    pub fn rename(&mut self, part: usize, name: String) {
        if part < self.parts.len() {
            self.checkpoint_gesture(Gesture::Rename(part));
        }
        if let Some(p) = self.parts.get_mut(part) {
            p.name = name.chars().take(placement::MAX_NAME).collect();
        }
    }

    /// Whether a part turns, so its joint matters: anything but a still
    /// static layer.
    pub fn has_joint(&self, part: usize) -> bool {
        self.parts.get(part).is_some_and(|p| {
            p.kind != PartType::Other || p.motion.preset != super::motion::Preset::Still
        })
    }

    /// The temporary Adjust joint tool for `part`: preview clicks move only
    /// that joint until Done or Escape.
    pub fn adjust_joint(&mut self, part: usize) {
        if self.has_joint(part) {
            self.adjusting_rest = false;
            self.select(Selected::Part(part));
            self.mode = Mode::Pivot;
        }
    }

    pub fn finish_joint(&mut self) {
        self.mode = Mode::Arrange;
    }

    /// The part whose joint is being adjusted.
    pub fn joint_part(&self) -> Option<usize> {
        match (self.mode, self.selected) {
            (Mode::Pivot, Selected::Part(i)) => Some(i),
            _ => None,
        }
    }

    pub fn set_kind(&mut self, part: usize, kind: PartType) {
        let has_other_head = self
            .parts
            .iter()
            .enumerate()
            .any(|(i, p)| i != part && p.kind == PartType::Head);
        if kind == PartType::Head && has_other_head {
            return;
        }
        if let Some(p) = self.parts.get_mut(part) {
            p.kind = kind;
            p.motion = MotionSpec::default_for(kind);
            p.behind = kind.behind_by_default();
            if kind != PartType::Head {
                p.faces = [None, None, None, None];
            } else {
                p.on_head = false;
            }
        }
        if self.head().is_none() {
            for p in &mut self.parts {
                p.on_head = false;
            }
        }
    }

    pub fn remove_part(&mut self, part: usize) {
        if part >= self.parts.len() {
            return;
        }
        self.parts.remove(part);
        if self.head().is_none() {
            for p in &mut self.parts {
                p.on_head = false;
            }
        }
        self.select(Selected::Body);
    }

    /// A click on the preview, as canvas fractions: the resting point while
    /// adjusting it, otherwise the selected part's joint.
    pub fn click(&mut self, x: f32, y: f32) {
        let at = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if self.adjusting_rest {
            self.ground = at;
            self.rest = RestMode::Manual;
            return;
        }
        // Only the Adjust joint tool moves a joint.
        if self.mode == Mode::Pivot
            && let Selected::Part(i) | Selected::Face(i, _) = self.selected
            && let Some(p) = self.parts.get_mut(i)
        {
            p.pivot = at;
        }
    }

    pub fn set_reference_reaction(
        &mut self,
        event: super::reactions::Event,
        path: &Path,
    ) -> Result<(), String> {
        let held = self.held.take();
        let result = self.set_image(Target::Held, path);
        if result.is_ok() {
            if let Some(layer) = self.held.take() {
                self.reference_art.push((event, layer));
            }
        }
        self.held = held;
        result
    }
    pub fn enable_reference_headpat(&mut self) {
        self.reactions
            .companionship
            .get_or_insert_with(Default::default)
            .petting = true;
    }

    /// The draft as a rig, so Test character shows unsaved artwork. Baking
    /// every layer is slow, so callers run it on a worker.
    pub fn draft_rig(&self) -> super::rig::Rig {
        use super::rig::{EyeArt, LayeredArt, PartArt, Rig};
        let bake = |l: &Layer| placement::bake(&l.image, l.placement, self.canvas);
        let ground = match self.rest {
            RestMode::Auto => self.auto_rest().unwrap_or(self.ground),
            RestMode::Manual => self.ground,
        };
        let body = bake(&self.body);
        let eyes: Vec<EyeArt> = self
            .eyes
            .iter()
            .filter_map(|e| {
                Some(EyeArt {
                    white: bake(e.white.as_ref()?),
                    pupil: bake(e.pupil.as_ref()?),
                    mask: bake(e.mask.as_ref()?),
                    blink: e.blink.as_ref().map(bake),
                    max_offset: (e.travel, e.travel * 0.75),
                })
            })
            .collect();
        if self.parts.is_empty() && eyes.is_empty() {
            return Rig::sprite(body, ground);
        }
        let head = self.head();
        let parts = self
            .parts
            .iter()
            .map(|p| PartArt {
                kind: p.kind,
                image: bake(&p.layer),
                pivot: p.pivot,
                parent: if p.on_head && p.kind != PartType::Head {
                    head
                } else {
                    None
                },
                behind: p.behind,
                motion: p.motion,
                faces: [0, 1, 2, 3].map(|i| p.faces[i].as_ref().map(bake)),
            })
            .collect();
        Rig::layered(LayeredArt {
            body,
            ground,
            parts,
            eyes,
        })
    }

    /// The automatic resting point for the body as arranged, as canvas
    /// fractions.
    pub fn auto_rest(&self) -> Option<(f32, f32)> {
        let baked = placement::bake(&self.body.image, self.body.placement, self.canvas);
        super::rest::estimate(&baked)
    }

    /// Re-estimates an automatic resting point once the body has settled
    /// (replaced, or a save); a manual one is left alone.
    pub fn settle_rest(&mut self) {
        if self.rest == RestMode::Auto
            && let Some(at) = self.auto_rest()
        {
            self.ground = at;
        }
    }

    /// Advanced → Resting point → Adjust manually: starts from the current
    /// point; clicks move it until Done.
    pub fn adjust_rest(&mut self) {
        self.settle_rest();
        self.adjusting_rest = true;
    }

    pub fn finish_rest(&mut self) {
        self.adjusting_rest = false;
    }

    /// Back to the automatic estimate.
    pub fn reset_rest(&mut self) {
        self.checkpoint();
        self.rest = RestMode::Auto;
        self.adjusting_rest = false;
        self.settle_rest();
    }

    pub fn add_eye(&mut self) {
        if self.eyes.len() < 4 {
            self.eyes.push(EditEye {
                travel: DEFAULT_TRAVEL,
                ..EditEye::default()
            });
        }
    }

    /// Scale the tracking layers together around the visible eye background.
    pub fn set_eye_size(&mut self, index: usize, scale: f32) -> Result<(), String> {
        let mut eye = self.eyes.get(index).ok_or("Unknown eye.")?.clone();
        let white = eye
            .white
            .as_ref()
            .ok_or("Choose an eye background first.")?;
        let (cx, cy, _) = white
            .visible_centre(self.canvas)
            .ok_or("The eye background is empty.")?;
        let (x, y) = (cx * self.canvas.0 as f32, cy * self.canvas.1 as f32);
        let ratio = scale / white.placement.scale;
        for layer in [
            &mut eye.white,
            &mut eye.mask,
            &mut eye.pupil,
            &mut eye.blink,
        ]
        .into_iter()
        .flatten()
        {
            let p = &mut layer.placement;
            p.x = x + (p.x - x) * ratio;
            p.y = y + (p.y - y) * ratio;
            p.scale *= ratio;
            if !p.valid(self.canvas) {
                return Err("This eye size exceeds the stage's placement range.".into());
            }
        }
        self.checkpoint_gesture(Gesture::EyeSize(index));
        self.eyes[index] = eye;
        Ok(())
    }

    /// Resize a single eye layer around its visible centre.
    pub fn set_eye_layer_size(
        &mut self,
        index: usize,
        slot: EyeSlot,
        scale: f32,
    ) -> Result<(), String> {
        let layer = self
            .eyes
            .get(index)
            .and_then(|eye| match slot {
                EyeSlot::White => eye.white.as_ref(),
                EyeSlot::Mask => eye.mask.as_ref(),
                EyeSlot::Blink => eye.blink.as_ref(),
                EyeSlot::Pupil => eye.pupil.as_ref(),
            })
            .ok_or("Choose an eye image first.")?;
        if !scale.is_finite() {
            return Err("Eye layer size must be a finite number.".into());
        }
        let centre = layer
            .visible_centre(self.canvas)
            .ok_or("The eye image is empty.")?;
        let mut placement = layer.placement;
        placement.scale = scale.clamp(0.01, 8.0);
        let mut resized = layer.clone();
        resized.placement = placement;
        let next = resized
            .visible_centre(self.canvas)
            .ok_or("The eye image is empty.")?;
        placement.x += (centre.0 - next.0) * self.canvas.0 as f32;
        placement.y += (centre.1 - next.1) * self.canvas.1 as f32;
        if !placement.valid(self.canvas) {
            return Err("This eye layer size exceeds the stage's placement range.".into());
        }
        self.checkpoint_gesture(Gesture::EyeLayerSize(index, slot));
        resized.placement = placement;
        *self.eyes[index].slot_mut(slot) = Some(resized);
        Ok(())
    }
}

impl Editor {
    pub fn from_body(path: &Path, taken: &[&str]) -> Result<Self, String> {
        Self::from_body_into(
            &pack::user_dir().ok_or("no per-user data folder")?,
            path,
            taken,
        )
    }

    fn from_body_into(root: &Path, path: &Path, taken: &[&str]) -> Result<Self, String> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Character")
            .chars()
            .take(60)
            .collect::<String>();
        let size = image::ImageReader::open(path)
            .map_err(|e| e.to_string())?
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .into_dimensions()
            .map_err(|e| e.to_string())?;
        let canvas = (
            (size.0.saturating_mul(2)).clamp(128, 2048),
            (size.1.saturating_mul(2)).clamp(128, 2048),
        );
        let id = crate::platform::intern(&pack::unique_id(root, &name, taken));
        let mut editor = Self {
            id,
            name,
            canvas,
            ground: (0.5, 0.9),
            rest: RestMode::Auto,
            adjusting_rest: false,
            body: Layer::new(RgbaImage::new(1, 1)),
            parts: Vec::new(),
            eyes: Vec::new(),
            selected: Selected::Body,
            source: None,
            author: None,
            lines: super::lines::Lines::default(),
            bop: None,
            music_art: None,
            // A new character's touch events start at None, written out.
            reactions: super::reactions::new_character(),
            others: Vec::new(),
            pose: "sit".into(),
            mode: Mode::Arrange,
            zoom: 1.0,
            background: Background::Checker,
            eye_style: None,
            gesture: None,
            held: None,
            reference_art: Vec::new(),
            fields: Default::default(),
            canvas_fields: [canvas.0.to_string(), canvas.1.to_string()],
            new_install: true,
            undo: Vec::new(),
            redo: Vec::new(),
            other_sources: Vec::new(),
        };
        editor.set_image(Target::Body, path)?;
        editor.undo.clear();
        Ok(editor)
    }

    fn layers(&self) -> Vec<&Layer> {
        layers(&self.body, &self.parts, &self.eyes)
    }
    fn decoded_bytes(&self) -> u64 {
        let mut seen = std::collections::HashSet::new();
        self.layers()
            .into_iter()
            .chain(
                self.undo
                    .iter()
                    .chain(&self.redo)
                    .flat_map(|s| layers(&s.body, &s.parts, &s.eyes)),
            )
            .filter(|l| seen.insert(Arc::as_ptr(&l.image) as usize))
            .map(|l| u64::from(l.image.width()) * u64::from(l.image.height()) * 8)
            .sum()
    }
    pub fn selected_layer(&self) -> Option<&Layer> {
        match self.selected {
            Selected::Body => Some(&self.body),
            Selected::Part(i) => self.parts.get(i).map(|p| &p.layer),
            Selected::Face(i, s) => self
                .parts
                .get(i)
                .and_then(|p| p.faces.get(s))
                .and_then(Option::as_ref),
        }
    }
    fn selected_layer_mut(&mut self) -> Option<&mut Layer> {
        match self.selected {
            Selected::Body => Some(&mut self.body),
            Selected::Part(i) => self.parts.get_mut(i).map(|p| &mut p.layer),
            Selected::Face(i, s) => self
                .parts
                .get_mut(i)
                .and_then(|p| p.faces.get_mut(s))
                .and_then(Option::as_mut),
        }
    }
    pub fn select(&mut self, selected: Selected) {
        // Choosing another layer always leaves the joint tool.
        if self.selected != selected {
            self.mode = Mode::Arrange;
        }
        self.selected = selected;
        self.sync_fields();
    }
    fn sync_fields(&mut self) {
        self.fields = self
            .selected_layer()
            .map(|l| {
                [
                    format!("{:.1}", l.placement.x),
                    format!("{:.1}", l.placement.y),
                    format!("{:.1}", l.placement.scale * 100.0),
                    format!("{:.1}", l.placement.angle),
                ]
            })
            .unwrap_or_default();
    }
    pub fn field(&mut self, index: usize, text: String) {
        if index >= 4 || text.len() > 20 {
            return;
        }
        if let Ok(number) = text.parse::<f32>()
            && let Some(layer) = self.selected_layer()
        {
            let mut placement = layer.placement;
            match index {
                0 => placement.x = number,
                1 => placement.y = number,
                2 => placement.scale = number / 100.0,
                _ => placement.angle = number,
            }
            if placement.valid(self.canvas) {
                self.checkpoint_gesture(Gesture::Field(index));
                self.place(placement);
            }
        }
        self.fields[index] = text;
    }
    /// Adjust a joint along with its artwork. Head children and authored faces
    /// receive the same canvas-space transform, so arrangements stay assembled.
    pub fn place(&mut self, next: Placement) {
        if !next.valid(self.canvas) {
            return;
        }
        let Some(layer) = self.selected_layer() else {
            return;
        };
        let old = layer.placement;
        let size = layer.image.dimensions();
        let canvas = self.canvas;
        let point = |p: (f32, f32)| next.map(old.unmap(p, size), size);
        let pivot = |p: (f32, f32)| {
            let p = point((p.0 * canvas.0 as f32, p.1 * canvas.1 as f32));
            (p.0 / canvas.0 as f32, p.1 / canvas.1 as f32)
        };
        let transform = |l: &mut Layer| {
            let d = l.image.dimensions();
            let centre = l.placement.map((d.0 as f32 / 2.0, d.1 as f32 / 2.0), d);
            let c = point(centre);
            let scale = l.placement.scale * next.scale / old.scale;
            l.placement = Placement {
                x: c.0 - d.0 as f32 * scale / 2.0,
                y: c.1 - d.1 as f32 * scale / 2.0,
                scale,
                angle: wrap_angle(l.placement.angle + next.angle - old.angle),
            };
        };
        match self.selected {
            Selected::Body => self.ground = pivot(self.ground),
            Selected::Part(i) => {
                let is_head = self.parts[i].kind == PartType::Head;
                self.parts[i].pivot = pivot(self.parts[i].pivot);
                for face in self.parts[i].faces.iter_mut().flatten() {
                    transform(face);
                }
                if is_head {
                    for (j, part) in self.parts.iter_mut().enumerate() {
                        if j != i && part.on_head {
                            transform(&mut part.layer);
                            part.pivot = pivot(part.pivot);
                        }
                    }
                    for eye in &mut self.eyes {
                        for layer in [
                            &mut eye.white,
                            &mut eye.pupil,
                            &mut eye.mask,
                            &mut eye.blink,
                        ]
                        .into_iter()
                        .flatten()
                        {
                            transform(layer);
                        }
                    }
                }
            }
            _ => {}
        }
        if let Some(layer) = self.selected_layer_mut() {
            layer.placement = next;
        }
        self.sync_fields();
    }
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if let Some(layer) = self.selected_layer() {
            self.place(Placement {
                x: layer.placement.x + dx,
                y: layer.placement.y + dy,
                ..layer.placement
            });
        }
    }
    pub fn reorder(&mut self, index: usize, up: bool) {
        let other = if up {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        };
        if let Some(other) = other
            && index < self.parts.len()
            && other < self.parts.len()
        {
            self.checkpoint();
            self.parts.swap(index, other);
            self.select(Selected::Part(other));
        }
    }
    pub fn keep_eye_static(&mut self, index: usize) -> Result<(), String> {
        let eye = self.eyes.get(index).ok_or("that eye is gone")?;
        let count = usize::from(eye.white.is_some()) + usize::from(eye.pupil.is_some());
        if count == 0 {
            return Err("choose visible eye art first, or remove the empty tracking eye".into());
        }
        if self.parts.len() + count > 16 {
            return Err("there is no room for the static eye layers (16 part limit)".into());
        }
        self.checkpoint();
        let eye = self.eyes.remove(index);
        for (label, layer) in [("background", eye.white), ("pupil", eye.pupil)] {
            let Some(layer) = layer else { continue };
            self.parts.push(EditPart {
                name: format!("Eye {} {label}", index + 1),
                kind: PartType::Other,
                layer,
                pivot: (0.5, 0.5),
                on_head: self.head().is_some(),
                behind: false,
                motion: MotionSpec::default_for(PartType::Other),
                faces: [None, None, None, None],
            });
        }
        self.select(Selected::Part(self.parts.len() - 1));
        Ok(())
    }

    pub fn validation_error(&self) -> Option<String> {
        for text in &self.canvas_fields {
            if text
                .parse::<u32>()
                .ok()
                .is_none_or(|n| !(1..=2048).contains(&n))
            {
                return Some(
                    "Stage dimensions must be whole numbers from 1 to 2048 pixels.".into(),
                );
            }
        }
        if self.selected_layer().is_some() {
            let values = self
                .fields
                .iter()
                .map(|s| s.parse::<f32>())
                .collect::<Result<Vec<_>, _>>();
            let Ok(v) = values else {
                return Some("Complete the position, scale and angle fields before saving.".into());
            };
            if !(Placement {
                x: v[0],
                y: v[1],
                scale: v[2] / 100.0,
                angle: v[3],
            })
            .valid(self.canvas)
            {
                return Some("Use a scale from 1–800%, angle from −180–180°, and positions within twice the stage size.".into());
            }
        }
        if self
            .layers()
            .iter()
            .any(|l| !l.placement.valid(self.canvas))
        {
            return Some("A layer lies outside the supported transform range. Enlarge the stage or adjust its position/scale.".into());
        }
        if std::iter::once(self.ground)
            .chain(self.parts.iter().map(|p| p.pivot))
            .any(|p| !(0.0..=1.0).contains(&p.0) || !(0.0..=1.0).contains(&p.1))
        {
            return Some("The resting point and attachment points must stay inside the stage. Enlarge the stage or adjust the joint.".into());
        }
        None
    }

    pub fn can_resize_canvas(&self) -> bool {
        self.others.is_empty()
    }
    pub fn resize_canvas(&mut self, axis: usize, text: String) {
        if axis > 1 || text.len() > 5 || !self.can_resize_canvas() {
            return;
        }
        if let Ok(side) = text.parse::<u32>()
            && (1..=2048).contains(&side)
        {
            let old = self.canvas;
            let next = if axis == 0 {
                (side, old.1)
            } else {
                (old.0, side)
            };
            self.checkpoint();
            self.canvas = next;
            let joint = |p: (f32, f32)| {
                (
                    p.0 * old.0 as f32 / next.0 as f32,
                    p.1 * old.1 as f32 / next.1 as f32,
                )
            };
            self.ground = joint(self.ground);
            for p in &mut self.parts {
                p.pivot = joint(p.pivot);
            }
        }
        self.canvas_fields[axis] = text;
    }
    pub fn nudge(&mut self, dx: f32, dy: f32) {
        if self.mode == Mode::Arrange && !self.adjusting_rest {
            self.translate(dx, dy);
        } else {
            let at = match self.selected {
                _ if self.adjusting_rest => self.ground,
                Selected::Part(i) | Selected::Face(i, _) => {
                    let Some(p) = self.parts.get(i) else { return };
                    p.pivot
                }
                Selected::Body => return,
            };
            self.click(
                at.0 + dx / self.canvas.0 as f32,
                at.1 + dy / self.canvas.1 as f32,
            );
        }
    }
    pub fn clipped(&self) -> bool {
        self.layers().iter().any(|l| {
            let d = l.image.dimensions();
            [
                (0.0, 0.0),
                (d.0 as f32, 0.0),
                (0.0, d.1 as f32),
                (d.0 as f32, d.1 as f32),
            ]
            .iter()
            .any(|&p| {
                let p = l.placement.map(p, d);
                p.0 < 0.0 || p.1 < 0.0 || p.0 > self.canvas.0 as f32 || p.1 > self.canvas.1 as f32
            })
        })
    }
}

fn layer_hit(layer: &Layer, x: f32, y: f32) -> bool {
    let p = layer.placement.unmap((x, y), layer.image.dimensions());
    p.0 >= 0.0
        && p.1 >= 0.0
        && p.0 < layer.image.width() as f32
        && p.1 < layer.image.height() as f32
        && layer.image.get_pixel(p.0 as u32, p.1 as u32)[3] >= pack::SOLID
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}
fn layers<'a>(body: &'a Layer, parts: &'a [EditPart], eyes: &'a [EditEye]) -> Vec<&'a Layer> {
    std::iter::once(body)
        .chain(
            parts
                .iter()
                .flat_map(|p| std::iter::once(&p.layer).chain(p.faces.iter().flatten())),
        )
        .chain(eyes.iter().flat_map(|e| {
            [&e.white, &e.pupil, &e.mask, &e.blink]
                .into_iter()
                .flatten()
        }))
        .collect()
}

/// A part type from the file name, e.g. `fox-tail.png`.
fn guess_kind(path: &Path, has_head: bool) -> PartType {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();
    for kind in PartType::ALL {
        if name == kind.name() { return kind; }
    }
    for (word, kind) in [
        ("tail", PartType::Tail),
        ("ear", PartType::Ear),
        ("wing", PartType::Wing),
        ("hair", PartType::Hair),
        ("head", PartType::Head),
    ] {
        if name.contains(word) && !(kind == PartType::Head && has_head) {
            return kind;
        }
    }
    PartType::Other
}

#[cfg(test)]
impl Editor {
    /// `open` against a test root instead of the user's folder.
    fn open_in(root: &Path, c: &Character) -> Result<Self, String> {
        Self::read(&root.join(c.id), c.id)
    }
}

mod history;
mod hit;
mod save;

#[cfg(test)]
mod tests;
