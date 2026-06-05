//! Core types for the kanban engine

mod board;
mod ids;
mod operation;
mod position;
mod short_id;

// Re-export all types
pub use board::default_column_entities;
pub use ids::{ActorId, ColumnId, OperationId, ProjectId, TagId, TaskId};
pub use operation::{is_valid_operation, Noun, Operation, Verb};
pub use position::{Ordinal, Position};
pub use short_id::{
    find_short_id_collisions, mint_unique_short_id, resolve_short_ref, short_id, ResolveResult,
    SHORT_ID_LEN,
};
