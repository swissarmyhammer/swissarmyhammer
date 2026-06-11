/**
 * Test helper: synthesize the global command registry that `useCommandList`
 * reads, from the static `BINDING_TABLES`.
 *
 * The hotkey path now sources its global keybindings from the metadata-driven
 * Command registry (`list command`) rather than a hardcoded table. In
 * production the host's registry surfaces those global commands with their
 * per-keymap `keys`; in tests the host is mocked, so integration tests that
 * exercise global keybindings (palette open, nav, dismiss) must publish the
 * same set through the `useCommandList` seam.
 *
 * `BINDING_TABLES` is the canonical encoding of the production global keymap,
 * so this helper collapses every keymap's `key → id` mapping into one
 * `CommandMetadata` per id carrying its per-keymap `keys` — exactly what
 * `extractKeymapBindings` reads back out. Tests route their `invoke` mock's
 * `command_tool_call` branch through {@link listCommandResult} (for the
 * `list command` op) so `useCommandList` resolves the global set.
 */

import { BINDING_TABLES } from "@/lib/keybindings";
import type { CommandMetadata } from "@/hooks/use-command-list";

/**
 * Key metadata for the `nav-commands` builtin plugin's directional commands,
 * mirrored 1:1 from `builtin/plugins/nav-commands/index.ts::NAV_DIRECTIONS`.
 *
 * In production these reach the registry through the plugin catalogue — NOT
 * `BINDING_TABLES`, which only carries the static no-focus fallback set (the
 * directional keys were removed from it when nav execution moved host-side).
 * The synthesized registry must include them or arrow-key / hjkl navigation
 * never resolves in tests.
 *
 * Exported for the drift guard
 * (`nav-plugin-commands-mirror.spatial.node.test.ts`), which parses the
 * plugin source from disk and fails loudly when this mirror drifts.
 */
export const NAV_PLUGIN_COMMANDS: CommandMetadata[] = [
  {
    id: "nav.up",
    name: "Navigate Up",
    keys: { vim: "k", cua: "ArrowUp", emacs: "Ctrl+p" },
  },
  {
    id: "nav.down",
    name: "Navigate Down",
    keys: { vim: "j", cua: "ArrowDown", emacs: "Ctrl+n" },
  },
  {
    id: "nav.left",
    name: "Navigate Left",
    keys: { vim: "h", cua: "ArrowLeft", emacs: "Ctrl+b" },
  },
  {
    id: "nav.right",
    name: "Navigate Right",
    keys: { vim: "l", cua: "ArrowRight", emacs: "Ctrl+f" },
  },
  {
    id: "nav.first",
    name: "Navigate to First",
    keys: { cua: "Home", emacs: "Alt+<" },
  },
  {
    id: "nav.last",
    name: "Navigate to Last",
    keys: { vim: "Shift+G", cua: "End", emacs: "Alt+>" },
  },
];

/**
 * Key metadata for the `grid-commands` builtin plugin's eleven commands,
 * mirrored 1:1 from `builtin/plugins/grid-commands/index.ts::GRID_COMMANDS`
 * (Card C — the grid command DEFINITIONS live in the plugin; grid-view.tsx
 * only registers webview-bus handlers for the ids).
 *
 * Every entry is scope-gated to the grid zone (`scope: ["ui:grid"]`), so
 * `extractKeymapBindings` never lifts its keys into the global table; the
 * keys bind only while the focused scope chain contains the literal
 * `ui:grid` moniker, via `extractChainBindings`. Tests that drive
 * grid keystrokes (Home / End / Enter / Mod+Home / …) need these in the
 * synthesized registry or the grid bindings never resolve.
 *
 * Exported for the drift guard
 * (`grid-plugin-commands-mirror.spatial.node.test.ts`), which parses the
 * plugin source from disk and fails loudly when this mirror drifts.
 */
export const GRID_PLUGIN_COMMANDS: CommandMetadata[] = [
  {
    id: "grid.moveToRowStart",
    name: "Row Start",
    scope: ["ui:grid"],
    keys: { vim: "0", cua: "Home" },
  },
  {
    id: "grid.moveToRowEnd",
    name: "Row End",
    scope: ["ui:grid"],
    keys: { vim: "$", cua: "End" },
  },
  {
    id: "grid.firstCell",
    name: "First Cell",
    scope: ["ui:grid"],
    keys: { cua: "Mod+Home" },
  },
  {
    id: "grid.lastCell",
    name: "Last Cell",
    scope: ["ui:grid"],
    keys: { cua: "Mod+End" },
  },
  {
    id: "grid.edit",
    name: "Edit Cell",
    scope: ["ui:grid"],
    keys: { vim: "i", cua: "Enter" },
  },
  {
    id: "grid.editEnter",
    name: "Edit Cell (Enter)",
    scope: ["ui:grid"],
    keys: { vim: "Enter" },
  },
  {
    id: "grid.exitEdit",
    name: "Exit Edit",
    scope: ["ui:grid"],
  },
  {
    id: "grid.toggleVisual",
    name: "Toggle Visual Mode",
    scope: ["ui:grid"],
    keys: { vim: "v" },
  },
  {
    id: "grid.deleteRow",
    name: "Delete Row",
    scope: ["ui:grid"],
  },
  {
    id: "grid.newBelow",
    name: "New Row Below",
    scope: ["ui:grid"],
    keys: { vim: "o", cua: "Mod+Enter" },
  },
  {
    id: "grid.newAbove",
    name: "New Row Above",
    scope: ["ui:grid"],
    keys: { vim: "O", cua: "Mod+Shift+Enter" },
  },
];

/**
 * Key metadata for the `ui-commands` builtin plugin's four UI-surface
 * commands, mirrored 1:1 from
 * `builtin/plugins/ui-commands/index.ts::UI_SURFACE_COMMANDS` (Card D — the
 * field-edit and pressable-activation command DEFINITIONS live in the plugin;
 * field.tsx and pressable.tsx only register webview-bus handlers for the ids
 * while spatial focus is within their instance's subtree — the zone itself
 * or a descendant such as a tag pill — matching the keymap's marker-in-chain
 * gate; a pressable is a spatial leaf, so containment degenerates to direct
 * focus).
 *
 * Each entry is scope-gated to its surface's marker moniker (`ui:field` /
 * `ui:pressable`) — the literal moniker the component mounts via a
 * `CommandScopeProvider` directly above its `<FocusScope>` — so
 * `extractKeymapBindings` never lifts its keys into the global table; the
 * keys bind only while the marker is in the focused chain, via the
 * depth-interleaved chain walk (`extractChainBindings`). Tests that drive
 * Enter / Space on a focused field zone or pressable leaf need these in the
 * synthesized registry or the bindings never resolve.
 *
 * Exported for the drift guard
 * (`ui-surface-plugin-commands-mirror.spatial.node.test.ts`), which parses
 * the plugin source from disk and fails loudly when this mirror drifts.
 */
export const UI_SURFACE_PLUGIN_COMMANDS: CommandMetadata[] = [
  {
    id: "field.edit",
    name: "Edit Field",
    scope: ["ui:field"],
    keys: { vim: "i", cua: "Enter" },
  },
  {
    id: "field.editEnter",
    name: "Edit Field (Enter)",
    scope: ["ui:field"],
    keys: { vim: "Enter" },
  },
  {
    id: "pressable.activate",
    name: "Activate",
    scope: ["ui:pressable"],
    keys: { vim: "Enter", cua: "Enter" },
  },
  {
    id: "pressable.activateSpace",
    name: "Activate (Space)",
    scope: ["ui:pressable"],
    keys: { cua: "Space" },
  },
];

/**
 * Build the global command registry from `BINDING_TABLES` (one command per id,
 * each carrying its `keys` map keyed by keymap mode) plus the
 * `nav-commands` plugin's directional commands, the `grid-commands`
 * plugin's grid commands, and the `ui-commands` plugin's UI-surface
 * commands, which carry their keys on the plugin catalogue rather than the
 * static tables.
 *
 * The plugin entries stay separate from the `byId` merge: a command id can
 * own several keys per mode (e.g. cua binds both `Tab` and `ArrowRight` to
 * `nav.right`), and `extractKeymapBindings` keys its table by KEY, so two
 * metadata entries with the same id simply contribute two bindings. The
 * grid and UI-surface entries are scope-gated, so they contribute nothing to
 * the global table — they surface only through the scoped chain walk when
 * their zone moniker is in the focused chain.
 */
export function globalCommandsFromBindingTables(): CommandMetadata[] {
  const byId: Record<string, CommandMetadata> = {};
  for (const mode of ["vim", "cua", "emacs"] as const) {
    for (const [key, id] of Object.entries(BINDING_TABLES[mode])) {
      byId[id] ??= { id, name: id, keys: {} };
      (byId[id].keys as Record<string, string>)[mode] = key;
    }
  }
  return [
    ...NAV_PLUGIN_COMMANDS,
    ...GRID_PLUGIN_COMMANDS,
    ...UI_SURFACE_PLUGIN_COMMANDS,
    ...Object.values(byId),
  ];
}

/**
 * The `{ ok, commands }` envelope `useCommandList` expects back from the
 * `list command` verb, populated with the global registry.
 */
export function listCommandResult(): {
  ok: true;
  commands: CommandMetadata[];
} {
  return { ok: true, commands: globalCommandsFromBindingTables() };
}

/**
 * Drop-in `invoke` branch for the `command_tool_call` Tauri command.
 *
 * Returns the global `list command` envelope for the `list command` op and an
 * empty result for every other op (e.g. `available command`). Tests fold this
 * into their existing `invoke` mock:
 *
 * ```ts
 * if (cmd === "command_tool_call") return commandToolCall(args);
 * ```
 */
export function commandToolCall(args: unknown): Promise<unknown> {
  const op = (args as { op?: string } | undefined)?.op;
  if (op === "list command") return Promise.resolve(listCommandResult());
  if (op === "available command")
    return Promise.resolve({ ok: true, available: true });
  return Promise.resolve(null);
}

/**
 * Collect every backend `dispatch_command` whose cmd is a `nav.*` id, in
 * order, from a test's Tauri `invoke` spy.
 *
 * The cardinal nav and drill commands execute host-side in the
 * `nav-commands` builtin plugin — the webview's contract is routing the
 * command id to the backend (`invoke("dispatch_command", { cmd: "nav.*" })`)
 * with no client-side kernel IPC. Tests pinning that contract pass their own
 * hoisted `mockInvoke` spy (or the shared shadow-harness spy); the parameter
 * is typed structurally so any `vi.fn` shape qualifies.
 *
 * @param spy - The vitest spy installed on `@tauri-apps/api/core::invoke`.
 * @returns The dispatched `nav.*` command ids, in call order.
 */
export function navDispatchCmds(spy: {
  mock: { calls: ReadonlyArray<ReadonlyArray<unknown>> };
}): string[] {
  return spy.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => (c[1] as { cmd?: string } | undefined)?.cmd ?? "")
    .filter((cmd) => cmd.startsWith("nav."));
}

/**
 * Drop-in `invoke` branch answering the Command service's `list command` op
 * with a caller-supplied registry — the seam `useContextMenu`'s click-time
 * fetch rides on (`invoke("command_tool_call", { op: "list command", … })`).
 *
 * Returns the `{ ok, commands }` envelope promise when the call is a
 * `list command`, and `undefined` for every other invoke so the host mock can
 * fall through to its own handling:
 *
 * ```ts
 * invoke: vi.fn((cmd, args) =>
 *   answerListCommand(cmd, args, mockRegistry) ?? Promise.resolve(null),
 * )
 * ```
 */
export function answerListCommand(
  cmd: string,
  args: unknown,
  commands: unknown[],
): Promise<unknown> | undefined {
  if (cmd !== "command_tool_call") return undefined;
  const op = (args as { op?: string } | undefined)?.op;
  if (op !== "list command") return undefined;
  return Promise.resolve({ ok: true, commands });
}
