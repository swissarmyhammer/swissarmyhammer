---
title: Rust Project Guidelines
description: Best practices and tooling for Rust projects
partial: true
---

### Rust Project Guidelines

**Testing — always use `cargo nextest`** (faster, more reliable than `cargo test`):
- All tests: `cargo nextest run --workspace`
- Package: `cargo nextest run --package <name>`
- Single test: `cargo nextest run <test_name>`
- Install if missing: `cargo install cargo-nextest --locked`

**Do NOT glob for test files.** Run `cargo nextest run` from the project root.

**Common commands:**
- Build: `cargo build` / `cargo build --release`
- Check (faster than build): `cargo check`
- Format: `cargo fmt` (verify: `cargo fmt --check`) — CI enforces
- Lint: `cargo clippy -- -D warnings` — CI enforces
- Docs: `cargo doc --open`

Run `cargo fmt` and `cargo clippy` before committing.

**File locations:** `src/` (source), `tests/` (integration), `examples/`, `src/bin/`, `target/` (git-ignored).

**Targeted testing** — picks up changed crate + reverse deps:

```
cargo nextest run -E 'rdeps(my-crate)'
cargo nextest run -E 'rdeps(crate-a) | rdeps(crate-b)'
```
