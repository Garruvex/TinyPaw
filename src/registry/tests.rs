use super::*;
use crate::features::{self, point, system};
use crate::session::Mode;

fn session(reg: &Registry) -> Session {
    let t = reg.default_tool();
    Session::new(t.id, t.factory, reg.defaults())
}

#[test]
fn builtins_validate() {
    let reg = Registry::build(features::BUILTINS).unwrap();
    assert_eq!(reg.default_tool().id, point::TOOL);
    assert!(reg.command(system::MASTER_TOGGLE).is_some());
}

#[test]
fn picker_shortcut_and_cli_reach_the_same_command() {
    let reg = Registry::build(features::BUILTINS).unwrap();
    let by_cli = reg.command_by_cli("toggle").unwrap();
    let (_, by_key) = reg
        .shortcuts(ShortcutContext::Global)
        .find(|(s, _)| s.label() == "Ctrl+Alt+P")
        .unwrap();
    assert_eq!(by_cli.id, by_key.id);

    let mut a = session(&reg);
    let mut b = session(&reg);
    reg.dispatch(by_cli.id, &mut a, 0).unwrap();
    reg.dispatch(by_key.id, &mut b, 0).unwrap();
    assert_eq!(a.mode(), b.mode());
    assert_eq!(a.mode(), Mode::Passthrough);
}

#[test]
fn unavailable_command_is_refused_via_cli() {
    let reg = Registry::build(features::BUILTINS).unwrap();
    let mut s = session(&reg);
    let undo = reg.command_by_cli("undo").unwrap();
    assert_eq!(
        reg.dispatch(undo.id, &mut s, 0),
        Err(Unavailable("TinyPaw is Off"))
    );
}

#[test]
fn setting_edit_and_reset_round_trip() {
    let reg = Registry::build(features::BUILTINS).unwrap();
    let mut v = reg.defaults();
    assert_eq!(v.float(system::INK_WIDTH), 4.0);
    reg.set(&mut v, system::INK_WIDTH, Value::Float(8.0))
        .unwrap();
    assert_eq!(v.float(system::INK_WIDTH), 8.0);
    assert!(
        reg.set(&mut v, system::INK_WIDTH, Value::Float(99.0))
            .is_err()
    );
    assert!(
        reg.set(&mut v, system::INK_WIDTH, Value::Bool(true))
            .is_err()
    );
    assert!(
        reg.set(&mut v, system::ACTIVATION_STYLE, Value::Choice("bogus"))
            .is_err()
    );
    reg.reset(&mut v, system::INK_WIDTH);
    assert_eq!(v, reg.defaults());
}

fn noop(_: &Session) -> Outcome {
    Outcome::Session(crate::session::Event::Undo)
}

fn cmd(id: &'static str, shortcut: Option<(ShortcutContext, &'static str)>) -> CommandDef {
    CommandDef {
        id,
        label: id,
        cli: None,
        surfaces: Surfaces::NONE,
        group: "test",
        order: 0,
        shortcut,
        available: always,
        handler: noop,
        on_release: None,
    }
}

#[test]
fn rejects_duplicate_ids() {
    fn a(r: &mut Registrar) {
        r.command(cmd("dup", None));
        r.command(cmd("dup", None));
    }
    assert_eq!(
        Registry::build(&[a]).err(),
        Some(RegistryError::DuplicateId("dup"))
    );
}

#[test]
fn rejects_dangling_tool_command() {
    fn a(r: &mut Registrar) {
        r.tool(ToolDef {
            id: "t",
            label: "t",
            select_command: "missing",
            factory: point::new_state,
        });
    }
    assert!(matches!(
        Registry::build(&[a]),
        Err(RegistryError::DanglingCommand { .. })
    ));
}

#[test]
fn rejects_invalid_default() {
    fn a(r: &mut Registrar) {
        r.setting(SettingDef {
            id: "s",
            label: "s",
            group: "g",
            order: 0,
            default: Value::Float(5.0),
            constraint: Constraint::Range { min: 0.0, max: 1.0 },
        });
    }
    assert_eq!(
        Registry::build(&[a]).err(),
        Some(RegistryError::InvalidDefault("s"))
    );
}

#[test]
fn rejects_ambiguous_shortcuts_across_overlapping_contexts() {
    fn a(r: &mut Registrar) {
        r.command(cmd("a", Some((ShortcutContext::Global, "Ctrl+Alt+P"))));
        r.command(cmd("b", Some((ShortcutContext::Overlay, "ctrl+alt+p"))));
    }
    assert!(matches!(
        Registry::build(&[a]),
        Err(RegistryError::AmbiguousShortcut { .. })
    ));
}

/// A small feature with one action and one setting, added using only its
/// module and the built-ins list.
mod sample_feature {
    use super::*;

    pub fn register(r: &mut Registrar) {
        r.command(CommandDef {
            id: "sample.ping",
            label: "Ping",
            cli: Some("ping"),
            surfaces: Surfaces::CLI,
            group: "sample",
            order: 0,
            shortcut: None,
            available: when_enabled,
            handler: |_| Outcome::Session(crate::session::Event::Clear),
            on_release: None,
        });
        r.setting(SettingDef {
            id: "sample.level",
            label: "Level",
            group: "Appearance",
            order: 99,
            default: Value::Float(0.5),
            constraint: Constraint::Range { min: 0.0, max: 1.0 },
        });
    }
}

#[test]
fn feature_added_through_builtins_list_only() {
    let mut list = features::BUILTINS.to_vec();
    list.push(sample_feature::register);
    let reg = Registry::build(&list).unwrap();
    let mut s = session(&reg);
    assert_eq!(s.settings().float("sample.level"), 0.5);
    assert!(reg.dispatch("sample.ping", &mut s, 0).is_err());
    reg.dispatch(system::MASTER_TOGGLE, &mut s, 0).unwrap();
    assert!(
        reg.dispatch(reg.command_by_cli("ping").unwrap().id, &mut s, 0)
            .is_ok()
    );
}

#[test]
fn shortcut_parsing() {
    let s = Shortcut::parse(ShortcutContext::Global, "Ctrl+Alt+P").unwrap();
    assert!(s.ctrl && s.alt && !s.shift);
    assert_eq!(s.key, Key::Char('P'));
    assert_eq!(s.label(), "Ctrl+Alt+P");
    assert!(Shortcut::parse(ShortcutContext::Overlay, "Escape").is_some());
    assert!(Shortcut::parse(ShortcutContext::Overlay, "F13").is_some());
    assert!(Shortcut::parse(ShortcutContext::Overlay, "Ctrl+Banana").is_none());
    assert!(Shortcut::parse(ShortcutContext::Overlay, "Ctrl").is_none());
}

#[test]
fn parses_text_into_declared_types() {
    let reg = Registry::build(features::BUILTINS).unwrap();
    assert_eq!(
        reg.parse_value(system::ACTIVATION_STYLE, "hold"),
        Ok(Value::Choice("hold"))
    );
    assert!(
        reg.parse_value(system::ACTIVATION_STYLE, "sometimes")
            .is_err()
    );
    assert_eq!(
        reg.parse_value(system::INK_WIDTH, "6"),
        Ok(Value::Float(6.0))
    );
    assert_eq!(
        reg.parse_value(system::INK_COLOR, "#ff8000"),
        Ok(Value::Color([0xFF, 0x80, 0x00, 0xFF]))
    );
    assert!(reg.parse_value("nope", "1").is_err());
}

#[test]
fn mute_command_flips_the_setting_through_set() {
    let reg = Registry::build(features::BUILTINS).unwrap();
    let mut s = session(&reg);
    let d = reg
        .dispatch(features::sound::TOGGLE_MUTE, &mut s, 0)
        .unwrap();
    assert_eq!(d, Dispatched::Set(features::sound::MUTE, Value::Bool(true)));
}

#[test]
fn lens_and_freeze_are_live() {
    let reg = Registry::build(features::BUILTINS).unwrap();
    let mut s = session(&reg);
    reg.dispatch(system::MASTER_TOGGLE, &mut s, 0).unwrap();
    assert!(reg.dispatch(features::upcoming::LENS, &mut s, 0).is_ok());
    assert_eq!(s.active_tool(), features::upcoming::TOOL);
    let freeze = reg.command_by_cli("freeze").unwrap();
    assert!(reg.dispatch(freeze.id, &mut s, 0).is_ok());
    assert_eq!(reg.shortcut_label(freeze.id).as_deref(), Some("Ctrl+Alt+F"));
    assert_eq!(
        reg.shortcut_label(features::wheel::OPEN).as_deref(),
        Some("Ctrl+Alt+W")
    );
}

#[test]
fn rebind_rejects_conflicts_without_changing_current_binding() {
    let mut r = Registry::build(features::BUILTINS).unwrap();
    r.rebind(system::MASTER_TOGGLE, "Ctrl+Shift+F9").unwrap();
    assert_eq!(
        r.shortcut_label(system::MASTER_TOGGLE).unwrap(),
        "Ctrl+Shift+F9"
    );
    assert!(r.rebind(system::HOLD, "Ctrl+Shift+F9").is_err());
    assert_eq!(r.shortcut_label(system::HOLD).unwrap(), "Ctrl+Alt+D");
    assert!(r.rebind(system::MASTER_TOGGLE, "Alt").is_err());
}

#[test]
fn restoring_swapped_shortcuts_validates_the_complete_set() {
    let mut r = Registry::build(features::BUILTINS).unwrap();
    let map = std::collections::BTreeMap::from([
        (system::MASTER_TOGGLE.to_string(), "Ctrl+Alt+D".into()),
        (system::HOLD.to_string(), "Ctrl+Alt+P".into()),
    ]);
    r.restore_bindings(&map).unwrap();
    assert_eq!(
        r.shortcut_label(system::MASTER_TOGGLE).unwrap(),
        "Ctrl+Alt+D"
    );
    assert_eq!(r.shortcut_label(system::HOLD).unwrap(), "Ctrl+Alt+P");
}
