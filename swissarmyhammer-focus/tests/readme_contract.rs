//! Regression guard: the crate-level `README.md` must contain the
//! load-bearing navigation contract. A future contributor cannot
//! silently delete the rule from the README without breaking this
//! test.
//!
//! See `swissarmyhammer-focus/README.md` itself for the prose, and
//! `src/navigate.rs` for the algorithm that enforces the contract.

/// The README exists at a known relative path and contains the
/// verbatim sibling-rule sentence. The path is resolved at compile
/// time via `CARGO_MANIFEST_DIR` so the test runs correctly regardless
/// of the workspace cwd at execution time.
#[test]
fn readme_contains_sibling_rule_contract() {
    let readme_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let body = std::fs::read_to_string(&readme_path)
        .unwrap_or_else(|e| panic!("failed to read README at {}: {e}", readme_path.display()));

    assert!(
        body.contains("zones and scopes are siblings"),
        "swissarmyhammer-focus/README.md must contain the literal substring \
         \"zones and scopes are siblings\" — the load-bearing navigation \
         contract. If you are deleting or rewording this phrase, you are \
         changing the kernel's contract; update the kernel and tests in \
         the same change."
    );
}
