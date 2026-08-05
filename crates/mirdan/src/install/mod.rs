//! Mirdan Install/Uninstall - Type-aware package deployment.
//!
//! Skills -> agent skill directories (one copy per detected agent)
//! Validators -> ./.validators/ (project) or ~/.validators/ (global)
//! Tools -> .tools/ store + agent MCP config files
//! Plugins -> agent plugin directories (e.g. .claude/plugins/)
//!
//! The module divides by concern. [`package`] installs packages from the
//! registry, a local path, or a git source. [`deploy`] writes each package
//! type to its targets. [`profile`] applies declarative profiles.
//! [`uninstall`] removes installed packages. [`applier`] applies
//! agent-config changes across detected agents. This file holds the
//! shared path and filesystem helpers.

mod applier;
mod deploy;
mod package;
mod profile;
mod uninstall;

pub use applier::*;
pub use deploy::*;
pub use package::*;
pub use profile::*;
pub use uninstall::*;

#[cfg(test)]
mod edit_redirect_tests;
#[cfg(test)]
mod profile_consistency_tests;
#[cfg(test)]
mod profile_tests;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use crate::registry::RegistryError;
use crate::store;

/// Sanitize a package name for use as a filesystem directory name.
///
/// Delegates to [`store::sanitize_dir_name`].
fn sanitize_dir_name(name: &str) -> String {
    store::sanitize_dir_name(name)
}

/// Sanitize a package name and reject any result that could escape a target
/// directory.
///
/// Runs [`sanitize_dir_name`], then validates the result with
/// [`store::is_safe_relative_path`], which rejects parent-directory
/// references (`..`), backslashes, absolute paths, and empty segments.
/// Multi-segment results (e.g. `owner/repo/skill` from a URL-derived name)
/// stay accepted because store entries deploy to nested paths.
///
/// The single name-validation step behind every path-building site in
/// [`uninstall`].
fn safe_dir_name(name: &str) -> Result<String, RegistryError> {
    let sanitized = sanitize_dir_name(name);
    if !store::is_safe_relative_path(&sanitized) {
        return Err(RegistryError::Validation(format!(
            "unsafe package name: {name:?}"
        )));
    }
    Ok(sanitized)
}

/// Resolve a project-scope relative path against an explicit `root`.
///
/// Returns `path` unchanged in global scope (its paths are already absolute) or
/// when no `root` is supplied (CWD-relative behavior). Otherwise joins the
/// relative `path` onto `root`, so deployment never reads `current_dir()`.
fn rooted(root: Option<&Path>, global: bool, path: impl Into<PathBuf>) -> PathBuf {
    let path = path.into();
    match root {
        Some(root) if !global => root.join(path),
        _ => path,
    }
}

/// Get the validators directory path.
///
/// Delegates to [`store::validators_store_dir`] so validators use the same
/// home-dotfile store convention as skills/agents/tools: `~/.validators/`
/// (global) and `./.validators/` (project).
pub fn validators_dir(global: bool) -> PathBuf {
    store::validators_store_dir(global)
}

pub(crate) use crate::store::copy_dir_recursive;

/// Remove now-empty directories walking up from `start` toward `boundary`.
///
/// Starting at `start`, removes the directory if it is empty, then repeats for
/// its parent, climbing the ancestry chain. The walk stops at (and never
/// removes) `boundary` itself, anything above it, or the first directory that is
/// not empty. `std::fs::remove_dir` fails on a non-empty directory, which is the
/// guard that preserves any user-authored files: a directory that still holds
/// content is left intact and halts the climb.
///
/// This generalizes empty-dir cleanup to arbitrary nesting depth, so a builtin
/// set whose embedded files live more than one subdirectory deep does not leave
/// empty intermediate directories behind. `start` must be a descendant of
/// `boundary`; if it is not, the function is a no-op.
pub(crate) fn remove_empty_dirs_up_to(start: &Path, boundary: &Path) {
    let mut current = start.to_path_buf();
    while current.starts_with(boundary) && current != *boundary {
        // `remove_dir` only succeeds on an empty directory; a non-empty dir
        // (user files present) errors out and stops the climb.
        if std::fs::remove_dir(&current).is_err() {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
}
