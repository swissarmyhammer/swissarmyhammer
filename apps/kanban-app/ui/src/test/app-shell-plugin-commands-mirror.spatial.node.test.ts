/**
 * Drift guard: the `app-shell-commands` builtin plugin's `app.*` declared
 * keybindings stay reachable from a real keydown — i.e. in the canonical
 * `normalizeKeyEvent` form encoded by `BINDING_TABLES`, or on an explicit,
 * commented allowlist of keys `BINDING_TABLES` does not track (menu-accelerator
 * chords and palette-mode duplicates).
 *
 * Since Card I deleted `app-shell.tsx`'s static scope defs, this plugin's
 * registry metadata is the ONLY key source for the webview hotkey path:
 * `extractKeymapBindings` reads the declared string and `createKeyHandler`
 * matches it LITERALLY against `normalizeKeyEvent` output, which emits
 * lowercase letters for unshifted chords. A key declared as `Mod+Q` (uppercase,
 * no Shift) is therefore UNREACHABLE from a real keydown — the silent
 * regression this guard exists to catch.
 *
 * The plugin module (`builtin/plugins/app-shell-commands/commands/app.ts`) is
 * NOT importable from vitest — it imports `@swissarmyhammer/plugin`, which
 * exists only inside the embedded plugin runtime — so this guard reads the
 * plugin SOURCE from disk (node project — `fs` is available), parses its
 * `APP_COMMANDS` data table via the shared `plugin-command-table.ts` helper,
 * and asserts each declared key is canonical or allowlisted.
 *
 * Two deltas from the `ai-commands` guard:
 *   1. `app.search` binds TWO vim keys in `BINDING_TABLES` (`/` and `Mod+f`).
 *      The plugin declares only one — so the check is MEMBERSHIP in the
 *      canonical key set, not single-key equality.
 *   2. Several `app.*` keys are intentionally absent from `BINDING_TABLES`:
 *      `app.quit` is a menu-accelerator-only chord (it rides the native menu,
 *      which parses letters case-insensitively, and has no global table entry);
 *      `app.help`'s `F1`, `app.command`'s `:`, and the `Mod+Shift+P` the
 *      palette pair declares are reachable but tracked under a different id
 *      (`app.palette.open`) or not at all. These live on an explicit allowlist
 *      so the guard still FAILS on a NEW unexplained key.
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
  "../../../../../builtin/plugins/app-shell-commands/commands/app.ts",
);

/** The nine app.* command ids the `APP_COMMANDS` table registers. */
const APP_IDS = [
  "app.about",
  "app.help",
  "app.quit",
  "app.command",
  "app.palette",
  "app.search",
  "app.dismiss",
  "app.undo",
  "app.redo",
] as const;

/** The three keymap modes tested: vim, cua, and emacs. */
const MODES: KeymapMode[] = ["vim", "cua", "emacs"];

/**
 * For each app.* id, the SET of canonical key strings `BINDING_TABLES` binds to
 * it per keymap mode. A command can own several keys per mode (app.search vim
 * binds both `/` and `Mod+f`), so the value is a set, not a single key.
 */
function canonicalAppKeys(): Record<string, Record<string, Set<string>>> {
  const byId: Record<string, Record<string, Set<string>>> = {};
  for (const mode of MODES) {
    for (const [key, id] of Object.entries(BINDING_TABLES[mode])) {
      if (!id.startsWith("app.")) continue;
      byId[id] ??= {};
      byId[id][mode] ??= new Set();
      byId[id][mode].add(key);
    }
  }
  return byId;
}

/**
 * Keys declared on `app.*` registrations that are reachable from
 * `normalizeKeyEvent` but are NOT bound to that id in `BINDING_TABLES` — each
 * with the reason it is legitimate. The guard accepts a declared key only if it
 * is canonical (membership above) OR listed here; a NEW uppercase/unreachable
 * key matches neither and FAILS.
 */
const ALLOWLIST: Record<string, Partial<Record<KeymapMode, string[]>>> = {
  // F1 has no `BINDING_TABLES` entry (Help is not a global keymap binding); the
  // function-key literal is reachable as-is from `normalizeKeyEvent`.
  "app.help": { vim: ["F1"], cua: ["F1"] },
  // Menu-accelerator-only: Quit rides the native App-menu accelerator (which
  // parses letters case-insensitively) and is deliberately NOT a webview global
  // binding. `Mod+q` is the canonical lowercase form (an uppercase `Mod+Q`
  // would be unreachable). `:q` is the reachable vim ex-command chord.
  "app.quit": { cua: ["Mod+q"], vim: [":q"] },
  // The command palette in command-mode. `:` is the reachable vim chord;
  // `Mod+Shift+P` is reachable (shifted letter keeps its case) but
  // `BINDING_TABLES` tracks it under `app.palette.open`, the unified opener.
  "app.command": {
    vim: [":"],
    cua: ["Mod+Shift+P"],
    emacs: ["Mod+Shift+P"],
  },
  // The hidden palette opener — same `Mod+Shift+P` reachable chord, tracked in
  // `BINDING_TABLES` under `app.palette.open`.
  "app.palette": {
    cua: ["Mod+Shift+P"],
    vim: ["Mod+Shift+P"],
    emacs: ["Mod+Shift+P"],
  },
  // Emacs undo: `Ctrl+/` is reachable (on macOS `normalizeKeyEvent` emits a
  // distinct `Ctrl` prefix) but has no static `BINDING_TABLES.emacs` entry — it
  // moved onto this registration from `app-shell.tsx`'s deleted statics (Card I).
  "app.undo": { emacs: ["Ctrl+/"] },
};

describe("app-shell-commands plugin keys drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "APP_COMMANDS");

  it("parses the APP_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(APP_IDS.length);
    expect(pluginEntries.map((entry) => entry.id).sort()).toEqual(
      [...APP_IDS].sort(),
    );
  });

  it("every declared app.* key is canonical (in BINDING_TABLES) or allowlisted", () => {
    const canonical = canonicalAppKeys();
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

  it("the canonicalized unshifted-letter keys are present in their declared mode", () => {
    // Lock the specific Card-sweep outcomes so a regression that reverts ONE
    // of them (e.g. app.search cua back to Mod+F) is caught even if it somehow
    // slipped the membership check. Each entry: id → mode → expected key.
    const byId = new Map(pluginEntries.map((entry) => [entry.id, entry]));
    const expectations: Array<[string, KeymapMode, string]> = [
      ["app.quit", "cua", "Mod+q"],
      ["app.search", "cua", "Mod+f"],
      ["app.undo", "cua", "Mod+z"],
      ["app.redo", "vim", "Mod+r"],
    ];
    for (const [id, mode, key] of expectations) {
      expect(byId.get(id)?.keys[mode]).toBe(key);
    }
    // app.search MUST NOT declare an emacs key — `Mod+f` is `nav.right` in
    // emacs (the conflict resolved deliberately, card
    // 01KMT56FTBAP8PQ4QQND08MP97); emacs Find stays palette-only.
    expect(byId.get("app.search")?.keys.emacs).toBeUndefined();
  });
});
