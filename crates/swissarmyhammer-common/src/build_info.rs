//! Build-time provenance baked into the binary.
//!
//! The git SHA is captured by this crate's `build.rs` at compile time and
//! exposed here so every SwissArmyHammer process can log exactly which build it
//! is running. Because a running process keeps its launch-time code even after
//! the on-disk binary is rebuilt, this is the ground truth for "which code is
//! this process actually executing".

/// Short git commit SHA the binary was built from.
///
/// Has a `-dirty` suffix when the working tree had uncommitted changes at build
/// time, or is `"unknown"` when git was unavailable during the build.
pub const GIT_SHA: &str = env!("SAH_GIT_SHA");
