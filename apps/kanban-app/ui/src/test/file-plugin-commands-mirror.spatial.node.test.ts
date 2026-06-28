/**
 * Drift guard: the `file-commands` builtin plugin's `file.*` declared
 * keybindings stay in the canonical `normalizeKeyEvent` form — every key is
 * either pinned to `BINDING_TABLES` or listed in a COMMENTED
 * menu-accelerator-only allowlist.
 *
 * Since Card I deleted `app-shell.tsx`'s static scope defs, the plugin's
 * registry metadata is the ONLY key source for the webview hotkey path:
 * `extractKeymapBindings` reads the declared string LITERALLY, and
 * `createKeyHandler` matches it against `normalizeKeyEvent` output, which emits
 * lowercase letters for unshifted chords. A plugin key declared as `Mod+O`
 * (uppercase, no Shift) is therefore UNREACHABLE from a real keydown — the
 * silent regression this guard exists to catch.
 *
 * The plugin module (`builtin/plugins/file-commands/index.ts`) is NOT
 * importable from vitest — it imports the SDK that exists only inside the
 * embedded plugin runtime — so this guard reads the plugin SOURCE from disk,
 * parses its `FILE_COMMANDS` data table via the shared `plugin-command-table.ts`
 * helpers, and checks every declared key two ways:
 *
 *   1. PINNED — if `BINDING_TABLES` binds any key to this id in this mode, the
 *      declared key must be a MEMBER of that set (`file.closeBoard`'s `Mod+w`
 *      is bound in vim and cua).
 *   2. ALLOWLISTED — if `BINDING_TABLES` carries NO key for this id in this
 *      mode, the (id, mode) must appear in {@link MENU_ACCELERATOR_ONLY} with
 *      its EXACT expected string. `file.newBoard` (`Mod+Shift+B`) and
 *      `file.openBoard` (`Mod+o`) ride the native File-menu accelerators only —
 *      no global-table binding — but the canonical form keeps the accelerator's
 *      case-insensitive parse working AND (for the lowercase letter chord)
 *      makes the chord reachable in the webview on non-Mac. Pinning the exact
 *      value keeps the guard failing on a NEW unexplained key AND on an
 *      uppercase regression of an allowlisted letter chord (`Mod+o` → `Mod+O`).
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
  "../../../../../builtin/plugins/file-commands/index.ts",
);

/** The four file.* command ids the plugin's `FILE_COMMANDS` table registers. */
const FILE_IDS = [
  "file.switchBoard",
  "file.closeBoard",
  "file.newBoard",
  "file.openBoard",
] as const;

/** The keymap modes the guard checks. */
const MODES = ["vim", "cua", "emacs"] as KeymapMode[];

/**
 * The COMMENTED allowlist of `file.*` (id, mode) → exact key for declared keys
 * that have NO `BINDING_TABLES` entry — the native File-menu accelerators:
 *
 *   - `file.newBoard` `Mod+Shift+B` — accelerator-only; already canonical (a
 *     shifted letter keeps its uppercase).
 *   - `file.openBoard` `Mod+o` — accelerator-only; canonicalized to lowercase
 *     so the chord is also reachable in the webview on non-Mac.
 *
 * Pinning the EXACT string (not mere presence) means an uppercase regression
 * of `file.openBoard` (`Mod+o` → `Mod+O`) still fails the guard, and a new key
 * on an unpinned (id, mode) fails too.
 */
const MENU_ACCELERATOR_ONLY: Record<
  string,
  Partial<Record<KeymapMode, string>>
> = {
  "file.newBoard": { cua: "Mod+Shift+B" },
  "file.openBoard": { cua: "Mod+o" },
};

/**
 * Invert `BINDING_TABLES` for the `file.*` ids: per mode, the SET of canonical
 * key strings bound to each file command id.
 */
function canonicalFileKeys(): Record<string, Record<string, Set<string>>> {
  const byId: Record<string, Record<string, Set<string>>> = {};
  for (const mode of MODES) {
    for (const [key, id] of Object.entries(BINDING_TABLES[mode])) {
      if (!id.startsWith("file.")) continue;
      byId[id] ??= {};
      byId[id][mode] ??= new Set();
      byId[id][mode].add(key);
    }
  }
  return byId;
}

describe("file-commands plugin keys drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "FILE_COMMANDS");

  it("parses the FILE_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(FILE_IDS.length);
    expect(pluginEntries.map((e) => e.id).sort()).toEqual([...FILE_IDS].sort());
  });

  it("every declared file.* key is canonical: a BINDING_TABLES member or an allowlisted accelerator key", () => {
    const canonical = canonicalFileKeys();
    const mismatches: string[] = [];
    for (const entry of pluginEntries) {
      for (const mode of MODES) {
        const declared = entry.keys[mode];
        if (declared === undefined) continue;
        const tableKeys = canonical[entry.id]?.[mode];
        if (tableKeys && tableKeys.size > 0) {
          // PINNED: the declared key must be one of the canonical keys
          // BINDING_TABLES binds to this id in this mode (membership).
          if (!tableKeys.has(declared)) {
            mismatches.push(
              `keys.${mode} for ${entry.id}: plugin ${JSON.stringify(
                declared,
              )} is not a BINDING_TABLES member of {${[...tableKeys]
                .map((k) => JSON.stringify(k))
                .join(", ")}}`,
            );
          }
          continue;
        }
        // ALLOWLISTED: no BINDING_TABLES entry — must match the commented
        // menu-accelerator allowlist's exact string.
        const allow = MENU_ACCELERATOR_ONLY[entry.id]?.[mode];
        if (allow !== declared) {
          mismatches.push(
            `keys.${mode} for ${entry.id}: plugin ${JSON.stringify(
              declared,
            )} has no BINDING_TABLES entry and is not the allowlisted ${JSON.stringify(
              allow,
            )} (a NEW unexplained key — add a BINDING_TABLES binding or a commented allowlist entry)`,
          );
        }
      }
    }
    expect(mismatches).toEqual([]);
  });
});
