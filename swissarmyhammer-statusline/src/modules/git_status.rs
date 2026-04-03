//! Git status module - shows file status counts and ahead/behind info.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the git status module.
///
/// Uses git2 to count modified, staged, untracked, deleted, and conflicted files.
/// Also shows ahead/behind counts relative to the upstream tracking branch.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    try_eval(ctx).unwrap_or_else(ModuleOutput::hidden)
}

fn try_eval(ctx: &ModuleContext) -> Option<ModuleOutput> {
    let mut repo = git2::Repository::discover(".").ok()?;
    let cfg = &ctx.config.git_status;

    let mut modified = 0u32;
    let mut staged = 0u32;
    let mut untracked = 0u32;
    let mut deleted = 0u32;
    let mut conflicted = 0u32;

    // Scope the immutable borrow of `repo` via `statuses` so we can later
    // call `stash_foreach` which requires `&mut self`.
    {
        let statuses = repo.statuses(None).ok()?;
        for entry in statuses.iter() {
            let s = entry.status();
            classify_status(
                s,
                &mut modified,
                &mut staged,
                &mut untracked,
                &mut deleted,
                &mut conflicted,
            );
        }
    }

    // Stash count
    let mut stashed = 0u32;
    let _ = repo.stash_foreach(|_, _, _| {
        stashed += 1;
        true
    });

    // Ahead/behind
    let (ahead, behind) = get_ahead_behind(&repo);

    let counts = StatusCounts {
        modified,
        staged,
        untracked,
        deleted,
        conflicted,
        stashed,
        ahead,
        behind,
    };
    Some(build_status_output(&counts, cfg))
}

/// Classify a single git status entry and increment the appropriate counters.
fn classify_status(
    s: git2::Status,
    modified: &mut u32,
    staged: &mut u32,
    untracked: &mut u32,
    deleted: &mut u32,
    conflicted: &mut u32,
) {
    if s.intersects(git2::Status::WT_MODIFIED | git2::Status::WT_RENAMED) {
        *modified += 1;
    }
    if s.intersects(
        git2::Status::INDEX_NEW | git2::Status::INDEX_MODIFIED | git2::Status::INDEX_RENAMED,
    ) {
        *staged += 1;
    }
    if s.contains(git2::Status::WT_NEW) {
        *untracked += 1;
    }
    if s.intersects(git2::Status::WT_DELETED | git2::Status::INDEX_DELETED) {
        *deleted += 1;
    }
    if s.contains(git2::Status::CONFLICTED) {
        *conflicted += 1;
    }
}

/// Raw counts for git status.
struct StatusCounts {
    modified: u32,
    staged: u32,
    untracked: u32,
    deleted: u32,
    conflicted: u32,
    stashed: u32,
    ahead: usize,
    behind: usize,
}

/// Build the styled module output from raw status counts.
fn build_status_output(
    counts: &StatusCounts,
    cfg: &crate::config::GitStatusModuleConfig,
) -> ModuleOutput {
    // Helper: format a symbol with an optional count
    let fmt = |symbol: &str, count: u32| -> String {
        if cfg.show_counts {
            format!("{}{}", symbol, count)
        } else {
            symbol.to_string()
        }
    };

    // Build all_status string — pack symbols tight when counts are off (like Starship),
    // space-separate when counts are on for readability.
    let mut all_status = String::new();
    let mut push_part = |part: String| {
        if cfg.show_counts && !all_status.is_empty() {
            all_status.push(' ');
        }
        all_status.push_str(&part);
    };
    if counts.modified > 0 {
        push_part(fmt(&cfg.modified, counts.modified));
    }
    if counts.staged > 0 {
        push_part(fmt(&cfg.staged, counts.staged));
    }
    if counts.untracked > 0 {
        push_part(fmt(&cfg.untracked, counts.untracked));
    }
    if counts.deleted > 0 {
        push_part(fmt(&cfg.deleted, counts.deleted));
    }
    if counts.conflicted > 0 {
        push_part(fmt(&cfg.conflicted, counts.conflicted));
    }
    if counts.stashed > 0 {
        push_part(fmt(&cfg.stashed, counts.stashed));
    }

    // Build ahead_behind string
    let mut ahead_behind = String::new();
    if counts.ahead > 0 && counts.behind > 0 {
        ahead_behind = fmt(&cfg.diverged, counts.ahead as u32);
    } else {
        if counts.ahead > 0 {
            ahead_behind.push_str(&fmt(&cfg.ahead, counts.ahead as u32));
        }
        if counts.behind > 0 {
            ahead_behind.push_str(&fmt(&cfg.behind, counts.behind as u32));
        }
    }

    // If nothing to show, hide
    if all_status.is_empty() && ahead_behind.is_empty() {
        return ModuleOutput::hidden();
    }

    // Add space separator if both parts present
    if !all_status.is_empty() && !ahead_behind.is_empty() {
        ahead_behind = format!(" {}", ahead_behind);
    }

    let mut vars = HashMap::new();
    vars.insert("all_status".into(), all_status);
    vars.insert("ahead_behind".into(), ahead_behind);
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(&cfg.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    fn default_cfg() -> crate::config::GitStatusModuleConfig {
        StatuslineConfig::default().git_status
    }

    #[test]
    fn test_git_status_in_repo() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let _out = eval(&ctx);
    }

    #[test]
    fn test_git_status_with_counts() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.git_status.show_counts = true;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let _out = eval(&ctx);
    }

    #[test]
    fn test_build_status_modified_only() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 3,
            staged: 0,
            untracked: 0,
            deleted: 0,
            conflicted: 0,
            stashed: 0,
            ahead: 0,
            behind: 0,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("!"));
    }

    #[test]
    fn test_build_status_all_types() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 1,
            staged: 2,
            untracked: 3,
            deleted: 1,
            conflicted: 1,
            stashed: 1,
            ahead: 0,
            behind: 0,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("!"));
        assert!(out.text.contains("+"));
        assert!(out.text.contains("?"));
    }

    #[test]
    fn test_build_status_with_counts_enabled() {
        let mut cfg = default_cfg();
        cfg.show_counts = true;
        let counts = StatusCounts {
            modified: 3,
            staged: 2,
            untracked: 5,
            deleted: 1,
            conflicted: 0,
            stashed: 4,
            ahead: 0,
            behind: 0,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(out.text.contains("!3"));
        assert!(out.text.contains("+2"));
        assert!(out.text.contains("?5"));
        assert!(out.text.contains("$4"));
    }

    #[test]
    fn test_build_status_empty_hidden() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 0,
            staged: 0,
            untracked: 0,
            deleted: 0,
            conflicted: 0,
            stashed: 0,
            ahead: 0,
            behind: 0,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(out.is_empty());
    }

    #[test]
    fn test_build_status_ahead_only() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 0,
            staged: 0,
            untracked: 0,
            deleted: 0,
            conflicted: 0,
            stashed: 0,
            ahead: 3,
            behind: 0,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_build_status_behind_only() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 0,
            staged: 0,
            untracked: 0,
            deleted: 0,
            conflicted: 0,
            stashed: 0,
            ahead: 0,
            behind: 2,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_build_status_diverged() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 0,
            staged: 0,
            untracked: 0,
            deleted: 0,
            conflicted: 0,
            stashed: 0,
            ahead: 2,
            behind: 3,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_build_status_mixed_status_and_ahead() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 1,
            staged: 0,
            untracked: 0,
            deleted: 0,
            conflicted: 0,
            stashed: 0,
            ahead: 1,
            behind: 0,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_build_status_conflicted_and_deleted() {
        let cfg = default_cfg();
        let counts = StatusCounts {
            modified: 0,
            staged: 0,
            untracked: 0,
            deleted: 2,
            conflicted: 1,
            stashed: 0,
            ahead: 0,
            behind: 0,
        };
        let out = build_status_output(&counts, &cfg);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_classify_modified() {
        let (mut m, mut s, mut u, mut d, mut c) = (0, 0, 0, 0, 0);
        classify_status(
            git2::Status::WT_MODIFIED,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(m, 1);
        assert_eq!((s, u, d, c), (0, 0, 0, 0));
    }

    #[test]
    fn test_classify_renamed() {
        let (mut m, mut s, mut u, mut d, mut c) = (0, 0, 0, 0, 0);
        classify_status(
            git2::Status::WT_RENAMED,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(m, 1);
    }

    #[test]
    fn test_classify_staged() {
        let (mut m, mut s, mut u, mut d, mut c) = (0, 0, 0, 0, 0);
        classify_status(
            git2::Status::INDEX_NEW,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(s, 1);
        classify_status(
            git2::Status::INDEX_MODIFIED,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(s, 2);
        classify_status(
            git2::Status::INDEX_RENAMED,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(s, 3);
    }

    #[test]
    fn test_classify_untracked() {
        let (mut m, mut s, mut u, mut d, mut c) = (0, 0, 0, 0, 0);
        classify_status(git2::Status::WT_NEW, &mut m, &mut s, &mut u, &mut d, &mut c);
        assert_eq!(u, 1);
    }

    #[test]
    fn test_classify_deleted() {
        let (mut m, mut s, mut u, mut d, mut c) = (0, 0, 0, 0, 0);
        classify_status(
            git2::Status::WT_DELETED,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(d, 1);
        classify_status(
            git2::Status::INDEX_DELETED,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(d, 2);
    }

    #[test]
    fn test_classify_conflicted() {
        let (mut m, mut s, mut u, mut d, mut c) = (0, 0, 0, 0, 0);
        classify_status(
            git2::Status::CONFLICTED,
            &mut m,
            &mut s,
            &mut u,
            &mut d,
            &mut c,
        );
        assert_eq!(c, 1);
    }

    #[test]
    fn test_get_ahead_behind_in_repo() {
        let repo = git2::Repository::discover(".").unwrap();
        let (ahead, behind) = get_ahead_behind(&repo);
        // Just verify it doesn't crash; actual values depend on repo state
        let _ = (ahead, behind);
    }
}

/// Get ahead/behind counts relative to the upstream tracking branch.
fn get_ahead_behind(repo: &git2::Repository) -> (usize, usize) {
    try_ahead_behind(repo).unwrap_or((0, 0))
}

fn try_ahead_behind(repo: &git2::Repository) -> Option<(usize, usize)> {
    let head = repo.head().ok()?;
    let local_oid = head.target()?;
    let branch_name = head.shorthand()?;
    let upstream_ref = format!("refs/remotes/origin/{}", branch_name);
    let upstream_oid = repo.refname_to_id(&upstream_ref).ok()?;
    repo.graph_ahead_behind(local_oid, upstream_oid).ok()
}
