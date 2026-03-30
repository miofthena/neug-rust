# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/miofthena/neug-rust/compare/neug-sys-v0.1.0...neug-sys-v0.2.1) - 2026-03-30

### Added

- migrate to sidecar architecture to prevent ODR violations

### Fixed

- preserve publish compatibility for access modes
- restore linux static link dependencies
- prefer bundled snappy and zstd in ci
- vendor third-party build patches
- stabilize static NeuG linking and worker execution
- link gflags_nothreads instead of gflags
- correct malformed patch for CMakeLists.txt build type
- *(build)* fix formatting and clippy warnings, update AGENTS.md
- *(build)* add linux system libs and expand absl dependencies for static linking
- *(build)* correct static link order and add abseil dependencies
- *(build)* final fix for neug-sys build.rs structure and library linking
- *(build)* correct link order and add protobuf dependencies
- *(build)* link all static dependencies and fix InitVertexNum warning
- *(build)* correct CMake nesting in patch 0004
- *(build)* final fix for Werror and static linking of dependencies
- *(build)* final attempt to suppress warnings and force static neug
- *(build)* only export yaml-cpp and arrow_lib, avoiding duplicate exports
- *(build)* remove duplicate exports in patch
- *(build)* export yaml-cpp and other dependencies to fix static build error
- *(build)* fix neug-sys compilation by patching C++ build type and Werror
- resolve memory allocation errors during DML operations

### Other

- collapse nested if to resolve clippy::collapsible_if warning
- statically link neug into neug-worker to fix shared library errors
- fix neug::utf8proc namespace resolution in list_len_function
- fix workspace dependency versions and add description to neug-protocol
- enable concurrent query execution in neug-worker
- fix undeclared identifier utf8proc compilation error
- run cargo fmt in neug-sys/build.rs
- fix glog cmake export error caused by gflags
- apply DML buffer fix via patch to keep submodule clean
