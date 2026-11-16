//! Editing shared variants and pose recipes through the ordinary character draft.
use super::super::composition::Composition;
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct DraftArtwork {
    pub rig: super::super::rig::Rig,
    pub arrange: Option<super::super::rig::Rig>,
    pub reactions: Option<(
        super::super::reactions::Config,
        Vec<(super::super::reactions::Art, RgbaImage)>,
    )>,
}

#[derive(Debug, Clone)]
pub struct LibraryDraft {
    pub composition: Composition,
    pub pose: String,
    pub variant: String,
    pub preview: super::super::rig::Rig,
    manifest: Manifest,
    folder: std::path::PathBuf,
    images: BTreeMap<String, Arc<RgbaImage>>,
}

#[derive(Debug, Clone)]
pub enum LibraryAction {
    Upgrade,
    Pose(String),
    Variant(String),
    Assign(String, String),
    MakeVariant,
    NewVariant(String),
    NewPose(String),
    Placement(String, usize, f32),
    // Handled and tested, but nothing in the studio offers it yet.
    #[allow(dead_code)]
    Pivot(usize, f32),
    Joint(String, usize, f32),
    Parent(String, String),
    EyeTravel(usize, usize, f32),
    Hands(bool),
    Wave(bool),
}

impl LibraryDraft {
    pub(super) fn new(manifest: &Manifest, folder: &Path) -> Result<Option<Self>, String> {
        let Some(composition) = &manifest.composition else {
            return Ok(None);
        };
        let preview = pack::build_rig(manifest, |file| read_png(folder, file, manifest.canvas))?;
        Ok(Some(Self {
            composition: composition.clone(),
            pose: manifest.default_pose.clone(),
            variant: composition
                .variants
                .keys()
                .next()
                .cloned()
                .unwrap_or_default(),
            preview,
            manifest: manifest.clone(),
            folder: folder.to_path_buf(),
            images: BTreeMap::new(),
        }))
    }

    pub(super) fn image_sources(&self) -> impl Iterator<Item = &Arc<RgbaImage>> {
        self.images.values()
    }

    fn manifest(&self, name: &str) -> Result<Manifest, String> {
        let mut value = self.manifest.to_json();
        value["name"] = serde_json::json!(name);
        value["composition"] = self.composition.to_json();
        pack::parse_manifest(&value.to_string())
    }

    pub fn error(&self, name: &str) -> Option<String> {
        if let Err(error) = self.manifest(name) {
            return Some(error);
        }
        let folder = Some(self.folder.clone());
        for file in self.composition.files() {
            if !self.images.contains_key(file)
                && folder
                    .as_ref()
                    .is_some_and(|path| !path.join(file).is_file())
            {
                return Some(format!("Choose the missing artwork for {file}"));
            }
        }
        None
    }

    pub fn build(
        &self,
        _folder: &Path,
        name: &str,
        selected: bool,
    ) -> Result<super::super::rig::Rig, String> {
        let mut manifest = self.manifest(name)?;
        if selected {
            manifest.default_pose = self.pose.clone();
        }
        pack::build_rig(&manifest, |file| {
            self.images
                .get(file)
                .map(|image| image.as_ref().clone())
                .map(Ok)
                .unwrap_or_else(|| read_png(&self.folder, file, manifest.canvas))
        })
    }

    fn reaction_draft(
        &self,
        name: &str,
    ) -> Result<
        (
            super::super::reactions::Config,
            Vec<(super::super::reactions::Art, RgbaImage)>,
        ),
        String,
    > {
        let manifest = self.manifest(name)?;
        let mut pictures = Vec::new();
        for name in ["doze", "curled", "held"] {
            let Some(pose) = manifest.poses.iter().find(|p| p.name == name) else {
                continue;
            };
            let rig = pack::build_pose_rig(&manifest, pose, &|file| {
                self.images
                    .get(file)
                    .map(|image| Ok(image.as_ref().clone()))
                    .unwrap_or_else(|| read_png(&self.folder, file, manifest.canvas))
            })?;
            let mut raster =
                super::super::surface::Raster::new(manifest.canvas.0, manifest.canvas.1)
                    .ok_or("could not create pose preview")?;
            rig.draw_pose(
                &mut raster,
                &super::super::motion::Pose {
                    expression: if name == "held" {
                        super::super::motion::Expression::Dragging
                    } else {
                        super::super::motion::Expression::Blink
                    },
                    ..Default::default()
                },
            );
            pictures.push((
                {
                    let mut art = super::super::reactions::Art::new(
                        super::super::lines::SoundRef::Pack(pose.body.clone()),
                    );
                    art.pose = Some(name.into());
                    art
                },
                RgbaImage::from_raw(
                    manifest.canvas.0,
                    manifest.canvas.1,
                    raster.to_straight_rgba(),
                )
                .ok_or("invalid pose pixels")?,
            ));
        }
        Ok((manifest.reactions, pictures))
    }

    pub(super) fn replace(
        &mut self,
        variant: usize,
        field: usize,
        layer: Layer,
        canvas: (u32, u32),
    ) -> Result<(), String> {
        let id = self
            .composition
            .variants
            .keys()
            .nth(variant)
            .cloned()
            .ok_or("variant is gone")?;
        let definition = self.composition.variants.get_mut(&id).unwrap();
        let fields = Composition::image_fields(definition);
        let (pointer, _) = fields.get(field).ok_or("image slot is gone")?;
        let file = format!("library/{id}/image-{field}.png");
        let image = placement::bake(&layer.image, layer.placement, canvas);
        let source_file = format!(
            "editor/sources/library/{id}/source-{field}-{}.png",
            self.manifest.editor.len()
        );
        self.manifest.editor.retain(|record| record.target != file);
        self.manifest.editor.push(SourceRecord {
            target: file.clone(),
            source: source_file.clone(),
            size: layer.image.dimensions(),
            placement: layer.placement,
            name: Some(id.clone()),
        });
        self.images.insert(source_file, layer.image.clone());
        let previous = fields[field].1.clone();
        if definition["sources"].is_null() {
            definition["sources"] = serde_json::json!([]);
        }
        let sources = definition["sources"]
            .as_array_mut()
            .ok_or("invalid variant sources")?;
        if (self.images.contains_key(&previous) || self.folder.join(&previous).is_file())
            && !sources
                .iter()
                .any(|source| source.as_str() == Some(&previous))
        {
            sources.push(serde_json::json!(previous));
        }

        *definition
            .pointer_mut(pointer)
            .ok_or("image slot is gone")? = serde_json::json!(file);
        self.images.insert(file, Arc::new(image));
        Ok(())
    }

    pub fn apply(&mut self, action: LibraryAction) -> Result<(), String> {
        use serde_json::json;
        match action {
            LibraryAction::Upgrade => {}
            LibraryAction::Wave(enabled) => {
                self.composition
                    .poses
                    .get_mut(&self.pose)
                    .ok_or("pose is gone")?["wave"] = json!(enabled);
            }
            LibraryAction::Hands(movable) => {
                let slots = [
                    "left_arm",
                    "right_arm",
                    "holding_arm",
                    "grip",
                    "phone",
                    "glow_stick",
                    "phone_glow",
                    "stick_glow",
                ];
                let pose = self
                    .composition
                    .poses
                    .get_mut(&self.pose)
                    .ok_or("pose is gone")?;
                if !movable {
                    let mut saved = serde_json::Map::new();
                    let assignments = pose["slots"].as_object_mut().ok_or("pose needs slots")?;
                    for slot in slots {
                        if let Some(value) = assignments.remove(slot) {
                            saved.insert(slot.into(), value);
                        }
                    }
                    if !saved.is_empty() {
                        pose["inactive_hands"] = json!(saved);
                    }
                } else if let Some(saved) = pose["inactive_hands"].as_object().cloned() {
                    pose["slots"]
                        .as_object_mut()
                        .ok_or("pose needs slots")?
                        .extend(saved);
                    pose.as_object_mut().unwrap().remove("inactive_hands");
                }
                pose["hands"] = json!(if movable { "movable" } else { "embedded" });
                pose["wave"] = json!(false);
                let mode = pose["hands"].as_str().unwrap();
                if let Some((id, _)) = self.composition.variants.iter().find(|(_, v)| {
                    v["kind"] == "body" && v["pose"] == self.pose && v["hands"] == mode
                }) {
                    let body = &mut pose["slots"]["body"];
                    if body.is_string() {
                        *body = json!(id);
                    } else {
                        body["variant"] = json!(id);
                    }
                }
            }
            LibraryAction::Pose(name) => {
                if !self.composition.poses.contains_key(&name) {
                    return Err("pose is gone".into());
                }
                self.pose = name;
            }
            LibraryAction::Variant(id) => {
                if !self.composition.variants.contains_key(&id) {
                    return Err("variant is gone".into());
                }
                self.variant = id;
            }
            LibraryAction::NewVariant(kind) => {
                if self.composition.variants.len() >= 64 {
                    return Err("the library supports up to 64 variants".into());
                }
                if !["body", "complete", "eyes"].contains(&kind.as_str())
                    && super::super::motion::PartType::parse(&kind).is_none()
                {
                    return Err("unknown part kind".into());
                }
                let mut n = 1;
                let id = loop {
                    let candidate = format!("{kind}.variant-{n}");
                    if !self.composition.variants.contains_key(&candidate) {
                        break candidate;
                    }
                    n += 1;
                };
                let variant = if kind == "eyes" {
                    json!({"kind":"eyes","eyes":([0,1].map(|eye|json!({
                        "white":format!("library/{id}/eye-{eye}-white.png"),
                        "pupil":format!("library/{id}/eye-{eye}-pupil.png"),
                        "mask":format!("library/{id}/eye-{eye}-mask.png"),
                        "blink":format!("library/{id}/eye-{eye}-closed.png"),"max_offset":[0.012,0.009]})))})
                } else {
                    json!({"kind":kind,"image":format!("library/{id}/image.png"),"pivot":[0.5,0.5]})
                };
                self.composition.variants.insert(id.clone(), variant);
                self.variant = id;
            }
            LibraryAction::MakeVariant => {
                if self.composition.variants.len() >= 64 {
                    return Err("the library supports up to 64 variants".into());
                }
                let mut number = 1;
                let id = loop {
                    let id = format!(
                        "{}-copy-{number}",
                        self.variant.chars().take(60).collect::<String>()
                    );
                    if !self.composition.variants.contains_key(&id) {
                        break id;
                    }
                    number += 1;
                };
                let variant = self
                    .composition
                    .variants
                    .get(&self.variant)
                    .ok_or("variant is gone")?
                    .clone();
                self.composition.variants.insert(id.clone(), variant);
                if let Some(slots) = self
                    .composition
                    .poses
                    .get_mut(&self.pose)
                    .and_then(|pose| pose["slots"].as_object_mut())
                {
                    for assignment in slots.values_mut() {
                        if Composition::variant_id(assignment) == Some(self.variant.as_str()) {
                            if assignment.is_string() {
                                *assignment = json!(id);
                            } else {
                                assignment["variant"] = json!(id);
                            }
                        }
                    }
                }
                self.variant = id;
            }
            LibraryAction::Assign(slot, id) => {
                let assignment = self
                    .composition
                    .poses
                    .get_mut(&self.pose)
                    .ok_or("pose is gone")?["slots"]
                    .as_object_mut()
                    .ok_or("pose needs slots")?;
                if id.is_empty() {
                    assignment.remove(&slot);
                } else if let Some(current) = assignment.get_mut(&slot).filter(|v| v.is_object()) {
                    current["variant"] = json!(id);
                } else {
                    let parent = if ["phone", "glow_stick", "grip", "phone_glow", "stick_glow"]
                        .contains(&slot.as_str())
                    {
                        Some("holding_arm")
                    } else {
                        None
                    };
                    assignment.insert(
                        slot,
                        if let Some(parent) = parent {
                            json!({"variant":id,"parent":parent})
                        } else {
                            json!(id)
                        },
                    );
                }
            }
            LibraryAction::NewPose(name) => {
                if !["sit", "stand", "doze", "curled", "held"].contains(&name.as_str())
                    || self.composition.poses.contains_key(&name)
                {
                    return Err("choose a new supported pose".into());
                }
                let mut recipe = self.composition.poses[&self.pose].clone();
                let scoped: Vec<_> = recipe["slots"]
                    .as_object()
                    .ok_or("pose needs slots")?
                    .iter()
                    .filter_map(|(slot, assignment)| {
                        let id = Composition::variant_id(assignment)?;
                        let variant = self.composition.variants.get(id)?;
                        ["left_arm", "right_arm", "holding_arm", "grip"]
                            .contains(&variant["kind"].as_str()?)
                            .then(|| (slot.clone(), variant.clone()))
                    })
                    .collect();
                if self.composition.variants.len() + scoped.len() > 64 {
                    return Err("not enough room for this pose's independent arms".into());
                }
                for (slot, variant) in scoped {
                    let mut n = 0;
                    let id = loop {
                        let id = if n == 0 {
                            format!("{slot}.{name}")
                        } else {
                            format!("{slot}.{name}-{n}")
                        };
                        if !self.composition.variants.contains_key(&id) {
                            break id;
                        }
                        n += 1;
                    };
                    self.composition.variants.insert(id.clone(), variant);
                    let assignment = &mut recipe["slots"][&slot];
                    if assignment.is_string() {
                        *assignment = json!(id);
                    } else {
                        assignment["variant"] = json!(id);
                    }
                }
                recipe["wave"] = json!(false);
                self.composition.poses.insert(name.clone(), recipe);
                self.pose = name;
            }
            LibraryAction::Placement(slot, field, value) => {
                if field > 3 || !value.is_finite() {
                    return Err("invalid placement".into());
                }
                let assignment = &mut self
                    .composition
                    .poses
                    .get_mut(&self.pose)
                    .ok_or("pose is gone")?["slots"][&slot];
                if let Some(id) = assignment.as_str() {
                    *assignment = json!({"variant":id});
                }
                if assignment["placement"].is_null() {
                    assignment["placement"] = json!([0.0, 0.0, 1.0, 0.0]);
                }
                assignment["placement"][field] = json!(value);
            }
            LibraryAction::Parent(slot, parent) => {
                let assignment = &mut self
                    .composition
                    .poses
                    .get_mut(&self.pose)
                    .ok_or("pose is gone")?["slots"][&slot];
                if let Some(id) = assignment.as_str() {
                    *assignment = json!({"variant":id});
                }
                assignment["parent"] = json!(parent);
            }
            LibraryAction::EyeTravel(eye, axis, value) => {
                if eye > 1 || axis > 1 || !value.is_finite() {
                    return Err("invalid eye travel".into());
                }
                let variant = self
                    .composition
                    .variants
                    .get_mut(&self.variant)
                    .ok_or("variant is gone")?;
                if variant["kind"] != "eyes" {
                    return Err("select an eye pair first".into());
                }
                variant["eyes"][eye]["max_offset"][axis] = json!(value.clamp(0.0, 0.05));
            }
            LibraryAction::Joint(slot, axis, value) => {
                if axis > 1 || !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err("invalid pose joint".into());
                }
                let assignment = &self.composition.poses[&self.pose]["slots"][&slot];
                let id = Composition::variant_id(assignment).ok_or("assign this part first")?;
                let pivot = assignment
                    .get("pivot")
                    .unwrap_or(&self.composition.variants[id]["pivot"])
                    .clone();
                if pivot.as_array().is_none_or(|p| p.len() != 2) {
                    return Err("this part needs a joint".into());
                }
                let assignment =
                    &mut self.composition.poses.get_mut(&self.pose).unwrap()["slots"][&slot];
                if let Some(id) = assignment.as_str() {
                    *assignment = json!({"variant":id});
                }
                assignment["pivot"] = pivot;
                assignment["pivot"][axis] = json!(value);
            }
            LibraryAction::Pivot(axis, value) => {
                if axis > 1 || !value.is_finite() {
                    return Err("invalid attachment".into());
                }
                self.composition
                    .variants
                    .get_mut(&self.variant)
                    .ok_or("variant is gone")?["pivot"][axis] = json!(value);
            }
        }
        Ok(())
    }

    pub(super) fn save(
        &self,
        root: &Path,
        name: &str,
        new_install: bool,
        job: Option<&std::sync::atomic::AtomicU8>,
    ) -> Result<Character, String> {
        let manifest = self.manifest(name)?;
        let installed = self.folder.clone();
        let write = |stage: &Path| {
            for (file, _) in manifest.resources() {
                let target = stage.join(file);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                if let Some(image) = self.images.get(file) {
                    image
                        .save_with_format(&target, image::ImageFormat::Png)
                        .map_err(|e| e.to_string())?;
                } else {
                    std::fs::copy(installed.join(file), target).map_err(|e| e.to_string())?;
                }
            }
            for file in manifest.sounds() {
                let target = stage.join(file);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(installed.join(file), target).map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        pack::stage_and_install_guarded(
            root,
            &manifest,
            (!new_install).then_some(manifest.id.as_str()),
            write,
            || {
                if let Some(job) = job {
                    job.compare_exchange(
                        0,
                        1,
                        std::sync::atomic::Ordering::SeqCst,
                        std::sync::atomic::Ordering::SeqCst,
                    )
                    .map_err(|_| "Import cancelled".to_owned())?;
                }
                Ok(())
            },
        )
    }
}

impl Editor {
    pub fn draft_artwork(&self) -> DraftArtwork {
        DraftArtwork {
            rig: self
                .library
                .as_ref()
                .and_then(|library| library.build(&library.folder, &self.name, false).ok())
                .unwrap_or_else(|| self.draft_rig()),
            arrange: self.library.as_ref().map(|_| self.draft_rig()),
            reactions: self
                .library
                .as_ref()
                .and_then(|library| library.reaction_draft(&self.name).ok()),
        }
    }

    pub fn from_reference_request(
        request: &super::super::reference::Request,
        taken: &[&str],
    ) -> Result<Self, String> {
        use super::super::reference::Role;
        let body = request
            .file_for(Role::Body)
            .ok_or("assign the body artwork")?;
        let mut editor = Self::from_body(body, taken)?;
        let mut value = request.manifest();
        value["id"] = serde_json::json!(editor.id);
        if value["composition"].is_null() {
            let manifest = pack::parse_manifest(&value.to_string())?;
            value["composition"] = Composition::from_poses(&manifest.poses).to_json();
        }
        let manifest = pack::parse_manifest(&value.to_string())?;
        let mut images = BTreeMap::new();
        let canvas = (request.canvas, request.canvas);
        for role in request.roles() {
            let path = request
                .file_for(role)
                .ok_or_else(|| format!("assign {}", role.file()))?;
            let reader = image::ImageReader::open(path)
                .map_err(|e| e.to_string())?
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let dimensions = reader.into_dimensions().map_err(|e| e.to_string())?;
            let held_bytes: u64 = images
                .values()
                .map(|image: &Arc<RgbaImage>| {
                    u64::from(image.width()) * u64::from(image.height()) * 4
                })
                .sum();
            if held_bytes + u64::from(dimensions.0) * u64::from(dimensions.1) * 8
                > 128 * 1024 * 1024
            {
                return Err(
                    "the requested artwork exceeds the editing memory budget; use a smaller canvas"
                        .into(),
                );
            }
            let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
            let placement = Placement::centred(image.dimensions(), canvas);
            images.insert(
                role.file().to_owned(),
                Arc::new(placement::bake(&image, placement, canvas)),
            );
        }
        let preview = pack::build_rig(&manifest, |file| {
            images
                .get(file)
                .map(|i| i.as_ref().clone())
                .ok_or_else(|| format!("missing {file}"))
        })?;
        editor.name = manifest.name.clone();
        editor.canvas = canvas;
        editor.ground = manifest.default_pose().ground;
        editor.canvas_fields = [canvas.0.to_string(), canvas.1.to_string()];
        editor.parts.clear();
        editor.eyes.clear();
        editor.body = Layer::new(images[Role::Body.file()].as_ref().clone());
        editor.library = Some(LibraryDraft {
            composition: manifest.composition.clone().unwrap(),
            pose: manifest.default_pose.clone(),
            variant: manifest
                .composition
                .as_ref()
                .unwrap()
                .variants
                .keys()
                .next()
                .unwrap()
                .clone(),
            manifest,
            folder: std::path::PathBuf::new(),
            images,
            preview,
        });
        editor.undo.clear();
        editor.redo.clear();
        editor.sync_fields();
        Ok(editor)
    }

    fn upgrade_library(&mut self) -> Result<(), String> {
        let folder = self.folder.clone();
        let mut manifest = if self.new_install {
            Manifest {
                id: self.id.into(),
                name: self.name.clone(),
                author: self.author.clone(),
                canvas: self.canvas,
                default_pose: self.pose.clone(),
                poses: vec![Pose {
                    name: self.pose.clone(),
                    body: "library/current/body.png".into(),
                    parts: vec![],
                    eyes: vec![],
                    ground: self.ground,
                    whole: None,
                }],
                source: self.source,
                editor: vec![],
                rest: self.rest,
                lines: self.lines.clone(),
                bop: self.bop,
                music_art: self.music_art.clone(),
                composition: None,
                reactions: self.reactions.clone(),
            }
        } else {
            read_manifest(&folder)?
        };
        let original = manifest.default_pose().clone();
        let mut images = BTreeMap::new();
        let mut put = |file: String, layer: &Layer| {
            images.insert(
                file.clone(),
                Arc::new(placement::bake(&layer.image, layer.placement, self.canvas)),
            );
            file
        };
        let body = put(original.body.clone(), &self.body);
        let mut parts = Vec::new();
        for (n, part) in self.parts.iter().enumerate() {
            let old = original.parts.get(n).filter(|old| old.kind == part.kind);
            let file = old
                .map(|p| p.image.clone())
                .unwrap_or_else(|| format!("library/current/part-{n}.png"));
            let mut faces = [None, None, None, None];
            for (slot, face) in part.faces.iter().enumerate() {
                if let Some(face) = face {
                    let file = old
                        .and_then(|p| p.faces[slot].clone())
                        .unwrap_or_else(|| format!("library/current/part-{n}-face-{slot}.png"));
                    faces[slot] = Some(put(file, face));
                }
            }
            parts.push(PartDef {
                kind: part.kind,
                image: put(file, &part.layer),
                pivot: part.pivot,
                parent: part.on_head.then(|| self.head()).flatten(),
                behind: part.behind,
                motion: part.motion,
                faces,
            });
        }
        let mut eyes = Vec::new();
        for (n, eye) in self.eyes.iter().enumerate() {
            let old = original.eyes.get(n);
            let (Some(white), Some(pupil), Some(mask)) = (&eye.white, &eye.pupil, &eye.mask) else {
                return Err("complete the eye pair first".into());
            };
            let name = |field: &str| format!("library/current/eye-{n}-{field}.png");
            eyes.push(EyeDef {
                static_image: false,
                white: put(
                    old.map(|e| e.white.clone())
                        .unwrap_or_else(|| name("white")),
                    white,
                ),
                pupil: put(
                    old.map(|e| e.pupil.clone())
                        .unwrap_or_else(|| name("pupil")),
                    pupil,
                ),
                mask: put(
                    old.map(|e| e.mask.clone()).unwrap_or_else(|| name("mask")),
                    mask,
                ),
                blink: eye.blink.as_ref().map(|image| {
                    put(
                        old.and_then(|e| e.blink.clone())
                            .unwrap_or_else(|| name("closed")),
                        image,
                    )
                }),
                max_offset: (eye.travel, eye.travel * 0.75),
            });
        }
        let pose = Pose {
            name: self.pose.clone(),
            body,
            parts,
            eyes,
            ground: self.ground,
            whole: None,
        };
        manifest.poses.retain(|p| p.name != pose.name);
        manifest.poses.insert(0, pose);
        manifest.name = self.name.clone();
        manifest.canvas = self.canvas;
        manifest.reactions = self.reactions.clone();
        use super::super::reactions::{Art, Entry, Event, Visual};
        for (event, layer) in &self.reference_art {
            let file = format!("reactions/{}.png", event.key());
            images.insert(
                file.clone(),
                Arc::new(placement::bake(&layer.image, layer.placement, self.canvas)),
            );
            let mut entry = manifest
                .reactions
                .entry(*event)
                .cloned()
                .unwrap_or_else(Entry::none);
            entry.visual = Some(Visual::Artwork);
            entry.art = Some(Art::new(super::super::lines::SoundRef::Pack(file)));
            manifest.reactions.set_entry(*event, Some(entry));
        }
        if let Some(layer) = &self.held {
            images.insert(
                HELD_FILE.into(),
                Arc::new(placement::bake(&layer.image, layer.placement, self.canvas)),
            );
            let mut entry = manifest
                .reactions
                .entry(Event::Pickup)
                .cloned()
                .unwrap_or_else(Entry::none);
            entry.held = Some(Art::new(super::super::lines::SoundRef::Pack(
                HELD_FILE.into(),
            )));
            manifest.reactions.set_entry(Event::Pickup, Some(entry));
        }
        let mut composition = Composition::from_poses(&manifest.poses);
        composition.add_complete_reactions(&manifest.reactions.to_json());
        // Preserve the original source records through shared replacements.
        let used: BTreeSet<_> = composition.files().into_iter().map(str::to_owned).collect();
        let archived: Vec<_> = original
            .assets()
            .into_iter()
            .filter(|file| !used.contains(*file))
            .map(str::to_owned)
            .collect();
        if let Some(variant) = composition
            .variants
            .values_mut()
            .find(|v| v["image"] == original.body)
        {
            variant["sources"] = serde_json::json!(archived);
        }
        manifest.composition = Some(composition.clone());
        let preview = pack::build_rig(&manifest, |file| {
            images
                .get(file)
                .map(|i| Ok(i.as_ref().clone()))
                .unwrap_or_else(|| read_png(&folder, file, self.canvas))
        })?;
        self.library = Some(LibraryDraft {
            variant: composition.variants.keys().next().unwrap().clone(),
            pose: manifest.default_pose.clone(),
            composition,
            preview,
            manifest,
            folder,
            images,
        });
        Ok(())
    }

    pub fn library_action(&mut self, action: LibraryAction) -> Result<(), String> {
        if matches!(action, LibraryAction::Pose(_) | LibraryAction::Variant(_)) {
            return self
                .library
                .as_mut()
                .ok_or("this character uses legacy artwork")?
                .apply(action);
        }
        self.checkpoint();
        if matches!(action, LibraryAction::Upgrade) {
            return self.upgrade_library();
        }
        self.library
            .as_mut()
            .ok_or("this character uses legacy artwork")?
            .apply(action)
    }
}
