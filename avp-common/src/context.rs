//! AVP Context - Manages .avp directory and logging.
//!
//! The `.avp` directory is created at the git repository root and contains:
//! - `avp.log` - Append-only log of hook events
//! - `.gitignore` - Excludes log files from version control

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use swissarmyhammer_common::utils::directory_utils::find_git_repository_root;

use crate::error::AvpError;

/// Directory name for AVP state and logs.
const AVP_DIR_NAME: &str = ".avp";

/// Log file name within .avp directory.
const LOG_FILE_NAME: &str = "avp.log";

/// Content for .gitignore file in .avp directory.
const GITIGNORE_CONTENT: &str = r#"# AVP logs and state
*.log
"#;

/// Decision outcome for a hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Hook allowed the action to proceed.
    Allow,
    /// Hook blocked the action.
    Block,
    /// Hook encountered an error.
    Error,
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Allow => write!(f, "allow"),
            Decision::Block => write!(f, "block"),
            Decision::Error => write!(f, "error"),
        }
    }
}

/// A hook event to log.
#[derive(Debug)]
pub struct HookEvent<'a> {
    /// The hook type (e.g., "PreToolUse", "PostToolUse").
    pub hook_type: &'a str,
    /// The decision outcome.
    pub decision: Decision,
    /// Optional details (tool name, reason, etc.).
    pub details: Option<String>,
}

/// AVP Context - manages .avp directory and logging.
///
/// All .avp directory logic is centralized here. The directory is created
/// at the git repository root (found via swissarmyhammer-common).
#[derive(Debug)]
pub struct AvpContext {
    /// Path to the .avp directory.
    avp_dir: PathBuf,
    /// Shared log file handle (None if logging failed to initialize).
    log_file: Option<Arc<Mutex<File>>>,
}

impl AvpContext {
    /// Initialize AVP context by finding git root and creating .avp directory.
    ///
    /// This will:
    /// 1. Find git repository root (via swissarmyhammer-common)
    /// 2. Create .avp directory at git root if it doesn't exist
    /// 3. Create .gitignore in .avp if it doesn't exist
    /// 4. Open log file for appending
    ///
    /// Returns Err if not in a git repository.
    pub fn init() -> Result<Self, AvpError> {
        // Use swissarmyhammer-common to find git root
        let git_root = find_git_repository_root().ok_or_else(|| {
            AvpError::Context("not in a git repository (no .git found)".to_string())
        })?;

        let avp_dir = git_root.join(AVP_DIR_NAME);

        // Create .avp directory if needed
        if !avp_dir.exists() {
            fs::create_dir_all(&avp_dir).map_err(AvpError::Io)?;
        }

        // Create .gitignore if needed (best-effort)
        let gitignore_path = avp_dir.join(".gitignore");
        if !gitignore_path.exists() {
            let _ = fs::write(&gitignore_path, GITIGNORE_CONTENT);
        }

        // Open log file for appending (best-effort)
        let log_file = open_log_file(&avp_dir);

        Ok(Self { avp_dir, log_file })
    }

    /// Get the .avp directory path.
    pub fn avp_dir(&self) -> &Path {
        &self.avp_dir
    }

    /// Log a hook event.
    ///
    /// Format: `2024-01-23T10:15:32.123Z PreToolUse decision=allow tool=Bash`
    pub fn log_event(&self, event: &HookEvent) {
        let Some(log_file) = &self.log_file else {
            return;
        };

        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let details_str = event
            .details
            .as_ref()
            .map(|d| format!(" {}", d))
            .unwrap_or_default();

        let line = format!(
            "{} {} decision={}{}\n",
            timestamp, event.hook_type, event.decision, details_str
        );

        if let Ok(mut file) = log_file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

/// Open log file for appending.
fn open_log_file(avp_dir: &Path) -> Option<Arc<Mutex<File>>> {
    let log_path = avp_dir.join(LOG_FILE_NAME);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(|f| Arc::new(Mutex::new(f)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_display() {
        assert_eq!(format!("{}", Decision::Allow), "allow");
        assert_eq!(format!("{}", Decision::Block), "block");
        assert_eq!(format!("{}", Decision::Error), "error");
    }
}
