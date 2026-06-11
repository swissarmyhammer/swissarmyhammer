/**
 * Drift guard: `UI_SURFACE_PLUGIN_COMMANDS` in `mock-command-list.ts` mirrors
 * the `ui-commands` builtin plugin's `UI_SURFACE_COMMANDS` table 1:1.
 *
 * Card D moved the four UI-surface command DEFINITIONS out of React —
 * `field.edit` / `field.editEnter` from `field.tsx`'s client-side
 * `CommandDef`s and `pressable.activate` / `pressable.activateSpace` from
 * `pressable.tsx`'s `usePressCommands` — and Card E moved the three editor
 * drill-in DEFINITIONS — `filter_editor.drillIn` from
 * `perspective-tab-bar.tsx`, `ui.ai-panel.composer.drillIn` from
 * `ai-prompt-composer.tsx`, and `ui.ai-panel.elicitation.field.drillIn`
 * from `ai-elements/elicitation.tsx` — into the `ui-commands` builtin
 * plugin (`builtin/plugins/ui-commands/index.ts`); the owning React
 * components only register webview-bus handlers for the ids while focused.
 * In production the commands' `keys` + per-surface `scope` reach the keymap
 * layer through the CommandService registry; in tests the host is mocked,
 * so the keymap tests publish the same metadata through
 * `mock-command-list.ts`'s `UI_SURFACE_PLUGIN_COMMANDS` mirror. Like the nav
 * and grid mirrors, the plugin module is not importable from vitest, so the
 * mirror is a manual copy — this guard reads the plugin SOURCE from disk and
 * fails loudly on any drift (an id added/removed/renamed, a key rebound, a
 * name change).
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { UI_SURFACE_PLUGIN_COMMANDS } from "./mock-command-list";
import { mirrorMismatches, parseCommandTable } from "./plugin-command-table";

/**
 * Absolute path to the plugin source, resolved relative to THIS test file so
 * the guard works regardless of the vitest invocation cwd.
 * `src/test/` → repo root is five levels up.
 */
const PLUGIN_SOURCE_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../../builtin/plugins/ui-commands/index.ts",
);

/**
 * The expected scope gate per command id — `ui:field` for the field-edit
 * pair, `ui:pressable` for the pressable-activation pair, and the Card E
 * editor drill-in gates: the `ui:filter_editor` /
 * `ui:ai-panel.elicitation.field` marker monikers (mounted via a
 * `CommandScopeProvider` above the surface's dynamic `<FocusScope>`) and
 * the composer `<FocusScope>`'s own constant `ui:ai-panel.composer`
 * moniker.
 */
const EXPECTED_SCOPES: Record<string, string[]> = {
  "field.edit": ["ui:field"],
  "field.editEnter": ["ui:field"],
  "pressable.activate": ["ui:pressable"],
  "pressable.activateSpace": ["ui:pressable"],
  "filter_editor.drillIn": ["ui:filter_editor"],
  "ui.ai-panel.composer.drillIn": ["ui:ai-panel.composer"],
  "ui.ai-panel.elicitation.field.drillIn": ["ui:ai-panel.elicitation.field"],
};

describe("UI_SURFACE_PLUGIN_COMMANDS drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginEntries = parseCommandTable(source, "UI_SURFACE_COMMANDS");

  it("parses the UI_SURFACE_COMMANDS table out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the table must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginEntries.length).toBe(7);
    expect(pluginEntries.map((e) => e.id)).toContain("field.edit");
    expect(pluginEntries.map((e) => e.id)).toContain("pressable.activate");
    expect(pluginEntries.map((e) => e.id)).toContain("filter_editor.drillIn");
  });

  it("mirror matches the plugin UI_SURFACE_COMMANDS 1:1 (ids, names, keys)", () => {
    expect(
      mirrorMismatches(
        UI_SURFACE_PLUGIN_COMMANDS,
        pluginEntries,
        "UI_SURFACE_COMMANDS",
      ),
    ).toEqual([]);
  });

  it("every mirror entry is gated to its surface's marker scope", () => {
    // The scope is what keeps the keys out of the global table
    // (`extractKeymapBindings`) and lights them only while the surface's
    // marker moniker is in the focused chain. A mirror entry that drops the
    // scope would silently turn Enter / Space global in tests.
    expect(UI_SURFACE_PLUGIN_COMMANDS.map((c) => c.id).sort()).toEqual(
      Object.keys(EXPECTED_SCOPES).sort(),
    );
    for (const cmd of UI_SURFACE_PLUGIN_COMMANDS) {
      expect(cmd.scope, `${cmd.id} scope`).toEqual(EXPECTED_SCOPES[cmd.id]);
    }
  });

  // ── Guard teeth — prove the comparison actually detects drift ──────────

  it("detects a rebound key", () => {
    const perturbed = UI_SURFACE_PLUGIN_COMMANDS.map((c) =>
      c.id === "field.edit" ? { ...c, keys: { ...c.keys, vim: "x" } } : c,
    );
    expect(
      mirrorMismatches(perturbed, pluginEntries, "UI_SURFACE_COMMANDS"),
    ).not.toEqual([]);
  });

  it("detects a missing id", () => {
    const perturbed = UI_SURFACE_PLUGIN_COMMANDS.filter(
      (c) => c.id !== "pressable.activateSpace",
    );
    expect(
      mirrorMismatches(perturbed, pluginEntries, "UI_SURFACE_COMMANDS"),
    ).not.toEqual([]);
  });

  it("detects an extra id the plugin does not declare", () => {
    const perturbed = [
      ...UI_SURFACE_PLUGIN_COMMANDS,
      { id: "field.bogus", name: "Bogus", keys: { vim: "b" } },
    ];
    expect(
      mirrorMismatches(perturbed, pluginEntries, "UI_SURFACE_COMMANDS"),
    ).not.toEqual([]);
  });
});
