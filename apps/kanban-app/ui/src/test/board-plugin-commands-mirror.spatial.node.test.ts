/**
 * Drift guard: `BOARD_PLUGIN_COMMANDS` in `mock-command-list.ts` mirrors the
 * `board-commands` builtin plugin's `BOARD_COMMANDS` table 1:1.
 *
 * Card F moved the three `board.*` command DEFINITIONS out of
 * `board-view.tsx`'s client-side `CommandDef` factories
 * (`makeNewTaskCommand` / `makeNavCommand`) into the `board-commands`
 * builtin plugin (`builtin/plugins/board-commands/index.ts`); the board
 * React tree only registers a webview-bus handler for `board.newTask`
 * (the column extremes execute server-side). In production the commands'
 * `keys` + `scope: ["ui:board"]` reach the keymap layer through the
 * CommandService registry; in tests the host is mocked, so the keymap tests
 * publish the same metadata through `mock-command-list.ts`'s
 * `BOARD_PLUGIN_COMMANDS` mirror. Like the nav / grid mirrors, the plugin
 * module is not importable from vitest, so the mirror is a manual copy —
 * this guard reads the plugin SOURCE from disk and fails loudly on any
 * drift (an id added/removed/renamed, a key rebound, a name change).
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { BOARD_PLUGIN_COMMANDS } from "./mock-command-list";
import { mirrorMismatches, parseCommandTable } from "./plugin-command-table";

/**
 * Absolute path to the plugin source, resolved relative to THIS test file so
 * the guard works regardless of the vitest invocation cwd.
 * `src/test/` → repo root is five levels up.
 */
const PLUGIN_SOURCE_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../../builtin/plugins/board-commands/index.ts",
);

describe("BOARD_PLUGIN_COMMANDS drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "BOARD_COMMANDS");

  it("parses the BOARD_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(3);
    expect(pluginEntries.map((e) => e.id)).toContain("board.newTask");
  });

  it("mirror matches the plugin BOARD_COMMANDS 1:1 (ids, names, keys)", () => {
    expect(
      mirrorMismatches(BOARD_PLUGIN_COMMANDS, pluginEntries, "BOARD_COMMANDS"),
    ).toEqual([]);
  });

  it("every mirror entry is gated to the board zone scope", () => {
    // The scope is what keeps the keys out of the global table
    // (`extractKeymapBindings`) and lights them only while `ui:board` is in
    // the focused chain (`extractChainBindings`). A mirror entry
    // that drops the scope would silently turn a board key global in tests.
    for (const cmd of BOARD_PLUGIN_COMMANDS) {
      expect(cmd.scope, `${cmd.id} scope`).toEqual(["ui:board"]);
    }
  });

  // ── Guard teeth — prove the comparison actually detects drift ──────────

  it("detects a rebound key", () => {
    const perturbed = BOARD_PLUGIN_COMMANDS.map((c) =>
      c.id === "board.newTask" ? { ...c, keys: { ...c.keys, vim: "x" } } : c,
    );
    expect(
      mirrorMismatches(perturbed, pluginEntries, "BOARD_COMMANDS"),
    ).not.toEqual([]);
  });

  it("detects a missing id", () => {
    const perturbed = BOARD_PLUGIN_COMMANDS.filter(
      (c) => c.id !== "board.lastColumn",
    );
    expect(
      mirrorMismatches(perturbed, pluginEntries, "BOARD_COMMANDS"),
    ).not.toEqual([]);
  });

  it("detects an extra id the plugin does not declare", () => {
    const perturbed = [
      ...BOARD_PLUGIN_COMMANDS,
      { id: "board.bogus", name: "Bogus", keys: { vim: "b" } },
    ];
    expect(
      mirrorMismatches(perturbed, pluginEntries, "BOARD_COMMANDS"),
    ).not.toEqual([]);
  });
});
