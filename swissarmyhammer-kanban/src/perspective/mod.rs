//! Perspective types and CRUD operations for saved view configurations.
//!
//! A perspective is a named, ordered list of fields with per-field overrides,
//! plus optional filter/group functions and sort entries. Perspectives are
//! stored as YAML and reference fields by ULID.
//!
//! Domain types (`Perspective`, `PerspectiveContext`, `PerspectiveChangelog`, etc.)
//! are owned by the `swissarmyhammer-perspectives` crate and re-exported here
//! so that downstream code can keep using `crate::perspective::*`.

pub mod add;
pub mod delete;
pub mod get;
pub mod list;
pub mod update;

// Re-export domain types from the standalone perspectives crate
pub use swissarmyhammer_perspectives::{
    Perspective, PerspectiveChangeEntry, PerspectiveChangeOp, PerspectiveChangelog,
    PerspectiveContext, PerspectiveError, PerspectiveFieldEntry, SortDirection, SortEntry,
};

pub use add::AddPerspective;
pub use delete::DeletePerspective;
pub use get::GetPerspective;
pub use list::ListPerspectives;
pub use update::UpdatePerspective;

#[cfg(test)]
mod tests;
