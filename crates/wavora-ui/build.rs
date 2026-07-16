//! Relay Optics Iris development rpaths to this crate's test and example
//! binaries. Installed system libraries need no entry; custom prefixes and
//! sibling Meson build trees are published by Iris as `links` metadata.

fn main() {
    println!("cargo:rerun-if-env-changed=DEP_IRIS_RS_RPATHS");
    let Ok(rpaths) = std::env::var("DEP_IRIS_RS_RPATHS") else {
        return;
    };
    let dirs: Vec<&str> = rpaths.split(';').filter(|path| !path.is_empty()).collect();
    if dirs.is_empty() {
        return;
    }

    println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags");
    for dir in dirs {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
    }
}
