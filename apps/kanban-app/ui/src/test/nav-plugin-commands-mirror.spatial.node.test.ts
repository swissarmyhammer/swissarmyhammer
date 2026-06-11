/**
 * Drift guard: `NAV_PLUGIN_COMMANDS` in `mock-command-list.ts` mirrors the
 * `nav-commands` builtin plugin's `NAV_DIRECTIONS` table 1:1.
 *
 * The plugin module (`builtin/plugins/nav-commands/index.ts`) is NOT
 * importable from vitest — it imports `@swissarmyhammer/plugin`, which exists
 * only inside the embedded plugin runtime — so the test mirror is a manual
 * copy. Without a guard, a change to the plugin's command ids or keybindings
 * would silently re-stale every keymap test while leaving them green (the
 * exact failure class card 01KTQEKP9E8TPQ547BWA5RGWH9 repaired).
 *
 * This guard reads the plugin SOURCE from disk (node project — `fs` is
 * available; browser-mode tests cannot read files), parses the
 * `NAV_DIRECTIONS` data table out of it, and asserts the mirror's
 * `{ id, name, keys }` triples match it exactly, in both directions. Any
 * drift — an id added/removed/renamed, a key rebound, a name change — fails
 * this suite loudly.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { NAV_PLUGIN_COMMANDS } from "./mock-command-list";

/**
 * Absolute path to the plugin source, resolved relative to THIS test file so
 * the guard works regardless of the vitest invocation cwd.
 * `src/test/` → repo root is five levels up.
 */
const PLUGIN_SOURCE_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../../builtin/plugins/nav-commands/index.ts",
);

/** One `{ id, name, keys }` triple — the shape both sides are compared as. */
interface NavEntry {
  id: string;
  name: string;
  keys: Record<string, string>;
}

/**
 * Parse the `NAV_DIRECTIONS` data table out of the plugin source text.
 *
 * Tolerant of formatting (whitespace, line breaks, entry order) but anchored
 * on the declaration name and the `id:` / `name:` / `keys:` properties, so a
 * structural rewrite of the table fails the guard (by parsing zero entries)
 * rather than passing vacuously.
 */
function parseNavDirections(source: string): NavEntry[] {
  const table = /NAV_DIRECTIONS[^=]*=\s*\[([\s\S]*?)\n\];/.exec(source)?.[1];
  if (!table) return [];
  // Split the table body into per-entry slices anchored on each `id:`.
  const idRe = /id:\s*"([^"]+)"/g;
  const anchors: Array<{ id: string; start: number }> = [];
  for (let m = idRe.exec(table); m; m = idRe.exec(table)) {
    anchors.push({ id: m[1], start: m.index });
  }
  return anchors.map(({ id, start }, i) => {
    const slice = table.slice(start, anchors[i + 1]?.start ?? table.length);
    const name = /name:\s*"([^"]+)"/.exec(slice)?.[1] ?? "";
    const keysBody = /keys:\s*\{([^}]*)\}/.exec(slice)?.[1] ?? "";
    const keys: Record<string, string> = {};
    const kvRe = /(\w+):\s*"([^"]*)"/g;
    for (let kv = kvRe.exec(keysBody); kv; kv = kvRe.exec(keysBody)) {
      keys[kv[1]] = kv[2];
    }
    return { id, name, keys };
  });
}

/**
 * Compare the mirror against the parsed plugin table and return a list of
 * human-readable mismatches — empty when the mirror is faithful. Checks id
 * set equality in both directions plus per-id `name` and `keys` equality.
 */
function mirrorMismatches(
  mirror: ReadonlyArray<{
    id: string;
    name: string;
    keys?: Record<string, string>;
  }>,
  pluginEntries: ReadonlyArray<NavEntry>,
): string[] {
  const mismatches: string[] = [];
  if (pluginEntries.length === 0) {
    return ["NAV_DIRECTIONS table not found / not parseable in plugin source"];
  }
  const byId = new Map(pluginEntries.map((e) => [e.id, e]));
  for (const m of mirror) {
    const plugin = byId.get(m.id);
    if (!plugin) {
      mismatches.push(`mirror id ${m.id} not present in plugin NAV_DIRECTIONS`);
      continue;
    }
    if (plugin.name !== m.name) {
      mismatches.push(
        `name mismatch for ${m.id}: mirror "${m.name}" vs plugin "${plugin.name}"`,
      );
    }
    const mirrorKeys = m.keys ?? {};
    const modes = new Set([
      ...Object.keys(mirrorKeys),
      ...Object.keys(plugin.keys),
    ]);
    for (const mode of modes) {
      if (mirrorKeys[mode] !== plugin.keys[mode]) {
        mismatches.push(
          `keys.${mode} mismatch for ${m.id}: mirror ${JSON.stringify(
            mirrorKeys[mode],
          )} vs plugin ${JSON.stringify(plugin.keys[mode])}`,
        );
      }
    }
  }
  const mirrorIds = new Set(mirror.map((m) => m.id));
  for (const e of pluginEntries) {
    if (!mirrorIds.has(e.id)) {
      mismatches.push(`plugin id ${e.id} missing from mirror`);
    }
  }
  return mismatches;
}

describe("NAV_PLUGIN_COMMANDS drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseNavDirections(source);

  it("parses the NAV_DIRECTIONS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBeGreaterThan(0);
    expect(pluginEntries.map((e) => e.id)).toContain("nav.up");
  });

  it("mirror matches the plugin NAV_DIRECTIONS 1:1 (ids, names, keys)", () => {
    expect(mirrorMismatches(NAV_PLUGIN_COMMANDS, pluginEntries)).toEqual([]);
  });

  // ── Guard teeth — prove the comparison actually detects drift ──────────

  it("detects a rebound key", () => {
    const perturbed = NAV_PLUGIN_COMMANDS.map((c) =>
      c.id === "nav.up" ? { ...c, keys: { ...c.keys, vim: "x" } } : c,
    );
    expect(mirrorMismatches(perturbed, pluginEntries)).not.toEqual([]);
  });

  it("detects a missing id", () => {
    const perturbed = NAV_PLUGIN_COMMANDS.filter((c) => c.id !== "nav.last");
    expect(mirrorMismatches(perturbed, pluginEntries)).not.toEqual([]);
  });

  it("detects an extra id the plugin does not declare", () => {
    const perturbed = [
      ...NAV_PLUGIN_COMMANDS,
      { id: "nav.bogus", name: "Bogus", keys: { vim: "b" } },
    ];
    expect(mirrorMismatches(perturbed, pluginEntries)).not.toEqual([]);
  });
});
