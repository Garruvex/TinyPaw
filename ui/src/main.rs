//! Opens the UI gallery (`cargo run`), or the sample page in the Settings shell
//! (`cargo run -- shell`). `-- check` builds every window and exits, so it runs
//! without a display.

slint::include_modules!();

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("shell") => ShellDemo::new().expect("shell").run().expect("run"),
        Some("check") => {
            let _windows = (
                ShellDemo::new().expect("shell"),
                Gallery::new().expect("gallery"),
                StudioTabDemo::new().expect("studio tab"),
            );
            println!("ui ok");
        }
        _ => Gallery::new().expect("gallery").run().expect("run"),
    }
}
