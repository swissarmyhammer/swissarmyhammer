/**
 * Perspective-bar spatial-nav fixture for vitest-browser tests.
 *
 * ## Purpose
 *
 * Verifies that the production `<PerspectiveTabBar />` component is
 * reachable from the active view via top-edge navigation (pressing `k`
 * from a top-row card or cell lands on the active perspective tab) and
 * that horizontal nav (`h`/`l`) moves between perspective tabs.
 *
 * The fixture composes the **real** `<PerspectiveTabBar />` sitting above
 * the 3x3 board fixture or the 3x3 grid fixture, under a single
 * `EntityFocusProvider` + `FixtureShell`. Mocks for
 * `@/lib/perspective-context`, `@/lib/views-context`,
 * `@/lib/schema-context`, `@/lib/ui-state-context`,
 * `@/lib/context-menu`, `@/lib/entity-store-context`, and
 * `@/components/window-container` live in the test file that imports
 * this fixture — same pattern as `perspective-tab-bar.test.tsx`.
 *
 * ## Why re-use the board/grid fixtures
 *
 * The top-edge-nav contract is "from any view, `k` reaches a
 * perspective tab". Re-using the existing 3x3 board and 3x3 grid
 * fixtures means the assertion space is the same as the per-view
 * spatial-nav tests, and the fixtures provide the deterministic rects
 * the beam test needs. This fixture just stacks the tab bar on top.
 *
 * ## Shape
 *
 * ```
 *   ┌──────────────────────────────────────┐
 *   │ [Default] [Archive]      (tab bar)   │   ← perspective:default, perspective:archive
 *   ├──────────────────────────────────────┤
 *   │   card-1-1   card-2-1   card-3-1     │   ← board or grid content
 *   │   card-1-2   card-2-2   card-3-2     │
 *   │   card-1-3   card-2-3   card-3-3     │
 *   └──────────────────────────────────────┘
 * ```
 *
 * Fixture exports two components: `AppWithBoardAndPerspectiveFixture`
 * and `AppWithGridAndPerspectiveFixture`.
 */

import { type ReactNode } from "react";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { PerspectiveTabBar } from "@/components/perspective-tab-bar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { moniker } from "@/lib/moniker";
import { FixtureShell } from "./spatial-fixture-shell";
import { BOARD_COLUMNS, FixtureColumn } from "./spatial-board-fixture";
import { GRID_ROWS, FixtureRow } from "./spatial-grid-fixture";

/**
 * Fixture perspective ids. Two perspectives (active + inactive) so
 * `h`/`l` between tabs has a sibling to move to — any more would be
 * redundant for nav assertions. Both the board and grid variants use
 * this same ordering.
 */
export const FIXTURE_PERSPECTIVE_IDS = ["default", "archive"] as const;

/** Pre-computed monikers in the same shape production uses (`perspective:<id>`). */
export const FIXTURE_PERSPECTIVE_MONIKERS: readonly string[] =
  FIXTURE_PERSPECTIVE_IDS.map((id) => moniker("perspective", id));

/**
 * Minimal CSS injected into the fixture so the perspective tab bar and
 * tab divs use the `inline-flex` / `h-8` layouts they rely on in
 * production. The vitest-browser test environment does not load
 * `index.css` (no Tailwind preflight, no utility classes), so the
 * `className="inline-flex items-center"` on the tab's root div is a
 * no-op — the div ends up `display: block` and stretches to the parent's
 * full width, which makes the beam test score the tabs as vertically
 * stacked 420px bands.
 *
 * The overrides below are scoped to `data-moniker^="perspective:"` so
 * they cannot leak into other fixtures, and they match the production
 * Tailwind classes (`inline-flex`, `items-center`) verbatim.
 */
const PERSPECTIVE_FIXTURE_CSS = `
  [data-moniker^="perspective:"] {
    display: inline-flex;
    align-items: center;
  }
`;

/**
 * Stack `<PerspectiveTabBar />` above the fixture body so the tab bar's
 * rect is guaranteed to sit above the first row of cards/cells.
 *
 * The `flex-col` layout ensures a straightforward vertical stack with
 * stable rects; tests rely on `getBoundingClientRect()` returning a
 * tab-bar rect that sits strictly above the card rects for the beam
 * test to find tab monikers when `k` fires from the top row.
 *
 * The inline `<style>` block is what makes the perspective tab divs
 * shrink to content in the headless browser — see
 * `PERSPECTIVE_FIXTURE_CSS` above for why this is necessary.
 */
function PerspectiveStack({ children }: { children: ReactNode }) {
  return (
    <div
      data-testid="perspective-stack-root"
      style={{
        display: "flex",
        flexDirection: "column",
        width: "420px",
      }}
    >
      <style>{PERSPECTIVE_FIXTURE_CSS}</style>
      <PerspectiveTabBar />
      {children}
    </div>
  );
}

/**
 * 3x3 board fixture with the perspective tab bar stacked on top.
 *
 * Tests drive `h`/`l` between tab monikers and `k` from any top-row
 * card into the active perspective tab.
 */
export function AppWithBoardAndPerspectiveFixture() {
  return (
    <EntityFocusProvider>
      <TooltipProvider delayDuration={0}>
        <FixtureShell>
          <PerspectiveStack>
            <div
              data-testid="board-fixture-root"
              style={{
                display: "flex",
                flexDirection: "row",
                padding: "16px",
                alignItems: "flex-start",
              }}
            >
              {Array.from({ length: BOARD_COLUMNS }, (_, c) => (
                <FixtureColumn key={c} col={c + 1} />
              ))}
            </div>
          </PerspectiveStack>
        </FixtureShell>
      </TooltipProvider>
    </EntityFocusProvider>
  );
}

/**
 * 3x3 grid fixture with the perspective tab bar stacked on top.
 *
 * Tests drive `k` from the top-row cell into the active perspective
 * tab and `j` from the active perspective tab back down into the top
 * row.
 */
export function AppWithGridAndPerspectiveFixture() {
  return (
    <EntityFocusProvider>
      <TooltipProvider delayDuration={0}>
        <FixtureShell>
          <PerspectiveStack>
            <div
              data-testid="grid-fixture-root"
              style={{
                width: "400px",
                display: "flex",
                flexDirection: "column",
              }}
            >
              {Array.from({ length: GRID_ROWS }, (_, r) => (
                <FixtureRow key={r} rowIndex={r} />
              ))}
            </div>
          </PerspectiveStack>
        </FixtureShell>
      </TooltipProvider>
    </EntityFocusProvider>
  );
}
