// cli-echo — the CLI stdio-transport and unload-lifecycle example.
//
// This plugin demonstrates two things no other example covers:
//
//   1. The `{ cli }` server source — an MCP server the host spawns as a stdio
//      subprocess, rather than an in-process `{ rust }` server. It is the only
//      example registering a non-`rust` `ServerSource`.
//   2. The `unload()` lifecycle hook — the teardown counterpart to `load()`.
//      It is the only example that overrides `unload()` to release what its
//      `load()` set up.
//
// ───────────────────────────────────────────────────────────────────────────
// The `{ cli }` server source
// ───────────────────────────────────────────────────────────────────────────
//
// A server source picks where an MCP server lives. `{ cli }` names a server
// the host runs as a child process and speaks MCP JSON-RPC to over the
// process's stdin/stdout:
//
//     this.register("echo", { cli: ["<command>", "<arg>", ...] })
//
// The first array element is the executable; the rest are its arguments. The
// host spawns the process, performs the MCP handshake over its stdio, and from
// then on `this.echo.<tool>(...)` dispatches a real `tools/call` across that
// pipe. (`{ cli }` also accepts optional `env` and `cwd` fields — see the SDK's
// `ServerSource` type — neither of which this example needs.)
//
// ───────────────────────────────────────────────────────────────────────────
// The command path: supplied by the host/test, NOT hard-coded
// ───────────────────────────────────────────────────────────────────────────
//
// A committed example cannot hard-code the absolute path of an MCP server
// binary: there is no such path that is correct on every machine, and the
// repository ships no such binary at a fixed location. So the command below is
// the named placeholder token `__CLI_ECHO_COMMAND__`.
//
// The end-to-end test that drives this bundle (`tests/cli_echo_e2e.rs`) stages
// it with the harness helper `support::stage_example_with`, which rewrites that
// token in the throwaway STAGED COPY with the real path of the crate's
// `cli_server_fixture` binary — a genuine stdio MCP server exposing a flat
// `echo` tool. The COMMITTED file you are reading stays a clean, readable
// example; only the temp copy a test runs is specialized.
//
// A plugin you write for real does the same thing in spirit: it resolves the
// command path from configuration, the environment, or a bundled binary it
// controls — never a fixed absolute literal baked into the source.
//
// ───────────────────────────────────────────────────────────────────────────
// The `load` / `unload` entry exports and the lifecycle hooks
// ───────────────────────────────────────────────────────────────────────────
//
// The host calls a module-level `load` export once when the plugin is
// discovered, and a module-level `unload` export once when the plugin is
// unloaded (on shutdown, or on a hot reload). Both run on the SAME isolate, so
// this module keeps the one `Plugin` instance in a module-level variable: the
// `load` export builds and stores it, and the `unload` export reaches the very
// same instance to tear it down. A plugin that wants teardown MUST export an
// `unload` function — without the export, the plugin's `unload()` hook is never
// reached.
//
// `load()` and `unload()` on the `Plugin` subclass are the lifecycle hooks the
// two exports drive: `load()` is where a plugin acquires what it needs;
// `unload()` is where it releases what it acquired — the symmetric teardown.
//
// The host ALSO disposes every registration a plugin made, automatically, on
// unload — so overriding `unload()` is not required just to drop a registered
// server. This example overrides it anyway: `unload()` is where a plugin runs
// any teardown the automatic disposal cannot do for it — flushing state,
// notifying another service, recording that it shut down cleanly. To make that
// concrete (and to give the end-to-end test something only the hook can
// produce), this `unload()` writes a sentinel task onto a kanban board before
// it unregisters the echo server. An override must call `super.unload()` so the
// base class's `track`-disposable cleanup still runs.

import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

// The command the `{ cli }` source spawns. This is a PLACEHOLDER — see the
// "command path" section above. The end-to-end test rewrites this token in the
// staged copy with a real stdio MCP server binary path; the committed value is
// intentionally not a runnable command.
const ECHO_COMMAND = "__CLI_ECHO_COMMAND__";

// The server name this plugin registers the CLI subprocess under. After
// `register`, `this.echo` is the dispatch index for the subprocess server.
const ECHO_SERVER = "echo";

// The server name the host-exposed in-process `kanban` operation tool is
// registered under. After `register`, `this.board` is the dispatch index for
// the `kanban` tool. The plugin's `unload()` uses it to record a sentinel task
// — the observable proof that the `unload()` hook itself ran.
const BOARD_SERVER = "board";

// The message this plugin sends to the subprocess's flat `echo` tool at load
// time. The end-to-end test asserts the round-trip independently, so any
// non-empty string works here — it exists to prove `load()` reaches the
// subprocess.
const ECHO_MESSAGE = "cli-echo: hello over the stdio subprocess transport";

// The title of the sentinel task this plugin's `unload()` adds to the kanban
// board. The end-to-end test asserts this task is ABSENT before unload and
// PRESENT after — an effect the host's automatic registration disposal can
// never produce, so its presence proves the plugin's own `unload()` hook ran.
const UNLOAD_SENTINEL_TITLE = "cli-echo unload() ran";

/**
 * Extracts the echoed text from an `echo` `tools/call` result.
 *
 * A `tools/call` result is a `CallToolResult` shape — an object with a
 * `content` array whose first entry's `text` is the tool's output. This walks
 * that shape and returns the text, so `load()` can verify the subprocess
 * actually echoed the message back rather than silently returning nothing.
 *
 * @param result - the value returned by `this.echo.echo({ message })`.
 * @returns the echoed text.
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
 * The cli-echo example plugin.
 *
 * Its `load()` registers a stdio MCP subprocess as the `echo` server and the
 * host-exposed in-process `kanban` tool as `board`, then calls the subprocess's
 * flat `echo` tool. Its `unload()` records a sentinel task on the board and
 * unregisters the echo server — the teardown counterpart to the `load()`-time
 * `register`.
 */
class CliEchoPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "CLI Echo Example";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Drives a stdio-subprocess MCP server and records a sentinel on unload().";

  /**
   * Registers the CLI subprocess server, registers the kanban board, and
   * calls the subprocess's flat `echo` tool.
   *
   * Steps:
   *   1. register `echo` as a `{ cli }` source — the host spawns the named
   *      command as a child process and connects to its stdio;
   *   2. register `board` as the host-exposed `{ rust: "kanban" }` operation
   *      tool, so `unload()` has somewhere observable to record that it ran;
   *   3. call the subprocess's flat `echo` tool through `this.echo.echo(...)`;
   *      the call crosses a real `tools/call` over the subprocess's stdio;
   *   4. verify the echoed text came back, so a broken transport fails `load()`
   *      loudly rather than passing silently.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    // (1) Register the stdio MCP subprocess under the name `echo`. After this,
    //     `this.echo` is the dispatch index for the subprocess's tools.
    this.register(ECHO_SERVER, { cli: [ECHO_COMMAND] });

    // (2) Register the host-exposed in-process `kanban` operation tool under
    //     the name `board`. `unload()` writes a sentinel task here so the
    //     teardown leaves an observable trace the host's automatic disposal
    //     could never produce.
    this.register(BOARD_SERVER, { rust: "kanban" });

    // (3) Call the subprocess's flat `echo` tool. `echo` is a FLAT tool — one
    //     entry point, no `noun.verb` path — so the call is `this.echo.echo`:
    //     the first segment is the server, the second is the tool. The
    //     arguments object crosses verbatim as a `tools/call` over stdio.
    const result = await this.echo.echo({ message: ECHO_MESSAGE });

    // (4) Confirm the subprocess echoed the message back. A non-trivial check
    //     makes the plugin fail loudly if the stdio round-trip is broken.
    const echoed = echoedText(result);
    if (echoed.indexOf(ECHO_MESSAGE) < 0) {
      throw new Error("echo did not return the sent message");
    }

    this.log.info("cli-echo: round-tripped a tools/call over the stdio subprocess");
  }

  /**
   * Releases what `load()` set up, and records that it ran.
   *
   * `unload()` is the teardown counterpart to `load()` — the hook where a
   * plugin does the cleanup the host's automatic registration disposal cannot
   * do for it. This `unload()`:
   *
   *   1. records a sentinel task on the still-live `board` server. The host
   *      runs `unload()` *before* it disposes a plugin's registrations, so the
   *      `board` server is still routable here. Adding a task is an effect no
   *      automatic disposal could produce — it is the observable proof that
   *      this hook body actually ran;
   *   2. unregisters the `echo` subprocess server. The host would dispose this
   *      registration anyway, so the call is not strictly required — it is the
   *      explicit, readable way to say "this plugin is done with the echo
   *      server", and it lets the host terminate the subprocess promptly;
   *   3. calls `super.unload()` — mandatory in any override — which runs the
   *      base class's cleanup of every disposable passed to `track`.
   *
   * The host calls this exactly once, when the plugin is unloaded.
   */
  async unload(): Promise<void> {
    // (1) Record that this hook ran. The `board` server is still live — the
    //     host disposes registrations only *after* `unload()` returns — so the
    //     sentinel task lands on the board. Its presence is an effect only the
    //     `unload()` hook body can produce.
    await this.board.kanban.task.add({ title: UNLOAD_SENTINEL_TITLE });

    // (2) Drop the `echo` server registered in `load()`. This removes it from
    //     the live registry; the host then terminates the spawned subprocess.
    this.unregister(ECHO_SERVER);

    // (3) Run the base class's teardown — disposing anything passed to `track`.
    //     An override must always call this.
    await super.unload();

    this.log.info("cli-echo: recorded the unload sentinel and tore down");
  }
}

/**
 * The one plugin instance, shared between the `load` and `unload` exports.
 *
 * The host drives both exports on the same isolate, so the instance the `load`
 * export builds is still here when the `unload` export runs — letting `unload`
 * tear down the very same plugin `load` set up. It is `undefined` until `load`
 * runs and is cleared back to `undefined` by `unload`.
 */
let instance: CliEchoPlugin | undefined;

/**
 * The plugin's load entry point.
 *
 * The host calls this once when the bundle is discovered. It builds the
 * plugin, wraps it with `makePluginThis` so `this.<server>` dispatch works,
 * stores it for the `unload` export, and runs the plugin's `load()` hook.
 *
 * @returns `null` — this plugin exposes no value to the host beyond its
 *   load-time and unload-time effects.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new CliEchoPlugin()) as CliEchoPlugin;
  instance = plugin;
  await plugin.load();
  return null;
}

/**
 * The plugin's unload entry point.
 *
 * The host calls this once when the plugin is unloaded. It runs the `unload()`
 * hook on the same instance the `load` export built — which records the unload
 * sentinel task, unregisters the `echo` server, and runs the base class
 * teardown — then clears the instance.
 *
 * Calling `unload` before `load` is a host error; the guard makes that fail
 * loudly rather than silently skipping teardown.
 *
 * @returns `null` — `unload` reports completion by returning normally.
 */
export async function unload(): Promise<unknown> {
  if (instance === undefined) {
    throw new Error("cli-echo: unload called before load");
  }
  await instance.unload();
  instance = undefined;
  return null;
}
