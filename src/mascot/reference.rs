//! Create from reference: from a reference picture and a few choices,
//! the exact artwork to ask an image tool for, one consistent brief to copy,
//! and matching the PNGs that come back to their roles.
//!
//! TinyPaw generates nothing itself here: the creator attaches the reference
//! to their own image tool, pastes the brief, and imports the results, which
//! open in the artwork editor like any other parts.

use std::path::{Path, PathBuf};

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
    /// Head faces that replace the head: eyes closed (blinking), pressed
    /// (clicked) and being carried. They need painted-in eyes.
    HeadBlink,
    HeadPressed,
    HeadCarried,
    /// The whole character in a carried pose, for Pick up's held pose.
    Held,
}

impl Role {
    pub fn file(self) -> &'static str {
        match self {
            Role::Body => "body.png",
            Role::Head => "head.png",
            Role::Tail => "tail.png",
            Role::LeftEye => "left_eye.png",
            Role::RightEye => "right_eye.png",
            Role::LeftBrow => "left_eyebrow.png",
            Role::RightBrow => "right_eyebrow.png",
            Role::HeadBlink => "head_eyes_closed.png",
            Role::HeadPressed => "head_pressed.png",
            Role::HeadCarried => "head_carried.png",
            Role::Held => "held.png",
        }
    }

    /// The head face slot a face role fills: blink, pressed, dragging.
    pub fn face_slot(self) -> Option<usize> {
        match self {
            Role::HeadBlink => Some(0),
            Role::HeadPressed => Some(1),
            Role::HeadCarried => Some(2),
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
            Role::HeadBlink => "Head, eyes closed",
            Role::HeadPressed => "Head, pressed",
            Role::HeadCarried => "Head, being carried",
            Role::Held => "Held pose",
        }
    }

    /// Eyes and eyebrows: still layers that move with the head.
    pub fn is_face(self) -> bool {
        matches!(self, Role::LeftEye | Role::RightEye | Role::LeftBrow | Role::RightBrow)
    }

    /// The role a returned file's name suggests. Eyebrows are checked
    /// before eyes, since "eyebrow" contains "eye".
    pub fn guess(path: &Path) -> Option<Role> {
        let name = path.file_stem()?.to_str()?.to_lowercase();
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
    /// Separate eye and eyebrow images instead of painted-in ones.
    pub eyes: bool,
    pub brows: bool,
    /// Head faces: eyes closed for blinking, and pressed and carried
    /// reactions. Only with a separate head and painted-in eyes.
    pub blink: bool,
    pub faces: bool,
    pub held: bool,
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
            eyes: false,
            brows: false,
            blink: false,
            faces: false,
            held: false,
            results: Vec::new(),
        }
    }

    /// The files to request, in order. A base-only request is one picture.
    pub fn roles(&self) -> Vec<Role> {
        let mut out = vec![Role::Body];
        for (on, roles) in [
            (self.head, &[Role::Head][..]),
            (self.tail, &[Role::Tail][..]),
            (self.eyes, &[Role::LeftEye, Role::RightEye][..]),
            (self.brows, &[Role::LeftBrow, Role::RightBrow][..]),
            (self.blink && self.faces_possible(), &[Role::HeadBlink][..]),
            (self.faces && self.faces_possible(), &[Role::HeadPressed, Role::HeadCarried][..]),
            (self.held, &[Role::Held][..]),
        ] {
            if on {
                out.extend_from_slice(roles);
            }
        }
        out
    }

    /// Head faces replace the head art, so they need a separate head with
    /// its eyes painted in: separate eye layers would stay open on top.
    pub fn faces_possible(&self) -> bool {
        self.head && !self.eyes
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
        match (self.eyes, self.brows) {
            (false, false) => "with the eyes and eyebrows painted in",
            (true, false) => "with the eyebrows painted in but the eyes left out: paint plain skin or fur where the eyes sit, since they come as separate files",
            (false, true) => "with the eyes painted in but the eyebrows left out: paint plain skin or fur where they sit, since they come as separate files",
            (true, true) => "with the eyes and eyebrows left out: paint plain skin or fur where they sit, since they come as separate files",
        }
    }

    /// What one file should show.
    fn describe(&self, role: Role) -> String {
        let face = self.painted_face();
        let mut removed = Vec::new();
        if self.head {
            removed.push("the head (keep the neck, with a little painted overlap where the head joins)");
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
            Role::Body if self.head => format!(
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
            Role::Tail => "only the tail, in the exact position where it attaches to the body file, with a little extra painted overlap at its base".into(),
            Role::LeftEye => facial("eye", "left"),
            Role::RightEye => facial("eye", "right"),
            Role::LeftBrow => facial("eyebrow", "left"),
            Role::RightBrow => facial("eyebrow", "right"),
            Role::HeadBlink => "only the head, identical to head.png in position, size and outline, with the eyes gently closed as in a blink".into(),
            Role::HeadPressed => "only the head, identical to head.png in position, size and outline, with a surprised or delighted expression, as when poked".into(),
            Role::HeadCarried => "only the head, identical to head.png in position, size and outline, with a startled or excited expression, as when being picked up".into(),
            Role::Held => "the whole character in a carried pose, as if lifted by the scruff and dangling, at the same scale as the body file, with the eyes and eyebrows painted in".into(),
        }
    }

    /// The shared rules every file follows, so the results line up.
    fn rules(&self) -> String {
        format!(
            "Use the attached picture as the character reference. Style: {}. Every file is a {c}×{c} PNG with a fully transparent background, the same camera angle, scale and position, so the files line up when stacked; keep markings, colours and proportions consistent with the reference. Leave transparent space around the character and don't crop it.",
            self.style(),
            c = self.canvas,
        )
    }

    /// One brief covering every requested file.
    pub fn brief(&self) -> String {
        let roles = self.roles();
        let mut out = format!(
            "Create {} for a desktop character.\n\n{}\n\nFiles:\n",
            if roles.len() == 1 { "one image".to_owned() } else { format!("{} images", roles.len()) },
            self.rules()
        );
        for role in roles {
            out.push_str(&format!("- {}: {}.\n", role.file(), self.describe(role)));
        }
        out
    }

    /// A prompt for one file, for tools that return one image at a time.
    pub fn prompt(&self, role: Role) -> String {
        format!(
            "Create one image for a desktop character, named {}.\n\n{}\n\nIt shows {}.",
            role.file(),
            self.rules(),
            self.describe(role)
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
        self.roles().into_iter().filter(|r| self.file_for(*r).is_none()).collect()
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
        assert!(brief.contains("one image") && brief.contains("body.png") && brief.contains("soft watercolour"));
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
        for f in ["body.png", "tail.png", "held.png", "2048×2048", "the tail (keep", "left out entirely"] {
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
            [Role::Body, Role::Head, Role::LeftEye, Role::RightEye, Role::LeftBrow, Role::RightBrow]
        );
        let brief = r.brief();
        for f in ["head.png", "left_eye.png", "right_eye.png", "left_eyebrow.png", "right_eyebrow.png"] {
            assert!(brief.contains(f), "{f}");
        }
        assert!(brief.contains("eyes and eyebrows left out"), "the head leaves room for them");
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
        assert_eq!(r.roles(), [Role::Body], "no head, no faces");
        r.head = true;
        assert_eq!(
            r.roles(),
            [Role::Body, Role::Head, Role::HeadBlink, Role::HeadPressed, Role::HeadCarried]
        );
        assert!(r.brief().contains("head_eyes_closed.png") && r.brief().contains("eyes gently closed"));
        r.eyes = true;
        assert!(!r.roles().contains(&Role::HeadBlink), "separate eyes would stay open on top");
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
        r.add_results(vec!["cat_body.png".into(), "Cat Tail.PNG".into(), "extra.png".into()]);
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
