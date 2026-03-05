use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Only re-run build.rs when the C wrapper or build script itself changes.
    // Rust-only changes won't trigger a rebuild of the C++ side.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/wrapper.c");
    println!("cargo:rerun-if-changed=src/wrapper.h");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ort_src = manifest_dir.join("onnxruntime");

    if !ort_src.join("cmake/CMakeLists.txt").exists() {
        panic!(
            "ONNX Runtime source not found at {}. \
             Run: git submodule update --init --recursive",
            ort_src.display()
        );
    }

    let build_dir = out_dir.join("onnxruntime-build");
    let install_dir = out_dir.join("onnxruntime-install");

    // Reuse cached build if available
    if has_ort_libs(&install_dir.join("lib")) {
        println!("cargo:warning=Using cached ONNX Runtime build");
    } else {
        std::fs::create_dir_all(&build_dir).unwrap();
        std::fs::create_dir_all(&install_dir).unwrap();

        // Patch Eigen hash in deps.txt if needed (GitLab regenerated the archive,
        // changing its SHA1). Without this, fresh checkouts fail on cmake fetch.
        patch_eigen_hash(&ort_src);

        if target_os == "macos" && target_arch == "aarch64" {
            build_macos_arm64(&ort_src, &build_dir, &install_dir);
        } else if target_os == "macos" {
            build_macos_x86(&ort_src, &build_dir, &install_dir);
        } else {
            build_cpu_only(&ort_src, &build_dir, &install_dir);
        }
    }

    compile_wrapper(&manifest_dir, &install_dir);
    emit_link_directives(&install_dir, &target_os);
}

fn has_ort_libs(lib_dir: &Path) -> bool {
    if !lib_dir.exists() {
        return false;
    }
    // Check for session lib + re2 (both must be present)
    lib_dir.join("libonnxruntime_session.a").exists() && lib_dir.join("libre2.a").exists()
}

/// Patch Eigen archive hash in ORT's cmake/deps.txt.
///
/// GitLab regenerated the Eigen archive, changing its SHA1.
/// This patch applies to ONNX Runtime v1.21.x (pinned submodule).
/// If the ORT submodule is updated and neither hash is found,
/// a cargo warning is emitted so the stale patch is noticed.
fn patch_eigen_hash(ort_src: &Path) {
    let deps_file = ort_src.join("cmake/deps.txt");
    if !deps_file.exists() {
        return;
    }
    let content = std::fs::read_to_string(&deps_file).unwrap();
    let old_hash = "5ea4d05e62d7f954a46b3213f9b2535bdd866803";
    let new_hash = "51982be81bbe52572b54180454df11a3ece9a934";
    if content.contains(old_hash) {
        let patched = content.replace(old_hash, new_hash);
        std::fs::write(&deps_file, patched).unwrap();
        println!("cargo:warning=Patched Eigen hash in deps.txt");
    } else if !content.contains(new_hash) {
        println!("cargo:warning=Eigen hash patch may be stale: neither old nor new hash found in cmake/deps.txt");
    }
}

fn compile_wrapper(manifest_dir: &Path, install_dir: &Path) {
    let include_dir = install_dir.join("include");
    let wrapper_src = manifest_dir.join("src").join("wrapper.c");

    cc::Build::new()
        .file(&wrapper_src)
        .include(&include_dir)
        .warnings(false)
        .compile("ort_wrapper");
}

fn run_ort_build(ort_src: &Path, build_dir: &Path, args: &[&str]) {
    let status = Command::new(ort_src.join("build.sh"))
        .current_dir(ort_src)
        .args(args)
        .status()
        .expect("Failed to execute build.sh");

    if !status.success() {
        panic!("ONNX Runtime build failed");
    }

    // re2 is fetched by CMake but not built as part of the default target.
    // Build it explicitly so we can collect libre2.a.
    let cmake_build_dir = build_dir.join("Release");
    let re2_status = Command::new("cmake")
        .args([
            "--build",
            &cmake_build_dir.to_string_lossy(),
            "--target",
            "re2",
            "--config",
            "Release",
        ])
        .status()
        .expect("Failed to build re2 target");
    if !re2_status.success() {
        println!("cargo:warning=re2 target build failed, re2 may be missing from output");
    }
}

fn build_macos_arm64(ort_src: &Path, build_dir: &Path, install_dir: &Path) {
    println!(
        "cargo:warning=Building ONNX Runtime with CoreML for macOS arm64 (~2 min on Apple Silicon)"
    );

    let build_dir_str = build_dir.to_string_lossy().to_string();
    run_ort_build(
        ort_src,
        build_dir,
        &[
            "--config",
            "Release",
            "--parallel",
            "--use_coreml",
            "--skip_tests",
            "--build_dir",
            &build_dir_str,
            "--cmake_extra_defines",
            "CMAKE_OSX_ARCHITECTURES=arm64",
            "--cmake_extra_defines",
            "CMAKE_OSX_DEPLOYMENT_TARGET=14.0",
            "--cmake_extra_defines",
            "onnxruntime_BUILD_SHARED_LIB=OFF",
            "--cmake_extra_defines",
            "onnxruntime_BUILD_UNIT_TESTS=OFF",
            "--cmake_extra_defines",
            "FETCHCONTENT_TRY_FIND_PACKAGE_MODE=NEVER",
            "--cmake_extra_defines",
            "CMAKE_POLICY_VERSION_MINIMUM=3.5",
        ],
    );

    collect_build_output(build_dir, ort_src, install_dir);
}

fn build_macos_x86(ort_src: &Path, build_dir: &Path, install_dir: &Path) {
    println!("cargo:warning=Building ONNX Runtime CPU-only for macOS x86_64");
    build_cpu_only(ort_src, build_dir, install_dir);
}

fn build_cpu_only(ort_src: &Path, build_dir: &Path, install_dir: &Path) {
    let build_dir_str = build_dir.to_string_lossy().to_string();
    run_ort_build(
        ort_src,
        build_dir,
        &[
            "--config",
            "Release",
            "--parallel",
            "--skip_tests",
            "--build_dir",
            &build_dir_str,
            "--cmake_extra_defines",
            "onnxruntime_BUILD_SHARED_LIB=OFF",
            "--cmake_extra_defines",
            "onnxruntime_BUILD_UNIT_TESTS=OFF",
            "--cmake_extra_defines",
            "FETCHCONTENT_TRY_FIND_PACKAGE_MODE=NEVER",
            "--cmake_extra_defines",
            "CMAKE_POLICY_VERSION_MINIMUM=3.5",
        ],
    );

    collect_build_output(build_dir, ort_src, install_dir);
}

fn collect_build_output(build_dir: &Path, ort_src: &Path, install_dir: &Path) {
    let lib_dir = install_dir.join("lib");
    let include_dir = install_dir.join("include");
    std::fs::create_dir_all(&lib_dir).unwrap();
    std::fs::create_dir_all(&include_dir).unwrap();

    // Copy all .a files from the build tree
    let mut count = 0;
    for entry in walkdir(build_dir) {
        let name = entry
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if name.ends_with(".a") {
            let dest = lib_dir.join(&name);
            if !dest.exists() {
                if let Err(e) = std::fs::copy(&entry, &dest) {
                    println!(
                        "cargo:warning=Failed to copy {}: {}",
                        entry.display(),
                        e
                    );
                } else {
                    count += 1;
                }
            }
        }
    }
    println!("cargo:warning=Collected {} static libraries", count);

    // Copy headers
    let headers = [
        "include/onnxruntime/core/session/onnxruntime_c_api.h",
        "include/onnxruntime/core/providers/coreml/coreml_provider_factory.h",
    ];
    for header in &headers {
        let src = ort_src.join(header);
        if src.exists() {
            let filename = src.file_name().unwrap();
            if let Err(e) = std::fs::copy(&src, include_dir.join(filename)) {
                println!(
                    "cargo:warning=Failed to copy header {}: {}",
                    src.display(),
                    e
                );
            }
        }
    }
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(walkdir(&path));
            } else {
                results.push(path);
            }
        }
    }
    results
}

fn emit_link_directives(install_dir: &Path, target_os: &str) {
    let lib_dir = install_dir.join("lib");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Collect all static library names from the lib directory
    let mut ort_libs = Vec::new();
    let mut re2_libs = Vec::new();
    let mut abseil_libs = Vec::new();
    let mut other_libs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(lib_name) = name.strip_prefix("lib").and_then(|n| n.strip_suffix(".a")) {
                let lib_name = lib_name.to_string();
                if lib_name.starts_with("onnxruntime") || lib_name == "ort_wrapper" {
                    ort_libs.push(lib_name);
                } else if lib_name.starts_with("re2") {
                    re2_libs.push(lib_name);
                } else if lib_name.starts_with("absl_") {
                    abseil_libs.push(lib_name);
                } else {
                    other_libs.push(lib_name);
                }
            }
        }
    }

    // Sort within groups for determinism
    ort_libs.sort();
    re2_libs.sort();
    abseil_libs.sort();
    other_libs.sort();

    // Link order matters for static libraries: dependents before dependencies.
    // ORT libs → other libs (protobuf, onnx, etc.) → re2 → abseil
    for lib in &ort_libs {
        println!("cargo:rustc-link-lib=static={}", lib);
    }
    for lib in &other_libs {
        println!("cargo:rustc-link-lib=static={}", lib);
    }
    for lib in &re2_libs {
        println!("cargo:rustc-link-lib=static={}", lib);
    }
    for lib in &abseil_libs {
        println!("cargo:rustc-link-lib=static={}", lib);
    }

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreML");
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=MetalPerformanceShaders");
        println!("cargo:rustc-link-lib=framework=Accelerate");
        println!("cargo:rustc-link-lib=c++");
    }

    // Export for downstream crates
    println!("cargo:LIB_DIR={}", lib_dir.display());
    println!(
        "cargo:INCLUDE_DIR={}",
        install_dir.join("include").display()
    );
}
