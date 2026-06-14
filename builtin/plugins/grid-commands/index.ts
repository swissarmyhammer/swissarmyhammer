// grid-commands — builtin plugin owning the eleven `grid.*` commands that the
// grid view (`apps/kanban-app/ui/src/components/grid-view.tsx`) used to define
// client-side as React `CommandDef`s. Card C of the ui-command-cleanup project
// moves the DEFINITIONS here so the CommandService catalogue is the single
// source of every command's metadata; the grid React tree only registers the
// live BEHAVIOR for each id on the webview command bus
// (`registerWebviewCommandHandler`, Card B).
//
// This mirrors the `nav-commands` template:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name — `grid-commands`).
//   2. `load()` calls `ensureServices(this, ["commands"])` FIRST, THEN
//      `registerCommands`. No other backend is reached: every grid command is
//      "handled in webview".
//   3. The eleven commands differ only by id / name / keys, so they live in
//      ONE data table interpreted by a single `map` (the `NAV_DIRECTIONS`
//      pattern), not eleven near-identical object literals.
//
// # Why every host `execute` is an inert no-op
//
// Each command's effect is pure presentation deep inside the grid React tree:
// moving the cell cursor through the spatial-nav kernel
// (`grid.moveToRowStart` / `grid.moveToRowEnd` / `grid.firstCell` /
// `grid.lastCell` re-dispatch `nav.focus` against a computed cell FQM),
// toggling the live grid handle's edit / visual mode (`grid.edit` /
// `grid.editEnter` / `grid.exitEdit` / `grid.toggleVisual`), or re-dispatching
// an existing backend-op command (`grid.deleteRow` → `entity.archive`
// targeting the cursor row's moniker, `grid.newBelow` / `grid.newAbove` →
// `entity.add:{entityType}`). The grid
// view registers a webview-bus handler per id on mount; `useDispatchCommand`
// runs that handler and skips the backend, exactly like `nav.jump`. The host
// `execute` registered here exists only to satisfy the registration contract
// (every command carries an `execute`) and to keep a direct host-side dispatch
// — e.g. the plugin e2e where no webview is mounted — a harmless success.
//
// # Scope gating
//
// Every command carries `scope: ["ui:grid"]` — the literal moniker of the grid
// body's `<FocusScope moniker="ui:grid">` zone. Scope-gated commands never
// claim a global keybinding (`extractKeymapBindings` skips them); their keys
// apply only while the focused scope chain contains the `ui:grid` moniker,
// via the keymap layer's scoped-registry binding extraction. That preserves
// the shadowing the React defs had: `Home` / `End` inside the grid mean
// row-start / row-end, while the global `nav.first` / `nav.last` keep those
// keys everywhere else.
//
// None of the eleven had a menu placement in the React defs, so none carries
// a `menu` here — the OS menu bar is unchanged.

import {
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/** One grid command's identity + metadata. `keys` is absent for the ids the
 * React defs bound no key to (`grid.exitEdit`, `grid.deleteRow`). */
interface GridCommandSpec {
  id: string;
  name: string;
  keys?: Record<string, string>;
}

/**
 * The eleven grid commands, as a data table.
 *
 * `id` / `name` / `keys` are copied 1:1 from the retired client-side
 * `CommandDef`s in `grid-view.tsx` (`buildGridExtremeCommands` /
 * `buildGridModeCommands` / `buildGridRowCommands`). Holding the variation as
 * data keeps the eleven registrations a single `map`.
 */
const GRID_COMMANDS: readonly GridCommandSpec[] = [
  // ── Row-extreme / grid-extreme cell jumps ────────────────────────────────
  // The webview handlers re-dispatch `nav.focus` against the computed
  // destination cell FQM — pure presentation routed through the kernel.
  {
    id: "grid.moveToRowStart",
    name: "Row Start",
    keys: { vim: "0", cua: "Home" },
  },
  {
    id: "grid.moveToRowEnd",
    name: "Row End",
    keys: { vim: "$", cua: "End" },
  },
  {
    id: "grid.firstCell",
    name: "First Cell",
    keys: { cua: "Mod+Home" },
  },
  {
    id: "grid.lastCell",
    name: "Last Cell",
    keys: { cua: "Mod+End" },
  },
  // ── Edit / visual mode toggles on the live grid handle ───────────────────
  {
    id: "grid.edit",
    name: "Edit Cell",
    keys: { vim: "i", cua: "Enter" },
  },
  {
    id: "grid.editEnter",
    name: "Edit Cell (Enter)",
    keys: { vim: "Enter" },
  },
  {
    id: "grid.exitEdit",
    name: "Exit Edit",
  },
  {
    id: "grid.toggleVisual",
    name: "Toggle Visual Mode",
    keys: { vim: "v" },
  },
  // ── Row mutations — the webview handlers re-dispatch backend-op commands
  // (`entity.archive` on the row's target moniker / `entity.add:{entityType}`),
  // never inline. ───────────────────────────────────────────────────────────
  {
    id: "grid.deleteRow",
    name: "Delete Row",
  },
  {
    id: "grid.newBelow",
    name: "New Row Below",
    keys: { vim: "o", cua: "Mod+Enter" },
  },
  {
    id: "grid.newAbove",
    name: "New Row Above",
    keys: { vim: "O", cua: "Mod+Shift+Enter" },
  },
];

/**
 * The grid-commands builtin plugin.
 *
 * Registers the eleven `grid.*` commands. Every one is webview-bus handled —
 * the grid React tree owns the live behavior; the host execute is an inert
 * no-op. Identity is the bundle directory name (`grid-commands`); `name` /
 * `description` are descriptive metadata only.
 */
export default class GridCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Grid Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin grid-view commands (cell jumps, edit/visual mode, row mutations) — definitions only; behaviors run in the webview via the command bus.";

  /**
   * Activate the `commands` registry, then register the eleven grid commands
   * from the data table. No other service is needed: every command is
   * presentation-only on the host side.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands"]);

    await registerCommands(
      this,
      GRID_COMMANDS.map((spec) => ({
        id: spec.id,
        name: spec.name,
        undoable: false,
        // Gate to the grid zone: keys apply only while `ui:grid` is in the
        // focused scope chain; never lifted into the global key table.
        scope: ["ui:grid"],
        ...(spec.keys ? { keys: spec.keys } : {}),
        // Presentation-only: the webview bus handler (registered by the grid
        // view on mount) intercepts this id in `useDispatchCommand` before
        // the backend, so this host `execute` is never reached in production.
        // It exists as an inert no-op only to satisfy the registration
        // contract and to keep a direct host-side dispatch a harmless
        // success (mirrors `nav.jump` in nav-commands).
        execute: async () => {
          return { ok: true };
        },
      })),
    );

    this.log.info(
      "grid-commands: registered 11 grid.* (moveToRowStart/moveToRowEnd/firstCell/lastCell/edit/editEnter/exitEdit/toggleVisual/deleteRow/newBelow/newAbove → webview bus)",
    );
  }
}
