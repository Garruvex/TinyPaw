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
use super::rest::RestMode;
use super::pack::{
    self, Character, EyeDef, Manifest, PartDef, Pose, Source, fit_png, installed_folder,
    read_manifest, read_png, stage_and_install,
};

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
    pub faces: [Option<Layer>; 3],
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
pub const FACE_LABELS: [&str; 3] = ["Eyes closed", "Pressed", "Being carried"];

/// A readable layer name from a file name: `eye_screen_left.png` becomes
/// "Eye screen left".
pub fn display_name(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
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
    /// A head face: part index and face slot (blink, pressed, dragging).
    Face(usize, usize),
    Eye(usize, EyeSlot),
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

pub const FACE_NAMES: [&str; 3] = ["Blink", "Pressed", "Dragging"];
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
            let mut faces = [None, None, None];
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
                    faces: [None, None, None],
                });
                self.select(Selected::Part(self.parts.len() - 1));
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
                    faces: [None, None, None],
                });
                self.select(Selected::Part(self.parts.len() - 1));
            }
            Target::Face(part, slot) => {
                let p = self.parts.get_mut(part).ok_or("that part is gone")?;
                if p.kind != PartType::Head {
                    return Err("only the head has faces".into());
                }
                p.faces[slot.min(2)] = Some(layer);
                self.select(Selected::Face(part, slot.min(2)));
            }
            Target::Eye(eye, slot) => {
                let e = self.eyes.get_mut(eye).ok_or("that eye is gone")?;
                let field = match slot {
                    EyeSlot::White => &mut e.white,
                    EyeSlot::Pupil => &mut e.pupil,
                    EyeSlot::Mask => &mut e.mask,
                    EyeSlot::Blink => &mut e.blink,
                };
                *field = Some(layer);
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
        if let Some(p) = self.parts.get_mut(part) {
            p.name = name.chars().take(placement::MAX_NAME).collect();
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
                p.faces = [None, None, None];
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
        if let Selected::Part(i) | Selected::Face(i, _) = self.selected
            && let Some(p) = self.parts.get_mut(i)
        {
            p.pivot = at;
        }
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
                parent: if p.on_head && p.kind != PartType::Head { head } else { None },
                behind: p.behind,
                motion: p.motion,
                faces: [0, 1, 2].map(|i| p.faces[i].as_ref().map(bake)),
            })
            .collect();
        Rig::layered(LayeredArt { body, ground, parts, eyes })
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

    /// Writes the edited character over its installed copy.
    pub fn save(&self, job: &std::sync::atomic::AtomicU8) -> Result<Character, String> {
        let root = pack::user_dir().ok_or("no per-user data folder")?;
        self.save_into_guarded(&root, Some(job))
    }

    #[cfg(test)]
    fn save_into(&self, root: &Path) -> Result<Character, String> {
        self.save_into_guarded(root, None)
    }

    fn save_into_guarded(
        &self,
        root: &Path,
        job: Option<&std::sync::atomic::AtomicU8>,
    ) -> Result<Character, String> {
        // Bundled Yohta's pack is never rewritten; Edit a copy instead.
        if self.id == pack::BUILTIN {
            return Err("Yohta is built in; edit a copy instead".into());
        }
        if let Some(error) = self.validation_error() {
            return Err(error);
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err("give the character a name".into());
        }
        let head = self.head();
        // Write each image under a name from its role, then the manifest.
        let dir = &self.pose;
        let mut files: Vec<(String, Layer)> = vec![(format!("{dir}/body.png"), self.body.clone())];
        // Display names travel in the editor metadata, by image.
        let mut names: Vec<(String, String)> = Vec::new();
        let mut parts = Vec::new();
        for (n, p) in self.parts.iter().enumerate() {
            let base = format!("{dir}/part-{}-{}", n + 1, p.kind.name());
            names.push((format!("{base}.png"), p.name.clone()));
            files.push((format!("{base}.png"), p.layer.clone()));
            let mut faces = [None, None, None];
            for (slot, face) in p.faces.iter().enumerate() {
                if let Some(face) = face {
                    let file = format!("{base}-{}.png", FACE_NAMES[slot].to_lowercase());
                    files.push((file.clone(), face.clone()));
                    faces[slot] = Some(file);
                }
            }
            parts.push(PartDef {
                kind: p.kind,
                image: format!("{base}.png"),
                pivot: p.pivot,
                parent: if p.on_head && p.kind != PartType::Head {
                    head
                } else {
                    None
                },
                behind: p.behind,
                motion: p.motion,
                faces,
            });
        }
        let mut eyes = Vec::new();
        for (n, e) in self.eyes.iter().enumerate() {
            let (Some(white), Some(pupil), Some(mask)) = (&e.white, &e.pupil, &e.mask) else {
                return Err(format!(
                    "eye {} needs a white, a pupil and a mask image",
                    n + 1
                ));
            };
            let base = format!("{dir}/eye-{}", n + 1);
            let mut eye = EyeDef {
                white: format!("{base}-white.png"),
                pupil: format!("{base}-pupil.png"),
                mask: format!("{base}-mask.png"),
                blink: None,
                max_offset: (e.travel, e.travel * 0.75),
            };
            files.push((eye.white.clone(), white.clone()));
            files.push((eye.pupil.clone(), pupil.clone()));
            files.push((eye.mask.clone(), mask.clone()));
            if let Some(blink) = &e.blink {
                eye.blink = Some(format!("{base}-blink.png"));
                files.push((format!("{base}-blink.png"), blink.clone()));
            }
            eyes.push(eye);
        }
        let ground = match self.rest {
            RestMode::Auto => self.auto_rest().unwrap_or(self.ground),
            RestMode::Manual => self.ground,
        };
        let mut poses = vec![Pose {
            // Layered poses give each part its own motion.
            whole: None,
            name: self.pose.clone(),
            ground,
            body: format!("{dir}/body.png"),
            parts,
            eyes,
        }];
        poses.extend(self.others.iter().cloned());
        let manifest = Manifest {
            id: self.id.to_owned(),
            name: name.chars().take(60).collect(),
            author: self.author.clone(),
            lines: self.lines.clone(),
            bop: self.bop,
            reactions: self.reactions.clone(),
            canvas: self.canvas,
            default_pose: self.pose.clone(),
            poses,
            source: self.source,
            rest: self.rest,
            editor: files
                .iter()
                .enumerate()
                .map(|(i, (file, layer))| SourceRecord {
                    name: names.iter().find(|(f, _)| f == file).map(|(_, n)| n.clone()),
                    target: file.clone(),
                    source: format!("editor/sources/{dir}/layer-{i}.png"),
                    size: layer.image.dimensions(),
                    placement: layer.placement,
                })
                .chain(self.other_sources.iter().cloned())
                .collect(),
        };
        // Other poses' images come from the installed copy.
        let installed = root.join(self.id);
        let mut kept: Vec<String> = self
            .others
            .iter()
            .flat_map(|p| {
                p.assets()
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        kept.extend(self.other_sources.iter().map(|r| r.source.clone()));
        let write = |stage: &Path| {
            for (file, image) in &files {
                let to = stage.join(file);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                placement::bake(&image.image, image.placement, self.canvas)
                    .save_with_format(&to, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
            }
            for ((_, layer), record) in files.iter().zip(&manifest.editor) {
                let to = stage.join(&record.source);
                std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
                layer
                    .image
                    .save_with_format(to, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
            }
            for asset in &kept {
                let to = stage.join(asset);
                if to.exists() {
                    continue;
                }
                let dimensions = manifest
                    .resources()
                    .into_iter()
                    .find(|(name, _)| *name == asset)
                    .map(|(_, dimensions)| dimensions)
                    .ok_or("undeclared image in another pose")?;
                read_png(&installed, asset, dimensions)?;
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(installed.join(asset), to).map_err(|e| e.to_string())?;
            }
            // Line sounds come along unchanged.
            for sound in manifest.sounds() {
                let to = stage.join(sound);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(installed.join(sound), to).map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        let replace = (!self.new_install).then_some(self.id);
        if let Some(job) = job {
            pack::stage_and_install_guarded(root, &manifest, replace, write, || {
                job.compare_exchange(
                    0,
                    1,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .map(|_| ())
                .map_err(|_| "Import cancelled".into())
            })
        } else {
            stage_and_install(root, &manifest, replace, write)
        }
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
            // A new character's touch events start at None, written out.
            reactions: super::reactions::new_character(),
            others: Vec::new(),
            pose: "sit".into(),
            mode: Mode::Arrange,
            zoom: 1.0,
            background: Background::Checker,
            eye_style: None,
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

    fn snapshot(&self) -> Snapshot {
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
    fn restore(&mut self, s: Snapshot) {
        self.body = s.body;
        self.parts = s.parts;
        self.eyes = s.eyes;
        self.ground = s.ground;
        self.rest = s.rest;
        self.canvas = s.canvas;
        self.canvas_fields = [s.canvas.0.to_string(), s.canvas.1.to_string()];
        self.select(s.selected);
    }
    pub fn checkpoint(&mut self) {
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
                self.checkpoint();
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
                faces: [None, None, None],
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
    fn append_part<'a>(&'a self, i: usize, out: &mut Vec<(Selected, &'a Layer)>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mascot::pack::{OnDuplicate, test_install_folder};

    #[test]
    fn cropped_placement_round_trip_keeps_sources_and_baked_pixels() {
        let dir =
            std::env::temp_dir().join(format!("tinypaw-crop-roundtrip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        let path = dir.join("head.png");
        let source = RgbaImage::from_pixel(20, 30, image::Rgba([120, 40, 220, 255]));
        source.save(&path).unwrap();
        e.set_image(Target::NewPart, &path).unwrap();
        assert_eq!(e.parts[0].layer.image.dimensions(), (20, 30));
        assert_eq!(e.parts[0].layer.placement.x, 40.0);
        e.checkpoint();
        let placement = Placement {
            x: 23.0,
            y: 18.0,
            scale: 1.3,
            angle: 25.0,
        };
        e.place(placement);
        let joint = e.parts[0].pivot;
        let expected = placement::bake(&source, placement, e.canvas);
        e.undo();
        assert_eq!(e.parts[0].layer.placement.angle, 0.0);
        e.redo();
        assert_eq!(e.parts[0].pivot, joint);
        let saved = e.save_into(&root).unwrap();
        std::fs::remove_file(path).unwrap();
        let reopened = Editor::open_in(&root, &saved).unwrap();
        assert_eq!(reopened.parts[0].layer.image.as_ref(), &source);
        assert_eq!(reopened.parts[0].layer.placement, placement);
        let m = read_manifest(saved.folder.as_ref().unwrap()).unwrap();
        assert_eq!(
            read_png(
                saved.folder.as_ref().unwrap(),
                &m.default_pose().parts[0].image,
                m.canvas
            )
            .unwrap(),
            expected
        );
        let export = pack::export(&saved, &dir.join("export")).unwrap();
        let exported = Editor::read(&export, saved.id).unwrap();
        assert_eq!(exported.parts[0].layer.placement, placement);
        assert_eq!(exported.parts[0].layer.image.as_ref(), &source);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn head_arrangement_moves_children_and_joints_while_pivot_mode_does_not() {
        let dir = std::env::temp_dir().join(format!("tinypaw-crop-joints-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        let head = dir.join("head.png");
        let eye = dir.join("eye.png");
        RgbaImage::from_pixel(30, 30, image::Rgba([80, 100, 120, 255]))
            .save(&head)
            .unwrap();
        RgbaImage::from_pixel(6, 4, image::Rgba([20, 30, 40, 255]))
            .save(&eye)
            .unwrap();
        e.set_image(Target::NewPart, &head).unwrap();
        e.set_image(Target::NewPart, &eye).unwrap();
        assert!(e.parts[1].on_head);
        let old = e.parts[1].layer.placement;
        let joint = e.parts[1].pivot;
        e.select(Selected::Part(0));
        e.mode = Mode::Arrange;
        e.checkpoint();
        e.translate(10.0, -5.0);
        assert!((e.parts[1].layer.placement.x - old.x - 10.0).abs() < 0.001);
        assert!((e.parts[1].pivot.0 - joint.0 - 0.1).abs() < 0.001);
        e.undo();
        assert_eq!(e.parts[1].layer.placement, old);
        e.mode = Mode::Pivot;
        e.click(0.4, 0.6);
        assert_eq!(e.parts[1].layer.placement, old);
        let ground = e.ground;
        e.resize_canvas(0, "200".into());
        assert_eq!(e.ground.0, ground.0 / 2.0);
        e.undo();
        assert_eq!(e.canvas, (100, 100));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_sources_and_cancelled_save_preserve_draft_and_installation() {
        let dir = std::env::temp_dir().join(format!("tinypaw-crop-failure-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        let empty = dir.join("empty.png");
        RgbaImage::new(12, 14).save(&empty).unwrap();
        assert!(
            e.set_image(Target::NewPart, &empty)
                .unwrap_err()
                .contains("empty")
        );
        assert!(e.parts.is_empty());
        assert!(!e.can_undo());
        let original = std::fs::read(root.join(c.id).join(pack::MANIFEST)).unwrap();
        let job = std::sync::atomic::AtomicU8::new(2);
        assert!(
            e.save_into_guarded(&root, Some(&job))
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(
            std::fs::read(root.join(c.id).join(pack::MANIFEST)).unwrap(),
            original
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "local seven-PNG Yohta artwork acceptance"]
    fn seven_yohta_crops_load_and_save() {
        let assets =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/mascot/yohta/separated-parts");
        let dir = std::env::temp_dir().join(format!("tinypaw-yohta-crops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut e = Editor::from_body_into(&dir, &assets.join("body_with_paws.png"), &[]).unwrap();
        let names = [
            "eye_screen_left.png",
            "eyebrow_screen_right.png",
            "head.png",
            "tail.png",
            "eye_screen_right.png",
            "eyebrow_screen_left.png",
        ];
        let errors = e.set_images(
            Target::NewPart,
            names.into_iter().map(|n| assets.join(n)).collect(),
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(e.parts.len(), 6);
        assert_eq!(
            e.parts
                .iter()
                .filter(|p| p.kind == PartType::Other && p.on_head)
                .count(),
            4
        );
        let saved = e.save_into(&dir).unwrap();
        let reopened = Editor::open_in(&dir, &saved).unwrap();
        assert_eq!(reopened.parts.len(), 6);
        assert_eq!(
            reopened.parts[0].layer.image.dimensions(),
            e.parts[0].layer.image.dimensions()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bundled_yohta_is_never_saved_over() {
        let dir = std::env::temp_dir().join(format!("tinypaw-builtin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        e.id = pack::BUILTIN;
        assert!(e.save_into(&root).unwrap_err().contains("copy"));
        assert!(!root.join(pack::BUILTIN).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_resting_point_follows_the_body_until_set_by_hand() {
        let dir = std::env::temp_dir().join(format!("tinypaw-rest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        // An installed pack opens in Arrange with the body selected and no
        // joint tool or resting-point guide active.
        assert_eq!((e.mode, e.selected, e.adjusting_rest), (Mode::Arrange, Selected::Body, false));
        // The fixture has no editor metadata: its anchor is kept by hand.
        assert_eq!(e.rest, RestMode::Manual);
        e.reset_rest();
        assert_eq!(e.rest, RestMode::Auto);
        let auto = e.auto_rest().unwrap();
        assert_eq!(e.ground, auto);
        // Moving the body moves the automatic point with it on save.
        e.translate(0.0, -10.0);
        let saved = e.save_into(&root).unwrap();
        let reopened = Editor::open_in(&root, &saved).unwrap();
        assert_eq!(reopened.rest, RestMode::Auto);
        assert!((reopened.ground.1 - (auto.1 - 0.1)).abs() < 0.02, "{:?} {auto:?}", reopened.ground);
        // Adjust manually: clicks place it and it stays set by hand.
        let mut e = reopened;
        e.adjust_rest();
        e.click(0.3, 0.7);
        e.finish_rest();
        assert_eq!((e.rest, e.ground), (RestMode::Manual, (0.3, 0.7)));
        let saved = e.save_into(&root).unwrap();
        let reopened = Editor::open_in(&root, &saved).unwrap();
        assert_eq!((reopened.rest, reopened.ground), (RestMode::Manual, (0.3, 0.7)));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn layers_have_readable_names_and_eye_images_ride_the_head() {
        assert_eq!(display_name(Path::new("eye_screen_left.png")), "Eye screen left");
        assert_eq!(display_name(Path::new("Right-Eyebrow.PNG")), "Right eyebrow");
        let dir = std::env::temp_dir().join(format!("tinypaw-names-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        assert_eq!(e.eye_style(), EyeStyle::InArt);
        let block = |x0: u32, y0: u32, x1: u32, y1: u32| {
            RgbaImage::from_fn(100, 100, |x, y| {
                let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
                image::Rgba(if on { [200, 90, 40, 255] } else { [0, 0, 0, 0] })
            })
        };
        let head = dir.join("cat-head.png");
        block(20, 10, 80, 55).save(&head).unwrap();
        let eye = dir.join("left_eye.png");
        block(35, 25, 45, 35).save(&eye).unwrap();
        e.set_image(Target::NewPart, &head).unwrap();
        e.set_image(Target::EyePart, &eye).unwrap();
        let p = &e.parts[1];
        assert_eq!(p.name, "Left eye");
        assert_eq!((p.kind, p.on_head, p.motion.preset), (PartType::Other, true, super::super::motion::Preset::Still));
        assert_eq!(e.eye_style(), EyeStyle::Separate);
        e.rename(0, "Cat head".into());
        // The draft rig matches what a save would install.
        let draft = e.draft_rig();
        let saved = e.save_into(&root).unwrap();
        let installed = saved.rig().unwrap();
        assert_eq!(draft.motion_parts().len(), installed.motion_parts().len());
        assert!(draft.anchor().distance(installed.anchor()) < 0.5);
        // Names come back from the editor metadata.
        let reopened = Editor::open_in(&root, &saved).unwrap();
        assert_eq!(reopened.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Cat head", "Left eye"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn edit_add_parts_and_save() {
        let dir = std::env::temp_dir().join(format!("tinypaw-editor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        assert!(e.parts.is_empty());
        let block = |x0: u32, y0: u32, x1: u32, y1: u32| {
            RgbaImage::from_fn(100, 100, |x, y| {
                let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
                image::Rgba(if on { [200, 90, 40, 255] } else { [0, 0, 0, 0] })
            })
        };
        let head = dir.join("cat-head.png");
        block(20, 10, 80, 55).save(&head).unwrap();
        let tail = dir.join("cat-tail.png");
        block(65, 60, 95, 70).save(&tail).unwrap();
        let ear = dir.join("ear.png");
        block(20, 5, 30, 15).save(&ear).unwrap();
        e.set_image(Target::NewPart, &head).unwrap();
        e.set_image(Target::NewPart, &tail).unwrap();
        e.set_image(Target::NewPart, &ear).unwrap();
        assert_eq!(
            e.parts.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [PartType::Head, PartType::Tail, PartType::Ear]
        );
        assert!(e.parts[2].on_head, "ears ride on the head");
        assert!(
            (e.parts[0].pivot.1 - 0.55).abs() < 0.01,
            "heads turn at the neck"
        );
        e.selected = Selected::Part(1);
        e.click(0.66, 0.65);
        assert_eq!(e.parts[1].pivot, (0.66, 0.65));
        e.set_image(Target::Face(0, 1), &head).unwrap();
        assert!(
            e.set_image(Target::Face(1, 1), &head).is_err(),
            "tails have no faces"
        );

        // An eye without its mask can't be saved; with one, it can.
        e.add_eye();
        e.set_image(Target::Eye(0, EyeSlot::White), &ear).unwrap();
        e.set_image(Target::Eye(0, EyeSlot::Pupil), &ear).unwrap();
        assert!(e.save_into(&root).unwrap_err().contains("mask"));
        e.set_image(Target::Eye(0, EyeSlot::Mask), &ear).unwrap();
        let saved = e.save_into(&root).unwrap();
        assert_eq!(saved.id, c.id, "saving keeps the character's identity");
        let rig = saved.rig().unwrap();
        assert_eq!(rig.motion_parts().len(), 3);
        assert!(rig.eye_point().is_some());

        // Reopening shows what was saved.
        let again = Editor::open_in(&root, &saved).unwrap();
        assert_eq!(again.parts.len(), 3);
        assert_eq!(again.parts[1].pivot, (0.66, 0.65));
        assert!(again.parts[0].faces[1].is_some());
        assert!(again.parts[2].on_head);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn retyping_and_removing_the_head_unhooks_its_riders() {
        let dir = std::env::temp_dir().join(format!("tinypaw-editor2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
        let mut e = Editor::open_in(&root, &c).unwrap();
        let png = dir.join("x.png");
        RgbaImage::from_pixel(100, 100, image::Rgba([1, 1, 1, 255]))
            .save(&png)
            .unwrap();
        let head = dir.join("head.png");
        std::fs::copy(&png, &head).unwrap();
        let ear = dir.join("ear.png");
        std::fs::copy(&png, &ear).unwrap();
        e.set_image(Target::NewPart, &head).unwrap();
        e.set_image(Target::NewPart, &ear).unwrap();
        assert!(e.parts[1].on_head);
        // A second head isn't allowed.
        e.set_kind(1, PartType::Head);
        assert_eq!(e.parts[1].kind, PartType::Ear);
        e.remove_part(0);
        assert!(!e.parts[0].on_head);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
