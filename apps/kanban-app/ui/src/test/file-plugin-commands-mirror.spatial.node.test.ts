/**
 * Drift guard: the `file-commands` builtin plugin's `file.*` declared
 * keybindings stay reachable from a real keydown — i.e. in the canonical
 * `normalizeKeyEvent` form encoded by `BINDING_TABLES`, or on an explicit,
 * commented allowlist of menu-accelerator-only keys `BINDING_TABLES` does not
 * track.
 *
 * Since Card I deleted `app-shell.tsx`'s static scope defs, this plugin's
 * registry metadata is the ONLY key source for the webview hotkey path:
 * `extractKeymapBindings` reads the declared string and `createKeyHandler`
 * matches it LITERALLY against `normalizeKeyEvent` output, which emits
 * lowercase letters for unshifted chords. A key declared as `Mod+O` (uppercase,
 * no Shift) is therefore UNREACHABLE from a real keydown — the silent
 * regression this guard exists to catch.
 *
 * The plugin module (`builtin/plugins/file-commands/index.ts`) is NOT
 * importable from vitest — it imports `@swissarmyhammer/plugin`, which exists
 * only inside the embedded plugin runtime — so this guard reads the plugin
 * SOURCE from disk (node project — `fs` is available), parses its
 * `FILE_COMMANDS` data table via the shared `plugin-command-table.ts` helper,
 * and asserts each declared key is canonical or allowlisted.
 *
 * `file.newBoard` (`Mod+Shift+B`) and `file.openBoard` (`Mod+o`) are
 * menu-accelerator-only: they ride the native File-menu accelerator (which
 * parses letters case-insensitively) and have no global `BINDING_TABLES` entry.
 * They live on an explicit allowlist so the guard still FAILS on a NEW
 * unexplained key (e.g. an uppercase `Mod+O` regression). `file.closeBoard`'s
 * `Mod+w` IS in `BINDING_TABLES`, so it is checked by canonical membership.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
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

/** The four file.* command ids the `FILE_COMMANDS` table registers. */
const FILE_IDS = [
  "file.switchBoard",
  "file.closeBoard",
  "file.newBoard",
  "file.openBoard",
] as const;

/** The three keymap modes tested: vim, cua, and emacs. */
const MODES: KeymapMode[] = ["vim", "cua", "emacs"];

/**
 * For each file.* id, the SET of canonical key strings `BINDING_TABLES` binds
 * to it per keymap mode (a command can own several keys per mode).
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

/**
 * Menu-accelerator-only keys declared on `file.*` registrations that are
 * reachable from `normalizeKeyEvent` but have no `BINDING_TABLES` entry — each
 * with the reason it is legitimate. The guard accepts a declared key only if it
 * is canonical (membership) OR listed here; a NEW uppercase/unreachable key
 * matches neither and FAILS.
 */
const ALLOWLIST: Record<string, Partial<Record<KeymapMode, string[]>>> = {
  // New Board rides the native File-menu accelerator. `Mod+Shift+B` is already
  // canonical (a shifted letter keeps its uppercase) and is not a webview
  // global binding.
  "file.newBoard": { cua: ["Mod+Shift+B"] },
  // Open Board rides the native File-menu accelerator (case-insensitive). The
  // canonical lowercase `Mod+o` keeps the accelerator AND makes the chord live
  // in the webview on non-Mac; an uppercase `Mod+O` would be unreachable.
  "file.openBoard": { cua: ["Mod+o"] },
};

describe("file-commands plugin keys drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "FILE_COMMANDS");

  it("parses the FILE_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(FILE_IDS.length);
    expect(pluginEntries.map((e) => e.id).sort()).toEqual([...FILE_IDS].sort());
  });

  it("every declared file.* key is canonical (in BINDING_TABLES) or allowlisted", () => {
    const canonical = canonicalFileKeys();
    const violations: string[] = [];
    for (const entry of pluginEntries) {
      for (const mode of MODES) {
        const declared = entry.keys[mode];
        if (declared === undefined) continue;
        const canonicalSet = canonical[entry.id]?.[mode];
        if (canonicalSet?.has(declared)) continue; // canonical membership
        const allowed = ALLOWLIST[entry.id]?.[mode] ?? [];
        if (allowed.includes(declared)) continue; // explicitly allowlisted
        violations.push(
          `keys.${mode} for ${entry.id}: declared ${JSON.stringify(
            declared,
          )} is neither bound to ${entry.id} in BINDING_TABLES (${JSON.stringify(
            [...(canonicalSet ?? [])],
          )}) nor allowlisted (${JSON.stringify(allowed)})`,
        );
      }
    }
    expect(violations).toEqual([]);
  });

  it("the canonicalized open-board key is the lowercase Mod+o form", () => {
    // Lock the specific Card-sweep outcome so a regression that re-uppercases
    // file.openBoard's accelerator is caught.
    const byId = new Map(pluginEntries.map((e) => [e.id, e]));
    expect(byId.get("file.openBoard")?.keys.cua).toBe("Mod+o");
    expect(byId.get("file.closeBoard")?.keys.cua).toBe("Mod+w");
    expect(byId.get("file.closeBoard")?.keys.vim).toBe("Mod+w");
  });
});
