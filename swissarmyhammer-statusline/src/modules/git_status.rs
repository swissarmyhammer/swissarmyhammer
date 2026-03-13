//! Git status module - shows file status counts and ahead/behind info.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the git status module.
///
/// Uses git2 to count modified, staged, untracked, deleted, and conflicted files.
/// Also shows ahead/behind counts relative to the upstream tracking branch.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let mut repo = match git2::Repository::discover(".") {
        Ok(r) => r,
        Err(_) => return ModuleOutput::hidden(),
    };

    let cfg = &ctx.config.git_status;

    let mut modified = 0u32;
    let mut staged = 0u32;
    let mut untracked = 0u32;
    let mut deleted = 0u32;
    let mut conflicted = 0u32;

    // Scope the immutable borrow of `repo` via `statuses` so we can later
    // call `stash_foreach` which requires `&mut self`.
    {
        let statuses = match repo.statuses(None) {
            Ok(s) => s,
            Err(_) => return ModuleOutput::hidden(),
        };

        for entry in statuses.iter() {
            let s = entry.status();
            if s.intersects(git2::Status::WT_MODIFIED | git2::Status::WT_RENAMED) {
                modified += 1;
            }
            if s.intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_RENAMED,
            ) {
                staged += 1;
            }
            if s.contains(git2::Status::WT_NEW) {
                untracked += 1;
            }
            if s.intersects(git2::Status::WT_DELETED | git2::Status::INDEX_DELETED) {
                deleted += 1;
            }
            if s.contains(git2::Status::CONFLICTED) {
                conflicted += 1;
            }
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
    if modified > 0 {
        push_part(fmt(&cfg.modified, modified));
    }
    if staged > 0 {
        push_part(fmt(&cfg.staged, staged));
    }
    if untracked > 0 {
        push_part(fmt(&cfg.untracked, untracked));
    }
    if deleted > 0 {
        push_part(fmt(&cfg.deleted, deleted));
    }
    if conflicted > 0 {
        push_part(fmt(&cfg.conflicted, conflicted));
    }
    if stashed > 0 {
        push_part(fmt(&cfg.stashed, stashed));
    }

    // Build ahead_behind string
    let mut ahead_behind = String::new();
    if ahead > 0 && behind > 0 {
        ahead_behind = fmt(&cfg.diverged, ahead as u32);
    } else {
        if ahead > 0 {
            ahead_behind.push_str(&fmt(&cfg.ahead, ahead as u32));
        }
        if behind > 0 {
            ahead_behind.push_str(&fmt(&cfg.behind, behind as u32));
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

/// Get ahead/behind counts relative to the upstream tracking branch.
fn get_ahead_behind(repo: &git2::Repository) -> (usize, usize) {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return (0, 0),
    };

    let local_oid = match head.target() {
        Some(oid) => oid,
        None => return (0, 0),
    };

    let branch_name = match head.shorthand() {
        Some(name) => name.to_string(),
        None => return (0, 0),
    };

    let upstream_ref = format!("refs/remotes/origin/{}", branch_name);
    let upstream_oid = match repo.refname_to_id(&upstream_ref) {
        Ok(oid) => oid,
        Err(_) => return (0, 0),
    };

    repo.graph_ahead_behind(local_oid, upstream_oid)
        .unwrap_or((0, 0))
}
