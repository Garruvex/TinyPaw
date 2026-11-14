//! Create from reference: from a reference picture and a few choices,
//! the exact artwork to ask an image tool for, one consistent brief to copy,
//! and matching the PNGs that come back to their roles.
//!
//! TinyPaw generates nothing itself here: the creator attaches the reference
//! to their own image tool, pastes the brief, and imports the results, which
//! open in the artwork editor like any other parts.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EyeLayer {
    White,
    Pupil,
    Mask,
    Blink,
}

/// A file the brief asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The complete character, or the body with separate parts removed.
    Body,
    /// A separate head, so it can tilt and nod on its own.
    Head,
    /// A separate tail, for Sway, Wag or Hang.
    Tail,
    /// Complete eye and eyebrow images that ride on the head. Left and
    /// right are as the viewer sees them.
    LeftEye,
    RightEye,
    LeftBrow,
    RightBrow,
    Tracking(bool, EyeLayer),
    /// Head faces that replace the head: eyes closed (blinking), pressed
    /// (clicked) and being carried. They need painted-in eyes.
    HeadBlink,
    HeadPressed,
    HeadCarried,
    HeadPetted,
    Petted,
    Sleep,
    Wake,
    Click,
    Drop,
    /// The whole character in a carried pose, for Pick up's held pose.
    Held,
    LeftArm,
    RightArm,
    HoldingArm,
    Grip,
    Phone,
    GlowStick,
    PhoneGlow,
    StickGlow,
}

impl Role {
    pub fn reaction(self) -> Option<super::reactions::Event> {
        use super::reactions::Event;
        match self {
            Role::Petted => Some(Event::Headpat),
            Role::Sleep => Some(Event::Sleep),
            Role::Wake => Some(Event::Wake),
            Role::Click => Some(Event::Click),
            Role::Drop => Some(Event::Drop),
            _ => None,
        }
    }
    pub fn file(self) -> &'static str {
        match self {
            Role::Body => "body.png",
            Role::Head => "head.png",
            Role::Tail => "tail.png",
            Role::LeftEye => "left_eye.png",
            Role::RightEye => "right_eye.png",
            Role::LeftBrow => "left_eyebrow.png",
            Role::RightBrow => "right_eyebrow.png",
            Role::Tracking(false, EyeLayer::White) => "left_eye_white.png",
            Role::Tracking(false, EyeLayer::Pupil) => "left_eye_pupil.png",
            Role::Tracking(false, EyeLayer::Mask) => "left_eye_mask.png",
            Role::Tracking(false, EyeLayer::Blink) => "left_eye_blink.png",
            Role::Tracking(true, EyeLayer::White) => "right_eye_white.png",
            Role::Tracking(true, EyeLayer::Pupil) => "right_eye_pupil.png",
            Role::Tracking(true, EyeLayer::Mask) => "right_eye_mask.png",
            Role::Tracking(true, EyeLayer::Blink) => "right_eye_blink.png",
            Role::HeadBlink => "head_eyes_closed.png",
            Role::HeadPressed => "head_pressed.png",
            Role::HeadCarried => "head_carried.png",
            Role::HeadPetted => "head_petted.png",
            Role::Petted => "petted.png",
            Role::Sleep => "sleep.png",
            Role::Wake => "wake.png",
            Role::Click => "click.png",
            Role::Drop => "drop.png",
            Role::Held => "held.png",
            Role::LeftArm => "left_arm.png",
            Role::RightArm => "right_arm.png",
            Role::HoldingArm => "holding_arm.png",
            Role::Grip => "grip.png",
            Role::Phone => "phone.png",
            Role::GlowStick => "glow_stick.png",
            Role::PhoneGlow => "phone_glow.png",
            Role::StickGlow => "stick_glow.png",
        }
    }

    /// The head face slot a face role fills: blink, pressed, dragging.
    pub fn face_slot(self) -> Option<usize> {
        match self {
            Role::HeadBlink => Some(0),
            Role::HeadPressed => Some(1),
            Role::HeadCarried => Some(2),
            Role::HeadPetted => Some(3),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Body => "Body",
            Role::Head => "Head",
            Role::Tail => "Tail",
            Role::LeftEye => "Left eye",
            Role::RightEye => "Right eye",
            Role::LeftBrow => "Left eyebrow",
            Role::RightBrow => "Right eyebrow",
            Role::Tracking(false, EyeLayer::White) => "Left eye background",
            Role::Tracking(false, EyeLayer::Pupil) => "Left pupil",
            Role::Tracking(false, EyeLayer::Mask) => "Left eye mask",
            Role::Tracking(false, EyeLayer::Blink) => "Left eye closed",
            Role::Tracking(true, EyeLayer::White) => "Right eye background",
            Role::Tracking(true, EyeLayer::Pupil) => "Right pupil",
            Role::Tracking(true, EyeLayer::Mask) => "Right eye mask",
            Role::Tracking(true, EyeLayer::Blink) => "Right eye closed",
            Role::HeadBlink => "Head, eyes closed",
            Role::HeadPressed => "Head, pressed",
            Role::HeadCarried => "Head, being carried",
            Role::HeadPetted => "Head, happy from headpat",
            Role::Petted => "Headpat pose",
            Role::Sleep => "Sleeping pose",
            Role::Wake => "Waking pose",
            Role::Click => "Click pose",
            Role::Drop => "Drop pose",
            Role::Held => "Held pose",
            Role::LeftArm => "Left arm",
            Role::RightArm => "Right arm",
            Role::HoldingArm => "Holding arm",
            Role::Grip => "Grip fingers",
            Role::Phone => "Phone",
            Role::GlowStick => "Glow stick",
            Role::PhoneGlow => "Phone glow",
            Role::StickGlow => "Stick glow",
        }
    }

    /// Eyes and eyebrows: still layers that move with the head.
    pub fn is_face(self) -> bool {
        matches!(
            self,
            Role::LeftEye | Role::RightEye | Role::LeftBrow | Role::RightBrow
        )
    }

    /// The role a returned file's name suggests. Eyebrows are checked
    /// before eyes, since "eyebrow" contains "eye".
    pub fn guess(path: &Path) -> Option<Role> {
        let name = path.file_stem()?.to_str()?.to_lowercase();
        for role in [
            Role::LeftArm,
            Role::RightArm,
            Role::HoldingArm,
            Role::Grip,
            Role::Phone,
            Role::GlowStick,
            Role::PhoneGlow,
            Role::StickGlow,
            Role::HeadPetted,
            Role::Petted,
            Role::Sleep,
            Role::Wake,
            Role::Click,
            Role::Drop,
        ] {
            if name == role.file().trim_end_matches(".png") {
                return Some(role);
            }
        }
        for right in [false, true] {
            for layer in [
                EyeLayer::White,
                EyeLayer::Pupil,
                EyeLayer::Mask,
                EyeLayer::Blink,
            ] {
                let role = Role::Tracking(right, layer);
                if name == role.file().trim_end_matches(".png") {
                    return Some(role);
                }
            }
        }
        // Head faces before eyes and the held pose: "head_eyes_closed" and
        // "head_carried" name those words too.
        if name.contains("head") {
            if name.contains("closed") || name.contains("blink") {
                return Some(Role::HeadBlink);
            }
            if name.contains("pressed") {
                return Some(Role::HeadPressed);
            }
            if name.contains("carried") || name.contains("drag") {
                return Some(Role::HeadCarried);
            }
        }
        let side = |left: Role, right: Role| {
            if name.contains("left") || name.ends_with("_l") || name.ends_with("-l") {
                Some(left)
            } else if name.contains("right") || name.ends_with("_r") || name.ends_with("-r") {
                Some(right)
            } else {
                None
            }
        };
        if name.contains("brow") {
            return side(Role::LeftBrow, Role::RightBrow);
        }
        if name.contains("eye") {
            return side(Role::LeftEye, Role::RightEye);
        }
        [
            (Role::Held, "held"),
            (Role::Held, "carried"),
            (Role::Tail, "tail"),
            (Role::Head, "head"),
            (Role::Body, "body"),
        ]
        .into_iter()
        .find(|(_, word)| name.contains(word))
        .map(|(r, _)| r)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Artwork resolution presets: square stages, in pixels.
pub const CANVASES: [u32; 3] = [512, 1024, 2048];
pub const DEFAULT_CANVAS: u32 = 1024;

/// A "Create from reference" draft. The reference stays a local file; it
/// is never copied into a pack.
#[derive(Debug, Clone)]
pub struct Request {
    pub reference: PathBuf,
    pub style: String,
    pub canvas: u32,
    /// Optional moving parts and poses, beyond the base picture.
    pub head: bool,
    pub tail: bool,
    pub dance: bool,
    /// Separate eye and eyebrow images instead of painted-in ones.
    pub eyes: bool,
    pub tracking: bool,
    pub brows: bool,
    /// Head faces: eyes closed for blinking, and pressed and carried
    /// reactions. Only with a separate head and painted-in eyes.
    pub blink: bool,
    pub faces: bool,
    pub held: bool,
    pub extras: Vec<Role>,
    /// Returned PNGs and the role each fills (`None`: not used).
    pub results: Vec<(PathBuf, Option<Role>)>,
}

impl Request {
    pub fn new(reference: PathBuf) -> Self {
        Self {
            reference,
            style: String::new(),
            canvas: DEFAULT_CANVAS,
            head: false,
            tail: false,
            dance: false,
            eyes: false,
            tracking: false,
            brows: false,
            blink: false,
            faces: false,
            held: false,
            extras: Vec::new(),
            results: Vec::new(),
        }
    }

    /// The files to request, in order. A base-only request is one picture.
    pub fn roles(&self) -> Vec<Role> {
        let mut out = vec![Role::Body];
        for (on, roles) in [
            (self.needs_head(), &[Role::Head][..]),
            (self.tail, &[Role::Tail][..]),
            (
                self.eyes && !self.tracking,
                &[Role::LeftEye, Role::RightEye][..],
            ),
            (self.brows, &[Role::LeftBrow, Role::RightBrow][..]),
            (
                self.blink && self.faces_possible() && !self.tracking,
                &[Role::HeadBlink][..],
            ),
            (
                self.faces && self.faces_possible(),
                &[Role::HeadPressed, Role::HeadCarried][..],
            ),
            (self.held, &[Role::Held][..]),
        ] {
            if on {
                out.extend_from_slice(roles);
            }
        }
        if self.tracking {
            for right in [false, true] {
                for layer in [EyeLayer::White, EyeLayer::Pupil, EyeLayer::Mask] {
                    out.push(Role::Tracking(right, layer));
                }
                if self.blink {
                    out.push(Role::Tracking(right, EyeLayer::Blink));
                }
            }
        }
        if self.dance {
            out.extend([
                Role::LeftArm,
                Role::RightArm,
                Role::HoldingArm,
                Role::Grip,
                Role::Phone,
                Role::GlowStick,
                Role::PhoneGlow,
                Role::StickGlow,
            ]);
        }
        out.extend(self.extras.iter().copied());
        out
    }

    /// Head faces replace the head art, so they need a separate head with
    /// its eyes painted in: separate eye layers would stay open on top.
    pub fn faces_possible(&self) -> bool {
        self.needs_head() && (!self.eyes || self.tracking)
    }

    fn needs_head(&self) -> bool {
        self.head
            || self.dance
            || self.tracking
            || self.blink
            || self.faces
            || self.extras.contains(&Role::HeadPetted)
    }

    fn style(&self) -> String {
        let style = self.style.trim();
        if style.is_empty() {
            "the reference's own style".into()
        } else {
            style.into()
        }
    }

    /// What the face carries painted in, given the separate layers asked
    /// for.
    fn painted_face(&self) -> &'static str {
        match (self.eyes || self.tracking, self.brows) {
            (false, false) => "with the eyes and eyebrows painted in",
            (true, false) => {
                "with the eyebrows painted in but the eyes left out: paint plain skin or fur where the eyes sit, since they come as separate files"
            }
            (false, true) => {
                "with the eyes painted in but the eyebrows left out: paint plain skin or fur where they sit, since they come as separate files"
            }
            (true, true) => {
                "with the eyes and eyebrows left out: paint plain skin or fur where they sit, since they come as separate files"
            }
        }
    }

    /// What one file should show.
    fn describe(&self, role: Role) -> String {
        let face = self.painted_face();
        let mut removed = Vec::new();
        if self.needs_head() {
            removed.push(
                "the head (keep the neck, with a little painted overlap where the head joins)",
            );
        }
        if self.dance {
            removed.push("both arms, with clean painted overlap at the shoulders");
        }
        if self.tail {
            removed.push("the tail (keep a little painted overlap where it joins)");
        }
        let facial = |what: &str, side: &str| {
            format!(
                "only the {side} {what} as the viewer sees it, complete and opaque, drawn at its exact size and position on the face, with nothing else in the image"
            )
        };
        match role {
            Role::Body if removed.is_empty() => format!("the whole character standing or sitting at rest, {face}"),
            Role::Body if self.needs_head() => format!(
                "the character's body at rest with {} left out entirely, so nothing of the face is in this file",
                removed.join(" and ")
            ),
            Role::Body => format!(
                "the whole character standing or sitting at rest, {face}, with {} left out entirely",
                removed.join(" and ")
            ),
            Role::Head => format!(
                "only the head, at the exact position it has on the body, {face}, with a little extra painted at the neck so it overlaps the body"
            ),
            Role::LeftArm => "only the relaxed left arm as seen by the viewer, from shoulder to fingertips; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::RightArm => "only the relaxed right arm as seen by the viewer, from shoulder to fingertips; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::HoldingArm => "only the viewer-right arm in its lowered resting position, elbow slightly bent and hand shaped to hold an upright item; omit the front grip fingers, supplied separately; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::Grip => "only the fingers overlapping the front of the held item on holding_arm.png; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::Phone => "only an upright phone with a lit screen facing the viewer, positioned in the lowered holding hand; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::GlowStick => "only an upright warm yellow glow stick positioned in the same lowered holding hand as the phone; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::PhoneGlow => "only a soft transparent light halo aligned to phone.png; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::StickGlow => "only a soft transparent light halo aligned to glow_stick.png; match the full shared canvas, and keep all other pixels transparent".into(),
            Role::Tail => "only the tail, in the exact position where it attaches to the body file, with a little extra painted overlap at its base".into(),
            Role::LeftEye => facial("eye", "left"),
            Role::RightEye => facial("eye", "right"),
            Role::LeftBrow => facial("eyebrow", "left"),
            Role::RightBrow => facial("eyebrow", "right"),
            Role::Tracking(right, layer) => {
                let side = if right { "right" } else { "left" };
                let content = match layer {
                    EyeLayer::White => "the eye socket and background, with the pupil and iris removed",
                    EyeLayer::Pupil => "only the pupil and iris at their neutral gaze position",
                    EyeLayer::Mask => "an alpha mask for the visible eye opening, opaque white inside the opening and fully transparent outside it; alpha controls clipping",
                    EyeLayer::Blink => "the closed eyelid replacing the entire eye, with the same alignment",
                };
                format!("the {side} eye as the viewer sees it: {content}, at its exact location on the head")
            }
            Role::HeadPetted => format!("only the head, identical to head.png in position, scale and outline, with a happy relaxed headpat expression and eyes painted in; leave hearts and motion trails out because TinyPaw animates those; {}", if self.brows { "leave eyebrows out because the separate eyebrow layers stay visible" } else { "keep eyebrows painted in" }),
            Role::Petted => "the complete character enjoying a headpat, happy relaxed expression, all eyes and eyebrows painted in, matching the resting pose's scale and resting anchor; leave hearts and motion trails out because TinyPaw animates those".into(),
            Role::Sleep => "one optional complete character curled up comfortably asleep, eyes closed and all face details painted in. Keep the reference camera angle, head size, full shared canvas and floor baseline; the silhouette may change naturally for this sleeping position. Create only this single still PNG, no animation frames, breathing marks, shadows, Zzz or motion trails: TinyPaw handles breathing and Zzz. Measure the mouth position as canvas fractions for reactions.companionship.mouth. This extra image is only needed for a different sleeping position; ordinary seated dozing reuses the existing closed-eye or blink artwork. Do not generate a wake image unless separately requested".into(),
            Role::Wake => "one optional complete character waking drowsily, all face details painted in, matching the resting camera angle, character scale and floor baseline. Attach the approved sleep.png as an additional reference and use a natural intermediate position between that sleep pose and the normal resting pose. Create a single still PNG, no animation frames or effects. Wake artwork is optional; TinyPaw can return to the normal character and animate the head lift and blink without it".into(),
            Role::Click => "the complete character reacting happily to a poke, all face details painted in, matching the resting anchor and scale".into(),
            Role::Drop => "the complete character reacting to being put down, all face details painted in, matching the resting anchor and scale".into(),
            Role::HeadBlink => "only the head, identical to head.png in position, size and outline, with the eyes gently closed as in a blink; leave eyebrows out if separate eyebrow files were requested".into(),
            Role::HeadPressed => "only the head, identical to head.png in position, size and outline, with eyes painted in and a surprised or delighted expression, as when poked; leave eyebrows out if separate eyebrow files were requested".into(),
            Role::HeadCarried => "only the head, identical to head.png in position, size and outline, with eyes painted in and a startled or excited expression, as when being picked up; leave eyebrows out if separate eyebrow files were requested".into(),
            Role::Held => "the whole character held up by the scruff of the neck, the way a cat is picked up: the scruff is the highest point, at the top centre of the character, with the head just below it and the body, legs and tail hanging straight down, relaxed and limp, so it can swing from that point like a pendulum. Match the head width and facial proportions of the approved resting head, with a naturally longer dangling body. Keep the full shared canvas and measure the grip at the intended scruff pickup point as [x / canvas width, y / canvas height]. Eyes and eyebrows painted in, and plenty of transparent space on every side so swinging never crops it".into(),
        }
    }

    /// The shared rules every file follows, so the results line up.
    fn rules(&self) -> String {
        format!(
            "Use the attached picture as the character reference. Style: {}. Every file is a {c}×{c} PNG with a fully transparent background, the same camera angle, scale and position, so the files line up when stacked; keep markings, colours and proportions consistent with the reference. Leave transparent space around the character and don't crop it. The shared position rule applies to resting layers; a complete sleeping or waking pose may change silhouette while preserving the character scale and floor baseline.",
            self.style(),
            c = self.canvas,
        )
    }

    fn manifest(&self) -> serde_json::Value {
        use super::motion::{MotionSpec, PartType};
        use super::pack::{EyeDef, PartDef, Pose, slug};
        use serde_json::json;

        let roles = self.roles();
        let mut parts = Vec::new();
        for role in &roles {
            let kind = match role {
                Role::Head => PartType::Head,
                Role::Tail => PartType::Tail,
                Role::LeftArm => PartType::LeftArm,
                Role::RightArm => PartType::RightArm,
                Role::HoldingArm => PartType::HoldingArm,
                Role::Grip => PartType::Grip,
                Role::Phone => PartType::Phone,
                Role::GlowStick => PartType::GlowStick,
                Role::PhoneGlow => PartType::PhoneGlow,
                Role::StickGlow => PartType::StickGlow,

                role if role.is_face() => PartType::Other,
                _ => continue,
            };
            let mut faces = [None, None, None, None];
            if kind == PartType::Head {
                for face in &roles {
                    if let Some(slot) = face.face_slot() {
                        faces[slot] = Some(face.file().to_owned());
                    }
                }
            }
            parts.push(PartDef {
                kind,
                image: role.file().to_owned(),
                pivot: (0.5, 0.5),
                parent: (role.is_face() && self.needs_head()).then_some(0),
                behind: kind.behind_by_default(),
                motion: MotionSpec::default_for(kind),
                faces,
            });
        }
        let pose = Pose {
            name: "sit".into(),
            ground: (0.5, 0.9),
            body: Role::Body.file().into(),
            parts,
            eyes: if self.tracking {
                [false, true]
                    .into_iter()
                    .map(|right| EyeDef {
                        white: Role::Tracking(right, EyeLayer::White).file().into(),
                        pupil: Role::Tracking(right, EyeLayer::Pupil).file().into(),
                        mask: Role::Tracking(right, EyeLayer::Mask).file().into(),
                        blink: self
                            .blink
                            .then(|| Role::Tracking(right, EyeLayer::Blink).file().into()),
                        max_offset: (0.012, 0.009),
                    })
                    .collect()
            } else {
                Vec::new()
            },
            whole: None,
        };
        let name: String = self
            .reference
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Character")
            .chars()
            .take(60)
            .collect();
        let mut manifest = json!({
            "schema_version": 1, "id": format!("{}-pet", slug(&name)), "name": name,
            "canvas": [self.canvas, self.canvas], "default_pose": pose.name,
            "poses": {(&pose.name): pose.to_json()}
        });
        if self.held {
            manifest["reactions"] =
                json!({"pickup": {"held_pose": {"image": Role::Held.file(), "place": [0, 0, 1]}}});
        }
        if !self.extras.is_empty() {
            if manifest["reactions"].is_null() {
                manifest["reactions"] = json!({});
            }
            let settings = super::companionship::Settings {
                petting: self
                    .extras
                    .iter()
                    .any(|r| matches!(r, Role::Petted | Role::HeadPetted)),
                ..Default::default()
            };
            manifest["reactions"]["companionship"] = settings.to_json();
            for role in &self.extras {
                if let Some(event) = role.reaction() {
                    manifest["reactions"][event.key()] =
                        json!({"visual":{"image":role.file(),"place":[0,0,1]}});
                }
            }
        }
        if self.head {
            manifest["music"] = json!({"bop": "nod"});
        }
        manifest
    }

    /// Refuse a completed folder that silently omits a selected feature.
    pub fn validate_folder(&self, folder: &Path) -> Result<(), String> {
        use super::pack::read_manifest;
        let manifest = read_manifest(folder)?;
        if manifest.canvas != (self.canvas, self.canvas) {
            return Err(format!(
                "The requested feature artwork uses a {} × {} canvas.",
                self.canvas, self.canvas
            ));
        }
        let pose = manifest.default_pose();
        let mapped = manifest.files();
        let missing: Vec<_> = self
            .roles()
            .into_iter()
            .filter(|role| !mapped.iter().any(|file| file == role.file()))
            .map(|role| role.file())
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "The selected features need these mapped files: {}. Ask your art tool to finish the folder.",
                missing.join(", ")
            ));
        }
        let expected = self.manifest();
        let actual = pose.to_json();
        let expected_pose = &expected["poses"]["sit"];
        // Match the layer relationships, preserving the creator's measured coordinates.
        for key in ["body", "parts", "eyes"] {
            if !mapping_contains(&actual[key], &expected_pose[key]) {
                return Err(format!(
                    "character.json has an incomplete {key} mapping for the selected features."
                ));
            }
        }
        if self.held
            && manifest.reactions.to_json()["pickup"]["held_pose"]["image"] != Role::Held.file()
        {
            return Err("character.json must map held.png to the pickup held pose.".into());
        }
        if self.head && manifest.bop != Some(super::motion::Bop::Nod) {
            return Err("character.json must include the selected head bop in music.bop.".into());
        }
        for role in &self.extras {
            if let Some(event) = role.reaction()
                && manifest
                    .reactions
                    .entry(event)
                    .and_then(|e| e.art.as_ref())
                    .map(|a| &a.image)
                    != Some(&super::lines::SoundRef::Pack(role.file().into()))
                {
                    return Err(format!(
                        "{} must map to the {} reaction",
                        role.file(),
                        event.key()
                    ));
                }
        }
        manifest.validate_files(folder)
    }

    pub fn fits_canvas(&self) -> bool {
        u64::from(self.canvas) * u64::from(self.canvas) * 4 * self.roles().len() as u64
            <= 128 * 1024 * 1024
    }

    /// One brief covering the complete folder and its import manifest.
    pub fn brief(&self) -> String {
        let roles = self.roles();
        let mut out = format!(
            "Create a TinyPaw character folder containing character.json and {}. Return the finished folder, or a downloadable ZIP of it. Put all files together at the folder root.\n\n{}\n\nArtwork files:\n",
            if roles.len() == 1 {
                "one image".to_owned()
            } else {
                format!("{} images", roles.len())
            },
            self.rules()
        );
        for role in roles {
            out.push_str(&format!("- {}: {}.\n", role.file(), self.describe(role)));
        }
        out.push_str(&format!(
            "\nSelected features to complete: {}.\n",
            self.features().join(", ")
        ));
        out.push_str("\nDeliver an actual character.json file inside the downloadable folder. Measure placement from the finished artwork and write the final numeric coordinates yourself. Each PNG must retain the full shared canvas, so its x/y placement is already encoded by transparent padding. Never trim or centre individual layers. Measure ground_anchor at the feet, each head pivot at the neck joint, each tail pivot at its attachment, and eye max_offset from the safe pupil travel inside its mask. Coordinates [x, y] are canvas fractions from the top left: divide pixel x by canvas width and pixel y by canvas height. For held_pose, include a measured grip: [x_fraction, y_fraction] on the intended pickup point in held.png, and include place: [0, 0, 1] for shared-canvas alignment, or measured [x_offset, y_offset, scale] if needed. Replace the example anchor, pivot and eye travel numbers below with measured values. Keep every selected feature and all filename mappings. Stack the resting layers to verify alignment, confirm eyes have a background/pupil/alpha mask each, and confirm each expression and held pose is mapped. Deliver every required file together.\n\n");
        if self.dance {
            out.push_str("Music artwork alignment: left_arm and right_arm are relaxed arms. holding_arm replaces right_arm during Light wave. Place both interchangeable items in its grip, and draw grip fingers in front of the item. For holding_arm, grip, phone, glow_stick and both glow layers, set the SAME pivot at the viewer-right SHOULDER, with parent omitted. The runtime rotates these aligned layers together. Give left_arm its own shoulder pivot. Keep the halo separate from the item. Leave enough canvas padding for the raised arm and both swing extremes. Stack and rotate the layers together to verify the grip.\n\n");
        }
        out.push_str(&serde_json::to_string_pretty(&self.manifest()).unwrap());
        out
    }

    pub fn features(&self) -> Vec<&'static str> {
        let mut features = vec!["resting character"];
        if self.dance {
            features.push("music gestures with separate arms, grip fingers, phone, glow stick and light halos");
        }
        for (enabled, label) in [
            (
                self.tracking,
                "cursor-tracking eyes (two backgrounds, pupils and masks; separate head required)",
            ),
            (
                self.head,
                "head bop to music (separate head, neck pivot and music nod mapping)",
            ),
            (
                self.tail,
                "moving tail (separate tail and attachment pivot)",
            ),
            (self.eyes && !self.tracking, "separate complete eyes"),
            (self.brows, "separate eyebrows"),
            (self.blink && self.faces_possible(), "blinking"),
            (
                self.faces && self.faces_possible(),
                "pressed and carried expressions with eyes painted in",
            ),
            (
                self.held,
                "held pose (complete hanging character and pickup mapping)",
            ),
        ] {
            if enabled {
                features.push(label);
            }
        }
        for role in &self.extras {
            features.push(match role {
                Role::HeadPetted | Role::Petted => "headpat reaction",
                Role::Sleep => "sleeping pose with mouth origin",
                Role::Wake => "wake reaction",
                Role::Click => "click artwork",
                Role::Drop => "drop artwork",
                _ => continue,
            });
        }
        features
    }

    /// A prompt for one file, for tools that return one image at a time.
    pub fn prompt(&self, role: Role) -> String {
        format!(
            "Create one image for a desktop character, named {}. Attach the approved body.png and head.png (when present) as alignment references alongside the character reference. Retain the full shared canvas. Never trim or centre individual layers.\n\n{}\n\nIt shows {}{}.",
            role.file(),
            self.rules(),
            self.describe(role),
            if self.dance {
                " Music gesture layers share full-canvas alignment. Holding arm, grip fingers, both items and both halos all use the same viewer-right shoulder pivot and remain independent root layers. Attach the approved holding_arm.png as a grip alignment reference. Keep padding for the raised arm and both swing extremes."
            } else {
                ""
            }
        )
    }

    /// Adds returned files, each to the role its name suggests if that
    /// role is still free.
    pub fn add_results(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            if self.results.iter().any(|(p, _)| *p == path) {
                continue;
            }
            let role = Role::guess(&path).filter(|r| self.file_for(*r).is_none());
            self.results.push((path, role));
        }
    }

    /// Gives `role` to result `index`, taking it from any other result.
    pub fn assign(&mut self, index: usize, role: Option<Role>) {
        if let Some(role) = role {
            for (_, r) in &mut self.results {
                if *r == Some(role) {
                    *r = None;
                }
            }
        }
        if let Some((_, r)) = self.results.get_mut(index) {
            *r = role;
        }
    }

    pub fn file_for(&self, role: Role) -> Option<&Path> {
        self.results
            .iter()
            .find(|(_, r)| *r == Some(role))
            .map(|(p, _)| p.as_path())
    }

    /// Requested roles no returned file fills yet.
    pub fn missing(&self) -> Vec<Role> {
        self.roles()
            .into_iter()
            .filter(|r| self.file_for(*r).is_none())
            .collect()
    }
}

fn mapping_contains(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    use serde_json::Value;
    match expected {
        Value::Null => true,
        Value::Object(fields) => fields
            .iter()
            .filter(|(key, _)| !["pivot", "ground_anchor", "max_offset"].contains(&key.as_str()))
            .all(|(key, value)| mapping_contains(&actual[key], value)),
        Value::Array(items) => actual.as_array().is_some_and(|values| {
            items
                .iter()
                .all(|item| values.iter().any(|value| mapping_contains(value, item)))
        }),
        _ => actual == expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_only_request_asks_for_one_picture() {
        let mut r = Request::new("ref.png".into());
        r.style = "soft watercolour".into();
        assert_eq!(r.roles(), [Role::Body]);
        let brief = r.brief();
        assert!(
            brief.contains("one image")
                && brief.contains("body.png")
                && brief.contains("soft watercolour")
        );
        assert!(brief.contains("1024×1024") && !brief.contains("tail.png"));
    }

    #[test]
    fn layered_requests_name_exactly_their_files() {
        let mut r = Request::new("ref.png".into());
        r.tail = true;
        r.held = true;
        r.canvas = 2048;
        assert_eq!(r.roles(), [Role::Body, Role::Tail, Role::Held]);
        let brief = r.brief();
        for f in [
            "body.png",
            "tail.png",
            "held.png",
            "2048×2048",
            "the tail (keep",
            "left out entirely",
            "scruff of the neck",
            "hanging straight down",
        ] {
            assert!(brief.contains(f), "{f}");
        }
        assert!(r.prompt(Role::Tail).contains("named tail.png"));
    }

    #[test]
    fn separate_head_eyes_and_eyebrows_are_requested_and_left_out_elsewhere() {
        let mut r = Request::new("ref.png".into());
        r.head = true;
        r.eyes = true;
        r.brows = true;
        assert_eq!(
            r.roles(),
            [
                Role::Body,
                Role::Head,
                Role::LeftEye,
                Role::RightEye,
                Role::LeftBrow,
                Role::RightBrow
            ]
        );
        let brief = r.brief();
        for f in [
            "head.png",
            "left_eye.png",
            "right_eye.png",
            "left_eyebrow.png",
            "right_eyebrow.png",
        ] {
            assert!(brief.contains(f), "{f}");
        }
        assert!(
            brief.contains("eyes and eyebrows left out"),
            "the head leaves room for them"
        );
        assert!(!brief.contains("eyes and eyebrows painted in\n"));
        // Without separate eyes, the face keeps them painted in.
        let plain = Request::new("ref.png".into());
        assert!(plain.brief().contains("eyes and eyebrows painted in"));
    }

    #[test]
    fn head_faces_need_a_head_with_painted_in_eyes() {
        let mut r = Request::new("ref.png".into());
        r.blink = true;
        r.faces = true;
        assert_eq!(
            r.roles(),
            [
                Role::Body,
                Role::Head,
                Role::HeadBlink,
                Role::HeadPressed,
                Role::HeadCarried
            ],
            "expression features automatically include the required head"
        );
        r.head = true;
        assert_eq!(
            r.roles(),
            [
                Role::Body,
                Role::Head,
                Role::HeadBlink,
                Role::HeadPressed,
                Role::HeadCarried
            ]
        );
        assert!(
            r.brief().contains("head_eyes_closed.png") && r.brief().contains("eyes gently closed")
        );
        r.eyes = true;
        assert!(
            !r.roles().contains(&Role::HeadBlink),
            "separate eyes would stay open on top"
        );
        for (name, role) in [
            ("head_eyes_closed.png", Role::HeadBlink),
            ("Head-Blink.png", Role::HeadBlink),
            ("head_pressed.png", Role::HeadPressed),
            ("head_carried.png", Role::HeadCarried),
            ("held.png", Role::Held),
        ] {
            assert_eq!(Role::guess(Path::new(name)), Some(role), "{name}");
        }
    }

    #[test]
    fn face_files_match_by_side_and_eyebrows_before_eyes() {
        for (name, role) in [
            ("Left_Eye.png", Role::LeftEye),
            ("eye-right.png", Role::RightEye),
            ("left_eyebrow.png", Role::LeftBrow),
            ("brow_r.png", Role::RightBrow),
            ("cat-head.png", Role::Head),
        ] {
            assert_eq!(Role::guess(Path::new(name)), Some(role), "{name}");
        }
        assert_eq!(Role::guess(Path::new("eye.png")), None, "no side, no guess");
    }

    #[test]
    fn returned_files_match_by_name_and_can_be_reassigned() {
        let mut r = Request::new("ref.png".into());
        r.tail = true;
        r.add_results(vec![
            "cat_body.png".into(),
            "Cat Tail.PNG".into(),
            "extra.png".into(),
        ]);
        assert_eq!(r.file_for(Role::Body), Some(Path::new("cat_body.png")));
        assert_eq!(r.file_for(Role::Tail), Some(Path::new("Cat Tail.PNG")));
        assert!(r.missing().is_empty());
        // Giving Body to another file frees it from the first.
        r.assign(2, Some(Role::Body));
        assert_eq!(r.file_for(Role::Body), Some(Path::new("extra.png")));
        assert_eq!(r.results[0].1, None);
        r.held = true;
        assert_eq!(r.missing(), [Role::Held]);
    }
}
