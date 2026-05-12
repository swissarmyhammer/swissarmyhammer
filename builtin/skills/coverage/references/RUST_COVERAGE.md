# Rust Test Coverage

## Running Coverage

**Preferred: cargo-tarpaulin**

```bash
# Full workspace
cargo tarpaulin --out lcov --output-dir .

# Specific crate
cargo tarpaulin -p <crate_name> --out lcov --output-dir .

# Specific directory (use manifest path)
cargo tarpaulin --manifest-path crates/foo/Cargo.toml --out lcov --output-dir .
```

Install if missing: `cargo install cargo-tarpaulin`

**Alternative: cargo-llvm-cov** (requires nightly or llvm-tools)

```bash
# Full workspace
cargo llvm-cov --lcov --output-path lcov.info

# Specific crate
cargo llvm-cov -p <crate_name> --lcov --output-path lcov.info
```

Install if missing: `cargo install cargo-llvm-cov`

## Output

Both tools write `lcov.info` in LCOV format. Parse `DA:<line>,<hits>` lines per file.

## Scoping

- To scope to a crate: use `-p <crate_name>` flag
- To scope to a workspace: run from workspace root with no `-p` flag
- Tarpaulin respects `--exclude` to skip crates

## Test Locations

- **Inline tests:** `#[cfg(test)] mod tests { ... }` at the bottom of source files
- **Integration tests:** `tests/` directory at crate root
- **Doc tests:** `///` code blocks on public items (tarpaulin measures these too)

## What Requires Tests

- All `pub` and `pub(crate)` functions
- All `impl` methods on public types
- Trait implementations (especially `From`, `TryFrom`, custom traits)
- Error variants and their `Display` implementations
- `unsafe` blocks
- Match arms that handle error cases

## Acceptable Without Direct Tests

- Private helpers called exclusively from tested public functions
- Derived trait implementations (`#[derive(Debug, Clone, ...)]`)
- Constants and type aliases
- Re-exports
- Build scripts (`build.rs`) unless they contain complex logic
