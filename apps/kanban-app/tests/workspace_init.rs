//! Integration tests for in-process board-workspace tool initialization on
//! board open.
//!
//! A board's workspace is a *set of tools*; opening a board folder installs the
//! board's `kanban` install profile rooted at the board folder. That deploys
//! every builtin skill through mirdan's one store + symlink mechanism — the
//! canonical copy of each skill lands in `<board>/.skills/`.
//! This must happen without ever shelling out to `sah init` and without mutating
//! the process working directory. Running it again must be idempotent.
//!
//! These tests exercise [`mirdan::install::init_profile`] with the same selector
//! and explicit board root the kanban app uses on board open
//! (`ensure_workspace_tools` → `deploy_workspace_tools` → `init_profile`).

use std::path::Path;

use mirdan::install::{init_profile, Profile, Selector};
use swissarmyhammer_common::lifecycle::{InitScope, InitStatus};
use swissarmyhammer_common::reporter::NullReporter;

/// Every builtin skill name, read from the same resolver the installer uses.
///
/// The board deploys all of them — there is no per-tool curation — so the
/// expectation is derived rather than transcribed, and cannot drift as builtins
/// are added or removed.
fn all_builtin_skills() -> Vec<String> {
    swissarmyhammer_skills::SkillResolver::new()
        .resolve_builtins()
        .into_keys()
        .collect()
}

/// Skills the board used to withhold, back when it deployed only the `kanban`
/// profile cluster. They must be deployed now — this is what "everything
/// deploys everywhere" means concretely.
const FORMERLY_WITHHELD_SKILLS: [&str; 4] = ["explore", "code-context", "commit", "ci"];

/// The board's install profile — every builtin skill, mirroring
/// `state::kanban_profile`. Kept as a test-local copy because the production
/// helper is private to the `kanban-app` binary.
fn kanban_profile() -> Profile {
    Profile {
        skills: Some(Selector::All),
        ..Default::default()
    }
}

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

/// Opening a fresh board folder installs the board's `kanban` profile, which
/// deploys every builtin skill into the `<board>/.skills/` store. No generic SAH
/// workspace step runs, so `.prompts/` is never created: the workspace is just
/// its tools.
#[test]
fn opening_a_board_deploys_the_kanban_tool_skills() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_board_at(tmp.path());

    let results = init_profile(
        &kanban_profile(),
        InitScope::Project,
        Some(tmp.path()),
        &NullReporter,
    );

    // No component may error.
    assert!(
        results.iter().all(|r| r.status != InitStatus::Error),
        "workspace tools init reported an error: {:?}",
        results
            .iter()
            .filter(|r| r.status == InitStatus::Error)
            .map(|r| (&r.name, &r.message))
            .collect::<Vec<_>>()
    );

    // Every builtin skill lands in the `<board>/.skills/` store, beside
    // `.kanban/`.
    let store_dir = tmp.path().join(".skills");
    assert!(store_dir.is_dir(), ".skills/ store must exist");
    let builtins = all_builtin_skills();
    assert!(
        !builtins.is_empty(),
        "the builtin resolver must report at least one skill"
    );
    for skill in &builtins {
        assert!(
            store_dir.join(skill).join("SKILL.md").is_file(),
            "builtin skill `{skill}` must be deployed at {}",
            store_dir.join(skill).join("SKILL.md").display()
        );
    }

    // The skills the old `kanban`-profile selector withheld are deployed now.
    for skill in FORMERLY_WITHHELD_SKILLS {
        assert!(
            store_dir.join(skill).join("SKILL.md").is_file(),
            "skill `{skill}` must be deployed — the board deploys every builtin skill"
        );
    }

    // No generic project-structure step runs on this path, so `.prompts/`
    // is never created — the workspace is exactly its tools.
    assert!(
        !tmp.path().join(".prompts").exists(),
        ".prompts/ must not be created — the board open path ensures tools only"
    );

    // The original `.kanban` board folder must be untouched.
    assert!(
        tmp.path()
            .join(".kanban")
            .join("boards")
            .join("board.yaml")
            .is_file(),
        "board.yaml must still exist after workspace tools init"
    );
}

/// Running the tools init twice — as happens every time a board is opened — is
/// idempotent: no error, and no duplicated skill content.
#[test]
fn repeated_board_open_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_board_at(tmp.path());

    let first = init_profile(
        &kanban_profile(),
        InitScope::Project,
        Some(tmp.path()),
        &NullReporter,
    );
    let second = init_profile(
        &kanban_profile(),
        InitScope::Project,
        Some(tmp.path()),
        &NullReporter,
    );

    assert!(
        first.iter().all(|r| r.status != InitStatus::Error),
        "first init must not error"
    );
    assert!(
        second.iter().all(|r| r.status != InitStatus::Error),
        "second init must not error — workspace tools init must be idempotent"
    );

    // The deployed skill must not be duplicated or corrupted by the re-run.
    let plan_skill = tmp.path().join(".skills").join("plan").join("SKILL.md");
    assert!(plan_skill.is_file(), "plan/SKILL.md must still exist");
    let content = std::fs::read_to_string(&plan_skill).unwrap();
    assert_eq!(
        content.matches("name: plan").count(),
        1,
        "idempotent re-init must not duplicate skill frontmatter"
    );
}

/// Workspace tools init never mutates the process working directory — it is
/// rooted purely at the explicit path argument.
#[test]
fn workspace_tools_init_does_not_mutate_process_cwd() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_board_at(tmp.path());

    let cwd_before = std::env::current_dir().unwrap();
    let _ = init_profile(
        &kanban_profile(),
        InitScope::Project,
        Some(tmp.path()),
        &NullReporter,
    );
    let cwd_after = std::env::current_dir().unwrap();

    assert_eq!(
        cwd_before, cwd_after,
        "workspace tools init must not change the process working directory"
    );
}
