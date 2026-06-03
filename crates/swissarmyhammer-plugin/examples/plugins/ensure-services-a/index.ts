// ensure-services-a — the first half of the two-plugin shared-services example.
//
// This bundle and its sibling `ensure-services-b` both call
// `ensureServices(this, ["commands"])` in their `load()` and then each register
// a distinct command through `registerCommands`. The platform's idempotent
// registration policy lets both plugins share ONE live `commands` server: the
// first plugin's `ensureServices` claims the registration; the second's call is
// a structurally-equal no-op that joins the live registration via refcount.
//
// ───────────────────────────────────────────────────────────────────────────
// Why two plugins for one example
// ───────────────────────────────────────────────────────────────────────────
//
// The convention is a host-wide property: every command-registering plugin
// calls `ensureServices(this, ["commands"])` first, regardless of whether
// another plugin has done so already. Demonstrating that with two real bundles
// proves the convention is actually idempotent end to end:
//
//   * Both plugins' `ensureServices` succeed without coordination.
//   * Each plugin's `registerCommands` lands its commands on the shared
//     `commands` server.
//   * Unloading one plugin purges its commands but leaves the other plugin's
//     commands untouched, and the `commands` server stays live.
//   * Unloading the last plugin tears the `commands` registration down.
//
// The companion `ensure-services-b` registers a different command id so the
// test can tell whose commands are present after each unload step.

import {
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/** The unique command id this plugin contributes to the shared registry. */
const COMMAND_ID = "ensure-services-a.greet";

/**
 * The ensure-services-a example plugin.
 *
 * Its `load()` follows the convention exactly: `ensureServices` first,
 * `registerCommands` second. No `unload()` body is needed — the platform's
 * per-plugin ledger purges both this plugin's commands and its half of the
 * shared `commands` registration when the host unloads it.
 */
export default class EnsureServicesAPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Ensure Services A";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "First half of the two-plugin shared-services example; the second half is `ensure-services-b`.";

  /**
   * Activates the `commands` service and registers this plugin's one command.
   *
   * The host calls this exactly once, when the plugin is discovered. The
   * `ensureServices` call is idempotent: if `ensure-services-b` ran first the
   * shared `commands` server is already live; if this plugin runs first the
   * server is created and joined by `ensure-services-b` later.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands"]);
    await registerCommands(this, [
      {
        id: COMMAND_ID,
        name: "Greet (from A)",
        execute: () => "ensure-services-a greeted",
      },
    ]);
    this.log.info(`ensure-services-a: registered '${COMMAND_ID}'`);
  }
}
