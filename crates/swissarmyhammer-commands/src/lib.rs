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
pub mod macros;
pub mod options_resolver;
pub mod registry;
pub mod types;
pub mod window_info;

pub use command::Command;
pub use context::{parse_moniker, CommandContext};
pub use error::{CommandError, Result};
pub use options_resolver::{
    register_command_resolvers, OptionsContext, OptionsRegistry, OptionsResolver, OptionsSources,
    SortDirectionsResolver,
};
pub use registry::{builtin_yaml_sources, load_yaml_dir, CommandsRegistry};
pub use types::{
    CommandDef, CommandInvocation, KeysDef, MenuPlacement, ParamDef, ParamOption, ParamShape,
    ParamSource, TabButtonDef,
};
pub use window_info::WindowInfo;
