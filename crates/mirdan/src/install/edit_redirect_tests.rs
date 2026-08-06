use super::*;
use serde_json::json;

/// The generated fragment must be valid Claude Code settings JSON: it denies
/// every native sah supersedes and installs **no** `PreToolUse` hook. The deny
/// alone closes the surface — the model is steered to the served `shell` and
/// `files` replacements.
#[test]
fn fragment_denies_superseded_natives_without_hook() {
    let fragment = desired_edit_redirect_fragment();

    // Deny entries: one per superseded native (order matches the constant).
    let deny = fragment["permissions"]["deny"]
        .as_array()
        .expect("permissions.deny must be an array");
    for tool in SUPERSEDED_NATIVE_DENY_TOOLS {
        assert!(
            deny.iter().any(|v| v == &json!(tool)),
            "deny must contain {tool}; got {deny:?}"
        );
    }

    // No PreToolUse redirect hook: the deny is the whole mechanism.
    assert!(
        fragment.get("hooks").is_none(),
        "fragment must not carry any hooks; got {fragment:?}"
    );
}

/// Installing into a fresh settings file writes the deny; doing it again is a
/// no-op (idempotent). No `PreToolUse` hook is written.
#[test]
fn apply_edit_redirect_at_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".claude/settings.local.json");

    assert!(
        apply_edit_redirect_at(&path, true).unwrap(),
        "first install must change the file"
    );
    assert!(
        !apply_edit_redirect_at(&path, true).unwrap(),
        "second install must be a no-op"
    );

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // The deny is present and not duplicated by the second install.
    let deny = written["permissions"]["deny"].as_array().unwrap();
    for tool in SUPERSEDED_NATIVE_DENY_TOOLS {
        assert_eq!(
            deny.iter().filter(|v| *v == &json!(tool)).count(),
            1,
            "{tool} deny must appear exactly once; got {deny:?}"
        );
    }
    // No redirect hook is installed.
    assert!(
        written.get("hooks").is_none(),
        "install must not write any hooks; got {written:?}"
    );
}

/// The install must merge the deny into an existing settings chain without
/// clobbering unrelated keys or pre-existing deny/hook entries.
#[test]
fn apply_edit_redirect_at_preserves_unrelated_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&json!({
            "model": "opus",
            "permissions": { "deny": ["Bash"], "allow": ["Read"] },
            "hooks": {
                "PostToolUse": [
                    { "matcher": "Write", "hooks": [{ "type": "command", "command": "fmt" }] }
                ]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(apply_edit_redirect_at(&path, true).unwrap());

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // Unrelated top-level key untouched.
    assert_eq!(written["model"], json!("opus"));
    // Pre-existing allow + the pre-existing Bash deny survive.
    assert_eq!(written["permissions"]["allow"], json!(["Read"]));
    let deny = written["permissions"]["deny"].as_array().unwrap();
    assert!(deny.iter().any(|v| v == &json!("Bash")));
    assert!(deny.iter().any(|v| v == &json!("Edit")));
    // The unrelated PostToolUse hook is preserved.
    assert!(written["hooks"]["PostToolUse"].is_array());
}

/// The deny set is the exact roster of natives sah supersedes: `Bash` (served
/// as `shell`) and `Read`/`Edit`/`Write` (served as `files`). Pinned so a silent
/// change to the roster fails a test instead of shipping.
#[test]
fn superseded_deny_set_is_exactly_the_four_natives() {
    assert_eq!(
        SUPERSEDED_NATIVE_DENY_TOOLS,
        ["Bash", "Edit", "Read", "Write"],
        "the superseded-native deny set must hold exactly the four natives sah replaces"
    );
}

/// The installer and the doctor probe must agree: a settings file the fragment
/// wrote reads back as installed. Regression test for `sah doctor` reporting
/// `Permissions ┆ missing` right after a successful `sah init`.
#[test]
fn edit_redirect_install_satisfies_permissions_detector() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");

    assert!(
        apply_edit_redirect_at(&path, true).unwrap(),
        "install must write the fragment"
    );

    assert!(
        crate::status::permissions_present(&path),
        "the fragment the installer wrote must satisfy the doctor's permissions probe"
    );
}

/// Removal strips the deny but leaves unrelated entries; removal on a missing
/// file is a no-op. No hooks are ever written, so none are left behind.
#[test]
fn apply_edit_redirect_at_removes_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");

    // Removal on a missing file is a no-op.
    assert!(!apply_edit_redirect_at(&path, false).unwrap());

    // Install then remove returns to a clean state for our keys.
    apply_edit_redirect_at(&path, true).unwrap();
    assert!(apply_edit_redirect_at(&path, false).unwrap());

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let deny = written["permissions"]["deny"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for tool in SUPERSEDED_NATIVE_DENY_TOOLS {
        assert!(
            !deny.iter().any(|v| v == &json!(tool)),
            "{tool} deny must be removed"
        );
    }
    // The redirect never installed a hook, so none should be present.
    assert!(
        written.get("hooks").is_none(),
        "no hooks must be present; got {written:?}"
    );
}
