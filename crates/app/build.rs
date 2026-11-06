fn main() {
    let cfg = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/settings.slint", cfg).expect("compiling settings UI");
    println!("cargo:rerun-if-changed=res");
    let host_windows = std::env::var("HOST").is_ok_and(|h| h.contains("windows"));
    let target_msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|e| e == "msvc");
    if host_windows && target_msvc {
        embed_resource::compile("res/vizz.rc", embed_resource::NONE).manifest_optional().unwrap();
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("res/vizz.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
    }
}
