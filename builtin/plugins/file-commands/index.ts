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
   * registration is the source YAML's metadata, 1:1. Binding {@link windowD}
   * before registering hands the hoisted `FILE_COMMANDS` table its dispatch.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "window"]);

    windowD = this as unknown as WindowDispatch;

    await registerCommands(this, FILE_COMMANDS);

    this.log.info(
      "file-commands: registered file.switchBoard, file.closeBoard, file.newBoard, file.openBoard",
    );
  }
}
