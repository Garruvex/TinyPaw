//! Platform-neutral shortcut descriptions such as `Ctrl+Alt+P`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutContext {
    /// Registered with the desktop; fires whatever has focus.
    Global,
    /// Handled only while a TinyPaw overlay has keyboard focus.
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A letter or digit, stored uppercase.
    Char(char),
    Alt,
    Escape,
    Space,
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub context: ShortcutContext,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: Key,
}

impl Shortcut {
    pub fn parse(context: ShortcutContext, text: &str) -> Option<Self> {
        if text.eq_ignore_ascii_case("alt") {
            return Some(Shortcut {
                context,
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Alt,
            });
        }
        let mut s = Shortcut {
            context,
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Escape,
        };
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            let lower = part.to_ascii_lowercase();
            if key.is_some()
                && !matches!(
                    lower.as_str(),
                    "ctrl" | "control" | "cmd" | "alt" | "option" | "shift"
                )
            {
                return None;
            }
            match lower.as_str() {
                "ctrl" | "control" | "cmd" => s.ctrl = true,
                "alt" | "option" => s.alt = true,
                "shift" => s.shift = true,
                "esc" | "escape" => key = Some(Key::Escape),
                "space" => key = Some(Key::Space),
                p if p.len() > 1 && p.starts_with('f') => {
                    let n: u8 = p[1..].parse().ok()?;
                    if !(1..=24).contains(&n) {
                        return None;
                    }
                    key = Some(Key::F(n));
                }
                p if p.len() == 1 && p.chars().all(|c| c.is_ascii_alphanumeric()) => {
                    key = Some(Key::Char(p.chars().next()?.to_ascii_uppercase()));
                }
                _ => return None,
            }
        }
        s.key = key?;
        Some(s)
    }

    /// Global shortcuts fire while the overlay has focus too, so they overlap
    /// every context.
    pub fn conflicts(&self, other: &Shortcut) -> bool {
        let overlap = self.context == other.context
            || self.context == ShortcutContext::Global
            || other.context == ShortcutContext::Global;
        overlap
            && self.ctrl == other.ctrl
            && self.alt == other.alt
            && self.shift == other.shift
            && self.key == other.key
    }

    pub fn label(&self) -> String {
        let mut out = String::new();
        if self.ctrl {
            out.push_str("Ctrl+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        match self.key {
            Key::Char(c) => out.push(c),
            Key::Alt => out.push_str("Alt"),
            Key::Escape => out.push_str("Esc"),
            Key::Space => out.push_str("Space"),
            Key::F(n) => out.push_str(&format!("F{n}")),
        }
        out
    }
}
