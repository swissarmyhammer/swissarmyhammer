/**
 * Architectural guard pinning the migration of the Space → inspect
 * binding off the BoardView's `board.inspect` and onto the per-entity
 * `<Inspectable>` wrapper (card 01KQ9XJ4XGKVW24EZSQCA6K3E2).
 *
 * The end-to-end Space-on-card / Space-on-perspective-tab tests live in
 * `spatial-nav-end-to-end.spatial.test.tsx` (Family 4) and exercise the
 * full board mount; this file pins the *contract* of the migration via
 * a source scan, identical in shape to `focus-architecture.guards.node.test.ts`:
 *
 *   1. The string literal `id: "board.inspect"` (the legacy command's
 *      registration site) must not appear in any production source.
 *      A future revert that re-introduces it would silently bring back
 *      duplicate Space ownership; this guard catches that without
 *      having to spin up the heavyweight `<BoardView>` provider stack.
 *
 *   2. The replacement binding follows a two-tier architecture:
 *      `inspectable.tsx` registers a per-Inspectable scope-level
 *      `id: "entity.inspect"` with `keys.cua: "Space"` (shadows when an
 *      Inspectable is in the focused scope chain), and
 *      `app-shell.tsx` registers a root-scope `entity.inspect` fallback
 *      with the same Space binding (handles the no-focus case so Space
 *      always resolves and the keybinding handler can `preventDefault`
 *      the browser's page-scroll). This guard pins only the
 *      per-Inspectable site; the root-scope fallback in `app-shell.tsx`
 *      is exercised by `spatial-nav-end-to-end.spatial.test.tsx`
 *      (Family 4) and the unit tests in
 *      `inspectable.space.browser.test.tsx`.
 *
 * Node-only because it reads source files from disk; lives under the
 * `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to the `kanban-app/ui/src` root that contains shipped UI code. */
const SRC_ROOT = resolve(__dirname, "..");

/**
 * Walk a directory tree, returning every file path whose name matches
 * one of the given extensions. Skips `node_modules` and any directory
 * whose name starts with a dot.
 */
function walkSources(root: string, exts: readonly string[]): string[] {
  const found: string[] = [];
  const visit = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      if (entry.startsWith(".") || entry === "node_modules") continue;
      const full = resolve(dir, entry);
      const stat = statSync(full);
      if (stat.isDirectory()) {
        visit(full);
      } else if (exts.some((ext) => entry.endsWith(ext))) {
        found.push(full);
      }
    }
  };
  visit(root);
  found.sort();
  return found;
}

/**
 * Treat a path as a "shipped UI source" — production .ts / .tsx files
 * that are NOT tests. Comments in test fixtures often reference legacy
 * command ids for documentation; we intentionally skip those.
 */
function isProductionSource(path: string): boolean {
  return (
    !path.endsWith(".test.ts") &&
    !path.endsWith(".test.tsx") &&
    !path.endsWith(".node.test.ts") &&
    !path.endsWith(".browser.test.ts") &&
    !path.endsWith(".browser.test.tsx") &&
    !path.endsWith(".spatial.test.tsx")
  );
}

/**
 * Strip JS / TS line and block comments from a source so the regression
 * scan ignores comment text that references the legacy command id by
 * name. Replaces comments with whitespace of the same length so line /
 * column numbers stay meaningful in the error message.
 */
function stripJsComments(src: string): string {
  // Block comments: /* ... */ — keep newlines so line numbers line up.
  let out = src.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));
  // Line comments: // ... to end of line.
  out = out.replace(/\/\/[^\n]*/g, (m) => m.replace(/./g, " "));
  return out;
}

describe("BoardView Space-inspect migration", () => {
  it("no production source registers the legacy `board.inspect` command id", () => {
    // The legacy `makeInspectCommand` factory registered
    // `id: "board.inspect"` at the BoardView's `<CommandScopeProvider>`.
    // After the migration, no production code path should mint that
    // command anymore — Space ownership lives on the per-entity
    // `<Inspectable>` wrapper, which uses `id: "entity.inspect"`.
    //
    // The regex matches both quoted forms (`"board.inspect"`) and
    // template-literal forms (`` `board.inspect` ``) since either could
    // be used to re-introduce the binding.
    const tsFiles = walkSources(SRC_ROOT, [".ts", ".tsx"]).filter(
      isProductionSource,
    );
    expect(tsFiles.length).toBeGreaterThan(0);

    const offenders: { file: string; line: number; text: string }[] = [];
    const pattern = /id\s*:\s*["'`]board\.inspect["'`]/;
    for (const file of tsFiles) {
      const original = readFileSync(file, "utf-8");
      const stripped = stripJsComments(original);
      const originalLines = original.split("\n");
      const strippedLines = stripped.split("\n");
      strippedLines.forEach((text, i) => {
        if (pattern.test(text)) {
          offenders.push({
            file: relative(SRC_ROOT, file),
            line: i + 1,
            text: originalLines[i].trim(),
          });
        }
      });
    }

    expect(
      offenders,
      `Found production source registering the legacy \`board.inspect\` command:\n` +
        offenders.map((o) => `  ${o.file}:${o.line}  ${o.text}`).join("\n") +
        `\n\nSpace ownership has moved to the per-entity \`<Inspectable>\`\n` +
        `wrapper (see \`inspectable.tsx\`). The board scope no longer\n` +
        `claims a Space binding; if you need to add an inspect-class\n` +
        `command at the board level, register it on \`<Inspectable>\`\n` +
        `directly so every wrapped entity (card, column, board zone,\n` +
        `field, attachment) responds consistently.`,
    ).toEqual([]);
  });

  it("the per-Inspectable Space binding lives in `inspectable.tsx`", () => {
    // After the migration, Space → inspect dispatch is wired in two
    // tiers: a scope-level `entity.inspect` `CommandDef` registered
    // inside `<Inspectable>` (this file pins that site), and a
    // root-scope `entity.inspect` fallback registered in
    // `components/app-shell.tsx` (`buildRootInspectCommand`) that
    // catches the no-focus case so Space never falls through to the
    // browser's page-scroll. This guard is a positive presence
    // assertion on the per-Inspectable site only — it does NOT enforce
    // exclusivity. A reader searching for "where is Space wired to
    // inspect" should expect to find both call sites; the canonical
    // entity-level wiring lives here in `inspectable.tsx`, while the
    // root fallback is asserted by the e2e Family 4 and the
    // `inspectable.space.browser.test.tsx` no-focus scenario.
    const inspectableSrc = readFileSync(
      resolve(SRC_ROOT, "components/inspectable.tsx"),
      "utf-8",
    );

    // The id and the cua-Space binding must both appear in the same
    // file. Match them as a flexible regex (multiline, allow whitespace
    // between key and value) so a reformat doesn't break the guard.
    expect(inspectableSrc).toMatch(/id\s*:\s*["'`]entity\.inspect["'`]/);
    expect(inspectableSrc).toMatch(/cua\s*:\s*["'`]Space["'`]/);
  });
});
