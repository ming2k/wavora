use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let Some(projects) = PathBuf::from(manifest)
        .ancestors()
        .nth(3)
        .map(PathBuf::from)
    else {
        return;
    };
    let build = projects.join("optics/build/libs");
    for component in ["iris", "lens", "flux"] {
        let directory = build.join(component);
        if directory.exists() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", directory.display());
        }
    }
}
