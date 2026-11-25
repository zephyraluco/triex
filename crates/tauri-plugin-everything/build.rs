fn main() {
    tauri_plugin::Builder::new(&["status", "search"]).build();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=sdk");
        cc::Build::new()
            .file("sdk/src/Everything.c")
            .define("UNICODE", None)
            .define("_UNICODE", None)
            .warnings(false)
            .compile("everything_sdk");
        println!("cargo:rustc-link-lib=user32");
    }
}
