// Kanban Builtin Probe — the read-only builtin-layer plugin shipped inside the
// kanban desktop app.
//
// It exercises the builtin plugin layer end to end: the kanban app compiles
// this bundle into its binary via `include_dir!`, extracts it at startup, and
// loads it into the application `PluginHost`. On `load()` the plugin activates
// the host-exposed in-process `kanban` tool module under the canonical registry
// name `kanban`, so a builtin plugin can drive real kanban operations.
//
// IMPORTANT: a `{ rust }` module is single-activation — `activate_rust_module`
// removes it from the host's available-modules table, and a second `register`
// of the SAME id under a DIFFERENT name fails with "unknown server". The kanban
// app loads this probe ALONGSIDE the `task-commands` / `kanban-misc-commands`
// builtin plugins, which also activate `{ rust: "kanban" }` — but under the
// canonical name `"kanban"` (via the SDK's `ensureServices`). Registering here
// under that SAME name shares the one live registration through the registry's
// structural-source dedupe, so the probe and the command plugins co-load. (A
// distinct name here would starve whichever plugin loaded second.)
import { Plugin } from "@swissarmyhammer/plugin";

/// The builtin probe plugin. Its `load()` registers the host's `kanban` Rust
/// module under the canonical server name so it shares the single-activation
/// module with the command plugins that also consume it.
///
/// The host instantiates this default-exported class, wraps it with the SDK's
/// dispatch Proxy, and runs its `load()` — no module-level entry boilerplate.
export default class KanbanBuiltinProbe extends Plugin {
  /// Human-readable name — descriptive metadata only, not plugin identity.
  readonly name = "Kanban Builtin Probe";

  /// Version string — descriptive metadata only.
  readonly version = "1.0.0";

  /// One-line description — descriptive metadata only.
  readonly description =
    "Builtin-layer probe that activates the host's kanban tool module.";

  /// Activate the host-exposed `kanban` tool module under its canonical name,
  /// sharing the single-activation module with the kanban command plugins.
  async load(): Promise<void> {
    this.register("kanban", { rust: "kanban" });
  }
}
