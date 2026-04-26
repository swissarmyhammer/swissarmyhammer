/**
 * Source-level guards for the perspective spatial-nav wrapping.
 *
 * These tests grep the shipped sources for tokens that anchor the new
 * spatial-nav contract — and tokens that should never appear because they
 * belong to the legacy keyboard-nav machinery the spatial-nav stack
 * replaces.
 *
 * Files under guard:
 *   - `perspective-tab-bar.tsx` — registers as `ui:perspective-bar` zone;
 *     each tab is a `perspective_tab:{id}` Focusable leaf.
 *   - `perspective-container.tsx` — registers as `ui:perspective` zone with
 *     the canonical `flex flex-col flex-1 min-h-0 min-w-0` layout.
 *   - `view-container.tsx` — registers as `ui:view` zone with the canonical
 *     `flex-1 flex flex-col min-h-0 min-w-0` layout.
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

describe("PerspectiveTabBar source-level guards", () => {
  const SRC = readSibling("perspective-tab-bar.tsx");

  it("does not import or reference ClaimPredicate", () => {
    expect(SRC).not.toMatch(/\bClaimPredicate\b/);
  });

  it("does not declare a claimWhen prop", () => {
    expect(SRC).not.toMatch(/\bclaimWhen\b/);
  });

  it("does not register a JSX onKeyDown handler", () => {
    // `onKeyDown=` would be a JSX prop; the spatial-nav layer owns keys.
    expect(SRC).not.toMatch(/\bonKeyDown\s*=/);
  });

  it("does not attach a 'keydown' DOM listener (legacy useEffect pattern)", () => {
    // The tab bar must not addEventListener('keydown') — those keystrokes
    // belong to spatial nav. Rename-mode keystrokes are handled inside the
    // CM6 keymap extension, not via DOM listeners on the tab bar itself.
    expect(SRC).not.toMatch(/['"]keydown['"]/);
  });

  it("wraps the tab-bar root via FocusZone with moniker ui:perspective-bar", () => {
    expect(SRC).toMatch(
      /<FocusZone\s+moniker=\{asMoniker\("ui:perspective-bar"\)/,
    );
  });

  it("wraps each tab in Focusable with moniker perspective_tab:${id}", () => {
    expect(SRC).toMatch(
      /<Focusable\s+moniker=\{asMoniker\(`perspective_tab:\$\{id\}`\)/,
    );
  });

  it("uses the asMoniker brand helper from @/types/spatial", () => {
    expect(SRC).toMatch(/from\s+["']@\/types\/spatial["']/);
    expect(SRC).toMatch(/\basMoniker\b/);
  });
});

describe("PerspectiveContainer source-level guards", () => {
  const SRC = readSibling("perspective-container.tsx");

  it("does not import or reference ClaimPredicate", () => {
    expect(SRC).not.toMatch(/\bClaimPredicate\b/);
  });

  it("does not declare a claimWhen prop", () => {
    expect(SRC).not.toMatch(/\bclaimWhen\b/);
  });

  it("does not register a JSX onKeyDown handler", () => {
    expect(SRC).not.toMatch(/\bonKeyDown\s*=/);
  });

  it("does not attach a 'keydown' DOM listener", () => {
    expect(SRC).not.toMatch(/['"]keydown['"]/);
  });

  it("wraps the perspective body in FocusZone with moniker ui:perspective", () => {
    expect(SRC).toMatch(/<FocusZone\s+moniker=\{asMoniker\("ui:perspective"\)/);
  });

  it("preserves the flex chain via className on the perspective zone", () => {
    expect(SRC).toMatch(/flex\s+flex-col\s+flex-1\s+min-h-0\s+min-w-0/);
  });

  it("uses the asMoniker brand helper from @/types/spatial", () => {
    expect(SRC).toMatch(/from\s+["']@\/types\/spatial["']/);
    expect(SRC).toMatch(/\basMoniker\b/);
  });
});

describe("ViewContainer source-level guards", () => {
  const SRC = readSibling("view-container.tsx");

  it("does not import or reference ClaimPredicate", () => {
    expect(SRC).not.toMatch(/\bClaimPredicate\b/);
  });

  it("does not declare a claimWhen prop", () => {
    expect(SRC).not.toMatch(/\bclaimWhen\b/);
  });

  it("does not register a JSX onKeyDown handler", () => {
    expect(SRC).not.toMatch(/\bonKeyDown\s*=/);
  });

  it("does not attach a 'keydown' DOM listener", () => {
    expect(SRC).not.toMatch(/['"]keydown['"]/);
  });

  it("wraps the rendered view in FocusZone with moniker ui:view", () => {
    expect(SRC).toMatch(/<FocusZone\s+moniker=\{asMoniker\("ui:view"\)/);
  });

  it("preserves the flex chain via className on the view zone", () => {
    expect(SRC).toMatch(/flex-1\s+flex\s+flex-col\s+min-h-0\s+min-w-0/);
  });

  it("uses the asMoniker brand helper from @/types/spatial", () => {
    expect(SRC).toMatch(/from\s+["']@\/types\/spatial["']/);
    expect(SRC).toMatch(/\basMoniker\b/);
  });
});
