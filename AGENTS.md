# Agent Rules

1. Before committing and pushing changes, ensure that the build using GitHub Actions will work.
   - Verify that any changes to build scripts or CMake files don't break the CI pipeline.
   - Test locally using commands that simulate the CI environment where possible (e.g., `cargo check`, `cargo test`).
