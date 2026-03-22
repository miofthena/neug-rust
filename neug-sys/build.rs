use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Check if we are in the local workspace with the submodule
    let local_neug_dir = manifest_dir.parent().unwrap().join("neug-cpp");
    let build_neug_dir = out_dir.join("neug-cpp-src");

    let neug_dir = if local_neug_dir.join("CMakeLists.txt").exists() {
        println!(
            "cargo:warning=Using local neug-cpp submodule at {}",
            local_neug_dir.display()
        );

        // We sync local repo to OUT_DIR so we can patch it without modifying the git submodule working tree
        let status = Command::new("rsync")
            .args([
                "-a",
                "--exclude=.git",
                "--exclude=build",
                &format!("{}/", local_neug_dir.display()),
                &format!("{}/", build_neug_dir.display()),
            ])
            .status()
            .expect("Failed to rsync local neug-cpp to OUT_DIR");

        if !status.success() {
            panic!("rsync failed");
        }

        // Apply our custom patch
        let patch_path = manifest_dir.join("patches/0001-fix-dml-buffer-overflows.patch");
        if patch_path.exists() {
            // Check if already patched to avoid patch failure
            let check_status = Command::new("patch")
                .current_dir(&build_neug_dir)
                .args(["-p1", "-R", "--dry-run", "-i", patch_path.to_str().unwrap()])
                .status()
                .unwrap_or_else(|_| panic!("Failed to run patch check"));

            if !check_status.success() {
                println!("cargo:warning=Applying patch 0001-fix-dml-buffer-overflows.patch");
                let patch_status = Command::new("patch")
                    .current_dir(&build_neug_dir)
                    .args(["-p1", "-N", "-i", patch_path.to_str().unwrap()])
                    .status()
                    .expect("Failed to apply patch");
                if !patch_status.success() {
                    println!("cargo:warning=Patch might have already been applied or failed.");
                }
            }
        }
        build_neug_dir
    } else {
        // We are likely being built from crates.io. Download it into OUT_DIR.
        let download_dir = out_dir.join("neug-cpp");
        if !download_dir.exists() {
            let git_ref = env::var("NEUG_GIT_REF").unwrap_or_else(|_| "main".to_string());
            println!(
                "cargo:warning=Downloading alibaba/neug {} into {}...",
                git_ref,
                download_dir.display()
            );

            let status = Command::new("git")
                .args([
                    "clone",
                    "--recursive",
                    "--depth",
                    "1",
                    "--branch",
                    &git_ref,
                    "https://github.com/alibaba/neug.git",
                    download_dir.to_str().unwrap(),
                ])
                .status()
                .expect("Failed to run git clone");

            if !status.success() {
                panic!("Failed to clone neug repository");
            }

            // Patch Arrow URL just like we did manually, because of Aliyun mirror timeout
            let arrow_cmake = download_dir.join("cmake/BuildArrowAsThirdParty.cmake");
            let cmake_content = std::fs::read_to_string(&arrow_cmake).unwrap();
            let new_content = cmake_content.replace(
                "https://graphscope.oss-cn-beijing.aliyuncs.com/apache-arrow-${ARROW_VERSION}.tar.gz",
                "https://github.com/apache/arrow/archive/refs/tags/apache-arrow-${ARROW_VERSION}.tar.gz"
            );
            std::fs::write(&arrow_cmake, new_content).unwrap();
        }
        download_dir
    };

    // Build the C++ library using CMake
    let mut config = cmake::Config::new(&neug_dir);
    config
        .define("BUILD_TESTS", "OFF")
        .define("BUILD_EXAMPLES", "OFF")
        .define("BUILD_HTTP_SERVER", "OFF");

    // Automatically use sccache or ccache if available to speed up C++ builds
    if Command::new("sccache").arg("--version").output().is_ok() {
        config.define("CMAKE_C_COMPILER_LAUNCHER", "sccache");
        config.define("CMAKE_CXX_COMPILER_LAUNCHER", "sccache");
    } else if Command::new("ccache").arg("--version").output().is_ok() {
        config.define("CMAKE_C_COMPILER_LAUNCHER", "ccache");
        config.define("CMAKE_CXX_COMPILER_LAUNCHER", "ccache");
    }

    // Use parallel building if configured, otherwise cmake crate defaults are used
    if let Ok(jobs) = env::var("ZVEC_BUILD_PARALLEL") {
        unsafe { env::set_var("CMAKE_BUILD_PARALLEL_LEVEL", jobs) };
    }

    let dst = config.build();

    // Link against the built `neug` library
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-search=native={}/lib64", dst.display());
    println!("cargo:rustc-link-lib=dylib=neug");

    // Compile the C API wrapper
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++20")
        .file("c_api.cpp")
        .include(format!("{}/include", neug_dir.display()))
        .include(format!("{}/include", dst.display()));

    if Command::new("sccache").arg("--version").output().is_ok() {
        build.compiler("sccache c++");
    } else if Command::new("ccache").arg("--version").output().is_ok() {
        build.compiler("ccache c++");
    }

    build.compile("neug_c_api");

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=c_api.h");
    println!("cargo:rerun-if-changed=c_api.cpp");

    if neug_dir == local_neug_dir {
        println!("cargo:rerun-if-changed=../neug-cpp/CMakeLists.txt");
    }

    // Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header("c_api.h")
        .clang_arg("-xc++")
        .clang_arg("-std=c++20")
        .clang_arg(format!("-I{}/include", dst.display()))
        // We also need the source include dirs
        .clang_arg(format!("-I{}/include", neug_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Only generate bindings for neug
        .allowlist_type("neug_.*")
        .allowlist_function("neug_.*")
        .allowlist_var("neug_.*")
        .layout_tests(false)
        .generate()
        .expect("Unable to generate bindings");
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
