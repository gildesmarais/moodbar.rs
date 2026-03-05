use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_file = crate_dir.join("include/moodbar_native_ffi.h");

    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml")).unwrap_or_default();
    let builder = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config);

    if let Ok(bindings) = builder.generate() {
        let _ = bindings.write_to_file(out_file);
    }
}
