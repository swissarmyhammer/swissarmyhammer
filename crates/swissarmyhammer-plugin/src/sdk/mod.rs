//! The `@swissarmyhammer/plugin` TypeScript SDK, embedded into the binary.
//!
//! A plugin imports the host SDK as `@swissarmyhammer/plugin`. The module
//! loader serves that specifier as an in-memory virtual module — it never
//! lives on disk — and the SDK source itself is embedded into this crate's
//! binary at build time via [`include_str!`].
//!
//! The SDK is authored in TypeScript ([`plugin.ts`]). V8 cannot evaluate
//! TypeScript, so the virtual-module table holds the *transpiled JavaScript*:
//! [`module_loader`](crate::runtime) transpiles [`SDK_PLUGIN_SOURCE`] once
//! when it builds the table and serves the resulting JavaScript thereafter.
//!
//! The SDK's surface — the `Plugin` base class and the generic dispatch
//! Proxy — is documented in [`plugin.ts`] itself. This Rust module exists
//! only to embed and expose that source.

/// The raw TypeScript source of the `@swissarmyhammer/plugin` SDK.
///
/// This is the verbatim text of [`plugin.ts`]; it is TypeScript, not
/// JavaScript, and must be transpiled before a V8 isolate can evaluate it.
/// The module loader does that transpilation when it builds its virtual-module
/// table — see [`crate::runtime::PluginModuleLoader`].
pub const SDK_PLUGIN_SOURCE: &str = include_str!("plugin.ts");

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded SDK source is present and carries its public surface.
    #[test]
    fn sdk_source_exposes_its_public_surface() {
        assert!(
            SDK_PLUGIN_SOURCE.contains("export abstract class Plugin"),
            "the SDK must export the Plugin base class"
        );
        assert!(
            SDK_PLUGIN_SOURCE.contains("export function makeDispatcher"),
            "the SDK must export makeDispatcher"
        );
        assert!(
            SDK_PLUGIN_SOURCE.contains("export function makePluginThis"),
            "the SDK must export makePluginThis"
        );
    }

    /// The embedded SDK source carries the callback primitive: the host→isolate
    /// invoke entry point and the function-marshalling transport method.
    #[test]
    fn sdk_source_carries_the_callback_primitive() {
        assert!(
            SDK_PLUGIN_SOURCE.contains("__sahInvokeCallback"),
            "the SDK must install the host→isolate callback-invoke global"
        );
        assert!(
            SDK_PLUGIN_SOURCE.contains("callbackDispatch"),
            "the SDK must expose the callback-bearing transport method"
        );
    }
}
