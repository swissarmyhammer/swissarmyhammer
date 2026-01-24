//! Path utilities for SwissArmyHammer

use crate::directory::SwissarmyhammerDirectory;
use std::path::PathBuf;

/// Get the SwissArmyHammer directory (.swissarmyhammer)
///
/// This function creates a simple .swissarmyhammer directory in the current working directory
/// if it doesn't exist. This is a simplified version for use in the memoranda crate.
///
/// # Deprecated
///
/// Use `SwissarmyhammerDirectory::from_git_root()` instead for Git-aware directory resolution.
#[deprecated(
    since = "0.3.0",
    note = "Use SwissarmyhammerDirectory::from_git_root() instead"
)]
pub fn get_swissarmyhammer_dir() -> Result<PathBuf, std::io::Error> {
    let current_dir = std::env::current_dir()?;
    SwissarmyhammerDirectory::from_custom_root(current_dir)
        .map(|dir| dir.root().to_path_buf())
        .map_err(|e| std::io::Error::other(e.to_string()))
}
