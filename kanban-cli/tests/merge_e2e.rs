//! End-to-end integration tests for the `kanban merge` git merge drivers.
//!
//! These tests exercise the full merge workflow: they create temporary files,
//! invoke the `kanban` binary as a child process, and verify exit codes and
//! output file contents.
//!
//! These tests use `env!("CARGO_BIN_EXE_kanban")` which nextest resolves
//! automatically, so no pre-build step is required.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Path to the compiled `kanban` binary.
const KANBAN_BIN: &str = env!("CARGO_BIN_EXE_kanban");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write content to a file inside `dir`, returning the absolute path as a string.
fn write_file(dir: &Path, name: &str, content: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, content).expect("write_file failed");
    path.to_string_lossy().into_owned()
}

/// Read file contents back from disk.
fn read_file(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    fs::read_to_string(&path).expect("read_file failed")
}

/// Invoke `kanban merge <format> <base> <ours> <theirs>` and return the exit code.
///
/// The `ours` file is modified in place by the merge driver.
fn run_merge(format: &str, base: &str, ours: &str, theirs: &str) -> i32 {
    let status = Command::new(KANBAN_BIN)
        .args(["merge", format, base, ours, theirs])
        .status()
        .expect("failed to launch kanban binary");
    status.code().unwrap_or(-1)
}

// ---------------------------------------------------------------------------
// JSONL tests
// ---------------------------------------------------------------------------

/// JSONL: disjoint appends from both branches merge cleanly (exit 0).
///
/// Base has entries A and B. Ours adds C. Theirs adds D.
/// Merged result must contain all four entries.
#[test]
fn jsonl_disjoint_appends_merge_clean() {
    let dir = TempDir::new().unwrap();

    let entry_a = r#"{"id":"01AAA000000000000000000000","op":"create","changes":[]}"#;
    let entry_b = r#"{"id":"01AAA000000000000000000001","op":"update","changes":[]}"#;
    let entry_c = r#"{"id":"01AAA000000000000000000002","op":"create","changes":[]}"#;
    let entry_d = r#"{"id":"01AAA000000000000000000003","op":"delete","changes":[]}"#;

    let base_content = format!("{entry_a}\n{entry_b}\n");
    let ours_content = format!("{entry_a}\n{entry_b}\n{entry_c}\n");
    let theirs_content = format!("{entry_a}\n{entry_b}\n{entry_d}\n");

    let base_path = write_file(dir.path(), "base.jsonl", &base_content);
    let ours_path = write_file(dir.path(), "ours.jsonl", &ours_content);
    let theirs_path = write_file(dir.path(), "theirs.jsonl", &theirs_content);

    let exit_code = run_merge("jsonl", &base_path, &ours_path, &theirs_path);
    assert_eq!(exit_code, 0, "disjoint appends should merge cleanly");

    let merged = read_file(dir.path(), "ours.jsonl");
    assert!(merged.contains(entry_a), "merged should contain A");
    assert!(merged.contains(entry_b), "merged should contain B");
    assert!(merged.contains(entry_c), "merged should contain C");
    assert!(merged.contains(entry_d), "merged should contain D");
    assert_eq!(
        merged.lines().count(),
        4,
        "merged should have exactly 4 lines"
    );
}

/// JSONL: same id with different content on both sides produces a conflict (exit 1).
#[test]
fn jsonl_same_id_different_content_conflicts() {
    let dir = TempDir::new().unwrap();

    let entry_a = r#"{"id":"01AAA000000000000000000000","op":"create","changes":[]}"#;
    let entry_x1 = r#"{"id":"01BBB000000000000000000000","op":"create","changes":["v1"]}"#;
    let entry_x2 = r#"{"id":"01BBB000000000000000000000","op":"create","changes":["v2"]}"#;

    let base_content = format!("{entry_a}\n");
    let ours_content = format!("{entry_a}\n{entry_x1}\n");
    let theirs_content = format!("{entry_a}\n{entry_x2}\n");

    let base_path = write_file(dir.path(), "base.jsonl", &base_content);
    let ours_path = write_file(dir.path(), "ours.jsonl", &ours_content);
    let theirs_path = write_file(dir.path(), "theirs.jsonl", &theirs_content);

    let exit_code = run_merge("jsonl", &base_path, &ours_path, &theirs_path);
    assert_eq!(exit_code, 1, "same-id-different-content should be exit 1");
}

// ---------------------------------------------------------------------------
// YAML tests
// ---------------------------------------------------------------------------

/// YAML: non-overlapping field changes from both branches merge cleanly (exit 0).
///
/// Base has `title: Base`. Ours changes `title` to `Ours`. Theirs adds `color: ff0000`.
/// Merged result should have `title: Ours` and `color: ff0000`.
#[test]
fn yaml_non_overlapping_fields_merge_clean() {
    let dir = TempDir::new().unwrap();

    let base_content = "id: 01AAA000000000000000000000\ntitle: Base\n";
    let ours_content = "id: 01AAA000000000000000000000\ntitle: Ours\n";
    let theirs_content = "id: 01AAA000000000000000000000\ntitle: Base\ncolor: ff0000\n";

    let base_path = write_file(dir.path(), "base.yaml", base_content);
    let ours_path = write_file(dir.path(), "ours.yaml", ours_content);
    let theirs_path = write_file(dir.path(), "theirs.yaml", theirs_content);

    let exit_code = run_merge("yaml", &base_path, &ours_path, &theirs_path);
    assert_eq!(
        exit_code, 0,
        "non-overlapping field changes should merge cleanly"
    );

    let merged = read_file(dir.path(), "ours.yaml");
    assert!(
        merged.contains("title: Ours"),
        "ours title change should be present"
    );
    assert!(
        merged.contains("color: ff0000"),
        "theirs color addition should be present"
    );
}

/// YAML: same-field conflict resolved by fallback (theirs wins by default) → exit 0.
///
/// Both sides change `title` independently. The driver should resolve without error
/// using the fallback precedence (theirs wins), so exit code is 0.
#[test]
fn yaml_same_field_conflict_resolved_by_fallback() {
    let dir = TempDir::new().unwrap();

    let base_content = "id: 01AAA000000000000000000000\ntitle: Base\n";
    let ours_content = "id: 01AAA000000000000000000000\ntitle: Ours\n";
    let theirs_content = "id: 01AAA000000000000000000000\ntitle: Theirs\n";

    let base_path = write_file(dir.path(), "base.yaml", base_content);
    let ours_path = write_file(dir.path(), "ours.yaml", ours_content);
    let theirs_path = write_file(dir.path(), "theirs.yaml", theirs_content);

    // YAML merge always resolves via fallback (theirs wins), never exit 1.
    let exit_code = run_merge("yaml", &base_path, &ours_path, &theirs_path);
    assert_eq!(
        exit_code, 0,
        "same-field YAML conflict should be resolved by fallback (exit 0)"
    );

    let merged = read_file(dir.path(), "ours.yaml");
    // Theirs wins by default fallback
    assert!(
        merged.contains("title: Theirs"),
        "theirs value should win by default fallback"
    );
}

// ---------------------------------------------------------------------------
// MD (Markdown) tests
// ---------------------------------------------------------------------------

/// MD: frontmatter-only change + body unchanged merges cleanly (exit 0).
///
/// Base has frontmatter `title: Base` and a body. Ours changes `title: Ours` in
/// frontmatter. Theirs adds a frontmatter field `tags: []`. Body is unchanged.
/// Merged result should have both frontmatter changes and the original body.
#[test]
fn md_frontmatter_and_body_changes_merge_clean() {
    let dir = TempDir::new().unwrap();

    let base_content = "---\ntitle: Base\n---\n\nThe body text.\n";
    let ours_content = "---\ntitle: Ours\n---\n\nThe body text.\n";
    let theirs_content = "---\ntitle: Base\ntags: []\n---\n\nThe body text.\n";

    let base_path = write_file(dir.path(), "base.md", base_content);
    let ours_path = write_file(dir.path(), "ours.md", ours_content);
    let theirs_path = write_file(dir.path(), "theirs.md", theirs_content);

    let exit_code = run_merge("md", &base_path, &ours_path, &theirs_path);
    assert_eq!(
        exit_code, 0,
        "non-overlapping frontmatter + unchanged body should merge cleanly"
    );

    let merged = read_file(dir.path(), "ours.md");
    assert!(
        merged.contains("title: Ours"),
        "ours frontmatter change should be present"
    );
    assert!(
        merged.contains("The body text."),
        "body should be preserved"
    );
}

/// MD: overlapping body edits produce conflict markers (exit 1).
///
/// Both branches modify the same line in the body differently. The driver should
/// write conflict markers to the `ours` file and exit 1.
#[test]
fn md_overlapping_body_produces_conflict_markers() {
    let dir = TempDir::new().unwrap();

    let base_content = "---\ntitle: Doc\n---\n\nConflict line.\n";
    let ours_content = "---\ntitle: Doc\n---\n\nOurs edited this line.\n";
    let theirs_content = "---\ntitle: Doc\n---\n\nTheirs edited this line.\n";

    let base_path = write_file(dir.path(), "base.md", base_content);
    let ours_path = write_file(dir.path(), "ours.md", ours_content);
    let theirs_path = write_file(dir.path(), "theirs.md", theirs_content);

    let exit_code = run_merge("md", &base_path, &ours_path, &theirs_path);
    assert_eq!(
        exit_code, 1,
        "overlapping body edits should produce conflict (exit 1)"
    );

    let merged = read_file(dir.path(), "ours.md");
    // The output should contain git-style conflict markers from diffy
    assert!(
        merged.contains("<<<<<<<") || merged.contains(">>>>>>>"),
        "conflict markers should be written to the ours file; got:\n{merged}"
    );
}
