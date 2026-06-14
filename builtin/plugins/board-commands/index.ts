// board-commands — builtin plugin owning the three `board.*` commands the
// board view (`apps/kanban-app/ui/src/components/board-view.tsx`) used to
// define client-side as React `CommandDef`s (`makeNewTaskCommand` /
// `makeNavCommand`). Card F of the ui-command-cleanup project moves the
// DEFINITIONS here so the CommandService catalogue is the single source of
// every command's metadata.
//
// This mirrors the `grid-commands` / `nav-commands` template:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name —
//      `board-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "focus"])` FIRST —
//      so the `commands` registry and the `focus` kernel are both live before
//      any registration — THEN `registerCommands`.
//   3. The three commands differ only by id / name / keys / wire direction,
//      so they live in ONE data table interpreted by a single `map` (the
//      `NAV_DIRECTIONS` / `GRID_COMMANDS` pattern), not three near-identical
//      object literals.
//
// # The two execution shapes
//
// `board.firstColumn` / `board.lastColumn` have a REAL backend op: they route
// to the focus kernel's `navigate focus` op host-driven with `{ window,
// direction: "first" | "last" }` — exactly the wire shape the `nav-commands`
// bundle's `nav.first` / `nav.last` use (the kernel resolves the current
// focus from `focus_by_window[window]` and PULLS the live geometry from the
// webview, so no snapshot crosses the wire). These two exist only to fill the
// keymap gap the global pair leaves: vim `0` / `$` and cua `Mod+Home` /
// `Mod+End` are NOT among `nav.first` / `nav.last`'s keys. They need no
// webview-bus handler — exactly the right case to keep OFF the bus.
//
// `board.newTask` has NO backend op: its effect is webview ORCHESTRATION —
// resolve the focused column, re-dispatch the backend-op `entity.add:task`
// command (the durable add — never inline), and focus the created card. The
// board view registers that handler on the webview command bus on mount
// (`registerWebviewCommandHandler`, Card B); `useDispatchCommand` runs the
// handler and skips the backend. The host `execute` registered here is an
// inert no-op, mirroring `nav.jump` / the `grid.*` set: it exists only to
// satisfy the registration contract and to keep a direct host-side dispatch
// — e.g. the plugin e2e where no webview is mounted — a harmless success.
//
// # Scope gating
//
// Every command carries `scope: ["ui:board"]` — the constant marker moniker
// the board view mounts via a `CommandScopeProvider` directly inside its
// `board:<id>` `<FocusScope>` (the board's spatial moniker is dynamic, so —
// like `ui:field` / `ui:filter_editor` — the component mounts a constant
// marker for the keymap's literal-moniker chain match). Scope-gated commands
// never claim a global keybinding (`extractKeymapBindings` skips them); their
// keys apply only while the focused scope chain contains the `ui:board`
// moniker, via the keymap layer's chain walk. That preserves the shadowing
// the React defs had: vim `0` / `$` inside the grid still mean row-start /
// row-end (the inner `ui:grid` gate wins), while on the board they mean
// first / last column.
//
// None of the three had a menu placement in the React defs, so none carries
// a `menu` here — the OS menu bar is unchanged.

import {
  CommandContext,
  Plugin,
  ensureServices,
  registerCommands,
  scopeId,
} from "@swissarmyhammer/plugin";

/**
 * The dispatch surface for the `focus` operation tool — the spatial-nav
 * kernel op the column-extreme commands route to.
 *
 * The dispatch Proxy turns a property path into an MCP `tools/call`:
 * `server.tool.noun.verb`. For the `focus` server the server name and the
 * single tool name are both `"focus"`; the noun/verb pair comes straight
 * from `crates/swissarmyhammer-focus/src/operations.rs`:
 *   `navigate focus` → this.focus.focus.focus.navigate
 */
interface FocusDispatch {
  focus: {
    focus: {
      focus: {
        navigate(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/** One board command's identity + metadata + optional wire direction.
 * `direction` present means the command routes to the focus kernel's
 * `navigate focus` op; absent means it is "handled in webview" and the host
 * execute is an inert no-op (`board.newTask`). */
interface BoardCommandSpec {
  id: string;
  name: string;
  keys: Record<string, string>;
  /** The `Direction` wire literal the `navigate focus` op accepts. */
  direction?: "first" | "last";
}

/**
 * The three board commands, as a data table.
 *
 * `id` / `name` / `keys` are copied 1:1 from the retired client-side
 * `CommandDef`s in `board-view.tsx` (`makeNewTaskCommand` /
 * `makeNavCommand`); `direction` is the lowercase `Direction` wire literal
 * (`operations.rs::Navigate`). Holding the variation as data keeps the three
 * registrations a single `map`.
 */
const BOARD_COMMANDS: readonly BoardCommandSpec[] = [
  // ── New task — webview orchestration (no backend op) ─────────────────────
  // The webview handler resolves the focused column, re-dispatches the
  // backend-op `entity.add:task` (the durable add), and focuses the new card.
  {
    id: "board.newTask",
    name: "New Task",
    keys: { vim: "o", cua: "Mod+Enter" },
  },
  // ── Column extremes — focus kernel `navigate focus` (first / last) ──────
  // Fill the keymap gap the global `nav.first` / `nav.last` leave: vim
  // `0` / `$`, cua `Mod+Home` / `Mod+End`. Same kernel op, board-gated keys.
  {
    id: "board.firstColumn",
    name: "First Column",
    keys: { vim: "0", cua: "Mod+Home" },
    direction: "first",
  },
  {
    id: "board.lastColumn",
    name: "Last Column",
    keys: { vim: "$", cua: "Mod+End" },
    direction: "last",
  },
];

/**
 * The board-commands builtin plugin.
 *
 * Registers the three `board.*` commands: `board.firstColumn` /
 * `board.lastColumn` route to the focus kernel's `navigate focus` op
 * host-driven (first / last); `board.newTask` is webview-bus handled — the
 * board React tree owns the live behavior and the host execute is an inert
 * no-op. Identity is the bundle directory name (`board-commands`); `name` /
 * `description` are descriptive metadata only.
 */
export default class BoardCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Board Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin board-view commands (new task, first/last column) — column extremes routed to the focus kernel; new-task orchestration runs in the webview via the command bus.";

  /**
   * Activate the `commands` registry and the `focus` kernel, then register
   * the three board commands from the data table.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "focus"]);

    const focus = this as unknown as FocusDispatch;

    await registerCommands(
      this,
      BOARD_COMMANDS.map((spec) => ({
        id: spec.id,
        name: spec.name,
        undoable: false,
        // Gate to the board zone: keys apply only while the `ui:board`
        // marker is in the focused scope chain; never lifted into the
        // global key table.
        scope: ["ui:board"],
        keys: spec.keys,
        execute: spec.direction
          ? // Host-driven navigate: send only `{ window, direction }`. The
            // kernel resolves the current focus from `focus_by_window` and
            // pulls the live geometry from the webview — no snapshot on the
            // wire. A dispatch with no resolvable window drops silently
            // (the op no-ops on an undefined window), matching the retired
            // React def's "no kernel, nothing to navigate" fallback.
            async (rawCtx: unknown) => {
              const ctx = (rawCtx ?? {}) as CommandContext;
              const window = scopeId(ctx, "window");
              return await focus.focus.focus.focus.navigate({
                window,
                direction: spec.direction,
              });
            }
          : // Presentation-only (`board.newTask`): the webview bus handler
            // (registered by the board view on mount) intercepts this id in
            // `useDispatchCommand` before the backend, so this host
            // `execute` is never reached in production. It exists as an
            // inert no-op only to satisfy the registration contract and to
            // keep a direct host-side dispatch a harmless success (mirrors
            // `nav.jump` in nav-commands).
            async () => {
              return { ok: true };
            },
      })),
    );

    this.log.info(
      "board-commands: registered 3 board.* (firstColumn/lastColumn → focus navigate first/last; newTask → webview bus)",
    );
  }
}
