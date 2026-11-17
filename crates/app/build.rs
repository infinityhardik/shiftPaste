fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Native-looking standard widgets: Cupertino on macOS, Fluent elsewhere.
    let style = if target_os == "macos" {
        "cupertino"
    } else {
        "fluent"
    };
    let config = slint_build::CompilerConfiguration::new().with_style(style.into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI failed to compile");

    // Windows: app icon, version info and manifest (DPI awareness, modern
    // controls) inside the .exe. Needs a resource compiler: the Windows SDK's
    // on Windows, or MinGW's windres when cross-building for windows-gnu.
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && (cfg!(windows) || target_env == "gnu") {
        println!("cargo:rerun-if-changed=assets/shiftpaste.ico");
        println!("cargo:rerun-if-changed=assets/shiftpaste.manifest");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/shiftpaste.ico")
            .set_manifest_file("assets/shiftpaste.manifest")
            .set("ProductName", "Shift Paste")
            .set("FileDescription", "Shift Paste")
            .set("CompanyName", "InfinityHardik")
            .set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            println!(
                "cargo:warning=Couldn't embed the app icon ({e}); is the Windows SDK (or MinGW windres) installed?"
            );
        }
    }
}
