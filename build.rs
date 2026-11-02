use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=assets/riffle.rc");
    println!("cargo:rerun-if-changed=assets/riffle.ico");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    // The version in the exe's file details always comes from Cargo.toml. Generated elsewhere, so the icon needs its full path.
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let root = env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
    let rc = fs::read_to_string("assets/riffle.rc")
        .unwrap()
        .replace("__ICON__", &format!("{root}/assets/riffle.ico"))
        .replace("__VERSION_NUMBERS__", &format!("{},0", version.replace('.', ",")))
        .replace("__VERSION__", &version);
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("riffle.rc");
    fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE).manifest_required().unwrap();
}
