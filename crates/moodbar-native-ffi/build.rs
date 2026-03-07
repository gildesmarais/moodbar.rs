use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_file = crate_dir.join("include/moodbar_native_ffi.h");

    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml")).unwrap_or_default();
    let builder = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config);

    let bindings = builder
        .generate()
        .expect("cbindgen failed to generate bindings");
    assert!(
        bindings.write_to_file(&out_file) || out_file.is_file(),
        "cbindgen failed to write header to {}",
        out_file.display()
    );
}
