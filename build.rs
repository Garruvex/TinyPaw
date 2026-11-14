fn main() {
    for path in ["ui/app.slint", "ui/kit.slint", "ui/tokens.slint", "ui/pages", "ui/templates"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("compile the Slint UI");
    println!("cargo:rerun-if-changed=assets/app/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/app/icon.ico")
            .set("ProductName", "TinyPaw")
            .set("FileDescription", "TinyPaw")
            .compile()
            .expect("could not embed the TinyPaw app icon");
    }
}
