use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn add_link_search_path(path: impl AsRef<Path>, seen: &mut BTreeSet<PathBuf>) {
    let path = path.as_ref();
    if path.is_dir() && seen.insert(path.to_path_buf()) {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
}

fn matches_static_library(path: &Path, name: &str) -> bool {
    if !path.is_file() {
        return false;
    }

    let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
        return false;
    };

    file_name == format!("lib{name}.a") || file_name == format!("{name}.lib")
}

fn collect_library_dirs(
    root: &Path,
    library_names: &[&str],
    max_depth: usize,
    search_paths: &mut BTreeSet<PathBuf>,
) {
    if !root.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    let mut subdirs = Vec::new();
    let mut contains_library = false;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && library_names
                .iter()
                .any(|name| matches_static_library(&path, name))
        {
            contains_library = true;
        } else if max_depth > 0 && path.is_dir() {
            let Some(dir_name) = path.file_name().and_then(|dir_name| dir_name.to_str()) else {
                continue;
            };
            if matches!(dir_name, "CMakeFiles" | "Testing" | "include") {
                continue;
            }
            subdirs.push(path);
        }
    }

    if contains_library {
        add_link_search_path(root, search_paths);
    }

    if max_depth == 0 {
        return;
    }

    for subdir in subdirs {
        collect_library_dirs(&subdir, library_names, max_depth - 1, search_paths);
    }
}

fn select_static_library_name<'a>(
    search_paths: &BTreeSet<PathBuf>,
    candidates: &'a [&'a str],
) -> Option<&'a str> {
    candidates.iter().copied().find(|candidate| {
        search_paths.iter().any(|search_path| {
            matches_static_library(&search_path.join(format!("lib{candidate}.a")), candidate)
                || matches_static_library(&search_path.join(format!("{candidate}.lib")), candidate)
        })
    })
}

fn emit_preferred_link(
    search_paths: &BTreeSet<PathBuf>,
    static_candidates: &[&str],
    dynamic_fallback: &str,
) {
    if let Some(lib) = select_static_library_name(search_paths, static_candidates) {
        println!("cargo:rustc-link-lib=static={}", lib);
    } else {
        println!("cargo:rustc-link-lib=dylib={}", dynamic_fallback);
    }
}

fn rewrite_arrow_download_url(neug_dir: &Path) {
    let arrow_cmake = neug_dir.join("cmake/BuildArrowAsThirdParty.cmake");
    if !arrow_cmake.exists() {
        return;
    }

    let cmake_content = std::fs::read_to_string(&arrow_cmake).unwrap();
    let new_content = cmake_content.replace(
        "https://graphscope.oss-cn-beijing.aliyuncs.com/apache-arrow-${ARROW_VERSION}.tar.gz",
        "https://github.com/apache/arrow/archive/refs/tags/apache-arrow-${ARROW_VERSION}.tar.gz",
    );

    if new_content != cmake_content {
        std::fs::write(&arrow_cmake, new_content).unwrap();
    }
}

fn apply_patches(source_dir: &Path, patches_dir: &Path) {
    if !patches_dir.exists() {
        return;
    }

    let mut patches: Vec<_> = std::fs::read_dir(patches_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "patch"))
        .collect();
    patches.sort();

    for patch_path in patches {
        let patch_name = patch_path.file_name().unwrap().to_string_lossy();

        // `patch -R --dry-run` is unreliable here because it exits successfully even when the
        // forward patch has not been applied yet. A forward dry-run gives us the signal we need.
        let dry_run_status = Command::new("patch")
            .current_dir(source_dir)
            .args([
                "--forward",
                "--dry-run",
                "-p1",
                "-i",
                patch_path.to_str().unwrap(),
            ])
            .status()
            .unwrap_or_else(|_| panic!("Failed to run patch dry-run"));

        if !dry_run_status.success() {
            continue;
        }

        println!("cargo:warning=Applying patch {}", patch_name);
        let patch_status = Command::new("patch")
            .current_dir(source_dir)
            .args(["-p1", "-N", "-i", patch_path.to_str().unwrap()])
            .status()
            .expect("Failed to apply patch");
        if !patch_status.success() {
            println!(
                "cargo:warning=Patch {} might have already been applied or failed.",
                patch_name
            );
        }
    }
}

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

        // The Aliyun mirror is flaky outside Alibaba's network, so we normalize the Arrow source URL here too.
        rewrite_arrow_download_url(&build_neug_dir);

        // Apply our custom patches
        let patches_dir = manifest_dir.join("patches");
        apply_patches(&build_neug_dir, &patches_dir);
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

            // The published crate does not include our workspace checkout, so we rewrite the Arrow URL after clone.
            rewrite_arrow_download_url(&download_dir);

            // Apply our custom patches
            let patches_dir = manifest_dir.join("patches");
            apply_patches(&download_dir, &patches_dir);
        }
        download_dir
    };

    // Build the C++ library using CMake
    let mut config = cmake::Config::new(&neug_dir);
    config
        .define("BUILD_TESTS", "OFF")
        .define("BUILD_EXAMPLES", "OFF")
        .define("BUILD_HTTP_SERVER", "OFF")
        .define("ENABLE_WERROR", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .cxxflag("-Wno-unused-parameter")
        .cxxflag("-Wno-deprecated-copy")
        .cxxflag("-Wno-ignored-qualifiers")
        .cxxflag("-Wno-dev");

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

    // Compile and link the C API wrapper first
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++20")
        .file("c_api.cpp")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-deprecated-copy")
        .flag("-Wno-ignored-qualifiers")
        .include(format!("{}/include", neug_dir.display()))
        .include(format!("{}/include", dst.display()));

    if Command::new("sccache").arg("--version").output().is_ok() {
        build.compiler("sccache c++");
    } else if Command::new("ccache").arg("--version").output().is_ok() {
        build.compiler("ccache c++");
    }

    build.compile("neug_c_api");

    // Link against the built libraries in the correct order:
    // IMPORTANT: Dependencies must come AFTER the libraries that use them.
    // 1. neug_c_api (wrapper, uses neug)
    // 2. neug (core engine, uses arrow, glog, etc.)
    // 3. dependencies (glog, arrow, protobuf, absl, etc.)
    // 4. system libs (ssl, crypto, stdc++, pthread, dl)

    let absl_libs = [
        "absl_log_entry",
        "absl_log_flags",
        "absl_log_globals",
        "absl_log_initialize",
        "absl_log_internal_check_op",
        "absl_log_internal_conditions",
        "absl_log_internal_fnmatch",
        "absl_log_internal_format",
        "absl_log_internal_globals",
        "absl_log_internal_log_sink_set",
        "absl_log_internal_message",
        "absl_log_internal_nullguard",
        "absl_log_internal_proto",
        "absl_log_sink",
        "absl_log_severity",
        "absl_status",
        "absl_statusor",
        "absl_str_format_internal",
        "absl_synchronization",
        "absl_time",
        "absl_time_zone",
        "absl_int128",
        "absl_throw_delegate",
        "absl_raw_logging_internal",
        "absl_base",
        "absl_kernel_timeout_internal",
        "absl_spinlock_wait",
        "absl_malloc_internal",
        "absl_hash",
        "absl_hashtablez_sampler",
        "absl_raw_hash_set",
        "absl_city",
        "absl_low_level_hash",
        "absl_cord",
        "absl_cord_internal",
        "absl_cordz_functions",
        "absl_cordz_info",
        "absl_cordz_handle",
        "absl_crc32c",
        "absl_crc_cord_state",
        "absl_crc_cpu_detect",
        "absl_crc_internal",
        "absl_debugging_internal",
        "absl_demangle_internal",
        "absl_examine_stack",
        "absl_stacktrace",
        "absl_strings",
        "absl_string_view",
        "absl_strings_internal",
        "absl_strerror",
        "absl_symbolize",
        "absl_vlog_config_internal",
        "absl_graphcycles_internal",
    ];
    let gflags_candidates = [
        "gflags_nothreads_debug",
        "gflags_nothreads",
        "gflags_debug",
        "gflags",
    ];
    let arrow_candidates = ["arrow_static", "arrow"];
    let protobuf_candidates = ["protobufd", "protobuf"];
    let protobuf_lite_candidates = ["protobuf-lited", "protobuf-lite"];
    let snappy_candidates = ["snappy"];
    let zstd_candidates = ["zstd", "zstd_static"];

    let mut static_libs = vec![
        "neug_c_api",
        "neug",
        "glog",
        "yaml-cpp",
        "re2",
        "utf8proc",
        "antlr4_runtime",
        "antlr4_cypher",
    ];
    static_libs.extend_from_slice(&gflags_candidates);
    static_libs.extend_from_slice(&arrow_candidates);
    static_libs.extend_from_slice(&protobuf_candidates);
    static_libs.extend_from_slice(&protobuf_lite_candidates);
    static_libs.extend_from_slice(&snappy_candidates);
    static_libs.extend_from_slice(&zstd_candidates);
    let mut search_paths = BTreeSet::new();

    for path in [
        dst.clone(),
        dst.join("lib"),
        dst.join("lib64"),
        out_dir.join("build"),
        out_dir.join("build/lib"),
        out_dir.join("build/lib64"),
    ] {
        add_link_search_path(path, &mut search_paths);
    }

    // CMake does not install every static dependency into the same directory on every platform.
    // We add the discovered archive directories so Rust can link against the actual build layout.
    for root in [
        dst.join("lib"),
        dst.join("lib64"),
        out_dir.join("build"),
        out_dir.join("build/third_party"),
        out_dir.join("build/_deps"),
    ] {
        collect_library_dirs(&root, &static_libs, 5, &mut search_paths);
        collect_library_dirs(&root, &absl_libs, 6, &mut search_paths);
    }

    for path in [
        env::var_os("OPENSSL_LIB_DIR").map(PathBuf::from),
        env::var_os("OPENSSL_DIR").map(|dir| PathBuf::from(dir).join("lib")),
        Some(PathBuf::from("/opt/homebrew/opt/openssl@3/lib")),
        Some(PathBuf::from("/opt/homebrew/opt/openssl/lib")),
        Some(PathBuf::from("/opt/homebrew/lib")),
        Some(PathBuf::from("/usr/local/opt/openssl@3/lib")),
        Some(PathBuf::from("/usr/local/opt/openssl/lib")),
        Some(PathBuf::from("/usr/local/lib")),
    ]
    .into_iter()
    .flatten()
    {
        add_link_search_path(path, &mut search_paths);
    }

    let gflags_lib =
        select_static_library_name(&search_paths, &gflags_candidates).unwrap_or("gflags_nothreads");
    let arrow_lib =
        select_static_library_name(&search_paths, &arrow_candidates).unwrap_or("arrow_static");
    let protobuf_lib =
        select_static_library_name(&search_paths, &protobuf_candidates).unwrap_or("protobuf");
    let protobuf_lite_lib = select_static_library_name(&search_paths, &protobuf_lite_candidates)
        .unwrap_or("protobuf-lite");

    // Core libraries
    println!("cargo:rustc-link-lib=static=neug_c_api");
    println!("cargo:rustc-link-lib=static=neug");

    // Static dependencies
    println!("cargo:rustc-link-lib=static=glog");
    println!("cargo:rustc-link-lib=static={}", gflags_lib);
    println!("cargo:rustc-link-lib=static=yaml-cpp");
    println!("cargo:rustc-link-lib=static={}", arrow_lib);
    println!("cargo:rustc-link-lib=static=arrow_dataset");
    println!("cargo:rustc-link-lib=static=arrow_acero");
    println!("cargo:rustc-link-lib=static={}", protobuf_lib);
    println!("cargo:rustc-link-lib=static={}", protobuf_lite_lib);
    println!("cargo:rustc-link-lib=static=re2");
    println!("cargo:rustc-link-lib=static=utf8proc");
    println!("cargo:rustc-link-lib=static=antlr4_runtime");
    println!("cargo:rustc-link-lib=static=antlr4_cypher");

    // Abseil's static archives have cross-dependencies that vary with Arrow/Protobuf features.
    // A second pass keeps the linker from dropping providers that only become needed later.
    for lib in absl_libs {
        println!("cargo:rustc-link-lib=static={}", lib);
    }
    for lib in absl_libs {
        println!("cargo:rustc-link-lib=static={}", lib);
    }

    // GitHub runners do not ship libsnappy-dev, while Arrow often vendors these archives into
    // OUT_DIR. Prefer the bundled static copies so Linux CI does not depend on extra apt packages.
    // Keep OpenSSL and zlib dynamic because they are expected to come from the host toolchain.
    println!("cargo:rustc-link-lib=dylib=ssl");
    println!("cargo:rustc-link-lib=dylib=crypto");
    println!("cargo:rustc-link-lib=dylib=z");
    emit_preferred_link(&search_paths, &snappy_candidates, "snappy");
    emit_preferred_link(&search_paths, &zstd_candidates, "zstd");

    // Link C++ standard library and system libs
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else {
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=dl");
        println!("cargo:rustc-link-lib=dylib=rt");
        println!("cargo:rustc-link-lib=dylib=m");
    }

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=c_api.h");
    println!("cargo:rerun-if-changed=c_api.cpp");
    // The vendored C++ source is patched during the build, so Cargo must rerun
    // when any patch file changes.
    println!("cargo:rerun-if-changed=patches");

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
