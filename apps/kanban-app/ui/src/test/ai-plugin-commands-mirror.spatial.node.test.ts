/**
 * Drift guard: the `ai-commands` builtin plugin's declared keybindings stay in
 * the canonical `normalizeKeyEvent` form encoded by `BINDING_TABLES`.
 *
 * Card I deleted `app-shell.tsx`'s `buildAiCommands` — the React scope defs
 * that used to carry the webview-matched lowercase keys (`Mod+j`, `Mod+i`,
 * `Mod+Shift+J`, `Mod+.`) for the five `ai.*` window-layer commands. After the
 * deletion the ONLY key source for the webview hotkey path is the plugin's
 * registry metadata (`extractKeymapBindings` reads it literally, and
 * `createKeyHandler` matches it against `normalizeKeyEvent` output, which
 * emits lowercase letters for unshifted chords). A plugin key declared as
 * `Mod+J` (uppercase, no Shift) is therefore UNREACHABLE from a real keydown —
 * the silent regression this guard exists to catch.
 *
 * The plugin module (`builtin/plugins/ai-commands/index.ts`) is NOT importable
 * from vitest — it imports `@swissarmyhammer/plugin`, which exists only inside
 * the embedded plugin runtime — so this guard reads the plugin SOURCE from
 * disk (node project — `fs` is available), parses its `AI_COMMANDS` data table
 * via the shared `plugin-command-table.ts` helpers, and asserts each command's
 * declared keys equal the key `BINDING_TABLES` binds to that id per keymap
 * mode. `BINDING_TABLES` is the canonical encoding of the production global
 * keymap (see `mock-command-list.ts`), so equality means every declared ai.*
 * key is reachable from a real keyboard event.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { BINDING_TABLES, type KeymapMode } from "@/lib/keybindings";
import { parseCommandTable } from "./plugin-command-table";

/**
 * Absolute path to the plugin source, resolved relative to THIS test file so
 * the guard works regardless of the vitest invocation cwd.
 * `src/test/` → repo root is five levels up.
 */
const PLUGIN_SOURCE_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../../builtin/plugins/ai-commands/index.ts",
);

/** The five ai.* command ids the plugin registers. */
const AI_IDS = [
  "ai.toggle",
  "ai.focus",
  "ai.newChat",
  "ai.model",
  "ai.cancel",
] as const;

/**
 * Invert `BINDING_TABLES` for the `ai.*` ids: per keymap mode, the canonical
 * key string each ai command is bound to (absent = the id carries no key in
 * that mode — e.g. `ai.model`, which is palette-only).
 */
function canonicalAiKeys(): Record<string, Record<string, string>> {
  const byId: Record<string, Record<string, string>> = {};
  for (const mode of ["vim", "cua", "emacs"] as KeymapMode[]) {
    for (const [key, id] of Object.entries(BINDING_TABLES[mode])) {
      if (!id.startsWith("ai.")) continue;
      byId[id] ??= {};
      byId[id][mode] = key;
    }
  }
  return byId;
}

describe("ai-commands plugin keys drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "AI_COMMANDS");

  it("parses the AI_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(AI_IDS.length);
    expect(pluginEntries.map((e) => e.id).sort()).toEqual([...AI_IDS].sort());
  });

  it("declared ai.* keys are the canonical normalizeKeyEvent form from BINDING_TABLES", () => {
    const canonical = canonicalAiKeys();
    const mismatches: string[] = [];
    for (const entry of pluginEntries) {
      const expected = canonical[entry.id] ?? {};
      for (const mode of ["vim", "cua", "emacs"]) {
        if (entry.keys[mode] !== expected[mode]) {
          mismatches.push(
            `keys.${mode} for ${entry.id}: plugin ${JSON.stringify(
              entry.keys[mode],
            )} vs canonical ${JSON.stringify(expected[mode])}`,
          );
        }
      }
    }
    expect(mismatches).toEqual([]);
  });
});
