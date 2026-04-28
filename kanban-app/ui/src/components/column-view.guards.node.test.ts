/**
 * Source-level guards for `column-view.tsx` — the legacy keyboard-nav
 * vestiges must stay deleted.
 *
 * These tests grep the shipped `column-view.tsx` source for tokens that
 * should no longer appear in it. They protect against regressions where a
 * future edit reintroduces pull-based claim machinery, neighbor-moniker
 * plumbing, or a column-level keydown listener that bypasses spatial nav.
 *
 * Node-only because they read the source file from disk; lives under the
 * `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to `column-view.tsx`, the file under guard. */
const COLUMN_VIEW_PATH = resolve(__dirname, "column-view.tsx");

/** Read the actual `column-view.tsx` source as a string. */
function readColumnViewSource(): string {
  return readFileSync(COLUMN_VIEW_PATH, "utf-8");
}

describe("ColumnView source-level guards", () => {
  it("does not import ClaimPredicate", () => {
    const src = readColumnViewSource();
    expect(src).not.toMatch(/\bClaimPredicate\b/);
  });

  it("contains no neighbor-moniker plumbing", () => {
    const src = readColumnViewSource();
    // These names belonged to the legacy pull-based claim machinery — every
    // occurrence is a regression.
    expect(src).not.toMatch(/\bleftColumnTaskMonikers\b/);
    expect(src).not.toMatch(/\brightColumnTaskMonikers\b/);
    expect(src).not.toMatch(/\bleftColumnHeaderMoniker\b/);
    expect(src).not.toMatch(/\brightColumnHeaderMoniker\b/);
    expect(src).not.toMatch(/\ballBoardTaskMonikers\b/);
    expect(src).not.toMatch(/\ballBoardHeaderMonikers\b/);
    expect(src).not.toMatch(/\bisFirstColumn\b/);
    expect(src).not.toMatch(/\bisLastColumn\b/);
    expect(src).not.toMatch(/\bcardClaimPredicates\b/);
    expect(src).not.toMatch(/\bnameFieldClaimWhen\b/);
    expect(src).not.toMatch(/\bcellMonikerMap\b/);
  });

  it("does not pass claimWhen to any child FocusScope or component", () => {
    const src = readColumnViewSource();
    // No `claimWhen=` JSX prop or object key anywhere — the column relies
    // on the spatial-nav layer's geometric beam-search instead of pull-based
    // predicates.
    expect(src).not.toMatch(/\bclaimWhen\b/);
  });

  it("does not register a column-level keyboard listener", () => {
    const src = readColumnViewSource();
    // No `onKeyDown` JSX prop — the spatial-nav layer owns keys at this level.
    expect(src).not.toMatch(/\bonKeyDown\s*=/);
    // No raw `keydown` listener strings — the only legitimate column-level
    // listeners are drag-driven, and drag/drop uses dragover/drop events,
    // never keydown.
    expect(src).not.toMatch(/['"]keydown['"]/);
  });

  it('wraps the column body in <FocusScope moniker={asMoniker(...)}>', () => {
    const src = readColumnViewSource();
    // The column registers as a navigable zone in the spatial graph; its
    // moniker is computed from `column.moniker`, so we look for the
    // structural pattern rather than a literal moniker string.
    expect(src).toMatch(/<FocusScope[\s\S]*?/);
    expect(src).toMatch(/moniker={asMoniker\(columnMoniker\)}/);
  });
});
