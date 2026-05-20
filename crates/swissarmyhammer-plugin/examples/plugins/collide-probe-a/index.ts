// collide-probe-a — the first half of the server-name collision example.
//
// This bundle is the WINNING side of a server-name collision. It registers an
// in-process `{ rust }` source under the shared name `"collide-probe"` and then
// drives the registered server's `echo` tool. A companion bundle,
// `collide-probe-b`, tries to register the SAME name a second time; the
// platform's first-registration-wins policy rejects that second attempt with
// `ServerNameTaken`, leaving this bundle's registration untouched.
//
// ───────────────────────────────────────────────────────────────────────────
// What this bundle demonstrates
// ───────────────────────────────────────────────────────────────────────────
//
// The MCP server registry has a single global namespace: a registered name is
// held by exactly one server at a time, and the first `register` call to claim
// a name wins. There is no override semantics — a later `register` of an
// already-taken name fails. This bundle plays the role of the FIRST registrant
// so an integration test can prove a second one fails cleanly without
// disturbing it.
//
// ───────────────────────────────────────────────────────────────────────────
// Why two distinct `{ rust }` ids back one registered name
// ───────────────────────────────────────────────────────────────────────────
//
// An in-process `{ rust }` source is *single-activation*: the host moves the
// module out of its available-modules table the first time a plugin activates
// it, so a second `{ rust: "<same-id>" }` from another bundle resolves to
// `UnknownServer` rather than reaching the name-uniqueness check. To genuinely
// observe `ServerNameTaken`, each `collide-probe-*` bundle activates its OWN
// `{ rust }` module — bundle A uses `collide-probe-a-mod`, bundle B uses
// `collide-probe-b-mod` — but both register under the same NAME
// (`"collide-probe"`). The collision the test exercises is on the registered
// name, exactly as the registry's policy specifies.
//
// The end-to-end test (`tests/server_name_collision_e2e.rs`) exposes both
// `{ rust }` modules through the shared test-support harness before loading
// either bundle.

import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

// The shared registered name both `collide-probe-*` bundles target. Held as a
// constant so the test, the support helper, and both bundles all agree on the
// literal string the collision happens on.
const SERVER_NAME = "collide-probe";

// The bundle-specific in-process `{ rust }` source id. Distinct from
// `collide-probe-b`'s — see the header comment for why two ids back one name.
const RUST_MODULE_ID = "collide-probe-a-mod";

// The probe message the bundle sends through the registered server's `echo`
// tool to prove the registration is live and callable. The end-to-end test
// asserts on a substring of this string in the `tools/call` result.
const PROBE_MESSAGE = "collide-probe-a is live";

/**
 * Extracts the echoed text from an `echo` tool's result.
 *
 * The probe `echo` tool returns its `message` argument verbatim wrapped in the
 * `CallToolResult` shape — an object with a `content` array whose first entry's
 * `text` is the echoed string. This walks that shape and returns the text so
 * `load()` can verify the round-trip succeeded.
 *
 * @param result - the value returned by `this.<server-name>.echo(...)`.
 * @returns the echoed message text.
 * @throws if the result is not the expected `CallToolResult` shape.
 */
function echoedText(result: unknown): string {
  const content = (result as { content?: Array<{ text?: string }> }).content;
  if (content === undefined || content.length === 0) {
    throw new Error("echo result carried no content");
  }
  const text = content[0].text;
  if (typeof text !== "string") {
    throw new Error("echo content[0].text was not a string");
  }
  return text;
}

/**
 * The collide-probe-a example plugin.
 *
 * Its `load()` registers an in-process Rust module under the shared name
 * `"collide-probe"` and round-trips a single `echo` call through it. A passing
 * load proves the first registration succeeded; the companion `collide-probe-b`
 * bundle then attempts the colliding second registration.
 */
class CollideProbeAPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Collide Probe A";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "First half of the server-name collision example: claims the shared name and stays live.";

  /**
   * Registers the probe module under the shared name and drives its `echo`.
   *
   * Steps:
   *   1. activate the host-exposed `collide-probe-a-mod` Rust module under the
   *      shared registered name `"collide-probe"` — the FIRST claim on that
   *      name in the test, so it succeeds;
   *   2. call the registered server's `echo` tool with a fixed probe message,
   *      confirming the registration is live.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    // (1) Claim the shared registered name `"collide-probe"` with this
    //     bundle's own `{ rust }` source. First registration wins; the
    //     companion bundle's later attempt will fail with `ServerNameTaken`.
    this.register(SERVER_NAME, { rust: RUST_MODULE_ID });

    // (2) Drive the registered server's flat `echo` tool to prove the
    //     registration is live and callable from inside the plugin. The
    //     end-to-end test asserts the same call works from the host side AFTER
    //     the collision, proving the failed second load did not disturb this
    //     registration.
    //
    //     The registered name contains a hyphen, so bracket access — not dot
    //     access — is the only way to spell `this["collide-probe"]` from
    //     TypeScript. The dispatcher proxy treats any string property name as
    //     a server name, so this resolves to the same dispatcher a dot-named
    //     register would have produced.
    const probeServer = (
      this as unknown as Record<string, Record<string, (
        args: Record<string, unknown>,
      ) => Promise<unknown>>>
    )[SERVER_NAME];
    const result = await probeServer.echo({ message: PROBE_MESSAGE });
    const echoed = echoedText(result);
    if (echoed !== PROBE_MESSAGE) {
      throw new Error(
        `collide-probe-a: echo round-trip returned '${echoed}', expected '${PROBE_MESSAGE}'`,
      );
    }
    this.log.info(`collide-probe-a: registered '${SERVER_NAME}' and echoed '${echoed}'`);
  }
}

/**
 * The plugin entry point.
 *
 * The host calls this once when the bundle is discovered. It builds the
 * plugin, wraps it with `makePluginThis` so `this.<server>` dispatch works,
 * and runs the plugin's `load()`.
 *
 * @returns `null` — this plugin exposes no value to the host beyond its
 *   load-time registration and the live server it leaves behind.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new CollideProbeAPlugin()) as CollideProbeAPlugin;
  await plugin.load();
  return null;
}
