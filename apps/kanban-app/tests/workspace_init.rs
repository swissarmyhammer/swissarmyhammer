//! Integration tests for in-process SwissArmyHammer workspace initialization
//! on board open.
//!
//! Opening a board folder must make it a SwissArmyHammer workspace — the
//! `.sah/` directory and the builtin skills must be present — without ever
//! shelling out to `sah init` and without mutating the process working
//! directory. Running it again must be idempotent.

use std::path::Path;

use swissarmyhammer_common::lifecycle::{InitScope, InitStatus};
use swissarmyhammer_common::reporter::NullReporter;
use swissarmyhammer_workspace_init::run_workspace_init;

/// Create a minimal `.kanban` board structure under `root` that the kanban
/// entity system can load. Mirrors the helper used by `state.rs` tests.
fn create_board_at(root: &Path) {
    let kanban_dir = root.join(".kanban");
    let boards_dir = kanban_dir.join("boards");
    std::fs::create_dir_all(&boards_dir).unwrap();
    std::fs::write(boards_dir.join("board.yaml"), "name: Test Board\n").unwrap();
    for sub in ["columns", "tasks", "tags", "actors", "perspectives"] {
        std::fs::create_dir_all(kanban_dir.join(sub)).unwrap();
    }
}

/// Opening a fresh board folder creates the SwissArmyHammer workspace layout —
/// `.sah/` with `workflows/`, `.prompts/`, and the builtin skills under
/// `.sah/skills/` — via in-process init.
#[test]
fn opening_a_board_creates_the_sah_workspace_and_skills() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_board_at(tmp.path());

    let results = run_workspace_init(tmp.path(), &InitScope::Project, &NullReporter);

    // No component may error.
    assert!(
        results.iter().all(|r| r.status != InitStatus::Error),
        "workspace init reported an error: {:?}",
        results
            .iter()
            .filter(|r| r.status == InitStatus::Error)
            .map(|r| (&r.name, &r.message))
            .collect::<Vec<_>>()
    );

    // The SAH directory layout exists as a sibling of `.kanban/`.
    assert!(tmp.path().join(".sah").is_dir(), ".sah/ must exist");
    assert!(
        tmp.path().join(".sah").join("workflows").is_dir(),
        ".sah/workflows/ must exist"
    );
    assert!(tmp.path().join(".prompts").is_dir(), ".prompts/ must exist");

    // The builtin skills are present in the board-local skills directory.
    let skills_dir = tmp.path().join(".sah").join("skills");
    assert!(skills_dir.is_dir(), ".sah/skills/ must exist");
    let plan_skill = skills_dir.join("plan").join("SKILL.md");
    assert!(
        plan_skill.is_file(),
        "builtin `plan` skill must be deployed at {}",
        plan_skill.display()
    );

    // The original `.kanban` board folder must be untouched.
    assert!(
        tmp.path()
            .join(".kanban")
            .join("boards")
            .join("board.yaml")
            .is_file(),
        "board.yaml must still exist after workspace init"
    );
}

/// Running workspace init twice — as happens every time a board is opened —
/// is idempotent: no error, and no duplicated skill content.
#[test]
fn repeated_board_open_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_board_at(tmp.path());

    let first = run_workspace_init(tmp.path(), &InitScope::Project, &NullReporter);
    let second = run_workspace_init(tmp.path(), &InitScope::Project, &NullReporter);

    assert!(
        first.iter().all(|r| r.status != InitStatus::Error),
        "first init must not error"
    );
    assert!(
        second.iter().all(|r| r.status != InitStatus::Error),
        "second init must not error — workspace init must be idempotent"
    );

    // The deployed skill must not be duplicated or corrupted by the re-run.
    let plan_skill = tmp
        .path()
        .join(".sah")
        .join("skills")
        .join("plan")
        .join("SKILL.md");
    assert!(plan_skill.is_file(), "plan/SKILL.md must still exist");
    let content = std::fs::read_to_string(&plan_skill).unwrap();
    assert_eq!(
        content.matches("name: plan").count(),
        1,
        "idempotent re-init must not duplicate skill frontmatter"
    );
}

/// Workspace init never mutates the process working directory — it is rooted
/// purely at the explicit path argument.
#[test]
fn workspace_init_does_not_mutate_process_cwd() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_board_at(tmp.path());

    let cwd_before = std::env::current_dir().unwrap();
    let _ = run_workspace_init(tmp.path(), &InitScope::Project, &NullReporter);
    let cwd_after = std::env::current_dir().unwrap();

    assert_eq!(
        cwd_before, cwd_after,
        "workspace init must not change the process working directory"
    );
}
