//! Turn state tracking for file change detection.
//!
//! This module tracks files that change during a turn (between user prompt and Stop hook)
//! by hashing files before and after tool execution.
//!
//! ## Concurrency Safety
//!
//! This module uses file-based locking to ensure safe concurrent access to turn state.
//! When multiple hook processes attempt to modify the same state file simultaneously,
//! they acquire an exclusive lock on a separate `.lock` file to prevent race conditions.

mod diff;
mod hash;
mod paths;
mod state;

pub use diff::{
    compute_diff, format_diffs_fenced, prepare_validator_context, render_hook_context, FileDiff,
    DIFF_TEXT_KEY,
};
pub use hash::{hash_bytes, hash_file, hash_files};
pub use paths::{extract_tool_paths, is_known_file_tool};
pub use state::{TurnState, TurnStateManager};
