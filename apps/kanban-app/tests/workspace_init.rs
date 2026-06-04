//! Integration tests for in-process board-workspace tool initialization on
//! board open.
//!
//! A board's workspace is a *set of tools*; opening a board folder installs the
//! board's `kanban` install profile rooted at the board folder. That deploys the
//! `kanban`-profile builtin skills through mirdan's one store + symlink
//! mechanism — the canonical copy of each skill lands in `<board>/.skills/`.
//! This must happen without ever shelling out to `sah init` and without mutating
//! the process working directory. Running it again must be idempotent.
//!
//! These tests exercise [`mirdan::install::init_profile`] with the same
//! `kanban`-profile selector and explicit board root the kanban app uses on
//! board open (`ensure_workspace_tools` → `deploy_workspace_tools` →
//! `init_profile`).

use std::path::Path;

use mirdan::install::{init_profile, Profile, Selector};
use swissarmyhammer_common::lifecycle::{InitScope, InitStatus};
use swissarmyhammer_common::reporter::NullReporter;

/// The six skills the kanban tool's init deploys (its profile cluster).
const KANBAN_PROFILE_SKILLS: [&str; 6] =
    ["kanban", "plan", "task", "finish", "implement", "review"];

/// Skills that must NOT be deployed by the kanban tool: `explore` and
/// `code-context` belong to the `code-context` profile; `commit` is untagged.
const NON_KANBAN_SKILLS: [&str; 3] = ["explore", "code-context", "commit"];

/// The board's install profile — the `kanban`-tagged builtin skills only,
/// mirroring `state::kanban_profile`. Kept as a test-local copy because the
/// production helper is private to the `kanban-app` binary.
fn kanban_profile() -> Profile {
    Profile {
        skills: Some(Selector::Profile("kanban".to_string())),
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
/// deploys exactly the `kanban`-profile skills into the `<board>/.skills/`
/// store. No generic SAH workspace step runs, so `.prompts/` is never created:
/// the workspace is just its tools.
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

    // The kanban tool's skills land in the `<board>/.skills/` store, beside
    // `.kanban/`.
    let store_dir = tmp.path().join(".skills");
    assert!(store_dir.is_dir(), ".skills/ store must exist");
    for skill in KANBAN_PROFILE_SKILLS {
        assert!(
            store_dir.join(skill).join("SKILL.md").is_file(),
            "kanban-tool skill `{skill}` must be deployed at {}",
            store_dir.join(skill).join("SKILL.md").display()
        );
    }

    // Skills in other profiles (`explore`, `code-context`) and untagged
    // builtins (`commit`) are not part of the kanban tool.
    for skill in NON_KANBAN_SKILLS {
        assert!(
            !store_dir.join(skill).exists(),
            "skill `{skill}` is not in the kanban tool and must not be deployed"
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
