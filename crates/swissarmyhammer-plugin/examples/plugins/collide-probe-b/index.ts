// collide-probe-b — the second half of the server-name collision example.
//
// This bundle is the LOSING side of a server-name collision when loaded after
// `collide-probe-a`. It calls `this.register("collide-probe", ...)` for the
// same name that bundle A already claimed; the platform's
// first-registration-wins policy rejects the second attempt with
// `ServerNameTaken`, which the SDK surfaces as a thrown JS error because the
// `register` method is synchronous.
//
// ───────────────────────────────────────────────────────────────────────────
// What this bundle demonstrates
// ───────────────────────────────────────────────────────────────────────────
//
// Two things, together:
//
//   1. The MCP server registry's no-override policy is enforced at runtime,
//      from a plugin author's perspective: the second registrant of a name
//      cannot silently displace the first, and it cannot block the first's
//      ongoing operation either.
//   2. The `ServerNameTaken` failure propagates from the Rust registry, across
//      the SDK bridge, into the V8 isolate, as a real JavaScript `Error` —
//      catchable, inspectable, and (here) deliberately rethrown to fail the
//      plugin's load. Because `register` is synchronous, the throw is
//      synchronous too: there is no promise to await.
//
// The end-to-end test (`tests/server_name_collision_e2e.rs`) loads this
// bundle AFTER `collide-probe-a` and asserts that `host.load` returns the
// thrown error, that bundle A's server is still live and callable, and that
// loading `collide-probe-b` fresh — after unloading bundle A — succeeds.
//
// ───────────────────────────────────────────────────────────────────────────
// Why this bundle activates its OWN `{ rust }` module
// ───────────────────────────────────────────────────────────────────────────
//
// See `collide-probe-a/index.ts` for the long-form explanation: an in-process
// `{ rust }` source is single-activation, so two bundles sharing one `{ rust }`
// id would hit `UnknownServer` on the second `register` rather than reaching
// the name-uniqueness check. Each `collide-probe-*` bundle therefore activates
// its own distinct `{ rust }` module — bundle B uses `collide-probe-b-mod` —
// behind the shared registered name `"collide-probe"`.

import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

// The shared registered name both `collide-probe-*` bundles target. Must
// match `collide-probe-a`'s `SERVER_NAME` literally — the collision is on this
// exact string. Held as a constant for the same reason as bundle A: one
// authoritative source for the colliding name.
const SERVER_NAME = "collide-probe";

// The bundle-specific in-process `{ rust }` source id. Distinct from bundle
// A's — see the header for why two ids back one name.
const RUST_MODULE_ID = "collide-probe-b-mod";

/**
 * The collide-probe-b example plugin.
 *
 * Its `load()` issues a colliding `register` against the name bundle A already
 * claimed. The SDK's synchronous `register` throws the platform's
 * `ServerNameTaken` failure straight to this method; the catch block logs a
 * brief diagnostic so a passing run leaves an audit trail, then re-raises the
 * error so the load fails — which is the behavior the end-to-end test
 * observes.
 */
class CollideProbeBPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Collide Probe B";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Second half of the server-name collision example: deliberately fails its register.";

  /**
   * Tries to register the already-claimed name; expects `ServerNameTaken`.
   *
   * When bundle A is loaded first the `register` call throws synchronously
   * with the platform's `ServerNameTaken` message; the catch block logs the
   * failure and re-raises so the load fails. When bundle A is NOT loaded
   * (e.g., a fresh load AFTER bundle A has been unloaded — the test's fourth
   * assertion), the `register` succeeds, and the bundle simply leaves the
   * server registered.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    try {
      this.register(SERVER_NAME, { rust: RUST_MODULE_ID });
    } catch (error) {
      // The host's `Err(string)` becomes a thrown JS `Error` whose message is
      // the host's `Display` of `Error::ServerNameTaken(name)` — namely
      // "server name '<name>' is already taken". Log it before re-raising so a
      // passing test leaves an audit trail.
      const message = error instanceof Error ? error.message : String(error);
      this.log.warn(
        `collide-probe-b: register('${SERVER_NAME}') was rejected: ${message}`,
      );
      throw error;
    }
    this.log.info(
      `collide-probe-b: registered '${SERVER_NAME}' from a fresh host (no prior claim)`,
    );
  }
}

/**
 * The plugin entry point.
 *
 * The host calls this once when the bundle is discovered. It builds the
 * plugin, wraps it with `makePluginThis` so `this.<server>` dispatch works,
 * and runs the plugin's `load()`. A thrown `ServerNameTaken` propagates out
 * through this function, becoming the `Err` the host's `load()` returns.
 *
 * @returns `null` on a successful load (no prior claim on the shared name).
 * @throws the platform's `ServerNameTaken` error when the shared name is
 *   already registered — which is the behavior the end-to-end test exercises.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new CollideProbeBPlugin()) as CollideProbeBPlugin;
  await plugin.load();
  return null;
}
