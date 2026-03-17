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

pub use diff::{compute_diff, format_diffs_fenced, FileDiff};
pub use hash::{hash_file, hash_files};
pub use paths::{extract_paths, extract_tool_paths};
pub use state::{TurnState, TurnStateManager};
