# Example Plugins

This directory holds committed, worked examples of SwissArmyHammer plugins.
Each subdirectory is a complete plugin bundle a plugin author can read,
copy, and adapt. The end-to-end test suite (`tests/example_plugins_e2e.rs`)
loads and drives these same bundles through the real plugin platform, so the
examples double as a living regression suite: if an example stops loading,
a test fails.

This README is plugin-author documentation. It explains the authoring model
the examples demonstrate.

## A plugin bundle

A plugin is a directory containing a single required file:

```
my-plugin/
  index.ts      # the entry module
```

There is **no manifest**. A bundle is just a directory with an `index.ts`
entry module — no `plugin.json`, no `id`, no `entry` field, no `provides`
list.

The bundle lives under a `plugins/` directory in any plugin layer. Discovery
scans `<layer-root>/plugins/<name>/` for bundles; the directory name **is** the
plugin's identity. When the same directory name appears in two layers, the
higher-precedence layer's copy is the one that loads.

A bundle is not limited to a single `index.ts`. The entry module — or any
module it imports — can pull in sibling source files with ordinary relative
imports (`./helper.ts`); the loader resolves them against the bundle directory.
The `multi-module` example is exactly such a multi-file bundle. The one rule:
a relative import may not escape the bundle directory, so the bundle stays a
self-contained sandbox.

## The `index.ts` module and the `load()` contract

The entry module is `index.ts` by convention — discovery loads it as the
bundle's entry point. It is TypeScript: the runtime transpiles it to
JavaScript and runs it inside a fresh V8 isolate. The module must export an
async function named `load`:

```ts
export async function load(): Promise<unknown> { /* ... */ }
```

The host calls `load()` exactly once when the plugin is discovered. It is the
plugin's one entry point — everything the plugin does at load time happens
inside it (or inside code it calls). Returning normally signals a successful
load; throwing fails the load.

## The `@swissarmyhammer/plugin` SDK

The entry module imports the host SDK as `@swissarmyhammer/plugin`. The SDK is
a virtual module served from host memory — it is not a file on disk and does
not need to be installed. It provides two pieces an example uses:

### The `Plugin` base class

Every plugin subclasses `Plugin`. The base class is the entire authoring
surface:

- `name` / `version` / `description` — optional `readonly` class props a
  subclass sets with a plain field initializer (`readonly name = "My Plugin"`,
  `readonly description = "what this plugin does"`). They are descriptive
  metadata only — used for the plugin's own logging and reporting — and play no
  part in plugin identity or discovery (the directory name is the identity). A
  subclass that omits them keeps the inert base defaults.
- `load()` / `unload()` — optional lifecycle hooks. Override `load()` to do
  setup work; override `unload()` (calling `super.unload()`) to do teardown.
- `register(name, source)` — point the platform at an MCP server, reachable
  afterward as `this.<name>`. `source` is one of:
  - `{ rust: "<module-id>" }` — a host-exposed in-process Rust server.
  - `{ url: "<endpoint>", headers?: {...} }` — an HTTP MCP endpoint.
  - `{ cli: ["<cmd>", ...], env?: {...}, cwd?: "..." }` — a stdio MCP subprocess.
- `unregister(name)` — drop a server registered with `register`.
- `log` — a scoped logger (`debug` / `info` / `warn` / `error`).
- `track(disposable)` — register a disposable for cleanup at `unload` time.

A registered server's tools are reached through a dynamic dispatch index:
`this.<server>.<tool>({ ... })` for a flat tool, or
`this.<server>.<tool>.<noun>.<verb>({ ... })` for an operation tool. The
plugin never describes a server's tools — the platform queries them from the
server itself.

### `makePluginThis`

A bare `Plugin` instance does not yet have the dynamic `this.<server>` index.
`makePluginThis(instance)` wraps an instance so unknown property reads become
server dispatchers. The entry module's `load()` builds the plugin, wraps it,
and runs the plugin's `load()`:

```ts
import { Plugin, makePluginThis } from '@swissarmyhammer/plugin';

class MyPlugin extends Plugin {
  readonly name = 'My Plugin';
  readonly version = '1.0.0';
  readonly description = 'What my plugin does in one line.';

  async load(): Promise<void> {
    this.register('my-server', { rust: 'files' });
    // this.my-server.<tool>(...) is now callable
  }
}

export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new MyPlugin()) as MyPlugin;
  await plugin.load();
  return null;
}
```

## Operation tools and the `noun.verb` path form

Some MCP tools are *operation tools*: a single tool multiplexes many related
operations, each selected by an `op` string of the form `"<verb> <noun>"`.
The in-process `kanban` tool is one — `"add task"`, `"list tasks"`, and
`"init board"` are all operations of the one `kanban` tool.

An operation tool publishes a discovery tree in its `tools/list` definition
under the `_meta` key `io.swissarmyhammer/operations`, keyed
`[<noun>][<verb>]`. The SDK reads it so a plugin can reach an operation two
ways:

- **Direct form** — `this.<server>.<tool>({ op: "add task", ... })`. The `op`
  is already in the arguments; the SDK passes it straight through.
- **Path form** — `this.<server>.<tool>.<noun>.<verb>({ ... })`. There is no
  `op`; the SDK walks the tool's `_meta` tree, reads the matching leaf's `op`,
  and builds the `tools/call` for you.

The path segments are the operation's *noun* and *verb* exactly as the tool's
`_meta` declares them — not an English pluralization you invent. The `kanban`
tool declares its add operation under noun `task` but its list operation under
noun `tasks` (plural): `this.board.kanban.task.add(...)` and
`this.board.kanban.tasks.list(...)`. The tool's `_meta` is the source of truth.

## Example index

The examples live in subdirectories alongside this README. Each is a
self-contained bundle with its own `index.ts` entry module, and each is
exercised by an end-to-end test.

| Example | Demonstrates | Test |
|---------|--------------|------|
| `kanban-tasks` | An operation tool driven through the `_meta` `noun.verb` path form: registers the in-process `kanban` tool as `board`, adds two tasks via `this.board.kanban.task.add(...)`, and lists them via `this.board.kanban.tasks.list(...)`. | `tests/kanban_tasks_e2e.rs` |
| `file-notes` | A real filesystem effect through the in-process `files` tool driven with the direct `op` form: registers `files` as `fs`, then writes a note, reads it back, and writes the read-back content into a second note — all against **relative** paths. | `tests/file_notes_e2e.rs` |
| `cli-echo` | The `{ cli }` stdio-subprocess transport and the `unload()` lifecycle hook: registers `echo` as a `{ cli }` server the host spawns as a child process, calls its flat `echo` tool over stdio, and overrides `unload()` to record a sentinel kanban task and `unregister` the server at teardown. | `tests/cli_echo_e2e.rs` |
| `multi-module` | A **multi-file** bundle: `index.ts` imports a sibling `board-helpers.ts` module with the relative specifier `./board-helpers.ts`. The sandboxed loader resolves the import against the bundle directory; the imported async helper adds one tagged task to the `kanban` board. | `tests/multi_module_e2e.rs` |

## `file-notes` and the relative-path contract

The `file-notes` example drives the in-process `files` tool to produce a real,
observable effect on disk — the most relatable example for a plugin author.
It registers `files` as `fs` and round-trips a note: `write file` →
`read file` → `write file`.

Its one subtlety is **path resolution**. The `files` tool resolves a *relative*
path against the host **process's** current working directory, and uses an
*absolute* path verbatim. A committed example cannot hard-code an absolute
path — there is no temp directory it could name at authoring time, and a fixed
absolute path would be unsafe to write to. So `file-notes` addresses the
`files` tool with relative paths (`notes/hello.txt`, `notes/echo.txt`), and
where those files land depends entirely on the process working directory at
load time.

A plugin you write should account for the same contract: either use a relative
path and know the process working directory, or compute an absolute path you
control. The example's end-to-end test (`tests/file_notes_e2e.rs`) pins the
process working directory to a throwaway temp directory so the notes land there
and the real source tree is never written to.

## `cli-echo`, the `{ cli }` transport, and the `unload()` hook

The `cli-echo` example is the one bundle that registers a **non-`rust`**
server source and the one that demonstrates **teardown**. Its `load()` does:

```ts
this.register("echo", { cli: ["<command>", ...] });
await this.echo.echo({ message: "..." });
```

A `{ cli }` source names an MCP server the host runs as a **stdio
subprocess** — it spawns the command as a child process, performs the MCP
handshake over the process's stdin/stdout, and from then on `this.echo.<tool>`
dispatches a real `tools/call` across that pipe. (`{ cli }` also accepts
optional `env` and `cwd` fields; see the SDK's `ServerSource` type.)

### The command path is supplied by the host, not hard-coded

A committed example cannot hard-code the absolute path of an MCP server
binary: no such path is correct on every machine, and the repository ships no
binary at a fixed location. So the committed `index.ts` carries the named
placeholder token `__CLI_ECHO_COMMAND__` where the command belongs.

The end-to-end test (`tests/cli_echo_e2e.rs`) stages the bundle with the
harness helper `support::stage_example_with`, which copies the bundle into a
temp layer root and then rewrites that token — in the **throwaway staged
copy only** — with the real path of the crate's `cli_server_fixture` binary, a
genuine stdio MCP server exposing a flat `echo` tool. The committed bundle
stays a clean, readable example; only the temp copy a test runs is
specialized. A plugin you write for real resolves the command path from
configuration, the environment, or a binary it bundles and controls — never a
fixed absolute literal baked into the source.

### `unload()` is where a plugin releases what it set up

`cli-echo` is also the only example that overrides the **`unload()` lifecycle
hook**. The host calls a module-level `load` export when the plugin is
discovered and a module-level `unload` export when it is unloaded; the bundle
keeps its one `Plugin` instance in a module-level variable so `unload` tears
down the same instance `load` built. A plugin that wants teardown **must**
export an `unload` function — without the export, the `unload()` hook is never
reached.

The plugin's `unload()` hook records a sentinel kanban task, calls
`this.unregister("echo")`, and then `super.unload()`. The host already disposes
every registration a plugin made automatically on unload, so the override is
not strictly required just to drop a server — `cli-echo` overrides it anyway to
show where teardown belongs and to do the cleanup automatic disposal cannot:
flushing state, notifying another service, or — as here — recording that the
plugin shut down. Any `unload()` override must call `super.unload()` so the
base class's `track`-disposable cleanup still runs.

### What the test proves, and what it does not

`PluginHost::unload` runs the plugin's `unload()` hook and *then*
unconditionally disposes every registration the plugin made. The disposal is
the authoritative cleanup — it is what produces the post-unload
`ServerUnavailable` tombstone for the `echo` server **whether or not the
plugin's own `unload()` did anything**. So observing the `echo` server is gone
proves only that the host's disposal ran; it does not, on its own, prove the
plugin's hook body executed.

That is why `cli-echo`'s `unload()` does something host-side disposal can never
do: it adds a sentinel task to a kanban board through a server still live at
`unload()` time. The end-to-end test (`tests/cli_echo_e2e.rs`) seeds an empty
board, asserts the sentinel is absent while the plugin is loaded, and asserts
it is present after `host.unload`. The sentinel's appearance proves the
plugin's `unload()` body ran; the `echo` server's `ServerUnavailable` tombstone
proves the host's registration disposal ran. Together they prove the teardown
end to end.

## `multi-module` and relative sibling-module imports

Every other example bundle is a single `index.ts`. `multi-module` is
deliberately two source files:

```
multi-module/
  index.ts            the entry module
  board-helpers.ts    a sibling module, imported by index.ts
```

Its `index.ts` opens with the line that *is* the example:

```ts
import { addBoardTask, normalizeTaskTitle } from "./board-helpers.ts";
```

The `./board-helpers.ts` specifier is **relative**. The sandboxed module loader
resolves it against the bundle's own directory, reads `board-helpers.ts` from
disk, transpiles it, and links it into the same V8 isolate as the entry module.
A plugin's logic can therefore be split across as many sibling files as the
author likes — helpers, shared types, pure functions — each pulled in with an
ordinary relative import.

### The sandbox rule on relative imports

The loader enforces one hard rule: a relative import's resolved path may not
escape the bundle directory. `./board-helpers.ts` stays inside the bundle, so
it resolves; `../outside.ts` would be rejected. The bundle directory is the
plugin's sandbox, and relative imports cannot reach beyond it.

### What the sibling module does, and how the test proves it ran

`board-helpers.ts` exports two helpers of different kinds — a **pure**
`normalizeTaskTitle` (trims and collapses whitespace in a task title) and an
**async** `addBoardTask` (adds a tagged task through a server dispatcher). It
imports only a shared SDK *type*; a sibling module need not re-import the SDK.

The entry module's `load()` registers the host-exposed `kanban` tool as `board`
and calls the imported `addBoardTask` helper to add one tagged task. The
end-to-end test (`tests/multi_module_e2e.rs`) reads the temp board back and
asserts it holds exactly that task, under the **normalized** title the helper
produced. The task can only be there if the relative import resolved and the
sibling module's code ran — a failed import throws at module resolution, before
`load()` reaches any board call.

## The capstone: the example suite across the layer stack

Each example above has its own per-bundle end-to-end test that stages the
bundle into a single layer and drives it. The capstone test,
`tests/example_layering_e2e.rs`, exercises the **whole example suite together**
through the real layer-stacking machinery — builtin, user, and project layers.

It is two tests, because a `{ rust }` module is *single-activation*: the three
examples that consume `{ rust: "kanban" }` cannot co-load into one host.

- `committed_examples_coload_across_layers` co-loads the two bundles that
  genuinely coexist — `file-notes` (consuming `{ rust: "files" }`) staged into
  a project layer, and the kanban app's built-in `kanban-builtin-probe`
  (consuming `{ rust: "kanban" }`) staged into a builtin layer — in one
  `discover_and_load_all`, asserts each plugin discovered with its layer's
  `FileSource`, asserts both effects, and unloads both cleanly.
- `each_committed_example_loads_from_its_layer` loads `kanban-tasks`,
  `multi-module`, and `cli-echo` individually, each with a fresh host and each
  staged into a different layer (user, project, builtin), so discovery is
  exercised from every layer source.

Every bundle is loaded exactly as committed — no `{ rust }` id or server name
is rewritten; the lone substitution is `cli-echo`'s `__CLI_ECHO_COMMAND__`
fixture-path token. So the example suite is not only a set of worked authoring
examples and per-bundle regression tests, but also a living proof that the
committed examples are discovered and stacked correctly across all three layers.
