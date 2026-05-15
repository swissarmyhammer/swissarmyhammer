//! Cross-language fixture verifier/regenerator for the sneak code algorithm.
//!
//! The Jump-To overlay's TS-side test
//! (`kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx`) used to carry a
//! hand-written port of [`generate_sneak_codes`]. The two implementations
//! had no compile-time link, so any change to the alphabet ordering or the
//! single→two-letter fill formula could drift silently — the overlay would
//! dispatch focus to the wrong scope at runtime without any test catching
//! it.
//!
//! This test eliminates the port. It runs [`generate_sneak_codes`] over a
//! representative count set and compares the result to
//! `kanban-app/ui/src/test/fixtures/sneak-fixture.json`. The TS test
//! imports that fixture directly and uses it as the IPC mock for
//! `generate_jump_codes`. The Rust crate is the single source of truth;
//! the TS test cannot drift because there is no TS port to drift.
//!
//! # Drift detection
//!
//! The default test path is read-only: it reads the fixture, recomputes
//! the expected map from [`generate_sneak_codes`], and asserts byte-for-byte
//! equality. A drifted fixture (algorithm changed but file not regenerated)
//! fails the test directly — no CI plumbing required.
//!
//! # Regeneration
//!
//! When the algorithm changes intentionally, regenerate the fixture by
//! running the test with the `BLESS` env var set:
//!
//! ```sh
//! BLESS=1 cargo test -p swissarmyhammer-focus --test sneak_fixture
//! ```
//!
//! Then commit the updated `kanban-app/ui/src/test/fixtures/sneak-fixture.json`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use swissarmyhammer_focus::{generate_sneak_codes, MAX_SNEAK_CODES};

/// Counts to include in the fixture.
///
/// Covers `0..=100` densely (the Jump-To overlay realistically presents
/// well under 100 targets — the 3×3 board fixture in the spatial-nav
/// tests yields ~30-50 enumerable scopes), plus boundary values around
/// the alphabet length (23, 24, 25), the maximum (528, 529), and a few
/// scattered larger values for completeness.
fn fixture_counts() -> Vec<usize> {
    let mut counts: Vec<usize> = (0..=100).collect();
    for extra in [150, 200, 300, 400, 500, 528, 529] {
        counts.push(extra);
    }
    counts.sort_unstable();
    counts.dedup();
    counts
}

/// Path the fixture is read from / written to, resolved from the workspace root.
///
/// `CARGO_MANIFEST_DIR` is `crates/swissarmyhammer-focus`; the workspace root
/// is two levels up.
fn fixture_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR must have a workspace-root grandparent");
    workspace_root
        .join("apps/kanban-app")
        .join("ui")
        .join("src")
        .join("test")
        .join("fixtures")
        .join("sneak-fixture.json")
}

/// Build the fixture map: stringified count → generated codes.
///
/// Stringified keys (rather than numeric) are deliberate — JSON object
/// keys are always strings, and stringifying on the Rust side keeps the
/// TS lookup ergonomic (`fixture[String(count)]`) without coercion.
fn build_fixture_map(counts: &[usize]) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();
    for &n in counts {
        let codes = generate_sneak_codes(n)
            .unwrap_or_else(|e| panic!("generate_sneak_codes({n}) failed: {e}"));
        map.insert(n.to_string(), codes);
    }
    map
}

/// Serialize the fixture map to the canonical on-disk JSON form.
///
/// Pretty-printed for clean diffs; trailing newline so editors with
/// final-newline-on-save don't show spurious diffs.
fn serialize_fixture(map: &BTreeMap<String, Vec<String>>) -> String {
    let mut json = serde_json::to_string_pretty(map).expect("fixture must serialize as JSON");
    json.push('\n');
    json
}

/// Verifies the on-disk fixture matches what [`generate_sneak_codes`] produces,
/// or regenerates it when `BLESS=1` is set in the environment.
///
/// Default path (no `BLESS`): read the fixture, recompute the expected
/// JSON, and assert byte-for-byte equality. A mismatch fails the test
/// with a message pointing at the regeneration command.
///
/// `BLESS=1` path: write the fresh JSON to disk. Re-run without `BLESS`
/// to confirm the new fixture is what the algorithm produces (it will be,
/// trivially, but it keeps the contract uniform).
#[test]
fn sneak_fixture_matches_algorithm() {
    let counts = fixture_counts();
    assert!(
        counts.iter().all(|&n| n <= MAX_SNEAK_CODES),
        "fixture counts must all stay within MAX_SNEAK_CODES ({MAX_SNEAK_CODES})",
    );

    let map = build_fixture_map(&counts);
    let expected = serialize_fixture(&map);
    let path = fixture_path();

    if std::env::var("BLESS").is_ok() {
        let parent = path
            .parent()
            .expect("fixture path must have a parent directory");
        std::fs::create_dir_all(parent).unwrap_or_else(|e| {
            panic!(
                "failed to create fixture directory {}: {e}",
                parent.display(),
            )
        });
        std::fs::write(&path, &expected)
            .unwrap_or_else(|e| panic!("failed to write fixture {}: {e}", path.display()));
        return;
    }

    let actual = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read fixture {}: {e}\n\
             regenerate with: BLESS=1 cargo test -p swissarmyhammer-focus --test sneak_fixture",
            path.display(),
        )
    });

    assert_eq!(
        actual,
        expected,
        "fixture {} is stale; regenerate with: \
         BLESS=1 cargo test -p swissarmyhammer-focus --test sneak_fixture",
        path.display(),
    );
}
