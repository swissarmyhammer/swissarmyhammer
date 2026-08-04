//! Regression guard for ^d525k4k: no generated skill/agent deploy artifact is
//! committed under `apps/`.
//!
//! `.skills/` (and its per-agent `.claude/skills/`, `.zed/skills/`, `.agents/`
//! siblings) is a runtime deploy target written by
//! `mirdan::install::init_profile` / `stage_and_deploy_skill`. Its destination
//! is always the process working directory (project scope) or the user's home
//! (global scope) -- never a path this repository controls. A copy left
//! behind inside `apps/<tool>-cli/` after someone ran `<tool> init` /
//! `<tool> skill` from that directory is not a source file:
//!
//! - nothing regenerates it (`build.rs` for the tool CLIs only emits docs,
//!   man pages, and shell completions),
//! - nothing reads it at runtime (`SkillResolver` only ever resolves
//!   `{git_root}/.skills`, and the git root for every crate in this
//!   workspace -- including the `apps/*-cli` crates -- is the repository
//!   root, never an `apps/*` subdirectory), and
//! - it silently drifts from its `builtin/skills/<name>/SKILL.md` source, as
//!   happened to `apps/kanban-cli/.skills/kanban/SKILL.md`,
//!   `apps/code-context-cli/.skills/code-context/SKILL.md`, and
//!   `apps/code-context-cli/.skills/lsp/SKILL.md` (found while ^3y5n9g6 fixed
//!   the `builtin/` source and could not touch the deployed copies).
//!
//! This walks `git ls-files` (not the filesystem) so it passes for a
//! developer who has locally run `<tool> init`/`<tool> skill` inside an
//! `apps/*-cli/` directory -- `.gitignore` keeps that output untracked -- and
//! only fails if such an artifact is actually committed, including via a
//! future `git add -f` that gets past `.gitignore`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Repository root, derived from the crate manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("swissarmyhammer-skills must live inside the workspace")
        .to_path_buf()
}

#[test]
fn no_generated_skill_deploy_artifacts_tracked_under_apps() {
    let root = repo_root();
    if !root.join(".git").exists() {
        // Not a git checkout (e.g. a packaged source tarball) -- nothing to
        // check against.
        return;
    }

    let output = Command::new("git")
        .args(["ls-files", "apps"])
        .current_dir(&root)
        .output()
        .expect("git ls-files should run");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tracked = String::from_utf8_lossy(&output.stdout);
    let offenders: Vec<&str> = tracked
        .lines()
        .filter(|path| {
            path.contains("/.skills/")
                || path.contains("/.agents/")
                || path.contains("/.claude/skills/")
                || path.contains("/.claude/agents/")
                || path.contains("/.zed/skills/")
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "generated skill/agent deploy artifacts must not be committed under apps/ \
         (the source of truth is builtin/skills/; these paths are runtime deploy \
         output that drifts silently -- see ^d525k4k):\n  {}",
        offenders.join("\n  ")
    );
}
