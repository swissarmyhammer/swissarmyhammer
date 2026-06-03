//! The `@swissarmyhammer/plugin` TypeScript SDK, embedded into the binary.
//!
//! A plugin imports the host SDK as `@swissarmyhammer/plugin`. The module
//! loader serves that specifier as an in-memory virtual module — it never
//! lives on disk — and the SDK source itself is embedded into this crate's
//! binary at build time via [`include_str!`].
//!
//! The SDK is authored across several TypeScript files:
//!
//! - [`plugin.ts`] — the core: the `Plugin` base class, the dispatch Proxy
//!   (`makeDispatcher` / `makePluginThis`), and the callback primitive.
//! - [`services.ts`] — the `ensureServices` convention helper for activating
//!   host-provided in-process services (`commands`, `window`, `app`, …).
//! - [`commands.ts`] — the `registerCommands` convention helper for installing
//!   palette/menu commands through the command service.
//!
//! The three sources are **concatenated** at build time into one virtual
//! module body — they share scope so later files reference symbols declared in
//! earlier ones without `import` statements. V8 cannot evaluate TypeScript, so
//! the virtual-module table holds the *transpiled JavaScript*: the module
//! loader transpiles [`SDK_PLUGIN_SOURCE`] once when it builds the table and
//! serves the resulting JavaScript thereafter.
//!
//! The SDK's surface is documented in the TypeScript files themselves. This
//! Rust module exists only to embed and expose them.

/// The raw TypeScript source of the `@swissarmyhammer/plugin` SDK's core file.
///
/// This is the verbatim text of [`plugin.ts`] — the `Plugin` base class, the
/// dispatch Proxy, and the callback primitive. It is TypeScript, not
/// JavaScript, and must be transpiled before a V8 isolate can evaluate it. The
/// module loader concatenates this with [`SDK_SERVICES_SOURCE`] and
/// [`SDK_COMMANDS_SOURCE`] into one body and transpiles the result when it
/// builds its virtual-module table — see
/// [`crate::runtime::PluginModuleLoader`].
pub const SDK_PLUGIN_SOURCE: &str = include_str!("plugin.ts");

/// The raw TypeScript source of the `@swissarmyhammer/plugin` SDK's
/// `ensureServices` helper file.
///
/// Concatenated after [`SDK_PLUGIN_SOURCE`] into the virtual SDK module body.
/// Carries no top-level `import` statements — it reads `Plugin` and the rest of
/// the core surface from the shared module scope created by the concatenation.
pub const SDK_SERVICES_SOURCE: &str = include_str!("services.ts");

/// The raw TypeScript source of the `@swissarmyhammer/plugin` SDK's
/// `registerCommands` helper file.
///
/// Concatenated after [`SDK_PLUGIN_SOURCE`] and [`SDK_SERVICES_SOURCE`] into
/// the virtual SDK module body. Carries no top-level `import` statements —
/// it reads `Plugin`, `PluginThis`, and the rest of the core surface from the
/// shared module scope created by the concatenation.
pub const SDK_COMMANDS_SOURCE: &str = include_str!("commands.ts");

/// Build the combined SDK TypeScript source the module loader transpiles.
///
/// The three SDK files share scope: later files reference `Plugin` and other
/// symbols declared in [`SDK_PLUGIN_SOURCE`] without an `import` statement.
/// Joining them with two newlines preserves comment and statement boundaries
/// so any TypeScript syntax error points at the right region of the right
/// file.
pub fn combined_sdk_source() -> String {
    let mut combined = String::with_capacity(
        SDK_PLUGIN_SOURCE.len() + SDK_SERVICES_SOURCE.len() + SDK_COMMANDS_SOURCE.len() + 4,
    );
    combined.push_str(SDK_PLUGIN_SOURCE);
    combined.push_str("\n\n");
    combined.push_str(SDK_SERVICES_SOURCE);
    combined.push_str("\n\n");
    combined.push_str(SDK_COMMANDS_SOURCE);
    combined
}

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
        assert!(
            SDK_PLUGIN_SOURCE.contains("export function unwrapResult"),
            "the SDK must export the unwrapResult inbound-result helper"
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

    /// The `ensureServices` and `registerCommands` convention helpers are
    /// embedded and export their public surface.
    #[test]
    fn sdk_source_exposes_convention_helpers() {
        assert!(
            SDK_SERVICES_SOURCE.contains("export async function ensureServices"),
            "the SDK must export the ensureServices helper"
        );
        assert!(
            SDK_SERVICES_SOURCE.contains("export const RUST_MODULE_IDS"),
            "the SDK must export the RUST_MODULE_IDS lookup table"
        );
        assert!(
            SDK_COMMANDS_SOURCE.contains("export async function registerCommands"),
            "the SDK must export the registerCommands helper"
        );
        assert!(
            SDK_COMMANDS_SOURCE.contains("export interface CommandRegistration"),
            "the SDK must export the CommandRegistration interface"
        );
    }

    /// The combined SDK source carries every file's surface end to end.
    #[test]
    fn combined_sdk_source_concatenates_every_file() {
        let combined = combined_sdk_source();
        assert!(
            combined.contains("export abstract class Plugin"),
            "the combined source must carry plugin.ts's Plugin class"
        );
        assert!(
            combined.contains("export async function ensureServices"),
            "the combined source must carry services.ts's ensureServices"
        );
        assert!(
            combined.contains("export async function registerCommands"),
            "the combined source must carry commands.ts's registerCommands"
        );
    }
}
