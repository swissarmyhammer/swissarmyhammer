---
title: Rust Project Guidelines
description: Best practices and tooling for Rust projects
partial: true
---

### Rust Project Guidelines

**Testing Strategy:**
- **ALWAYS use `cargo nextest` for running tests** - it's faster and more reliable than `cargo test`
- **To run ALL tests:** `cargo nextest run --workspace` (recommended)
- **To run tests for a specific package:** `cargo nextest run --package package-name`
- **To run a specific test:** `cargo nextest run test_name`
- **If nextest is not installed:** Install with `cargo install cargo-nextest --locked`
- **Check nextest is available:** `cargo nextest --version` (if this fails, install it first)

**IMPORTANT:** Do NOT use glob patterns to discover tests. Use the project detection system and run `cargo nextest run ` to execute all tests from the root of the project. 

**Common Commands:**
- Build: `cargo build` (debug) or `cargo build --release` (optimized)
- Check: `cargo check` (faster than build, validates code)
- Format: `cargo fmt` (auto-format code)
- Lint: `cargo clippy` (catch common mistakes)
- Documentation: `cargo doc --open`
- Test: `cargo nextest run`, you might need to install it first with `cargo install cargo-nextest --locked`
end


**Best Practices:**
- Run `cargo clippy` before committing to catch common issues
- Use `cargo fmt` to maintain consistent code style
- Prefer `cargo check` for quick validation during development
- Use workspace features if this is part of a Cargo workspace

**File Locations:**
- Source code: `src/`
- Tests: `tests/` (integration tests) or inline in `src/` (unit tests)
- Examples: `examples/`
- Binaries: `src/bin/`
- Build output: `target/` (git-ignored)
