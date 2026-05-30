//! Backend resolver registry for `ParamDef.options_from`.
//!
//! Re-exported from the `swissarmyhammer-command-options` leaf crate, which
//! owns the option-resolver machinery so consumer crates can depend on it
//! without depending on this crate.

pub use swissarmyhammer_command_options::{
    register_command_resolvers, OptionsContext, OptionsRegistry, OptionsResolver, OptionsSources,
    SortDirectionsResolver,
};
