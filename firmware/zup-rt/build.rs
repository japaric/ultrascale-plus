use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();

    if target.starts_with("armv") {
        fs::copy(
            format!("bin/{}.a", target),
            out_dir.join("libzup-rt.a"),
        )
        .unwrap();
        println!("cargo:rustc-link-lib=static=zup-rt");
    }

    // Put the linker script somewhere the linker can find it
    fs::write(out_dir.join("link.x"), &include_bytes!("link.x")[..]).unwrap();
    println!("cargo:rustc-link-search={}", out_dir.display());

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=link.x");
}
