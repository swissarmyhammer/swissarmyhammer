// Builtin Probe — the committed test fixture for the read-only builtin
// discovery layer.
//
// Tests that need a real builtin bundle point a `PluginHost`'s builtin layer
// root at `test/builtin/` (this file lives under its `plugins/` subdirectory),
// so `discover_and_load_all` discovers this bundle tagged `FileSource::Builtin`
// and stacks it below the writable user and project layers.
//
// The plugin is deliberately self-contained: its `load()` runs real plugin
// code (a `log` call) but registers no server and activates no host module, so
// a test can prove the builtin *layer* genuinely loads a plugin — manifest
// parsed, isolate created, lifecycle run — without contending for any exposed
// Rust module.
import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

/// The builtin probe plugin. Its `load()` runs real plugin code without
/// registering a server.
class BuiltinProbe extends Plugin {
  /// Run real plugin lifecycle code on the builtin layer's isolate.
  async load(): Promise<void> {
    this.log.info("builtin-probe loaded from the builtin layer");
  }
}

/// Plugin entry point invoked by the platform runtime.
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new BuiltinProbe()) as BuiltinProbe;
  await plugin.load();
  return null;
}
