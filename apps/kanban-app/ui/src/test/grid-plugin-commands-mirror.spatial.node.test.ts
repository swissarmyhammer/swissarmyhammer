/**
 * Drift guard: `GRID_PLUGIN_COMMANDS` in `mock-command-list.ts` mirrors the
 * `grid-commands` builtin plugin's `GRID_COMMANDS` table 1:1.
 *
 * Card C moved the eleven `grid.*` command DEFINITIONS out of
 * `grid-view.tsx`'s client-side `CommandDef`s into the `grid-commands`
 * builtin plugin (`builtin/plugins/grid-commands/index.ts`); the grid React
 * tree only registers webview-bus handlers for the ids. In production the
 * commands' `keys` + `scope: ["ui:grid"]` reach the keymap layer through the
 * CommandService registry; in tests the host is mocked, so the keymap tests
 * publish the same metadata through `mock-command-list.ts`'s
 * `GRID_PLUGIN_COMMANDS` mirror. Like the nav mirror, the plugin module is
 * not importable from vitest, so the mirror is a manual copy — this guard
 * reads the plugin SOURCE from disk and fails loudly on any drift (an id
 * added/removed/renamed, a key rebound, a name change).
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { GRID_PLUGIN_COMMANDS } from "./mock-command-list";
import { mirrorMismatches, parseCommandTable } from "./plugin-command-table";

/**
 * Absolute path to the plugin source, resolved relative to THIS test file so
 * the guard works regardless of the vitest invocation cwd.
 * `src/test/` → repo root is five levels up.
 */
const PLUGIN_SOURCE_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../../builtin/plugins/grid-commands/index.ts",
);

describe("GRID_PLUGIN_COMMANDS drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "GRID_COMMANDS");

  it("parses the GRID_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(11);
    expect(pluginEntries.map((e) => e.id)).toContain("grid.edit");
  });

  it("mirror matches the plugin GRID_COMMANDS 1:1 (ids, names, keys)", () => {
    expect(
      mirrorMismatches(GRID_PLUGIN_COMMANDS, pluginEntries, "GRID_COMMANDS"),
    ).toEqual([]);
  });

  it("every mirror entry is gated to the grid zone scope", () => {
    // The scope is what keeps the keys out of the global table
    // (`extractKeymapBindings`) and lights them only while `ui:grid` is in
    // the focused chain (`extractChainBindings`). A mirror entry
    // that drops the scope would silently turn a grid key global in tests.
    for (const cmd of GRID_PLUGIN_COMMANDS) {
      expect(cmd.scope, `${cmd.id} scope`).toEqual(["ui:grid"]);
    }
  });

  // ── Guard teeth — prove the comparison actually detects drift ──────────

  it("detects a rebound key", () => {
    const perturbed = GRID_PLUGIN_COMMANDS.map((c) =>
      c.id === "grid.edit" ? { ...c, keys: { ...c.keys, vim: "x" } } : c,
    );
    expect(
      mirrorMismatches(perturbed, pluginEntries, "GRID_COMMANDS"),
    ).not.toEqual([]);
  });

  it("detects a missing id", () => {
    const perturbed = GRID_PLUGIN_COMMANDS.filter(
      (c) => c.id !== "grid.deleteRow",
    );
    expect(
      mirrorMismatches(perturbed, pluginEntries, "GRID_COMMANDS"),
    ).not.toEqual([]);
  });

  it("detects an extra id the plugin does not declare", () => {
    const perturbed = [
      ...GRID_PLUGIN_COMMANDS,
      { id: "grid.bogus", name: "Bogus", keys: { vim: "b" } },
    ];
    expect(
      mirrorMismatches(perturbed, pluginEntries, "GRID_COMMANDS"),
    ).not.toEqual([]);
  });
});
