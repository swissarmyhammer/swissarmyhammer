//! CI gate: the deleted `swissarmyhammer_commands` crate must not reappear.
//!
//! Stage 4 of the kanban cut-over deleted `swissarmyhammer-commands`
//! and inlined its contents into `swissarmyhammer-kanban::commands_core`.
//! This test grep-scans the live source tree (excluding the markdown
//! notes under `.kanban/`, `ideas/`, and `doc/`) for stray
//! `swissarmyhammer_commands::` paths so a future refactor cannot
//! silently bring the crate back via cargo path.
//!
//! Add new exclusions sparingly — the whole point of this gate is to
//! force the conversation when something tries to re-import the dead
//! crate.

use std::path::{Path, PathBuf};

/// Resolve the workspace root from this crate's manifest dir
/// (two `..` hops from `crates/swissarmyhammer-command-service`).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest dir")
        .to_path_buf()
}

/// Directories whose contents are exempt from the scan. Each is a
/// path relative to the workspace root.
const EXEMPT_DIRS: &[&str] = &[".kanban", "ideas", "doc", "target", "node_modules"];

fn is_exempt(path: &Path, root: &Path) -> bool {
    let rel = match path.strip_prefix(root) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let first = match rel.components().next() {
        Some(c) => c.as_os_str().to_string_lossy().into_owned(),
        None => return false,
    };
    EXEMPT_DIRS.iter().any(|d| *d == first.as_str())
}

/// Walk every `.rs` and `.toml` file under the workspace and assert none
/// contains a live `swissarmyhammer_commands::` or `swissarmyhammer-commands`
/// reference.
#[test]
fn no_stale_swissarmyhammer_commands_references() {
    let root = workspace_root();
    let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();

    let mut stack: Vec<PathBuf> = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if is_exempt(&path, &root) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "toml") {
                continue;
            }
            // Exempt this very test file — its source contains the literals
            // it's grep-scanning for, and any test that fails its own
            // assertion is a tautological gate, not a useful one.
            if path.ends_with("no_stale_imports.rs") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (lineno, line) in content.lines().enumerate() {
                // The `::` path form catches Rust use-sites; the bare crate
                // name catches Cargo.toml `[dependencies]` entries.
                if line.contains("swissarmyhammer_commands::")
                    || line.contains("swissarmyhammer-commands")
                {
                    // Allow comments / docstrings that mention the deleted
                    // crate by name when explaining the cut-over — they're
                    // not live imports. The heuristic: a line whose first
                    // non-whitespace prefix is `//`, `#`, or `*` is a
                    // comment / doc-comment / block-comment continuation.
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//")
                        || trimmed.starts_with('#')
                        || trimmed.starts_with('*')
                    {
                        continue;
                    }
                    offenders.push((path.clone(), lineno + 1, line.to_string()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "found {} live references to the deleted `swissarmyhammer-commands` crate:\n{}",
        offenders.len(),
        offenders
            .iter()
            .map(|(p, l, line)| format!(
                "  {}:{}: {}",
                p.strip_prefix(&root).unwrap_or(p).display(),
                l,
                line.trim()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
