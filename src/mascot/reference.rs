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
    /// The complete character, or the body with a separate tail removed.
    Body,
    /// A separate tail, for Sway, Wag or Hang.
    Tail,
    /// The whole character in a carried pose, for Pick up's held pose.
    Held,
}

impl Role {
    pub fn file(self) -> &'static str {
        match self {
            Role::Body => "body.png",
            Role::Tail => "tail.png",
            Role::Held => "held.png",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Body => "Body",
            Role::Tail => "Tail",
            Role::Held => "Held pose",
        }
    }

    /// The role a returned file's name suggests.
    pub fn guess(path: &Path) -> Option<Role> {
        let name = path.file_stem()?.to_str()?.to_lowercase();
        [(Role::Held, "held"), (Role::Held, "carried"), (Role::Tail, "tail"), (Role::Body, "body")]
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
    pub tail: bool,
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
            tail: false,
            held: false,
            results: Vec::new(),
        }
    }

    /// The files to request, in order. A base-only request is one picture.
    pub fn roles(&self) -> Vec<Role> {
        let mut out = vec![Role::Body];
        if self.tail {
            out.push(Role::Tail);
        }
        if self.held {
            out.push(Role::Held);
        }
        out
    }

    fn style(&self) -> String {
        let style = self.style.trim();
        if style.is_empty() {
            "the reference's own style".into()
        } else {
            style.into()
        }
    }

    /// What one file should show.
    fn describe(&self, role: Role) -> String {
        match role {
            Role::Body if self.tail => "the whole character standing or sitting at rest, with the tail left out entirely, keeping a little painted overlap where the tail joins so there is no seam".into(),
            Role::Body => "the whole character standing or sitting at rest, eyes and face painted in".into(),
            Role::Tail => "only the tail, in the exact position where it attaches to the body file, with a little extra painted overlap at its base".into(),
            Role::Held => "the whole character in a carried pose, as if lifted by the scruff and dangling, at the same scale as the body file".into(),
        }
    }

    /// The shared rules every file follows, so the results line up.
    fn rules(&self) -> String {
        format!(
            "Use the attached picture as the character reference. Style: {}. Every file is a {c}×{c} PNG with a fully transparent background, the same camera angle, scale and position, so the files line up when stacked; keep markings, colours and proportions consistent with the reference. Keep the eyes and eyebrows painted into the face. Leave transparent space around the character and don't crop it.",
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
        for f in ["body.png", "tail.png", "held.png", "2048×2048", "tail left out"] {
            assert!(brief.contains(f), "{f}");
        }
        assert!(r.prompt(Role::Tail).contains("named tail.png"));
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
