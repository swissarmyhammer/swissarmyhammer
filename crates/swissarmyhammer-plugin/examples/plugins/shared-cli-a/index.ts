// shared-cli-a — the first half of the shared-CLI-source example.
//
// This bundle and its sibling `shared-cli-b` both register the SAME server name
// (`"shared-cli"`) backed by the SAME `{ cli }` server source. Under the
// platform's idempotent registration policy, both registrations succeed and
// share ONE underlying subprocess: the first registrant spawns it, the second
// joins the existing one. Refcounting threads through unload, so the
// subprocess is only torn down when the LAST holder is gone.
//
// ───────────────────────────────────────────────────────────────────────────
// Why a shared `{ cli }` source is the demonstration
// ───────────────────────────────────────────────────────────────────────────
//
// Two plugins that both depend on the same external MCP server (a community
// `weather` server, a project-local data tool, a shared subprocess transport)
// should NOT have to coordinate to avoid clobbering each other — the first
// successful registration would today reject the second with `ServerNameTaken`.
// The platform fixes that by recognizing structurally-equal sources and
// merging them: the same `{ cli: [<binary>] }` registered by two plugins is
// the same subprocess, kept live as long as ANY plugin still holds it.
//
// `{ cli }` is the right source for this example because it produces a real,
// observable, host-managed resource (the spawned subprocess) — the test can
// prove the subprocess is alive after one plugin unloads and dies after the
// other does. `{ rust }` would not work: in-process modules are
// single-activation (the host moves them out of the available-modules table
// on first use), which is a separate concern from name-collision.
//
// ───────────────────────────────────────────────────────────────────────────
// The command path is supplied by the test
// ───────────────────────────────────────────────────────────────────────────
//
// The committed source carries the placeholder token `__CLI_ECHO_COMMAND__`.
// The end-to-end test stages this bundle with `support::stage_example_with`,
// which rewrites the token in the throwaway copy with the real path of the
// crate's `cli_server_fixture` binary — a genuine stdio MCP server with a
// flat `echo` tool.

import { Plugin } from "@swissarmyhammer/plugin";

// The shared registered name both `shared-cli-*` bundles target. Both bundles
// register this name against the SAME `{ cli }` source, so the second call is
// a structural duplicate of the first and merges into it.
const SERVER_NAME = "shared-cli";

// The command the `{ cli }` source spawns. PLACEHOLDER — the test rewrites
// this in the staged copy with the real fixture binary path. The committed
// value is intentionally not a runnable command.
const ECHO_COMMAND = "__CLI_ECHO_COMMAND__";

/**
 * The shared-cli-a example plugin.
 *
 * Its `load()` registers the shared CLI server. The host's first registration
 * of this `(name, source)` spawns the subprocess; a subsequent registration by
 * `shared-cli-b` recognizes the same source and joins the live subprocess.
 */
export default class SharedCliAPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Shared CLI A";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "First holder of a shared `{ cli }` server; the second holder is `shared-cli-b`.";

  /**
   * Registers the shared CLI server. No tool call is issued here — the test
   * drives `echo` through `PluginHost::call` once both bundles have loaded, so
   * the registration is the only effect this `load()` produces.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    this.register(SERVER_NAME, { cli: [ECHO_COMMAND] });
    this.log.info(`shared-cli-a: registered '${SERVER_NAME}'`);
  }
}

