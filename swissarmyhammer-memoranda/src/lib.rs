//! SwissArmyHammer Memoranda Crate
//!
//! A dedicated crate for memo management providing simple CRUD operations
//! for markdown-based memos with title-based identifiers.

pub mod types;
pub mod error;
pub mod storage;
pub mod operations;

// Re-export main types
pub use types::*;
pub use error::*;
pub use storage::*;
pub use operations::*;