/**
 * Focus indicator CSS parity tests.
 *
 * The `[data-focused]` rule in `src/index.css` is the single source of
 * truth for the spatial-focus visual. These tests lock down two
 * guarantees that the React tree has no way to express directly:
 *
 *   1. The global rule paints ONLY the left-edge bar — no ring/box
 *      surround. A regression here would put two overlapping indicators
 *      on every focused element.
 *
 *   2. The per-consumer override classes exist for every element type
 *      whose natural parent clips a negative-left bar. Without these
 *      overrides, consumers like data-table cells, LeftNav buttons, and
 *      perspective tabs would lose their focus indicator entirely once
 *      the ring is removed.
 *
 * Implemented as a node test (`fs.readFile`) because `getComputedStyle`
 * against a Tailwind `@apply` utility reports the resolved box-shadow,
 * not the source utility — and the contract we want to enforce is about
 * the source CSS, not the resolved style.
 */
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to the project's `index.css`. */
const INDEX_CSS_PATH = resolve(__dirname, "..", "index.css");

/**
 * Extract the body of a CSS rule by selector from a stylesheet string.
 *
 * Matches `<selector> { ... }` and returns everything between the
 * outermost braces (or `null` if the selector is not found). Assumes
 * the body contains no nested braces — which is true for all rules in
 * `index.css` except `@keyframes`, which we never query here.
 */
function ruleBody(css: string, selector: string): string | null {
  // Escape selector for regex — square brackets and dots matter here.
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(`${escaped}\\s*\\{([^}]*)\\}`).exec(css);
  return match ? match[1] : null;
}

describe("[data-focused] global focus rule", () => {
  const css = readFileSync(INDEX_CSS_PATH, "utf8");

  it("does NOT apply any ring-* utility (only the left bar remains)", () => {
    const body = ruleBody(css, "[data-focused]");
    expect(body).not.toBeNull();
    // The ring was `@apply ring-2 ring-primary ring-inset`. Any of those
    // three utilities reappearing would regress the "bar-only" contract.
    expect(body).not.toMatch(/\bring-\d+\b/);
    expect(body).not.toMatch(/\bring-primary\b/);
    expect(body).not.toMatch(/\bring-inset\b/);
  });

  it("keeps `position: relative` so ::before can anchor against it", () => {
    // The bar is absolutely positioned, so the focused element must
    // remain a positioning context. This is a load-bearing part of the
    // rule that the ring removal must not accidentally drop.
    const body = ruleBody(css, "[data-focused]");
    expect(body).toMatch(/position:\s*relative/);
  });

  it("still renders the left-edge bar via ::before", () => {
    // The bar is the only indicator now — verify the ::before rule is
    // still in place with its absolute positioning and primary color.
    const body = ruleBody(css, "[data-focused]::before");
    expect(body).not.toBeNull();
    expect(body).toMatch(/position:\s*absolute/);
    expect(body).toMatch(/background-color:\s*var\(--primary\)/);
  });
});

describe("per-consumer focus overrides", () => {
  const css = readFileSync(INDEX_CSS_PATH, "utf8");

  /**
   * The default bar position is `left: -0.5rem` (outside the element).
   * Containers that clip overflow cannot show that — their override
   * repositions the bar to `left: 0` or similar so it renders inside.
   */
  const OVERRIDES = [
    // Existing overrides (regression guards — these must stay).
    { selector: ".column-header-focus[data-focused]::before" },
    { selector: ".mention-pill-focus[data-focused]::before" },
    // New overrides introduced alongside the ring removal.
    { selector: ".cell-focus[data-focused]::before" },
    { selector: ".nav-button-focus[data-focused]::before" },
    { selector: ".tab-focus[data-focused]::before" },
  ];

  it.each(OVERRIDES)(
    "defines $selector with a `left` positioning override",
    ({ selector }) => {
      const body = ruleBody(css, selector);
      expect(body).not.toBeNull();
      // Every override at minimum repositions the bar's left edge so it
      // lands inside the clipping parent. The value varies per consumer
      // (0, 0.25rem, -0.25rem, etc.) — the contract is just "has a left
      // override," not a specific value.
      expect(body).toMatch(/left:\s*[-\d]/);
    },
  );
});
