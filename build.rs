fn main() {
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
