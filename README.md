# neug-rust

A Rust FFI wrapper for the [alibaba/neug](https://github.com/alibaba/neug) C++ graph learning framework.

## Overview

This project provides low-level Rust bindings to the `neug` C++ library using `bindgen` and builds the library from source using the `cmake` crate. 

This is an initialized repository ready to be expanded with safe Rust abstractions and eventually published to crates.io.

## Prerequisites

Building `neug` from source requires several C++ dependencies installed on your system, as defined by its CMake configuration. Please ensure you have the following installed (e.g., via `brew` on macOS or `apt` on Linux):

- CMake (>= 3.16)
- C++17 compatible compiler (Clang/GCC)
- OpenSSL
- Apache Arrow
- gflags, glog
- Protobuf
- yaml-cpp

## Getting Started

1. **Clone the repository with submodules:**
   ```bash
   git clone --recursive <your-repo-url>
   cd neug-rust
   ```
   *(If already cloned, run `git submodule update --init --recursive`)*

2. **Build the bindings and C++ library:**
   ```bash
   cargo build
   ```
   
   *Note: The first build will take some time as it compiles the C++ codebase.*

3. **Run Tests:**
   ```bash
   cargo test
   ```

## Project Structure

- `neug-cpp/`: A git submodule pointing to the upstream `alibaba/neug` repository.
- `wrapper.h`: The C++ header file that `bindgen` uses as the entry point to generate bindings.
- `build.rs`: The build script that configures `cmake` to compile `neug-cpp` and generates the Rust bindings via `bindgen`.
- `src/lib.rs`: The main entry point for the Rust library that includes the generated bindings.

## Contributing

1. Add high-level, safe Rust abstractions in `src/`.
2. Ensure you adhere to standard Rust community practices (e.g., `cargo fmt`, `cargo clippy`).
3. Add tests to verify your safe wrappers.

## License

This wrapper is licensed under the same terms as the `neug` project (Apache License 2.0).
