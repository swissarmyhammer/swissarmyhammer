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
 * The guards also pin the deletions from card
 * `01KQCTJY1QZ710A05SE975GHNR`:
 *
 *   - The `<InspectorFocusBridge>` component is gone.
 *   - The `panel:*` moniker shape is gone — the per-entity zone added
 *     by card `01KQFCQ9QMQKCDYVWGTXSVK5PZ` uses the entity moniker
 *     itself (e.g. `task:abc`) as the segment, not a `panel:` prefix.
 *   - `inspector.edit / editEnter / exitEdit` commands stay deleted
 *     (`field.edit / field.editEnter` cover the semantics).
 *
 * The guards explicitly *permit* a single `<FocusScope>` import — the
 * entity-scope wrap inside `<InspectorPanel>` from card
 * `01KQFCQ9QMQKCDYVWGTXSVK5PZ` is the only legitimate consumer.
 * Imported from `@/components/focus-scope` after parent task
 * `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed `<FocusZone>` and
 * `<FocusScope>` into a single primitive.
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
    // The parentLayerFq prop must be threaded through.
    expect(src).toMatch(/parentLayerFq=\{windowLayerFq\}/);
    // And `windowLayerFq` is read from the FQM context, not synthesized.
    expect(src).toMatch(/useFullyQualifiedMoniker\s*\(\s*\)/);
  });

  it("does not register a panel-prefixed zone (the panel:* moniker shape is deleted)", () => {
    // Two stacked deletions stay pinned here:
    //
    //   - Card `01KQCTJY1QZ710A05SE975GHNR` deleted the per-panel
    //     `<FocusScope moniker="panel:type:id">` wrap.
    //   - Card `01KQFCQ9QMQKCDYVWGTXSVK5PZ` reintroduced a per-entity
    //     `<FocusScope>`, but keyed by the **entity moniker itself**
    //     (e.g. `task:abc`), NOT a `panel:*` prefix. The entity moniker
    //     is the natural identity; the panel is just chrome.
    //
    // The guard therefore forbids `panel:*` template literals while
    // allowing a `<FocusScope>` element so the entity-zone wrap can
    // live in this file. Doc comments may still mention `panel:*` in
    // migration context.
    const code = readCodeOnly();
    expect(code).not.toMatch(/panel:\$\{/);
  });

  it("permits a single FocusScope import (the entity-scope wrap from card 01KQFCQ9QMQKCDYVWGTXSVK5PZ)", () => {
    // The entity-scope wrap added in card `01KQFCQ9QMQKCDYVWGTXSVK5PZ`
    // imports `<FocusScope>` and uses it inside `<InspectorPanel>`. This
    // guard confirms exactly one `<FocusScope>` element appears in code
    // (a regression to multi-scope or panel-scope shapes would surface
    // as a count mismatch).
    //
    // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed
    // `<FocusZone>` and `<FocusScope>` into a single primitive, the
    // import lives at `@/components/focus-scope` rather than the
    // legacy `focus-zone` path.
    const code = readCodeOnly();
    const matches = code.match(/<FocusScope\b/g) ?? [];
    expect(
      matches.length,
      "expected exactly one <FocusScope> in inspectors-container.tsx — the entity-scope wrap inside <InspectorPanel>",
    ).toBe(1);
    // The import must come from `@/components/focus-scope` (the
    // production primitive), not from a renamed bridge or wrapper.
    expect(code).toMatch(
      /import\s*\{\s*FocusScope\s*\}\s*from\s*"@\/components\/focus-scope"/,
    );
  });

  it("does not import the deleted InspectorFocusBridge", () => {
    const code = readCodeOnly();
    // The bridge was deleted in card 01KQCTJY1QZ710A05SE975GHNR and
    // stays deleted under card 01KQFCQ9QMQKCDYVWGTXSVK5PZ — the
    // entity-zone wrap is a direct `<FocusScope>` in this file, not a
    // bridge component.
    expect(code).not.toMatch(/InspectorFocusBridge/);
    expect(code).not.toMatch(/inspector-focus-bridge/);
  });

  it("does not reintroduce the inspector.edit / editEnter / exitEdit commands", () => {
    // Card `01KQCTJY1QZ710A05SE975GHNR` deleted these commands; the
    // field-level equivalents (`field.edit` / `field.editEnter`) cover
    // the semantics. Card `01KQFCQ9QMQKCDYVWGTXSVK5PZ` does not bring
    // them back.
    const code = readCodeOnly();
    expect(code).not.toMatch(/inspector\.edit\b/);
    expect(code).not.toMatch(/inspector\.editEnter\b/);
    expect(code).not.toMatch(/inspector\.exitEdit\b/);
  });

  it("uses the asSegment brand helper (no raw string casts)", () => {
    const src = readSource();
    // The branded layer name must come from `asSegment(...)`.
    expect(src).toMatch(/asSegment\(\s*"inspector"\s*\)/);
  });
});
