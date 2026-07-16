use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DEP_IRIS_RS_RPATHS");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../../lib/wavora");

    if let Ok(rpaths) = std::env::var("DEP_IRIS_RS_RPATHS") {
        let dirs = rpaths
            .split(';')
            .filter(|path| !path.is_empty())
            .collect::<Vec<_>>();
        if !dirs.is_empty() {
            println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags");
            for dir in dirs {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
            }
            return;
        }
    }

    // Compatibility fallback for older Iris checkouts that do not publish
    // safe-wrapper rpath metadata yet.
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
