//! Compiles `cpp/shim.cpp` against the vendored, header-only RTNeural
//! (`modules/rtneural`, Eigen backend, same configuration as the C++ plugin's
//! CMakeLists.txt) and the plugin's own `src/model_variant.hpp`.
//!
//! Environment:
//! * `RTNEURAL_DIR` – override the RTNeural checkout (default: `<repo>/modules/rtneural`).
//!   Must contain `RTNeural/RTNeural.h` and `modules/{Eigen,json}`.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir.join("../..");

    println!("cargo:rerun-if-env-changed=RTNEURAL_DIR");
    let rtneural_dir = env::var_os("RTNEURAL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("modules/rtneural"));

    let header = rtneural_dir.join("RTNeural/RTNeural.h");
    let eigen = rtneural_dir.join("modules/Eigen");
    if !header.is_file() || !eigen.join("Eigen/Dense").is_file() {
        panic!(
            "RTNeural not found at {} (missing RTNeural/RTNeural.h or modules/Eigen).\n\
             Run `git submodule update --init --recursive modules/rtneural` \
             or set RTNEURAL_DIR to an RTNeural checkout.",
            rtneural_dir.display()
        );
    }

    let plugin_src = repo_root.join("src");
    let cpp_dir = manifest_dir.join("cpp");
    let variant_header = plugin_src.join("model_variant.hpp");

    // shim.cpp holds the C ABI; parts/*.cpp instantiate the heavy per-model
    // templates (9 models each) so they compile in parallel.
    let mut sources = vec![cpp_dir.join("shim.cpp")];
    sources.extend((0..6).map(|i| cpp_dir.join(format!("parts/part{i}.cpp"))));

    rerun(&cpp_dir);
    rerun(&variant_header);
    rerun(&rtneural_dir.join("RTNeural"));

    let target = env::var("TARGET").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .files(&sources)
        .include(&rtneural_dir)
        .include(&eigen)
        .include(&plugin_src)
        // Same backend as the C++ plugin (RTNEURAL_XSIMD=OFF -> Eigen, no AVX).
        .define("RTNEURAL_USE_EIGEN", "1")
        .define("RTNEURAL_DEFAULT_ALIGNMENT", "16")
        // Eigen is unusably slow unoptimised; the audio path must stay real-time
        // even in debug builds of the Rust code.
        .opt_level(2)
        .warnings(false);

    if target.contains("msvc") {
        // 54 template instantiations of Eigen + nlohmann::json exceed the default
        // section limit; /EHsc is needed for the try/catch in the shim.
        build.flag("/bigobj").flag("/EHsc").flag("/Zc:__cplusplus");
    }
    if target_arch == "wasm32" {
        // Mirrors CMakeLists.txt (EMSCRIPTEN): no Eigen vectorisation.
        build.define("EIGEN_DONT_VECTORIZE", "1");
    }

    build.compile("aidadx_rtneural_shim");
}

fn rerun(p: &Path) {
    println!("cargo:rerun-if-changed={}", p.display());
}
