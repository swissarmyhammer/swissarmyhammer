//! Test support shared by the kanban integration test binaries.
//!
//! Each file in `tests/` is its own crate. Include this module with
//! `mod common;`.

/// Number of commands in the composed registry: the generic commands-crate
/// builtins plus the kanban-crate builtins. Change this one value when a
/// command is added or removed.
pub const EXPECTED_COMPOSED_COMMAND_COUNT: usize = 63;
