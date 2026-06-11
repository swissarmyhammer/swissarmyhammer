/**
 * Drift guard: the webview-side `INSPECTABLE_ENTITY_PREFIXES` mirror
 * (`inspectable-entity-prefixes.ts`) matches the `ui-commands` builtin
 * plugin's `INSPECTABLE_ENTITY_PREFIXES` declaration 1:1.
 *
 * The list exists in two places that cannot import each other:
 *
 *   - `builtin/plugins/ui-commands/index.ts` — the server-side filter
 *     `entity.inspect` uses to resolve its target from a dispatch's scope
 *     chain (Card G moved it out of React).
 *   - `src/test/inspectable-entity-prefixes.ts` — the webview-side copy
 *     that `focus-architecture.guards.node.test.ts` (Guards B + C) pins
 *     against the `<Inspectable>` JSX call sites.
 *
 * The plugin module is NOT importable from vitest — it imports
 * `@swissarmyhammer/plugin`, which exists only inside the embedded plugin
 * runtime — so, exactly like the `*-plugin-commands-mirror` guards, this
 * test reads the plugin SOURCE from disk (node project — `fs` is available;
 * browser-mode tests cannot read files), parses the prefix array out of it
 * via the shared `plugin-command-table.ts` helper, and asserts set equality
 * with the webview-side mirror. Without this guard the lists could silently
 * drift — an entity kind added to the webview guard but not the plugin
 * breaks Space-to-inspect for that kind with no test failing.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { INSPECTABLE_ENTITY_PREFIXES } from "./inspectable-entity-prefixes";
import { parseStringArrayConst } from "./plugin-command-table";

/**
 * Absolute path to the plugin source, resolved relative to THIS test file so
 * the guard works regardless of the vitest invocation cwd.
 * `src/test/` → repo root is five levels up.
 */
const PLUGIN_SOURCE_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../../builtin/plugins/ui-commands/index.ts",
);

/** Sorted copy for order-insensitive set comparison. */
function sorted(list: readonly string[]): string[] {
  return [...list].sort();
}

describe("INSPECTABLE_ENTITY_PREFIXES drift guard", () => {
  const source = readFileSync(PLUGIN_SOURCE_PATH, "utf-8");
  const pluginPrefixes = parseStringArrayConst(
    source,
    "INSPECTABLE_ENTITY_PREFIXES",
  );

  it("parses the INSPECTABLE_ENTITY_PREFIXES array out of the plugin source", () => {
    // Anchor sanity: a refactor that renames/moves the array must fail HERE,
    // not let the comparison pass vacuously against zero entries.
    expect(pluginPrefixes.length).toBeGreaterThan(0);
    expect(pluginPrefixes).toContain("task:");
  });

  it("mirror matches the plugin INSPECTABLE_ENTITY_PREFIXES 1:1", () => {
    expect(sorted(INSPECTABLE_ENTITY_PREFIXES)).toEqual(sorted(pluginPrefixes));
  });

  // ── Guard teeth — prove the comparison actually detects drift ──────────

  it("detects a prefix missing from the mirror", () => {
    const perturbed = INSPECTABLE_ENTITY_PREFIXES.filter((p) => p !== "task:");
    expect(sorted(perturbed)).not.toEqual(sorted(pluginPrefixes));
  });

  it("detects an extra prefix the plugin does not declare", () => {
    const perturbed = [...INSPECTABLE_ENTITY_PREFIXES, "bogus:"];
    expect(sorted(perturbed)).not.toEqual(sorted(pluginPrefixes));
  });
});
