// ensure-services-b — the second half of the two-plugin shared-services example.
//
// See the header comment in `examples/plugins/ensure-services-a/index.ts` for
// the full description of what this example demonstrates. In short: both
// bundles follow the canonical command-registering plugin convention
// (`ensureServices` then `registerCommands`), and the platform's idempotent
// registration policy lets them share ONE live `commands` server.
//
// This plugin registers a different command id from `ensure-services-a` so the
// end-to-end test can tell which plugin's commands are present after each
// unload step.

import {
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/** The unique command id this plugin contributes to the shared registry. */
const COMMAND_ID = "ensure-services-b.farewell";

/**
 * The ensure-services-b example plugin.
 *
 * Its `load()` follows the same convention as `ensure-services-a`. Both
 * plugins independently call `ensureServices(this, ["commands"])` and the
 * platform's structural-equality dedupe merges the two registrations into one
 * shared refcounted hold on the `commands` server.
 */
export default class EnsureServicesBPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Ensure Services B";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Second half of the two-plugin shared-services example; the first half is `ensure-services-a`.";

  /**
   * Activates the `commands` service and registers this plugin's one command.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands"]);
    await registerCommands(this, [
      {
        id: COMMAND_ID,
        name: "Farewell (from B)",
        execute: () => "ensure-services-b said farewell",
      },
    ]);
    this.log.info(`ensure-services-b: registered '${COMMAND_ID}'`);
  }
}
