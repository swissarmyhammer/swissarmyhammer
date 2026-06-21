// file-commands — builtin plugin porting `file.yaml` (the board-file lifecycle
// commands) to the TypeScript plugin SDK.
//
// This mirrors the `task-commands` / `kanban-misc-commands` template exactly:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name — `file-commands`).
//   2. `load()` calls `ensureServices(this, [...])` FIRST to activate every
//      host service the plugin's commands route to (`commands`, `window`),
//      THEN `registerCommands`.
//   3. Each registration carries the FULL UI metadata from the source YAML —
//      `keys`, `menu`, `undoable` — 1:1, so the command behaves identically to
//      the YAML-driven version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into the backing `window` server's board-lifecycle verb.
//
// The four registrations differ only by id / name / keys / menu / wire verb,
// so they live in ONE module-level `FILE_COMMANDS` data table interpreted by a
// single `map` (the `BOARD_COMMANDS` / `AI_COMMANDS` pattern). Holding the
// static metadata at module scope lets the keymap drift guard
// (`file-plugin-commands-mirror.spatial.node.test.ts`) parse the keys from
// source and pin them against `BINDING_TABLES`.
//
// # Key canonicalization (Card I sweep)
//
// `file.closeBoard` and `file.openBoard` declare their unshifted-letter chord
// in the canonical lowercase form `normalizeKeyEvent` emits (`Mod+w` / `Mod+o`,
// not `Mod+W` / `Mod+O`): since Card I deleted `app-shell.tsx`'s static scope
// defs, this registry metadata is the only webview key source, and
// `extractKeymapBindings` matches the literal — an uppercase unshifted letter
// is unreachable from a real keydown. Neither has a `BINDING_TABLES` entry
// (both ride the native menu accelerator, which parses letters
// case-insensitively); the lowercase form keeps the accelerator AND makes the
// chord reachable in the webview on non-Mac.
//
// Backend routing (one `window` MCP call per command):
//   file.switchBoard → window `switch board` (this.window.window.board.switch)
//   file.closeBoard  → window `close board`  (this.window.window.board.close)
//   file.newBoard    → window `new board`    (this.window.window.board.new)
//   file.openBoard   → window `open board`   (this.window.window.board.open)
//
// (Note: `window.new` is NOT here — it is sourced from `ui.yaml` and ported by
// the app-shell-commands plugin task.)

import {
  CommandContext,
  Plugin,
  bindCommandRun,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/**
 * The dispatch surface for the `window` operation tool's board-lifecycle
 * verbs, rooted at the plugin instance (`this`). The dispatch Proxy turns a
 * property path into an MCP `tools/call`:
 * `this.window.window.board.{switch,close,new,open}` (server `window`, tool
 * `window`, noun `board`, verb).
 */
interface WindowBoardDispatch {
  window: {
    window: {
      board: {
        switch(args: Record<string, unknown>): Promise<unknown>;
        close(args: Record<string, unknown>): Promise<unknown>;
        new (args: Record<string, unknown>): Promise<unknown>;
        open(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/** The board-lifecycle verbs a `file.*` command routes to (server `window`,
 * tool `window`, noun `board`). */
type BoardVerb = "switch" | "close" | "new" | "open";

/** One `file.*` registration: the static `file.yaml` metadata literals
 * (`id` / `name` / `keys` / `menu` / `undoable`) the catalogue and the keymap
 * drift guard read, plus the routing data (`verb` + `passPath`) that — through
 * `boardRun` — drives the single `window` MCP call. The four commands differ
 * ONLY in this metadata + routing data, so `run` is derived (not hand-written
 * per row) to keep them from drifting out of lockstep. */
interface FileCommandSpec {
  id: string;
  name: string;
  undoable: boolean;
  keys?: Record<string, string>;
  menu?: Record<string, unknown>;
  /** The `board.<verb>` op this command dispatches. */
  verb: BoardVerb;
  /** Whether the command threads the target board `path` from `ctx.args.path`
   * (switch / close act on a specific board; new / open drive a host picker
   * and take no path). Absent ⇒ no path argument. */
  passPath?: boolean;
}

/**
 * Build the `run` for a `FileCommandSpec` from its routing data: one code path
 * dispatching `window.window.window.board[verb](...)`, threading the target
 * board `path` only when `passPath` is set. This collapses what were four
 * parallel `run` closures (differing solely by verb and whether they pass a
 * path) into a single interpreter of the table's routing fields.
 */
function boardRun(
  spec: FileCommandSpec,
): (ctx: CommandContext, window: WindowBoardDispatch) => Promise<unknown> {
  return (ctx, window) =>
    window.window.window.board[spec.verb](
      spec.passPath ? { path: ctx.args?.path } : {},
    );
}

/**
 * The four board-file lifecycle commands, as a module-level data table.
 *
 * `id` / `name` / `keys` / `menu` / `undoable` are `file.yaml`'s metadata 1:1 —
 * held as literals at module scope so the keymap drift guard
 * (`file-plugin-commands-mirror.spatial.node.test.ts`) can parse the keys from
 * source. The backend call is expressed as DATA (`verb` + `passPath`)
 * interpreted by the single `boardRun` code path, which `load()` binds to an
 * `execute` over the live `window` dispatch surface.
 */
const FILE_COMMANDS: readonly FileCommandSpec[] = [
  // ─── file.switchBoard ───────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, no keys/menu. Routes to window
  // `switch board`, threading the target board `path` from args.
  {
    id: "file.switchBoard",
    name: "Switch Board",
    undoable: false,
    verb: "switch",
    passPath: true,
  },

  // ─── file.closeBoard ────────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, keys cua/vim Mod+w (canonical lowercase,
  // see the file header), menu File/0/2. Routes to window `close board`,
  // threading the target board `path`.
  {
    id: "file.closeBoard",
    name: "Close Board",
    undoable: false,
    keys: { cua: "Mod+w", vim: "Mod+w" },
    menu: { path: ["File"], group: 0, order: 2 },
    verb: "close",
    passPath: true,
  },

  // ─── file.newBoard ──────────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, keys cua Mod+Shift+B, menu File/0/0.
  // Routes to window `new board`, which drives the host folder picker and
  // initializes a board at the chosen folder.
  {
    id: "file.newBoard",
    name: "New Board",
    undoable: false,
    keys: { cua: "Mod+Shift+B" },
    menu: { path: ["File"], group: 0, order: 0 },
    verb: "new",
  },

  // ─── file.openBoard ─────────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, keys cua Mod+o (canonical lowercase, see
  // the file header), menu File/0/1. Routes to window `open board`, which
  // drives the host OS file-open dialog and opens the chosen board.
  {
    id: "file.openBoard",
    name: "Open Board",
    undoable: false,
    keys: { cua: "Mod+o" },
    menu: { path: ["File"], group: 0, order: 1 },
    verb: "open",
  },
];

/**
 * The file-commands builtin plugin.
 *
 * Registers the four board-file lifecycle commands ported from `file.yaml`,
 * each wired to the `window` MCP server's board-lifecycle verb. Identity is
 * the bundle directory name (`file-commands`); `name` / `description` are
 * descriptive metadata only.
 */
export default class FileCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "File Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin board-file lifecycle commands (switch / close / new / open board) routed to the window server.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry and the `window` backend are both live
   * before any registration — then `registerCommands`. The metadata on each
   * registration is the source YAML's metadata, 1:1, mapped from the
   * `FILE_COMMANDS` data table: each row's routing data (`verb` / `passPath`)
   * is interpreted by `boardRun` into a `run`, which `bindCommandRun` binds to
   * an `execute` over the live `window` dispatch surface.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "window"]);

    const window = this as unknown as WindowBoardDispatch;

    await registerCommands(
      this,
      FILE_COMMANDS.map((spec) => {
        // Strip the routing-only fields (`verb` / `passPath`) — they drive
        // `boardRun` but are not `register command` metadata — and replace
        // them with the synthesized `run` that `bindCommandRun` turns into
        // `execute`.
        const { verb: _verb, passPath: _passPath, ...metadata } = spec;
        return bindCommandRun({ ...metadata, run: boardRun(spec) }, window);
      }),
    );

    this.log.info(
      "file-commands: registered file.switchBoard, file.closeBoard, file.newBoard, file.openBoard",
    );
  }
}
