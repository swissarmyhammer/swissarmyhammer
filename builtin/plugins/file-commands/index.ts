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
//   file.switchBoard → window `switch board` (windowD.window.window.board.switch)
//   file.closeBoard  → window `close board`  (windowD.window.window.board.close)
//   file.newBoard    → window `new board`    (windowD.window.window.board.new)
//   file.openBoard   → window `open board`   (windowD.window.window.board.open)
//
// (Note: `window.new` is NOT here — it is sourced from `ui.yaml` and ported by
// the app-shell-commands plugin task.)
//
// ───────────────────────────────────────────────────────────────────────────
// Hotkey-canonical key literals
// ───────────────────────────────────────────────────────────────────────────
//
// Since Card I deleted `app-shell.tsx`'s static scope defs, this registry
// metadata is the ONLY key source for the webview hotkey path:
// `extractKeymapBindings` matches the declared string LITERALLY against
// `normalizeKeyEvent` output, which emits lowercase letters for unshifted
// chords. So every unshifted-letter chord is declared lowercase (`Mod+w`,
// `Mod+o`, not `Mod+W` / `Mod+O`) or it is structurally unreachable from a real
// keydown. The macOS native menu accelerators parse letters case-insensitively,
// so the lowercase form serves both sides. The
// `file-plugin-commands-mirror.spatial.node.test.ts` drift guard pins the
// `FILE_COMMANDS` keys against `BINDING_TABLES`.

import {
  CommandContext,
  Plugin,
  bindCommandRun,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/** A registration row, as `registerCommands` accepts. */
type CommandSpec = Record<string, unknown>;

/**
 * The dispatch surface for the `window` operation tool — the board-file
 * lifecycle verbs the four `file.*` commands route to.
 *
 * The dispatch Proxy turns a property path into an MCP `tools/call`:
 * `server.tool.noun.verb`. For the `window` server the server name and the
 * single tool name are both `"window"`; the noun/verb pairs come from
 * `crates/swissarmyhammer-window-service/src/operations.rs`:
 *   `switch board` → windowD.window.window.board.switch
 *   `close board`  → windowD.window.window.board.close
 *   `new board`    → windowD.window.window.board.new
 *   `open board`   → windowD.window.window.board.open
 */
interface WindowDispatch {
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

/**
 * Dispatch holder the {@link FILE_COMMANDS} executes close over.
 *
 * `FILE_COMMANDS` is a module-level data table (so the frontend drift guard can
 * parse its `id` / `name` / `keys` from source the way it parses
 * `nav-commands`' `NAV_DIRECTIONS`), but the `file.*` executes route to the real
 * `window` backend. Rather than nest the array inside `load()` (which would
 * defeat the `const NAME = [ … ];` anchor the guard relies on), the executes
 * read this holder, which `load()` sets before registering. Each plugin runs in
 * its OWN isolate with exactly one `FileCommandsPlugin`, so this module-level
 * state is per-plugin-instance — no cross-instance sharing.
 */
let windowD: WindowDispatch | null = null;

/**
 * The four `file.*` board-file lifecycle command registrations, as a
 * module-level data table (the same hoisted-table structure as `nav-commands`'
 * `NAV_DIRECTIONS`, which lets the frontend drift guard
 * `file-plugin-commands-mirror.spatial.node.test.ts` parse it from source).
 * Each `execute` reads the {@link windowD} dispatch holder `load()` sets.
 *
 * `keys` use the canonical lowercase form `normalizeKeyEvent` emits for an
 * unshifted letter chord — see the module header. The drift guard pins this
 * against `BINDING_TABLES`.
 */
const FILE_COMMANDS: CommandSpec[] = [
  // ─── file.switchBoard ───────────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, no keys/menu. Routes to window
  // `switch board`, threading the target board `path` from args.
  {
    id: "file.switchBoard",
    name: "Switch Board",
    undoable: false,
    execute: async (rawCtx) => {
      const ctx = (rawCtx ?? {}) as CommandContext;
      const path = ctx.args?.path;
      return await windowD!.window.window.board.switch({ path });
    },
  },

  // ─── file.closeBoard ────────────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, keys cua/vim Mod+w, menu File/0/2.
  // Routes to window `close board`, threading the target board `path`.
  //
  // The keys use the canonical lowercase form `normalizeKeyEvent` emits for an
  // unshifted letter chord (`Mod+w`, not `Mod+W`). `BINDING_TABLES` binds
  // `Mod+w` → `file.closeBoard` in cua and vim, so the drift guard pins it.
  {
    id: "file.closeBoard",
    name: "Close Board",
    undoable: false,
    keys: { cua: "Mod+w", vim: "Mod+w" },
    menu: { path: ["File"], group: 0, order: 2 },
    execute: async (rawCtx) => {
      const ctx = (rawCtx ?? {}) as CommandContext;
      const path = ctx.args?.path;
      return await windowD!.window.window.board.close({ path });
    },
  },

  // ─── file.newBoard ──────────────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, keys cua Mod+Shift+B, menu File/0/0.
  // Routes to window `new board`, which drives the host folder picker and
  // initializes a board at the chosen folder.
  //
  // `Mod+Shift+B` is already canonical (a shifted letter keeps its uppercase).
  // There is no `BINDING_TABLES` entry — it rides the native File-menu
  // accelerator only — so the drift guard treats it as a COMMENTED
  // menu-accelerator-only allowlist entry.
  {
    id: "file.newBoard",
    name: "New Board",
    undoable: false,
    keys: { cua: "Mod+Shift+B" },
    menu: { path: ["File"], group: 0, order: 0 },
    execute: async () => {
      return await windowD!.window.window.board.new({});
    },
  },

  // ─── file.openBoard ─────────────────────────────────────────────────────
  // YAML (file.yaml): undoable:false, keys cua Mod+o, menu File/0/1.
  // Routes to window `open board`, which drives the host OS file-open
  // dialog and opens the chosen board.
  //
  // The cua key is canonicalized to lowercase `Mod+o` (was `Mod+O`): there is
  // NO `BINDING_TABLES` entry for file.openBoard — it rides the native
  // File-menu accelerator — but the lowercase form keeps the accelerator
  // working (its letter parse is case-insensitive) AND makes the chord
  // reachable in the webview on non-Mac, matching the `file.closeBoard`
  // precedent (Card I). The drift guard treats file.openBoard as a COMMENTED
  // menu-accelerator-only allowlist entry (no `BINDING_TABLES` row to pin
  // against).
  {
    id: "file.openBoard",
    name: "Open Board",
    undoable: false,
    keys: { cua: "Mod+o" },
    menu: { path: ["File"], group: 0, order: 1 },
    execute: async () => {
      return await windowD!.window.window.board.open({});
    },
  },
];

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
