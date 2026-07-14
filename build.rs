use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../../lib/wavora");

    let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let Some(parent) = PathBuf::from(manifest).parent().map(PathBuf::from) else {
        return;
    };
    let build = parent.join("optics/build/libs");
    for component in ["iris", "lens", "flux"] {
        let dir = build.join(component);
        if dir.exists() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
        }
    }
}
