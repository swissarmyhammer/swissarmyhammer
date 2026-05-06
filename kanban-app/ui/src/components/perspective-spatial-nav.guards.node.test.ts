/**
 * Source-level guards for the perspective spatial-nav wrapping.
 *
 * These tests grep the shipped sources for tokens that anchor the new
 * spatial-nav contract — and tokens that should never appear because they
 * belong to the legacy keyboard-nav machinery the spatial-nav stack
 * replaces.
 *
 * Files under guard:
 *   - `perspective-tab-bar.tsx` — registers as `ui:perspective-bar`
 *     scope-with-children; each tab is a `perspective_tab:{id}`
 *     FocusScope wrapper containing `perspective_tab.name:{id}` (and,
 *     when rendered, the `perspective_tab.filter:{id}` /
 *     `perspective_tab.group:{id}` icon button) FocusScope leaves.
 *   - `perspective-container.tsx` — registers as `ui:perspective`
 *     scope-with-children with the canonical
 *     `flex flex-col flex-1 min-h-0 min-w-0` layout.
 *   - `view-container.tsx` — does NOT register a `ui:view` scope. The
 *     redundant viewport-sized chrome wrapper was deleted because it
 *     overlapped the inner `ui:board` / `ui:grid` scope-with-children
 *     for the same rect.
 *
 * After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
 * legacy container primitive and `<FocusScope>` into a single
 * primitive, "scope" in these guards refers to the unified
 * `<FocusScope>` — a scope with no children behaves as a leaf, a
 * scope with children behaves as a navigable container.
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

  it("wraps the tab-bar root via FocusScope with moniker ui:perspective-bar", () => {
    expect(SRC).toMatch(
      /<FocusScope\s+moniker=\{asSegment\("ui:perspective-bar"\)/,
    );
  });

  it("wraps each tab in FocusScope with moniker perspective_tab:${id}", () => {
    // Post-reshape (card 01KQQSVS4EBKKFN5SS7MW5P8CN) the per-tab
    // wrapper is a scope-with-children rather than a leaf — promoting
    // it lets the inner Pressable-based icon buttons mount their own
    // leaf FocusScopes without violating the legacy "scope cannot
    // also act as a leaf" rule. After parent task
    // `01KQSDP4ZJY5ERAJ68TFPVFRRE` both shapes are the same
    // `<FocusScope>` primitive; the guard's intent is structural
    // ("the per-tab scope contains inner leaves"), not
    // primitive-name-based.
    expect(SRC).toMatch(
      /<FocusScope\s+moniker=\{asSegment\(`perspective_tab:\$\{id\}`\)/,
    );
  });

  it("does NOT wrap the tab name button in a redundant perspective_tab.name FocusScope", () => {
    // Post-`8789dcc15`, the inner `perspective_tab.name:${id}` FocusScope
    // was dropped because it sat at the exact same rect as the outer
    // `perspective_tab:${id}` wrapper (an inactive tab's only content is
    // the name button — same x, y, width, height). The outer
    // `perspective_tab` is itself the focusable target now; Enter-on-focus
    // routes through the existing `ui.entity.startRename` CommandDef. A
    // regression that re-introduces the inner scope would re-create the
    // needless-nesting warning the kernel emits for overlapping rects.
    expect(SRC).not.toMatch(/asSegment\(`perspective_tab\.name:\$\{id\}`\)/);
  });

  it("wraps the filter formula bar in FocusScope with moniker filter_editor:${activePerspectiveId}", () => {
    // Mirror of the per-tab guard — the filter formula bar must be a leaf
    // peer of the tabs inside the `ui:perspective-bar` zone so beam-search
    // can land on it via `nav.left` / `nav.right`. The per-perspective
    // segment matches the existing `key={activePerspective.id}` remount on
    // `<FilterFormulaBar>` so the kernel sees a distinct leaf per perspective
    // rather than a shared one whose identity flips on perspective change.
    expect(SRC).toMatch(
      /<FocusScope\s+moniker=\{asSegment\(`filter_editor:\$\{perspectiveId\}`\)/,
    );
  });

  it("uses the asSegment brand helper from @/types/spatial", () => {
    expect(SRC).toMatch(/from\s+["']@\/types\/spatial["']/);
    expect(SRC).toMatch(/\basSegment\b/);
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

  it("wraps the perspective body in FocusScope with moniker ui:perspective", () => {
    expect(SRC).toMatch(
      /<FocusScope\s+moniker=\{asSegment\("ui:perspective"\)/,
    );
  });

  it("preserves the flex chain via className on the perspective scope", () => {
    expect(SRC).toMatch(/flex\s+flex-col\s+flex-1\s+min-h-0\s+min-w-0/);
  });

  it("uses the asSegment brand helper from @/types/spatial", () => {
    expect(SRC).toMatch(/from\s+["']@\/types\/spatial["']/);
    expect(SRC).toMatch(/\basSegment\b/);
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

  it("does NOT wrap the rendered view in a FocusScope with moniker ui:view", () => {
    // The redundant `ui:view` chrome scope overlapped the inner view's
    // own viewport-sized scope (`ui:board` / `ui:grid`) for the same
    // rect. It was deleted to remove the no-op graph hop. Regression:
    // keep the source free of any `<FocusScope moniker={asSegment("ui:view")}>`.
    expect(SRC).not.toMatch(/<FocusScope\s+moniker=\{asSegment\("ui:view"\)/);
  });

  it("does NOT import the FocusScope primitive (no spatial scope is mounted here)", () => {
    // After the wrapper deletion, view-container.tsx has no `<FocusScope>`
    // at all — its only spatial-related responsibility is the
    // `<CommandScopeProvider moniker={`view:${viewId}`}>` frame. Pin the
    // import absence so a follow-up that re-introduces a scope is forced
    // to update this guard explicitly.
    //
    // Parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy
    // container primitive into `<FocusScope>`; the canonical import
    // path is now `@/components/focus-scope`.
    expect(SRC).not.toMatch(/from\s+["']@\/components\/focus-scope["']/);
  });
});
