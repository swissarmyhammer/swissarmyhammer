// shared-cli-b — the second half of the shared-CLI-source example.
//
// See `shared-cli-a/index.ts` for the long-form explanation. Briefly: both
// bundles register the SAME `(name, source)` and rely on the platform's
// idempotent-registration policy to merge them into ONE underlying subprocess.
// This bundle is the SECOND registrant: its `register` call recognizes the
// already-live source from `shared-cli-a` and bumps its refcount instead of
// rejecting with `ServerNameTaken`.

import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

// The shared registered name. Must match `shared-cli-a`'s `SERVER_NAME`
// literally — the dedup is on the (name, source) pair, and the name is the
// same exact string.
const SERVER_NAME = "shared-cli";

// The command the `{ cli }` source spawns. Must be the SAME placeholder token
// as `shared-cli-a` — the end-to-end test rewrites both bundles' tokens with
// the SAME real fixture path, so the two `{ cli }` sources are structurally
// equal.
const ECHO_COMMAND = "__CLI_ECHO_COMMAND__";

/**
 * The shared-cli-b example plugin.
 *
 * Its `load()` registers the same `(name, source)` pair `shared-cli-a` did.
 * When `shared-cli-a` has already loaded the source is already live and this
 * second registration shares it; when this bundle loads alone the source is
 * fresh and this registration spawns the subprocess.
 */
class SharedCliBPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Shared CLI B";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Second holder of a shared `{ cli }` server; the first holder is `shared-cli-a`.";

  /**
   * Registers the shared CLI server. The test asserts BOTH bundles' loads
   * succeed against the SAME registered server name — proof that idempotent
   * registration merged the two calls into one subprocess.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    this.register(SERVER_NAME, { cli: [ECHO_COMMAND] });
    this.log.info(`shared-cli-b: registered '${SERVER_NAME}'`);
  }
}

/**
 * The plugin entry point.
 *
 * @returns `null` — this plugin's only effect is its load-time registration.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new SharedCliBPlugin()) as SharedCliBPlugin;
  await plugin.load();
  return null;
}
