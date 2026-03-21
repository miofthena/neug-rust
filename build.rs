use std::env;
use std::path::PathBuf;

fn main() {
    // Build the C++ library using CMake
    let dst = cmake::Config::new("neug-cpp")
        .define("BUILD_TESTS", "OFF")
        .define("BUILD_EXAMPLES", "OFF")
        .define("BUILD_HTTP_SERVER", "OFF")
        .build();

    // Link against the built `neug` library
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-search=native={}/lib64", dst.display());
    println!("cargo:rustc-link-lib=dylib=neug");

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=neug-cpp/CMakeLists.txt");

    // Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg("-xc++")
        .clang_arg("-std=c++20")
        .clang_arg(format!("-I{}/include", dst.display()))
        // Since neug-cpp contains headers in include/neug, we also need the source include dirs
        .clang_arg("-Ineug-cpp/include")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Only generate bindings for neug
        .allowlist_type("neug::.*")
        .allowlist_function("neug::.*")
        .allowlist_var("neug::.*")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
