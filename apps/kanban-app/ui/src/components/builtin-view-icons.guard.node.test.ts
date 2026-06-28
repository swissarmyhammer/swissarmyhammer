/**
 * Metadata guard: every builtin view definition shipped in
 * `crates/swissarmyhammer-kanban/builtin/views/` must declare an `icon`
 * that resolves to a real lucide-react component via `viewIcon`.
 *
 * Regression context ("all views show the board icon"): the left-nav
 * renders `viewIcon(view) ?? LayoutGrid`. If a builtin view ships without
 * an icon — or with a name that does not exist in lucide — it silently
 * renders the LayoutGrid fallback and every such view looks like the
 * board. This guard fails the build instead.
 *
 * Node-only because it reads the builtin YAML files from disk.
 */
import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { load } from "js-yaml";
import { viewIcon } from "./view-icon";
import type { ViewDef } from "@/types/kanban";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to the builtin view definitions embedded by the kanban crate. */
const BUILTIN_VIEWS_DIR = resolve(
  __dirname,
  "../../../../../crates/swissarmyhammer-kanban/builtin/views",
);

/** Load every builtin view YAML as `(filename, ViewDef)`. */
function loadBuiltinViews(): Array<[string, ViewDef]> {
  return readdirSync(BUILTIN_VIEWS_DIR)
    .filter((f) => f.endsWith(".yaml"))
    .map((f) => [
      f,
      load(readFileSync(resolve(BUILTIN_VIEWS_DIR, f), "utf-8")) as ViewDef,
    ]);
}

describe("builtin view icon metadata", () => {
  it("finds the builtin view definitions (path guard)", () => {
    const views = loadBuiltinViews();
    expect(views.length).toBeGreaterThanOrEqual(4);
  });

  it("every builtin view declares an icon that resolves to a real lucide component", () => {
    for (const [file, def] of loadBuiltinViews()) {
      expect(def.icon, `${file} must declare an icon`).toBeTruthy();
      expect(
        viewIcon(def),
        `${file} icon "${def.icon}" must resolve to a lucide component — ` +
          "an unresolvable name silently renders the LayoutGrid fallback",
      ).not.toBeNull();
    }
  });

  it("builtin views have pairwise-distinct icons", () => {
    const views = loadBuiltinViews();
    const iconNames = views.map(([, def]) => def.icon);
    expect(new Set(iconNames).size).toBe(views.length);
  });
});
