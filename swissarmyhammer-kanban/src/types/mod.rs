//! Core types for the kanban engine

mod board;
mod ids;
mod log;
mod operation;
mod position;
mod task;

// Re-export all types
pub use board::{Actor, Board, Column, Swimlane, Tag};
pub use ids::{
    ActorId, AttachmentId, ColumnId, CommentId, LogEntryId, SubtaskId, SwimlaneId, TagId, TaskId,
};
pub use log::{LogEntry, OperationResult};
pub use operation::{is_valid_operation, Noun, Operation, Verb};
pub use position::{Ordinal, Position};
pub use task::{Attachment, Comment, Subtask, Task};
