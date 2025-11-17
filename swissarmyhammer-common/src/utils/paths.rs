//! Path utilities for SwissArmyHammer

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
    let swissarmyhammer_dir = current_dir.join(".swissarmyhammer");

    if !swissarmyhammer_dir.exists() {
        std::fs::create_dir_all(&swissarmyhammer_dir)?;
    }

    Ok(swissarmyhammer_dir)
}
