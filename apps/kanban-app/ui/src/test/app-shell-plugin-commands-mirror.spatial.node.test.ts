/**
 * Drift guard: the `app-shell-commands` builtin plugin's `app.*` declared
 * keybindings stay in the canonical `normalizeKeyEvent` form — every key is
 * either pinned to `BINDING_TABLES` or listed in a COMMENTED
 * menu-accelerator-only allowlist.
 *
 * Since Card I deleted `app-shell.tsx`'s static scope defs, the plugin's
 * registry metadata is the ONLY key source for the webview hotkey path:
 * `extractKeymapBindings` reads the declared string LITERALLY, and
 * `createKeyHandler` matches it against `normalizeKeyEvent` output, which emits
 * lowercase letters for unshifted chords. A plugin key declared as `Mod+Q`
 * (uppercase, no Shift) is therefore UNREACHABLE from a real keydown — the
 * silent regression this guard exists to catch (cards: this follow-up to
 * 01KTED9JYGWM815K2X41N4QDBY's warning W1).
 *
 * The plugin module (`builtin/plugins/app-shell-commands/commands/app.ts`) is
 * NOT importable from vitest — it imports the SDK that exists only inside the
 * embedded plugin runtime — so this guard reads the plugin SOURCE from disk,
 * parses its `APP_COMMANDS` data table via the shared `plugin-command-table.ts`
 * helpers, and checks every declared key two ways:
 *
 *   1. PINNED — if `BINDING_TABLES` binds any key to this id in this mode, the
 *      declared key must be a MEMBER of that set (membership, not equality:
 *      `app.search`'s vim id is bound to BOTH `/` and `Mod+f` in
 *      `BINDING_TABLES.vim`, and the plugin declares only `/`).
 *   2. ALLOWLISTED — if `BINDING_TABLES` carries NO key for this id in this
 *      mode, the (id, mode) must appear in {@link MENU_ACCELERATOR_OR_PALETTE}
 *      with its EXACT expected string. These are menu-accelerator-only chords
 *      (no global-table binding) or palette/command keys that live outside the
 *      no-focus fallback table. Pinning the exact value keeps the guard failing
 *      on a NEW unexplained key AND on an uppercase regression of an
 *      allowlisted letter chord (e.g. app.quit `Mod+q` → `Mod+Q`).
 *
 * `BINDING_TABLES` is the canonical encoding of the production global keymap
 * (see `mock-command-list.ts`), so a passing run means every `app.*` key the
 * plugin declares is reachable from a real keyboard event or deliberately
 * documented as accelerator/palette-only.
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
  "../../../../../builtin/plugins/app-shell-commands/commands/app.ts",
);

/** The nine app.* command ids the plugin's `APP_COMMANDS` table registers. */
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

/** The keymap modes the guard checks. */
const MODES = ["vim", "cua", "emacs"] as KeymapMode[];

/**
 * The COMMENTED allowlist of `app.*` (id, mode) → exact key for declared keys
 * that have NO `BINDING_TABLES` entry. Two kinds live here:
 *
 *   - MENU-ACCELERATOR-ONLY: `app.quit` rides the native App-menu accelerator
 *     (`Mod+q`); there is no global-table binding, but the lowercase form is
 *     the reachable canonical chord and keeps the accelerator's
 *     case-insensitive parse working.
 *   - PALETTE / COMMAND keys that are NOT in the no-focus fallback table:
 *     `app.help` F1, the `app.command` / `app.palette` palette openers
 *     (`Mod+Shift+P` — `BINDING_TABLES` binds that chord to `app.palette.open`,
 *     a different id), the `app.command` vim `:`, and `app.undo`'s emacs
 *     `Ctrl+/` (macOS-distinct Ctrl form, never lifted into the global table).
 *
 * Pinning the EXACT string (not mere presence) means an uppercase regression
 * of an allowlisted letter chord (e.g. `Mod+q` → `Mod+Q`) still fails the
 * guard, and a brand-new key on an unpinned (id, mode) fails too.
 */
const MENU_ACCELERATOR_OR_PALETTE: Record<
  string,
  Partial<Record<KeymapMode, string>>
> = {
  "app.help": { vim: "F1", cua: "F1" },
  "app.quit": { cua: "Mod+q", vim: ":q" },
  "app.command": { vim: ":", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
  "app.palette": { vim: "Mod+Shift+P", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
  "app.undo": { emacs: "Ctrl+/" },
};

/**
 * Invert `BINDING_TABLES` for the `app.*` ids: per mode, the SET of canonical
 * key strings bound to each app command id (a set because one id can carry
 * several keys in a mode — `app.search`'s vim `/` and `Mod+f`).
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

describe("app-shell-commands plugin keys drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "APP_COMMANDS");

  it("parses the APP_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(APP_IDS.length);
    expect(pluginEntries.map((e) => e.id).sort()).toEqual([...APP_IDS].sort());
  });

  it("every declared app.* key is canonical: a BINDING_TABLES member or an allowlisted accelerator/palette key", () => {
    const canonical = canonicalAppKeys();
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
        // menu-accelerator/palette allowlist's exact string.
        const allow = MENU_ACCELERATOR_OR_PALETTE[entry.id]?.[mode];
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
