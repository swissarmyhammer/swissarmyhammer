//! Command trait, registry types, and dispatch context.
//!
//! Inlined into `swissarmyhammer-kanban` during the Stage 4 cut-over after the
//! standalone `swissarmyhammer-commands` crate was deleted. The module is
//! consumer-agnostic — it knows nothing about kanban-specific entities — and
//! is re-exported by every consumer that used to depend on the standalone
//! crate (`apps/kanban-app`, `swissarmyhammer-entity-mcp`, and the kanban
//! crate's own `commands` submodule).

pub mod command;
pub mod context;
pub mod error;
pub mod macros;
pub mod options_resolver;
pub mod registry;
pub mod types;

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
