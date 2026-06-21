//! Lean shared test fixtures for the plugin-crate integration tests.
//!
//! This module is deliberately small and dependency-free beyond `std`: the
//! heavier example-plugin staging helpers live in `tests/support/mod.rs`, which
//! pulls in `swissarmyhammer-tools` / `swissarmyhammer-git`. Tests that only
//! need to stamp a one-file probe plugin pull *this* module in with
//! `#[path = "fixture/mod.rs"] mod fixture;` so they do not drag those heavy
//! deps into their test binary.
//!
//! Cargo compiles only top-level `tests/*.rs` files into their own test
//! binaries — a nested `tests/fixture/mod.rs` is never a binary of its own, so
//! this file is shared source rather than a standalone target.

#![allow(dead_code)] // each test file uses only the subset it needs

use std::path::Path;

/// Which `Promise` shape and result-capture behavior a probe bundle's `load()`
/// hook uses.
///
/// The plugin platform runs a bundle's `load()` lifecycle hook; the two variants
/// differ only in whether that hook reports a value back to the host:
///
/// - [`LoadResult::Void`] — `load(): Promise<void>`, no result. The body runs
///   purely for its side effects (registering modules, dispatching callbacks,
///   subscribing to notifications).
/// - [`LoadResult::Captured`] — `load(): Promise<unknown>`, returning
///   `globalThis.__result ?? null`. A body that records a value on
///   `globalThis.__result` then observes it as the lifecycle call's return
///   value — the SDK wire-shape tests use this to read a dispatch return value.
#[derive(Clone, Copy, Debug)]
pub enum LoadResult {
    /// `Promise<void>` — the `load()` hook returns nothing.
    Void,
    /// `Promise<unknown>` — the `load()` hook returns `globalThis.__result`.
    Captured,
}

/// Write a one-file plugin bundle whose default-class `load()` runs `body`.
///
/// The entry imports the SDK and default-exports a `Plugin` subclass whose
/// `load()` contains `body`. The host instantiates the default export, wraps it
/// with the SDK's plugin Proxy, and runs its `load()` — the bundle shape the
/// host's `load(plugin_dir)` expects.
///
/// `entry_file` is the file name the bundle is written under (`"index.ts"` for
/// the host-discovered shape, `"entry.ts"` for the runtime's explicit
/// `call_plugin_lifecycle(..., "entry.ts", ...)` shape). `result` selects the
/// `load()` `Promise` shape and whether it returns `globalThis.__result` (see
/// [`LoadResult`]).
pub fn write_plugin(dir: &Path, entry_file: &str, body: &str, result: LoadResult) {
    let load_signature = match result {
        LoadResult::Void => "async load(): Promise<void>",
        LoadResult::Captured => "async load(): Promise<unknown>",
    };
    let load_tail = match result {
        LoadResult::Void => String::new(),
        LoadResult::Captured => "             return globalThis.__result ?? null;\n".to_string(),
    };
    let entry = format!(
        "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
         export default class P extends Plugin {{\n\
           {load_signature} {{\n{body}\n{load_tail}}}\n\
         }}\n"
    );
    std::fs::write(dir.join(entry_file), entry)
        .unwrap_or_else(|e| panic!("{entry_file} should be written: {e}"));
}
