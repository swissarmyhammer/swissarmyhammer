// Kanban Builtin Probe — the read-only builtin-layer plugin shipped inside the
// kanban desktop app.
//
// It exercises the builtin plugin layer end to end: the kanban app compiles
// this bundle into its binary via `include_dir!`, extracts it at startup, and
// loads it into the application `PluginHost`. On `load()` the plugin activates
// the host-exposed in-process `kanban` tool module under the registry name
// `kanban-builtin-probe`, so a builtin plugin can drive real kanban operations.
import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

/// The builtin probe plugin. Its `load()` registers the host's `kanban` Rust
/// module under the plugin's own server name.
class KanbanBuiltinProbe extends Plugin {
  /// Human-readable name — descriptive metadata only, not plugin identity.
  readonly name = "Kanban Builtin Probe";

  /// Version string — descriptive metadata only.
  readonly version = "1.0.0";

  /// One-line description — descriptive metadata only.
  readonly description =
    "Builtin-layer probe that activates the host's kanban tool module.";

  /// Activate the host-exposed `kanban` tool module under this plugin's name.
  async load(): Promise<void> {
    this.register("kanban-builtin-probe", { rust: "kanban" });
  }
}

/// Plugin entry point invoked by the platform runtime.
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new KanbanBuiltinProbe()) as KanbanBuiltinProbe;
  await plugin.load();
  return null;
}
