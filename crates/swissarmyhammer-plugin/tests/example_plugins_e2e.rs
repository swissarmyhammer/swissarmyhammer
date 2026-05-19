//! End-to-end integration tests driven by the committed example plugins.
//!
//! The bundles under `examples/plugins/` are real, author-shaped plugins —
//! a `plugin.json` manifest plus a TypeScript `entry.ts` — committed so a
//! plugin author can read them as worked examples. This test file exercises
//! those committed bundles through the real plugin platform, so the examples
//! are also a living regression suite: if an example stops loading, a test
//! here fails.
//!
//! Every test pulls in the shared [`support`] module, which locates the
//! example bundles, stages them into temp layer roots, and stands up the real
//! MCP server. This smoke test only verifies the scaffolding itself — the
//! examples directory exists and is documented; later tests in this file load
//! and drive the individual example bundles.

mod support;

/// The example bundles directory exists and ships its author documentation.
///
/// This is the scaffolding smoke test: it asserts [`support::examples_root`]
/// resolves to a real directory and that the plugin-author `README.md` is
/// present inside it. A passing run proves the `examples/plugins/` tree is in
/// place and discoverable before any example-specific test relies on it.
#[test]
fn examples_root_is_present() {
    let root = support::examples_root();
    assert!(
        root.is_dir(),
        "the example plugins directory must exist at {}",
        root.display(),
    );

    let readme = root.join("README.md");
    assert!(
        readme.is_file(),
        "the plugin-author README must be present at {}",
        readme.display(),
    );
}
