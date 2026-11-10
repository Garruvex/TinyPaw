//! TinyPaw: a paw pointer for pointing and drawing over presentations and
//! game streams. `tinypaw` starts the resident instance; `tinypaw <command>`
//! forwards a command to it.

mod app;
mod audio;
mod features;
mod packs;
mod platform;
mod preferences;
mod registry;
mod session;
mod view;

use std::process::ExitCode;

use registry::Registry;

fn usage(registry: &Registry) -> String {
    let mut out = String::from("Usage: tinypaw [command]\n\nWith no command, starts TinyPaw in the tray.\n\nCommands:\n");
    out.push_str("  status       Report whether TinyPaw is On\n");
    out.push_str("  set <setting> <value>\n               Change a setting, e.g. `set system.activation_style hold`\n");
    for def in registry.cli_commands() {
        if let Some(name) = def.cli {
            out.push_str(&format!("  {name:<12} {}\n", def.label));
        }
    }
    out
}

fn main() -> ExitCode {
    let registry: &'static Registry = match Registry::build(features::BUILTINS) {
        Ok(r) => Box::leak(Box::new(r)),
        Err(e) => {
            eprintln!("tinypaw: invalid feature registration: {e}");
            return ExitCode::FAILURE;
        }
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(cmd) = args.first() {
        if matches!(cmd.as_str(), "help" | "--help" | "-h") {
            print!("{}", usage(registry));
            return ExitCode::SUCCESS;
        }
        // Let the resident instance take focus for this request.
        #[cfg(windows)]
        platform::windows::allow_any_foreground();
        return match platform::ipc::request(&args.join(" ")) {
            Ok(reply) => {
                println!("{reply}");
                if reply.starts_with("ok") {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(_) => {
                eprintln!("TinyPaw is not running. Start it with `tinypaw`.");
                ExitCode::from(2)
            }
        };
    }

    let listener = match platform::ipc::bind() {
        Ok(platform::ipc::Bind::Listening(l)) => l,
        Ok(platform::ipc::Bind::AlreadyRunning) => {
            println!("TinyPaw is already running. Use `tinypaw toggle` or the tray icon.");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("tinypaw: cannot open command endpoint: {e}");
            return ExitCode::FAILURE;
        }
    };

    #[cfg(windows)]
    platform::windows::release_own_console();
    platform::init_bridge();
    platform::ipc::serve(listener);

    match app::run(registry) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tinypaw: {e}");
            ExitCode::FAILURE
        }
    }
}
