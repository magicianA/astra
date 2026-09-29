use std::{env, path::PathBuf, process::Command};
mod hash;

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let spice = env::var_os("CSPICE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("third_party/cspice"));
    println!("cargo:rerun-if-env-changed=CSPICE_ROOT");
    println!("cargo:rerun-if-env-changed=ERFA_LIB_DIR");
    println!(
        "cargo:rustc-link-search=native={}",
        spice.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=cspice");
    if let Some(dir) = env::var_os("ERFA_LIB_DIR") {
        println!(
            "cargo:rustc-link-search=native={}",
            PathBuf::from(dir).display()
        );
    } else if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
        println!("cargo:rustc-link-search=native=/usr/local/lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,/opt/homebrew/lib");
        println!("cargo:rustc-link-arg=-Wl,-headerpad_max_install_names");
    }
    println!("cargo:rustc-link-lib=erfa");
    if env::var_os("CARGO_FEATURE_GPU").is_none() {
        return;
    }
    let mut hashes = String::new();
    for file in [
        "shaders/atmosphere_filter.glsl",
        "shaders/atmosphere_model.glsl",
        "shaders/atmosphere_spectrum.glsl",
        "shaders/atmosphere.comp",
        "vendor/bruneton/definitions.glsl",
        "vendor/bruneton/functions.glsl",
    ] {
        hashes.push_str(&hash::sha256(&std::fs::read(root.join(file)).unwrap()));
    }
    println!(
        "cargo:rustc-env=ASTRA_ATMOSPHERE_ID={}",
        hash::sha256(hashes.as_bytes())
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let validator = env::var_os("GLSLANG_VALIDATOR").unwrap_or_else(|| {
        if PathBuf::from("/opt/homebrew/bin/glslangValidator").exists() {
            "/opt/homebrew/bin/glslangValidator".into()
        } else {
            "glslangValidator".into()
        }
    });
    println!("cargo:rerun-if-env-changed=GLSLANG_VALIDATOR");
    println!("cargo:rerun-if-changed=shaders");
    println!("cargo:rerun-if-changed=vendor/bruneton");
    for name in [
        "sky.vert",
        "sky.frag",
        "stars.vert",
        "stars.frag",
        "tonemap.frag",
        "atmosphere.comp",
        "exposure.comp",
    ] {
        let status = Command::new(&validator)
            .args(["-V", "--target-env", "vulkan1.2"])
            .arg(root.join("shaders").join(name))
            .arg("-o")
            .arg(out.join(format!("{name}.spv")))
            .status()
            .expect("Install glslangValidator or set GLSLANG_VALIDATOR");
        assert!(status.success(), "shader compilation failed: {name}");
    }
}
