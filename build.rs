fn main() {
    println!("cargo:rerun-if-changed=assets/riffle.rc");
    println!("cargo:rerun-if-changed=assets/riffle.ico");
    embed_resource::compile("assets/riffle.rc", embed_resource::NONE).manifest_required().unwrap();
}
