/**
 * Source-level guards for `inspectors-container.tsx` — assertions that
 * check the file's textual contents rather than its runtime behaviour.
 *
 * The inspector container migrated from the legacy `useRestoreFocus()`
 * hook to a layer-based focus-restore model: the inspector
 * `<FocusLayer>` pops on close and emits the parent layer's
 * `last_focused`, which is what the Rust spatial registry uses to
 * route focus back to the board. With that machinery in place, every
 * `useRestoreFocus()` call is a regression — these tests pin that
 * the import and the call site stay deleted.
 *
 * Node-only because they read the source file from disk; lives under
 * the `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to `inspectors-container.tsx`, the file under guard. */
const INSPECTORS_CONTAINER_PATH = resolve(
  __dirname,
  "inspectors-container.tsx",
);

/** Read the actual `inspectors-container.tsx` source as a string. */
function readSource(): string {
  return readFileSync(INSPECTORS_CONTAINER_PATH, "utf-8");
}

/**
 * Strip comments (block + line) from the source so the guards only match
 * real code. Doc comments routinely cite legacy symbol names while
 * explaining the migration that removed them — the goal of these guards
 * is to catch a *call* or *import*, not a stray mention in prose.
 */
function readCodeOnly(): string {
  const src = readSource();
  // Remove block comments first (non-greedy across newlines), then line
  // comments. `[\s\S]` matches newlines without flags. The order matters:
  // a block comment can contain `//` text that we don't want to keep.
  return src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^[ \t]*\/\/.*$/gm, "");
}

describe("InspectorsContainer source-level guards", () => {
  it("does not import or call useRestoreFocus", () => {
    const code = readCodeOnly();
    // No `useRestoreFocus` symbol in code — neither imported nor called.
    // (Comments may still mention the legacy hook for migration context.)
    expect(code).not.toMatch(/\buseRestoreFocus\b/);
  });

  it("mounts a single inspector FocusLayer with parentLayerKey wired to useCurrentLayerKey", () => {
    const src = readSource();
    // The inspector layer must be mounted via <FocusLayer ... name=INSPECTOR_LAYER_NAME>.
    expect(src).toMatch(/<FocusLayer\b[\s\S]*?name=\{INSPECTOR_LAYER_NAME\}/);
    // The parentLayerKey prop must be threaded through.
    expect(src).toMatch(/parentLayerKey=\{windowLayerKey\}/);
    // And `windowLayerKey` is read from the layer context, not synthesized.
    expect(src).toMatch(/useCurrentLayerKey\s*\(\s*\)/);
  });

  it("wraps each panel in a FocusScope kind=zone with a panel:<type>:<id> moniker", () => {
    const src = readSource();
    expect(src).toMatch(/<FocusScope\b[\s\S]*?kind="zone"/);
    expect(src).toMatch(/panel:\$\{entry\.entityType\}:\$\{entry\.entityId\}/);
  });

  it("uses the asLayerName / asMoniker brand helpers (no raw string casts)", () => {
    const src = readSource();
    // The branded layer name must come from `asLayerName(...)`.
    expect(src).toMatch(/asLayerName\(\s*"inspector"\s*\)/);
    // The branded panel moniker must come from `asMoniker(...)`.
    // Match across optional whitespace/newline so prettier line-breaks don't break the guard.
    expect(src).toMatch(/asMoniker\(\s*`panel:/);
  });
});
