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

  it("mounts a single inspector FocusLayer with parentLayerKey wired to useEnclosingLayerFq", () => {
    const src = readSource();
    // The inspector layer must be mounted via <FocusLayer ... name=INSPECTOR_LAYER_NAME>.
    expect(src).toMatch(/<FocusLayer\b[\s\S]*?name=\{INSPECTOR_LAYER_NAME\}/);
    // The parentLayerKey prop must be threaded through.
    expect(src).toMatch(/parentLayerFq=\{windowLayerKey\}/);
    // And `windowLayerKey` is read from the layer context, not synthesized.
    expect(src).toMatch(/useEnclosingLayerFq\s*\(\s*\)/);
  });

  it("does not register a panel zone (deleted in card 01KQCTJY1QZ710A05SE975GHNR)", () => {
    // Per the card: the per-panel `<FocusZone moniker="panel:...">` wrap
    // was deleted. Field zones inside the inspector now register at the
    // layer root (`parentZone === null`); cross-panel nav uses the
    // kernel's beam-search cascade across all field zones in the
    // inspector layer, not a panel-as-parent fallback.
    const code = readCodeOnly();
    // No `panel:` moniker template literal in code (comments may still
    // mention it in migration context).
    expect(code).not.toMatch(/panel:\$\{/);
    // No `<FocusZone>` element in code — `InspectorsContainer` mounts
    // only `<FocusLayer>`. Field zones live inside `<EntityInspector>`
    // descendants and are not its concern.
    expect(code).not.toMatch(/<FocusZone\b/);
  });

  it("does not import the deleted InspectorFocusBridge", () => {
    const code = readCodeOnly();
    // The bridge was deleted in card 01KQCTJY1QZ710A05SE975GHNR.
    expect(code).not.toMatch(/InspectorFocusBridge/);
    expect(code).not.toMatch(/inspector-focus-bridge/);
  });

  it("uses the asSegment brand helper (no raw string casts)", () => {
    const src = readSource();
    // The branded layer name must come from `asSegment(...)`.
    expect(src).toMatch(/asSegment\(\s*"inspector"\s*\)/);
  });
});
