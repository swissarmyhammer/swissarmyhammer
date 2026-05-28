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
//   file.switchBoard → window `switch board` (this.window.window.board.switch)
//   file.closeBoard  → window `close board`  (this.window.window.board.close)
//   file.newBoard    → window `new board`    (this.window.window.board.new)
//   file.openBoard   → window `open board`   (this.window.window.board.open)
//
// (Note: `window.new` is NOT here — it is sourced from `ui.yaml` and ported by
// the ui-commands plugin task.)

import {
  Plugin,
  ensureServices,
  registerCommands,
  makePluginThis,
} from "@swissarmyhammer/plugin";

// ───────────────────────────────────────────────────────────────────────────
// The command context the host hands every `execute` callback.
// ───────────────────────────────────────────────────────────────────────────

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext`: the active scope
 * monikers, the optional context-menu target moniker, and a free-form args
 * bag the dispatching surface populates. The board-file commands carry no
 * scope or params in `file.yaml`, so they read only `args` (the `switchBoard`
 * / `closeBoard` palette pre-fills the target board `path`); `newBoard` /
 * `openBoard` take no input and drive the OS picker on the host side.
 */
interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/**
 * The file-commands builtin plugin.
 *
 * Registers the four board-file lifecycle commands ported from `file.yaml`,
 * each wired to the `window` MCP server's board-lifecycle verb. Identity is
 * the bundle directory name (`file-commands`); `name` / `description` are
 * descriptive metadata only.
 */
class FileCommandsPlugin extends Plugin {
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
   * registration is the source YAML's metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "window"]);

    await registerCommands(this, [
      // ─── file.switchBoard ───────────────────────────────────────────────
      // YAML (file.yaml): undoable:false, no keys/menu. Routes to window
      // `switch board`, threading the target board `path` from args.
      {
        id: "file.switchBoard",
        name: "Switch Board",
        undoable: false,
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const path = ctx.args?.path;
          return await this.window.window.board.switch({ path });
        },
      },

      // ─── file.closeBoard ────────────────────────────────────────────────
      // YAML (file.yaml): undoable:false, keys cua/vim Mod+W, menu File/0/2.
      // Routes to window `close board`, threading the target board `path`.
      {
        id: "file.closeBoard",
        name: "Close Board",
        undoable: false,
        keys: { cua: "Mod+W", vim: "Mod+W" },
        menu: { path: ["File"], group: 0, order: 2 },
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const path = ctx.args?.path;
          return await this.window.window.board.close({ path });
        },
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
        execute: async () => {
          return await this.window.window.board.new({});
        },
      },

      // ─── file.openBoard ─────────────────────────────────────────────────
      // YAML (file.yaml): undoable:false, keys cua Mod+O, menu File/0/1.
      // Routes to window `open board`, which drives the host OS file-open
      // dialog and opens the chosen board.
      {
        id: "file.openBoard",
        name: "Open Board",
        undoable: false,
        keys: { cua: "Mod+O" },
        menu: { path: ["File"], group: 0, order: 1 },
        execute: async () => {
          return await this.window.window.board.open({});
        },
      },
    ]);

    this.log.info(
      "file-commands: registered file.switchBoard, file.closeBoard, file.newBoard, file.openBoard",
    );
  }
}

/**
 * The plugin entry point.
 *
 * The host calls this once when the bundle is discovered: build the plugin,
 * wrap it with `makePluginThis` so `this.<server>` dispatch works, and run
 * `load()`.
 *
 * @returns `null` — this plugin's only effect is its load-time registrations.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new FileCommandsPlugin()) as FileCommandsPlugin;
  await plugin.load();
  return null;
}
