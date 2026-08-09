//! Resolve a caller-named path inside the workspace, or refuse it.
//!
//! Every op that opens a file the caller named resolves it here. A file path
//! is an ordinary parameter of the MCP op and of `sah tool`, so
//! `../../../etc/passwd` and `/etc/passwd` both arrive the way any other path
//! does; joining one to the workspace root reads exactly the file it names.
//! Containment is what makes the workspace root a boundary instead of a
//! suggestion.

use std::path::{Path, PathBuf};

/// The absolute path `file` names inside `working_dir`, or `None` when it
/// escapes that directory or cannot be resolved.
///
/// Both sides are canonicalized before the comparison, so a `..` component, a
/// symbolic link that points out of the tree, and an absolute path elsewhere
/// on disk are each refused. An absolute path that lands inside `working_dir`
/// is kept: the boundary is where the path resolves, not how the caller
/// spelled it.
///
/// A path that names no existing file resolves to `None` as well, which is the
/// silence every caller already keeps for a file it cannot read.
///
/// A refusal is logged at `warn`, because a caller reaching outside the
/// workspace is worth seeing.
pub fn resolve_within(working_dir: &Path, file: impl AsRef<Path>) -> Option<PathBuf> {
    let file = file.as_ref();
    let root = std::fs::canonicalize(working_dir).ok()?;
    let resolved = std::fs::canonicalize(root.join(file)).ok()?;

    if resolved.starts_with(&root) {
        return Some(resolved);
    }

    tracing::warn!(
        path = %file.display(),
        workspace = %root.display(),
        "refused a file path that resolves outside the workspace"
    );
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace directory holding `inside.txt`, beside an `outside.txt`
    /// the workspace must not reach.
    fn workspace_beside_an_outside_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("create a scratch directory");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir(&workspace).expect("create the workspace directory");
        std::fs::write(workspace.join("inside.txt"), "inside").expect("write the inside file");
        std::fs::write(dir.path().join("outside.txt"), "outside").expect("write the outside file");
        (dir, workspace)
    }

    #[test]
    fn a_relative_path_inside_the_workspace_resolves() {
        let (_dir, workspace) = workspace_beside_an_outside_file();

        let resolved = resolve_within(&workspace, "inside.txt").expect("the file is inside");

        assert!(resolved.ends_with("inside.txt"));
    }

    #[test]
    fn an_absolute_path_inside_the_workspace_resolves() {
        let (_dir, workspace) = workspace_beside_an_outside_file();

        assert!(resolve_within(&workspace, workspace.join("inside.txt")).is_some());
    }

    #[test]
    fn a_relative_path_that_climbs_out_of_the_workspace_is_refused() {
        let (_dir, workspace) = workspace_beside_an_outside_file();

        assert_eq!(resolve_within(&workspace, "../outside.txt"), None);
    }

    #[test]
    fn an_absolute_path_outside_the_workspace_is_refused() {
        let (dir, workspace) = workspace_beside_an_outside_file();

        assert_eq!(
            resolve_within(&workspace, dir.path().join("outside.txt")),
            None
        );
    }

    #[test]
    fn a_climb_that_returns_inside_the_workspace_resolves() {
        let (_dir, workspace) = workspace_beside_an_outside_file();

        assert!(resolve_within(&workspace, "../workspace/inside.txt").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_that_points_out_of_the_workspace_is_refused() {
        let (dir, workspace) = workspace_beside_an_outside_file();
        std::os::unix::fs::symlink(dir.path().join("outside.txt"), workspace.join("link.txt"))
            .expect("link to the outside file");

        assert_eq!(resolve_within(&workspace, "link.txt"), None);
    }

    #[test]
    fn a_path_that_names_no_file_resolves_to_nothing() {
        let (_dir, workspace) = workspace_beside_an_outside_file();

        assert_eq!(resolve_within(&workspace, "gone.txt"), None);
    }
}
