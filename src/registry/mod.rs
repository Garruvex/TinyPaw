//! Feature registration contract, definition validation and lookup indexes.
//!
//! Every user-facing command and setting enters through [`Registrar`]. The
//! registry is built once at startup from the built-ins list and frozen.

mod shortcut;

pub use shortcut::{Key, Shortcut, ShortcutContext};

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::session::{Effect, Event, Millis, Session, ToolFactory};

/// Where a command may be offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surfaces(u8);

impl Surfaces {
    pub const TRAY: Surfaces = Surfaces(1);
    pub const TOOLBAR: Surfaces = Surfaces(2);
    pub const PICKER: Surfaces = Surfaces(4);
    pub const CLI: Surfaces = Surfaces(8);
    #[cfg(test)]
    pub const NONE: Surfaces = Surfaces(0);

    pub const fn with(self, other: Surfaces) -> Surfaces {
        Surfaces(self.0 | other.0)
    }

    pub const fn contains(self, other: Surfaces) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Why a command cannot run right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable(pub &'static str);

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

pub type Availability = fn(&Session) -> Result<(), Unavailable>;

/// What a command handler asks the session or the shell to do.
pub enum Outcome {
    Session(Event),
    /// Handled by the shell, such as Quit.
    Shell(ShellAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellAction {
    Quit,
}

pub type Handler = fn(&Session) -> Outcome;

pub struct CommandDef {
    pub id: &'static str,
    pub label: &'static str,
    /// Name accepted by `tinypaw <name>`, if offered on the CLI.
    pub cli: Option<&'static str>,
    pub surfaces: Surfaces,
    pub group: &'static str,
    pub order: u16,
    pub shortcut: Option<(ShortcutContext, &'static str)>,
    pub available: Availability,
    pub handler: Handler,
    /// For hold commands: the event sent when the bound key is released.
    pub on_release: Option<Handler>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Bool backs Mute and Lens toggles in phase 2.
pub enum Value {
    Bool(bool),
    Float(f64),
    Color([u8; 4]),
    Choice(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    None,
    Range { min: f64, max: f64 },
    Choices(&'static [&'static str]),
}

/// `label`, `group` and `order` lay out the settings window (phase 2).
#[allow(dead_code)]
pub struct SettingDef {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub order: u16,
    pub default: Value,
    pub constraint: Constraint,
}

#[allow(dead_code)] // `label` is shown by the tool picker (phase 3).
pub struct ToolDef {
    pub id: &'static str,
    pub label: &'static str,
    pub select_command: &'static str,
    pub factory: ToolFactory,
}

/// Current, validated setting values keyed by setting ID.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SettingValues(BTreeMap<&'static str, Value>);

impl SettingValues {
    pub fn float(&self, id: &str) -> f64 {
        match self.0.get(id) {
            Some(Value::Float(v)) => *v,
            _ => 0.0,
        }
    }

    pub fn color(&self, id: &str) -> [u8; 4] {
        match self.0.get(id) {
            Some(Value::Color(c)) => *c,
            _ => [0xFF; 4],
        }
    }

    pub fn choice(&self, id: &str) -> &'static str {
        match self.0.get(id) {
            Some(Value::Choice(c)) => c,
            _ => "",
        }
    }

}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateId(&'static str),
    DanglingCommand { tool: &'static str, command: &'static str },
    InvalidDefault(&'static str),
    BadShortcut { command: &'static str, binding: &'static str },
    AmbiguousShortcut { a: &'static str, b: &'static str },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate registration ID `{id}`"),
            Self::DanglingCommand { tool, command } => {
                write!(f, "tool `{tool}` refers to unknown command `{command}`")
            }
            Self::InvalidDefault(id) => write!(f, "setting `{id}` has an invalid default"),
            Self::BadShortcut { command, binding } => {
                write!(f, "command `{command}` has unparseable shortcut `{binding}`")
            }
            Self::AmbiguousShortcut { a, b } => {
                write!(f, "commands `{a}` and `{b}` share a default shortcut")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Collects definitions from one feature module.
#[derive(Default)]
pub struct Registrar {
    commands: Vec<CommandDef>,
    settings: Vec<SettingDef>,
    tools: Vec<ToolDef>,
}

impl Registrar {
    pub fn command(&mut self, def: CommandDef) {
        self.commands.push(def);
    }

    pub fn setting(&mut self, def: SettingDef) {
        self.settings.push(def);
    }

    pub fn tool(&mut self, def: ToolDef) {
        self.tools.push(def);
    }
}

pub type RegisterFn = fn(&mut Registrar);

/// Frozen, validated definitions with dispatch indexes.
pub struct Registry {
    commands: Vec<CommandDef>,
    settings: Vec<SettingDef>,
    tools: Vec<ToolDef>,
    by_id: HashMap<&'static str, usize>,
    by_cli: HashMap<&'static str, usize>,
    shortcuts: Vec<(Shortcut, usize)>,
}

impl Registry {
    pub fn build(builtins: &[RegisterFn]) -> Result<Self, RegistryError> {
        let mut r = Registrar::default();
        for register in builtins {
            register(&mut r);
        }

        let mut seen: HashMap<&'static str, ()> = HashMap::new();
        let ids = r
            .commands
            .iter()
            .map(|c| c.id)
            .chain(r.settings.iter().map(|s| s.id))
            .chain(r.tools.iter().map(|t| t.id));
        for id in ids {
            if seen.insert(id, ()).is_some() {
                return Err(RegistryError::DuplicateId(id));
            }
        }

        let mut by_id = HashMap::new();
        let mut by_cli = HashMap::new();
        let mut shortcuts: Vec<(Shortcut, usize)> = Vec::new();
        for (i, c) in r.commands.iter().enumerate() {
            by_id.insert(c.id, i);
            if let Some(name) = c.cli
                && by_cli.insert(name, i).is_some()
            {
                return Err(RegistryError::DuplicateId(name));
            }
            if let Some((context, binding)) = c.shortcut {
                let shortcut = Shortcut::parse(context, binding).ok_or(
                    RegistryError::BadShortcut {
                        command: c.id,
                        binding,
                    },
                )?;
                if let Some((_, j)) = shortcuts.iter().find(|(s, _)| s.conflicts(&shortcut)) {
                    return Err(RegistryError::AmbiguousShortcut {
                        a: r.commands[*j].id,
                        b: c.id,
                    });
                }
                shortcuts.push((shortcut, i));
            }
        }

        for t in &r.tools {
            if !by_id.contains_key(t.select_command) {
                return Err(RegistryError::DanglingCommand {
                    tool: t.id,
                    command: t.select_command,
                });
            }
        }

        for s in &r.settings {
            if !valid(&s.default, &s.constraint) {
                return Err(RegistryError::InvalidDefault(s.id));
            }
        }

        Ok(Self {
            commands: r.commands,
            settings: r.settings,
            tools: r.tools,
            by_id,
            by_cli,
            shortcuts,
        })
    }

    pub fn command(&self, id: &str) -> Option<&CommandDef> {
        self.by_id.get(id).map(|&i| &self.commands[i])
    }

    pub fn command_by_cli(&self, name: &str) -> Option<&CommandDef> {
        self.by_cli.get(name).map(|&i| &self.commands[i])
    }

    pub fn default_tool(&self) -> &ToolDef {
        self.tools.first().expect("at least one interactive tool")
    }

    /// Commands offered on a surface, in group/order sequence.
    pub fn commands_for(&self, surface: Surfaces) -> Vec<&CommandDef> {
        let mut list: Vec<_> = self
            .commands
            .iter()
            .filter(|c| c.surfaces.contains(surface))
            .collect();
        list.sort_by_key(|c| (c.group, c.order));
        list
    }

    pub fn cli_commands(&self) -> Vec<&CommandDef> {
        let mut list: Vec<_> = self.commands.iter().filter(|c| c.cli.is_some()).collect();
        list.sort_by_key(|c| c.cli);
        list
    }

    pub fn shortcuts(&self, context: ShortcutContext) -> impl Iterator<Item = (&Shortcut, &CommandDef)> {
        self.shortcuts
            .iter()
            .filter(move |(s, _)| s.context == context)
            .map(|(s, i)| (s, &self.commands[*i]))
    }

    pub fn defaults(&self) -> SettingValues {
        SettingValues(self.settings.iter().map(|s| (s.id, s.default.clone())).collect())
    }

    /// The one typed update path for every editor of a setting.
    pub fn set(&self, values: &mut SettingValues, id: &str, value: Value) -> Result<(), Unavailable> {
        let def = self
            .settings
            .iter()
            .find(|s| s.id == id)
            .ok_or(Unavailable("unknown setting"))?;
        if !valid(&value, &def.constraint)
            || std::mem::discriminant(&value) != std::mem::discriminant(&def.default)
        {
            return Err(Unavailable("value outside the declared constraints"));
        }
        values.0.insert(def.id, value);
        Ok(())
    }

    /// Parses text (from the CLI or a text field) into the setting's declared type.
    pub fn parse_value(&self, id: &str, text: &str) -> Result<Value, Unavailable> {
        let def = self
            .settings
            .iter()
            .find(|s| s.id == id)
            .ok_or(Unavailable("unknown setting"))?;
        let bad = Unavailable("value does not match the setting's type");
        Ok(match (&def.default, &def.constraint) {
            (Value::Bool(_), _) => Value::Bool(match text {
                "true" | "on" | "1" => true,
                "false" | "off" | "0" => false,
                _ => return Err(bad),
            }),
            (Value::Float(_), _) => Value::Float(text.parse().map_err(|_| bad)?),
            (Value::Color(_), _) => {
                let hex = text.trim_start_matches('#');
                let n = u32::from_str_radix(hex, 16).map_err(|_| bad.clone())?;
                match hex.len() {
                    6 => Value::Color([(n >> 16) as u8, (n >> 8) as u8, n as u8, 0xFF]),
                    8 => Value::Color((n).to_be_bytes()),
                    _ => return Err(bad),
                }
            }
            (Value::Choice(_), Constraint::Choices(list)) => {
                Value::Choice(list.iter().find(|c| **c == text).ok_or(bad)?)
            }
            (Value::Choice(_), _) => return Err(bad),
        })
    }

    #[cfg(test)]
    pub fn reset(&self, values: &mut SettingValues, id: &str) {
        if let Some(def) = self.settings.iter().find(|s| s.id == id) {
            values.0.insert(def.id, def.default.clone());
        }
    }

    /// Runs a command after checking availability, regardless of which
    /// surface invoked it. Shell actions are returned to the caller.
    pub fn dispatch(
        &self,
        id: &str,
        session: &mut Session,
        now: Millis,
    ) -> Result<Dispatched, Unavailable> {
        let def = self.command(id).ok_or(Unavailable("unknown command"))?;
        self.run(def, def.handler, session, now)
    }

    /// Runs a hold command's release half. Release is never refused.
    pub fn dispatch_release(&self, id: &str, session: &mut Session, now: Millis) -> Option<Dispatched> {
        let def = self.command(id)?;
        let handler = def.on_release?;
        Some(Self::apply(handler(session), session, now))
    }

    fn run(
        &self,
        def: &CommandDef,
        handler: Handler,
        session: &mut Session,
        now: Millis,
    ) -> Result<Dispatched, Unavailable> {
        (def.available)(session)?;
        Ok(Self::apply(handler(session), session, now))
    }

    fn apply(outcome: Outcome, session: &mut Session, now: Millis) -> Dispatched {
        match outcome {
            Outcome::Session(event) => Dispatched::Effects(session.handle(event, now)),
            Outcome::Shell(action) => Dispatched::Shell(action),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Dispatched {
    Effects(Vec<Effect>),
    Shell(ShellAction),
}

fn valid(value: &Value, constraint: &Constraint) -> bool {
    match (value, constraint) {
        (_, Constraint::None) => true,
        (Value::Float(v), Constraint::Range { min, max }) => v.is_finite() && *v >= *min && *v <= *max,
        (Value::Choice(c), Constraint::Choices(list)) => list.contains(c),
        _ => false,
    }
}

/// Always available.
pub fn always(_: &Session) -> Result<(), Unavailable> {
    Ok(())
}

/// Available only while TinyPaw is switched On.
pub fn when_enabled(s: &Session) -> Result<(), Unavailable> {
    if s.is_enabled() {
        Ok(())
    } else {
        Err(Unavailable("TinyPaw is Off"))
    }
}

#[cfg(test)]
mod tests;
