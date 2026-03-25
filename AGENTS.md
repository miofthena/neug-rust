# Agent Rules for NeuG-Rust

This repository provides Rust bindings for the NeuG C++ graph database engine. It consists of `neug-sys` (raw FFI), `neug-bindings` (idiomatic wrapper), and `neug-benchmarks`.

## General AI Rules
- You MUST follow all instructions in this file.
- **NEVER** use emoji or unicode emulators (✓, ✗) in code, comments, or CLI output.
- **MUST** focus comments on "Why", not "What".
- **MUST** keep `README.md` and documentation synchronized with the actual implementation.

## Repository Hygiene
- **NEVER** commit local build artifacts: `a.out`, `bench_results.txt`, `.DS_Store`, `test.rs`.
- **NEVER** leave `.orig`, `.bak`, or temporary `.patch` files in the repository root.
- **NEVER** search or grep the `target/` or `neug-cpp/build/` directories.
- **MUST** clean up any temporary directories created during FFI testing (e.g., `neug-cpp-test`).
- **MUST** update `.gitignore` immediately if a new type of local artifact is generated.

## C++ & Build System (FFI)
- **MUST** manage C++ modifications via patches in `neug-sys/patches/` using 4-digit numeric prefixes (e.g., `0004-fix-build.patch`).
- **NEVER** modify `neug-cpp` submodule files directly without creating a corresponding patch in `neug-sys/patches/`.
- **ALWAYS** prefer `STATIC` linking for the `neug` library to ensure the resulting Rust crate is portable.
- **NEVER** allow `-Werror` to break the CI build due to minor warnings in third-party code; use specific `-Wno-` flags in `build.rs` or patches instead.
- **MUST** ensure `CMAKE_POSITION_INDEPENDENT_CODE=ON` is set when building the static C++ library for use in Rust.

## Rust-Specific Rules
- **MUST** run `cargo clippy` and `cargo test` before any push.
- **NEVER** use `.unwrap()` or `.expect()` in `neug-sys` or `neug-bindings`. Handle all errors gracefully.
- **MUST** use `thiserror` for defining error types in library crates and `anyhow` for benchmarks/examples.
- **MUST** document all `unsafe` blocks with a `// SAFETY:` comment explaining why the operation is safe.
- **MUST** use the `tracing` crate for logging instead of `println!`.

## CI/CD & Verification
- **MUST** verify that any changes to `build.rs`, `c_api.cpp`, or CMake files are compatible with the GitHub Actions environment.
- **MUST** reproduce reported bugs with a test case before applying a fix.
- **CRITICAL:** Before pushing, double-check that your changes don't introduce regression in the C++ build time (e.g., by accidentally triggering a full recompile of Arrow/Protobuf).
