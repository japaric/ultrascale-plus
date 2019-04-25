use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let target = env::var("TARGET")?;

    if target.starts_with("armv") {
        fs::copy(format!("bin/{}.a", target), out_dir.join("libzup-rt.a"))?;
        println!("cargo:rustc-link-lib=static=zup-rt");
    }

    // Put the linker script somewhere the linker can find it
    for file in &["common.x", "link.x", "core0.x", "core1.x"] {
        fs::copy(manifest_dir.join(file), out_dir.join(file))?;
    }
    println!("cargo:rustc-link-search={}", out_dir.display());

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=link.x");

    Ok(())
}
