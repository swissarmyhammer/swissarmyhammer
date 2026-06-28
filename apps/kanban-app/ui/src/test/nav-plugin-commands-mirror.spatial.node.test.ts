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
 * `NAV_DIRECTIONS` data table out of it via the shared
 * `plugin-command-table.ts` helpers, and asserts the mirror's
 * `{ id, name, keys }` triples match it exactly, in both directions. Any
 * drift — an id added/removed/renamed, a key rebound, a name change — fails
 * this suite loudly.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { NAV_PLUGIN_COMMANDS } from "./mock-command-list";
import { mirrorMismatches, parseCommandTable } from "./plugin-command-table";

/**
 * Absolute path to the plugin source, resolved relative to THIS test file so
 * the guard works regardless of the vitest invocation cwd.
 * `src/test/` → repo root is five levels up.
 */
const PLUGIN_SOURCE_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../../builtin/plugins/nav-commands/index.ts",
);

describe("NAV_PLUGIN_COMMANDS drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "NAV_DIRECTIONS");

  it("parses the NAV_DIRECTIONS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBeGreaterThan(0);
    expect(pluginEntries.map((e) => e.id)).toContain("nav.up");
  });

  it("mirror matches the plugin NAV_DIRECTIONS 1:1 (ids, names, keys)", () => {
    expect(
      mirrorMismatches(NAV_PLUGIN_COMMANDS, pluginEntries, "NAV_DIRECTIONS"),
    ).toEqual([]);
  });

  // ── Guard teeth — prove the comparison actually detects drift ──────────

  it("detects a rebound key", () => {
    const perturbed = NAV_PLUGIN_COMMANDS.map((c) =>
      c.id === "nav.up" ? { ...c, keys: { ...c.keys, vim: "x" } } : c,
    );
    expect(
      mirrorMismatches(perturbed, pluginEntries, "NAV_DIRECTIONS"),
    ).not.toEqual([]);
  });

  it("detects a missing id", () => {
    const perturbed = NAV_PLUGIN_COMMANDS.filter((c) => c.id !== "nav.last");
    expect(
      mirrorMismatches(perturbed, pluginEntries, "NAV_DIRECTIONS"),
    ).not.toEqual([]);
  });

  it("detects an extra id the plugin does not declare", () => {
    const perturbed = [
      ...NAV_PLUGIN_COMMANDS,
      { id: "nav.bogus", name: "Bogus", keys: { vim: "b" } },
    ];
    expect(
      mirrorMismatches(perturbed, pluginEntries, "NAV_DIRECTIONS"),
    ).not.toEqual([]);
  });
});
