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
 * Build the global command registry from `BINDING_TABLES`: one command per id,
 * each carrying its `keys` map keyed by keymap mode.
 */
export function globalCommandsFromBindingTables(): CommandMetadata[] {
  const byId: Record<string, CommandMetadata> = {};
  for (const mode of ["vim", "cua", "emacs"] as const) {
    for (const [key, id] of Object.entries(BINDING_TABLES[mode])) {
      byId[id] ??= { id, name: id, keys: {} };
      (byId[id].keys as Record<string, string>)[mode] = key;
    }
  }
  return Object.values(byId);
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
