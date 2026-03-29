//! Command trait, registry types, and dispatch context for SwissArmyHammer.
//!
//! This crate defines the `Command` trait (available + execute), the
//! `CommandContext` for scope chain resolution and argument access, and
//! the `CommandDef` / `CommandInvocation` wire types for YAML-loaded
//! command metadata.
//!
//! It is consumer-agnostic -- it knows nothing about kanban, tasks, or
//! specific entity types. Consumers implement `Command` for their domain
//! operations and register them with a `CommandsRegistry` (defined in a
//! later card).

pub mod command;
pub mod context;
pub mod error;
pub mod registry;
pub mod types;
pub mod ui_state;

pub use command::Command;
pub use context::{parse_moniker, CommandContext};
pub use error::{CommandError, Result};
pub use registry::{builtin_yaml_sources, load_yaml_dir, CommandsRegistry};
pub use types::{CommandDef, CommandInvocation, KeysDef, ParamDef, ParamSource};
pub use ui_state::{ClipboardEntry, DragSession, RecentBoard, UIState, UIStateChange, WindowState};
