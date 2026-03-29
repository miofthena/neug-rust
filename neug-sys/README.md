# neug-sys

Raw FFI bindings and the `neug-worker` sidecar binary for the NeuG C++ graph database engine.

`neug-sys` is the low-level crate in this workspace. Most applications should depend on
[`neug-rust`](https://crates.io/crates/neug-rust) instead, which exposes the safe Rust API.

## What This Crate Provides

- Unsafe Rust bindings generated from the NeuG C API.
- The `build.rs` logic that prepares and compiles the vendored C++ engine.
- The `neug-worker` binary used by the safe wrapper to isolate the C++ runtime in a sidecar process.

## Build Behavior

During `cargo build`, the build script:

1. Copies the local `neug-cpp` checkout when it is available, otherwise clones the upstream source.
2. Applies the patch set from `patches/` before configuring CMake.
3. Builds NeuG as a static library and links the Rust bindings against the resulting artifacts.

## Requirements

Building from source requires:

- CMake 3.16 or newer
- A C++20-capable compiler
- OpenSSL development libraries

Installing `sccache` or `ccache` is recommended to reduce rebuild time for the C++ portion.

## Usage

Direct use is intended for low-level integrations:

```toml
[dependencies]
neug-sys = "0.2.0"
```

Most users should use the safe wrapper instead:

```toml
[dependencies]
neug-rust = "0.2.0"
```

## License

Licensed under Apache-2.0.
