use std::path::{Path, PathBuf};
use std::process::Command;

fn find_fxc() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FXC") {
        return Some(p.into());
    }
    let pf86 = std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    let bin = Path::new(&pf86).join(r"Windows Kits\10\bin");
    let mut vers: Vec<PathBuf> = std::fs::read_dir(&bin).ok()?.flatten().map(|e| e.path()).collect();
    vers.sort();
    vers.into_iter().rev().map(|v| v.join(r"x64\fxc.exe")).find(|p| p.exists())
}

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let fxc = find_fxc().expect("fxc.exe not found: install the Windows SDK or set FXC");
    let shaders = [
        ("fullscreen", "vs_4_0"),
        ("ring", "ps_4_0"),
        ("present", "ps_4_0"),
        ("flux", "ps_4_0"),
        ("flux_post", "ps_4_0"),
        ("cosmos_post", "ps_4_0"),
        ("warp", "ps_4_0"),
        ("pop", "ps_4_0"),
        ("pop2", "ps_4_0"),
        ("geo", "ps_4_0"),
        ("sketch", "ps_4_0"),
        ("sketch_post", "ps_4_0"),
    ];
    println!("cargo:rerun-if-changed=shaders");
    for (name, profile) in shaders {
        let src = format!("shaders/{name}.hlsl");
        let dst = out.join(format!("{name}.cso"));
        let o = Command::new(&fxc)
            .args(["/nologo", "/O3", "/T", profile, "/E", "main", "/Fo"])
            .arg(&dst)
            .arg(&src)
            .output()
            .expect("run fxc");
        if !o.status.success() {
            panic!("fxc {src}:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        }
    }
}
