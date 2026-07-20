use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DEP_IRIS_RS_RPATHS");

    // Relay the rpaths that `iris` (the safe wrapper crate) publishes via
    // its `links = "iris_rs"` metadata. These point at the meson build
    // tree (../optics/build) when the *_BUILD_DIR env vars or an adjacent
    // optics checkout are used, so `cargo run` finds libiris/liblens/libflux
    // without an install step. Unused for static-link builds.
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
    // safe-wrapper rpath metadata yet: detect a sibling optics checkout.
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
