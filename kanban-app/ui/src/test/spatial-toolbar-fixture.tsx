/**
 * Toolbar + PerspectiveTabBar + LeftNav spatial-nav fixture.
 *
 * ## Purpose
 *
 * Exercises the top-edge nav contract for the production `<NavBar />`:
 * pressing `k` (Up) from a focused perspective tab or LeftNav button
 * must land on one of the toolbar's FocusScope-wrapped interactive
 * elements (`toolbar:board-selector`, `toolbar:inspect-board`,
 * `toolbar:percent-complete`, `toolbar:search`).
 *
 * ## Shape
 *
 * ```
 *   ┌── NavBar (toolbar)                            ────┐
 *   │ [board-selector] [inspect-board] [percent] [search]│
 *   ├──────────────────────────────────────────────────┤
 *   │ [Default] [Archive]      (perspective tab bar)   │
 *   ├────┬────────────────────────────────────────────┤
 *   │ L  │                                             │
 *   │ N  │         (empty view body — not needed)      │
 *   │ a  │                                             │
 *   │ v  │                                             │
 *   └────┴────────────────────────────────────────────┘
 * ```
 *
 * The LeftNav sits in a flex row beside the stacked toolbar +
 * perspective tab bar. This geometry mirrors the production
 * `ViewsContainer`/`AppShell` stack so the rects the spatial engine
 * sees behave like the real app.
 *
 * Per-context mocks for `@/lib/perspective-context`,
 * `@/lib/views-context`, `@/lib/schema-context`,
 * `@/lib/ui-state-context`, `@/lib/entity-store-context`, and
 * `@/components/window-container` live in the consuming test file
 * (hoist-safety). This fixture only defines the layout and exports
 * the expected moniker constants.
 */

import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { NavBar } from "@/components/nav-bar";
import { PerspectiveTabBar } from "@/components/perspective-tab-bar";
import { LeftNav } from "@/components/left-nav";
import { moniker } from "@/lib/moniker";
import { FixtureShell } from "./spatial-fixture-shell";

/**
 * Toolbar FocusScope monikers — duplicated verbatim from
 * `components/nav-bar.tsx` so the fixture and the production bar agree
 * on every moniker string. The production file does not export these
 * (they're internal); if the production list changes, this list must
 * change too.
 */
export const TOOLBAR_MONIKERS = {
  boardSelector: "toolbar:board-selector",
  inspectBoard: "toolbar:inspect-board",
  percentComplete: "toolbar:percent-complete",
  search: "toolbar:search",
} as const;

/** Regex that matches any toolbar moniker, for prefix assertions. */
export const TOOLBAR_MONIKER_PREFIX = /^toolbar:/;

/** Fixture perspective ids — one active, one inactive. */
export const FIXTURE_PERSPECTIVE_IDS = ["default", "archive"] as const;

/** Pre-computed perspective monikers using production's `perspective:<id>` shape. */
export const FIXTURE_PERSPECTIVE_MONIKERS: readonly string[] =
  FIXTURE_PERSPECTIVE_IDS.map((id) => moniker("perspective", id));

/** Fixture view ids — one active, one inactive. */
export const FIXTURE_VIEW_IDS = ["board", "grid"] as const;

/** Pre-computed view monikers using production's `view:<id>` shape. */
export const FIXTURE_VIEW_MONIKERS: readonly string[] = FIXTURE_VIEW_IDS.map(
  (id) => moniker("view", id),
);

/**
 * Minimal CSS needed in the headless browser so the perspective tabs
 * and toolbar buttons shrink to content instead of stretching to the
 * parent's full width. The vitest-browser environment does not load
 * `index.css` (no Tailwind preflight), so utility classes like
 * `inline-flex` are no-ops by default.
 *
 * Scoped to `data-moniker` values the fixture uses so they cannot leak
 * into other fixtures.
 */
const TOOLBAR_FIXTURE_CSS = `
  [data-moniker^="perspective:"] {
    display: inline-flex;
    align-items: center;
  }
  [data-moniker^="toolbar:"] {
    display: inline-flex;
    align-items: center;
  }
`;

/**
 * Root fixture composing NavBar + PerspectiveTabBar above a LeftNav +
 * empty view body. The outer flex column stacks toolbar, perspective
 * bar, and (row with LeftNav beside a filler) so the rects the Rust
 * beam-test graph sees reflect the real production layout.
 *
 * `TooltipProvider` wraps the whole tree because both NavBar (inspect
 * / search tooltips) and LeftNav (view tooltips) render Radix Tooltips.
 * `EntityFocusProvider` owns the focus registry; `FixtureShell`
 * installs the `window` FocusLayer and binds `h/j/k/l` through the
 * same code path production uses.
 */
export function AppWithToolbarFixture() {
  return (
    <EntityFocusProvider>
      <TooltipProvider delayDuration={0}>
        <FixtureShell>
          <style>{TOOLBAR_FIXTURE_CSS}</style>
          <div
            data-testid="toolbar-fixture-root"
            style={{
              display: "flex",
              flexDirection: "column",
              width: "600px",
            }}
          >
            <NavBar />
            <PerspectiveTabBar />
            <div
              style={{
                display: "flex",
                flexDirection: "row",
                minHeight: "200px",
              }}
            >
              <LeftNav />
              <div
                data-testid="view-body-placeholder"
                style={{ flex: 1, padding: "16px" }}
              />
            </div>
          </div>
        </FixtureShell>
      </TooltipProvider>
    </EntityFocusProvider>
  );
}
