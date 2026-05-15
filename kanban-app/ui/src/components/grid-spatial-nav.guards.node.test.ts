/**
 * Source-level guards for the grid-view spatial-nav wrapping.
 *
 * These tests grep the shipped sources for tokens that anchor the new
 * spatial-nav contract — and tokens that should never appear because they
 * belong to the legacy keyboard-nav machinery the spatial-nav stack
 * replaces.
 *
 * Files under guard:
 *   - `grid-view.tsx` — wraps the grid body in `<FocusScope moniker="ui:grid">`,
 *     derives the cursor from the focused moniker via `resolveCursorFromFocus`,
 *     and no longer threads pull-based predicate machinery (`buildCellPredicates`,
 *     `cellMonikerMap`, `claimPredicates`, `ClaimPredicate`).
 *   - `data-table.tsx` — each cell registers as `<FocusScope moniker="grid_cell:R:K">`,
 *     and no longer accepts `cellMonikers` / `claimPredicates` / `claimWhen` props.
 *
 * Node-only because they read source files from disk; lives under the
 * `*.node.test.ts` suffix recognised by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Read a sibling source file as a string. */
function readSibling(name: string): string {
  return readFileSync(resolve(__dirname, name), "utf-8");
}

describe("GridView source-level guards", () => {
  const SRC = readSibling("grid-view.tsx");

  it("does not import or reference ClaimPredicate", () => {
    expect(SRC).not.toMatch(/\bClaimPredicate\b/);
  });

  it("does not declare a claimWhen prop or reference it on JSX", () => {
    expect(SRC).not.toMatch(/\bclaimWhen\b/);
  });

  it("does not reference the deleted buildCellPredicates helper", () => {
    expect(SRC).not.toMatch(/\bbuildCellPredicates\b/);
  });

  it("does not reference the deleted cellMonikerMap matrix", () => {
    expect(SRC).not.toMatch(/\bcellMonikerMap\b/);
  });

  it("does not reference the deleted claimPredicates memo", () => {
    expect(SRC).not.toMatch(/\bclaimPredicates\b/);
  });

  it("does not register a JSX onKeyDown handler", () => {
    // `onKeyDown=` would be a JSX prop; the spatial-nav layer owns keys.
    expect(SRC).not.toMatch(/\bonKeyDown\s*=/);
  });

  it("does not attach a 'keydown' DOM listener", () => {
    // The grid view must not addEventListener('keydown') — those keystrokes
    // belong to the spatial-nav keymap layer.
    expect(SRC).not.toMatch(/['"]keydown['"]/);
  });

  it("wraps the grid body via FocusScope with moniker ui:grid", () => {
    expect(SRC).toMatch(/<FocusScope\s+moniker=\{asSegment\("ui:grid"\)/);
  });

  it("uses the asSegment brand helper from @/types/spatial", () => {
    expect(SRC).toMatch(/from\s+["']@\/types\/spatial["']/);
    expect(SRC).toMatch(/\basSegment\b/);
  });

  it("imports gridCellMoniker / parseGridCellMoniker from @/lib/moniker", () => {
    // The grid uses these helpers to seed initial focus and to resolve the
    // cursor from the focused moniker. Importing them anchors the
    // grid_cell:R:K wire shape as the single source of truth.
    expect(SRC).toMatch(/from\s+["']@\/lib\/moniker["']/);
    expect(SRC).toMatch(/\bgridCellMoniker\b/);
    expect(SRC).toMatch(/\bparseGridCellMoniker\b/);
  });
});

describe("DataTable source-level guards", () => {
  const SRC = readSibling("data-table.tsx");

  it("does not import or reference ClaimPredicate", () => {
    expect(SRC).not.toMatch(/\bClaimPredicate\b/);
  });

  it("does not declare a claimWhen prop", () => {
    expect(SRC).not.toMatch(/\bclaimWhen\b/);
  });

  it("does not declare cellMonikers / claimPredicates props", () => {
    // The legacy 2-D matrices that drove pull-based nav. Cell monikers are
    // now derived from `(di, colKey)` directly inside `<GridCellFocusable>`.
    expect(SRC).not.toMatch(/\bcellMonikers\b/);
    expect(SRC).not.toMatch(/\bclaimPredicates\b/);
  });

  it("does not register a JSX onKeyDown handler", () => {
    expect(SRC).not.toMatch(/\bonKeyDown\s*=/);
  });

  it("does not attach a 'keydown' DOM listener", () => {
    expect(SRC).not.toMatch(/['"]keydown['"]/);
  });

  it("imports the FocusScope primitive from @/components/focus-scope", () => {
    expect(SRC).toMatch(/from\s+["']@\/components\/focus-scope["']/);
    expect(SRC).toMatch(/\bFocusScope\b/);
  });

  it("imports gridCellMoniker from @/lib/moniker", () => {
    // The cell wrapping mints `grid_cell:R:K` monikers via the shared
    // helper so the wire shape stays coupled to `parseGridCellMoniker`
    // in `grid-view.tsx`.
    expect(SRC).toMatch(/from\s+["']@\/lib\/moniker["']/);
    expect(SRC).toMatch(/\bgridCellMoniker\b/);
  });

  it("uses asSegment on the cell moniker before passing to FocusScope", () => {
    // Mirrors the convention in `nav-bar.tsx` and other call sites: the
    // `asSegment(...)` brand helper applies at the boundary, not inside
    // the `FocusScope` props elsewhere.
    expect(SRC).toMatch(/asSegment\(gridCellMoniker\(/);
  });
});
