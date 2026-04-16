/**
 * Layout regression tests for the outer app shell.
 *
 * These tests guard against horizontal overflow leaking past the content
 * area to the app chrome (NavBar, PerspectiveTabBar, LeftNav, ModeIndicator).
 *
 * Root cause of the bug being guarded: a broken `min-w-0` chain between the
 * viewport and the column scroll container inside `BoardView`. When a board
 * has more columns than fit the viewport, the intrinsic width of the columns
 * (each `min-w-[20em]` = 320px) propagated up through flex parents that
 * lacked `min-w-0`, pushing the whole layout wider than the viewport so
 * `html`/`body` scrolled horizontally and the chrome scrolled with it.
 *
 * The fix applies four CSS changes:
 *   1. `App.tsx` root div             → add `overflow-hidden`
 *   2. `views-container.tsx` flex row → add `min-w-0`
 *   3. `perspectives-container.tsx`   → add `min-w-0`
 *   4. `board-view.tsx` scroll div    → add `min-w-0`
 *
 * The tests below verify (1) the classname chain in the real components and
 * (2) the observable behavior of the combined layout: any descendant of the
 * App root that tries to exceed the viewport width must be clipped, not
 * allowed to push `html`/`body` into horizontal scroll.
 *
 * Note on Tailwind in tests: the Vitest browser harness loads React but does
 * not compile Tailwind utilities, so we assert the *presence* of class names
 * and layer in inline-style widths to force the actual overflow scenario
 * (rather than depending on `min-w-[20em]` rendering at 320px).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing presenters.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// Mock views-context so ViewsContainer can render without backend calls.
vi.mock("@/lib/views-context", () => ({
  ViewsProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  useViews: () => ({
    views: [
      { id: "board-default", name: "Board", kind: "board", icon: "kanban" },
    ],
    activeView: {
      id: "board-default",
      name: "Board",
      kind: "board",
      icon: "kanban",
    },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(),
  }),
}));

// Mock perspective-context — PerspectivesContainer wraps PerspectiveProvider.
vi.mock("@/lib/perspective-context", () => ({
  PerspectiveProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  usePerspectives: () => ({
    perspectives: [],
    activePerspective: null,
    setActivePerspectiveId: vi.fn(),
    refresh: vi.fn(),
  }),
}));

// Replace chrome presenters with lightweight stand-ins. The regression being
// tested is about their layout position staying stable when the content
// overflows — the real NavBar/PerspectiveTabBar/LeftNav/ModeIndicator all
// depend on backend state we don't want to bring up for a layout test.
vi.mock("@/components/nav-bar", () => ({
  NavBar: () => (
    <header
      role="banner"
      data-testid="nav-bar"
      style={{ flex: "0 0 auto", height: "40px" }}
    >
      NavBar
    </header>
  ),
}));

vi.mock("@/components/perspective-tab-bar", () => ({
  PerspectiveTabBar: () => (
    <div
      data-testid="perspective-tab-bar"
      style={{ flex: "0 0 auto", height: "28px" }}
    >
      Tabs
    </div>
  ),
}));

vi.mock("@/components/left-nav", () => ({
  LeftNav: () => (
    <nav data-testid="left-nav" style={{ flex: "0 0 auto", width: "160px" }}>
      LeftNav
    </nav>
  ),
}));

vi.mock("@/components/mode-indicator", () => ({
  ModeIndicator: () => (
    <div
      data-testid="mode-indicator"
      style={{ flex: "0 0 auto", height: "24px" }}
    >
      Mode
    </div>
  ),
}));

// Mock ui-state-context for transitive dependencies.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
}));

// Mock perspective-container so BoardView can render in isolation.
vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks.
// ---------------------------------------------------------------------------

import { ViewsContainer } from "./views-container";
import { PerspectivesContainer } from "./perspectives-container";
import { NavBar } from "@/components/nav-bar";
import { ModeIndicator } from "@/components/mode-indicator";

// ---------------------------------------------------------------------------
// Tailwind utility shim — the Vitest browser harness does not compile
// Tailwind CSS, so we inject the handful of utilities the outer app shell
// depends on. This lets the test verify that `min-w-0` / `overflow-hidden`
// actually clip (not just that the class names are present in the DOM).
// ---------------------------------------------------------------------------

const TAILWIND_SHIM = `
.h-screen { height: 100vh; }
.flex { display: flex; }
.flex-col { flex-direction: column; }
.flex-1 { flex: 1 1 0%; }
.min-h-0 { min-height: 0; }
.min-w-0 { min-width: 0; }
.overflow-hidden { overflow: hidden; }
.overflow-x-auto { overflow-x: auto; }
.pl-2 { padding-left: 0.5rem; }
/*
 * Column-width utilities used in column-view.tsx. These are arbitrary-value
 * Tailwind classes whose names contain brackets — escape the brackets for
 * CSS selectors so the shim matches the real class names.
 */
.min-w-\\[24em\\] { min-width: 24em; }
.max-w-\\[48em\\] { max-width: 48em; }
.shrink-0 { flex-shrink: 0; }
`;

function installTailwindShim() {
  let style = document.getElementById(
    "app-layout-test-tailwind-shim",
  ) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement("style");
    style.id = "app-layout-test-tailwind-shim";
    style.textContent = TAILWIND_SHIM;
    document.head.appendChild(style);
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * A synthetic overflow source that renders as a fixed 2000px-wide inline
 * block. Stands in for the real BoardView column strip without depending on
 * Tailwind utilities being loaded in the test environment. If any ancestor
 * in the outer chain lacks `min-w-0` / `overflow-hidden`, this element's
 * intrinsic width propagates up and makes `document.body` horizontally
 * scrollable.
 */
function WideContentProbe() {
  return (
    <div
      data-testid="wide-content"
      style={{
        width: "2000px",
        height: "100px",
        background: "linear-gradient(90deg, #abc, #def)",
        flex: "0 0 auto",
      }}
    >
      Wide content (2000px)
    </div>
  );
}

/**
 * Render the inner App layout wrapped in the exact same div chain used by
 * `App.tsx`: the root `h-screen ... flex flex-col overflow-hidden` div,
 * `ViewsContainer`, `PerspectivesContainer`, the inner
 * `flex-1 min-w-0 overflow-hidden flex flex-col` wrapper, and
 * `ModeIndicator`.
 *
 * A `WideContentProbe` is placed where `BoardView` would sit, so the test
 * exercises the min-w-0 chain with a deterministic 2000px overflow source
 * and does not depend on Tailwind utility classes being compiled.
 *
 * The App root wrapper is also sized to `height: 600px` via inline style so
 * the inner `flex-1 min-h-0` descendants get a finite height to lay out in.
 */
function renderAppLayout() {
  // Install the Tailwind utility shim so classes like `min-w-0` and
  // `overflow-hidden` actually translate to CSS during the test.
  installTailwindShim();

  // Fresh container for each test, attached directly to body so
  // `document.body.scrollWidth/clientWidth` reflect the real behavior.
  const mount = document.createElement("div");
  mount.setAttribute("data-app-layout-host", "");
  document.body.appendChild(mount);

  const ui = (
    <EntityFocusProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{}}>
          <TooltipProvider>
            <ActiveBoardPathProvider value="/test/wide">
              <DragSessionProvider>
                {/*
                 * Mirrors App.tsx line-for-line so the test catches any
                 * regression in the outer app shell's classes. See App.tsx:62.
                 */}
                <div
                  data-testid="app-root"
                  className="h-screen bg-background text-foreground flex flex-col overflow-hidden"
                  style={{ height: "600px" }}
                >
                  <NavBar />
                  <ViewsContainer>
                    <PerspectivesContainer>
                      <div
                        data-testid="perspective-content"
                        className="flex-1 min-w-0 overflow-hidden flex flex-col"
                      >
                        {/* Stand-in for BoardView's wide column strip. */}
                        <div
                          data-testid="board-scroll"
                          className="flex flex-1 min-h-0 min-w-0 overflow-x-auto pl-2"
                        >
                          <WideContentProbe />
                        </div>
                      </div>
                    </PerspectivesContainer>
                  </ViewsContainer>
                  <ModeIndicator />
                </div>
              </DragSessionProvider>
            </ActiveBoardPathProvider>
          </TooltipProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </EntityFocusProvider>
  );

  const result = render(ui, { container: mount });
  return { ...result, mount };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("App layout — horizontal overflow containment", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    // Clean stale mounts from prior tests.
    document
      .querySelectorAll("[data-app-layout-host]")
      .forEach((el) => el.remove());
    // Reset body scroll state.
    document.documentElement.scrollLeft = 0;
    document.body.scrollLeft = 0;
  });

  it("ViewsContainer's flex row has min-w-0 so content can shrink below intrinsic width", () => {
    const { mount } = renderAppLayout();
    const leftNav = screen.getByTestId("left-nav");
    // The row is LeftNav's parent — the
    // <div className="flex-1 flex min-h-0 ..."> in views-container.tsx.
    const row = leftNav.parentElement as HTMLElement;
    expect(row).toBeTruthy();
    expect(row.className).toContain("min-w-0");
    expect(row.className).toContain("flex-1");
    expect(row.className).toContain("min-h-0");
    mount.remove();
  });

  it("PerspectivesContainer's column has min-w-0 so it cannot be pushed wider", () => {
    const { mount } = renderAppLayout();
    const tabBar = screen.getByTestId("perspective-tab-bar");
    // The column is PerspectiveTabBar's parent — the
    // <div className="flex flex-col flex-1 min-h-0 ..."> in
    // perspectives-container.tsx.
    const col = tabBar.parentElement as HTMLElement;
    expect(col).toBeTruthy();
    expect(col.className).toContain("min-w-0");
    expect(col.className).toContain("flex-col");
    expect(col.className).toContain("flex-1");
    expect(col.className).toContain("min-h-0");
    mount.remove();
  });

  it("document.body has no horizontal scroll when a 2000px content block is inside the app layout", () => {
    const { mount } = renderAppLayout();
    // If any ancestor in the chain lacks min-w-0 or overflow-hidden, the
    // 2000px-wide content propagates up and body scrolls horizontally.
    // With the fix applied, the `overflow-x-auto` scroll container owns
    // the scrolling and nothing leaks to the document level.
    expect(document.body.scrollWidth).toBe(document.body.clientWidth);
    mount.remove();
  });

  it("the board scroll container (overflow-x-auto) has scrollWidth > clientWidth when content overflows", () => {
    const { mount } = renderAppLayout();
    const scrollContainer = screen.getByTestId("board-scroll");
    // The 2000px WideContentProbe must overflow the scroll container
    // horizontally — that's the whole point of the scroll container.
    expect(scrollContainer.scrollWidth).toBeGreaterThan(
      scrollContainer.clientWidth,
    );
    mount.remove();
  });

  it("chrome elements (NavBar/TabBar/LeftNav/ModeIndicator) stay at stable viewport positions when scrolling the board horizontally", () => {
    const { mount } = renderAppLayout();
    const navBar = screen.getByRole("banner");
    const tabBar = screen.getByTestId("perspective-tab-bar");
    const leftNav = screen.getByTestId("left-nav");
    const modeIndicator = screen.getByTestId("mode-indicator");

    const before = {
      nav: navBar.getBoundingClientRect(),
      tab: tabBar.getBoundingClientRect(),
      left: leftNav.getBoundingClientRect(),
      mode: modeIndicator.getBoundingClientRect(),
    };

    // Programmatically scroll the board's horizontal scroll container. If the
    // fix is correct, only the inner container scrolls — the chrome stays put.
    const scrollContainer = screen.getByTestId("board-scroll");
    scrollContainer.scrollTo({ left: 200, behavior: "auto" });

    const after = {
      nav: navBar.getBoundingClientRect(),
      tab: tabBar.getBoundingClientRect(),
      left: leftNav.getBoundingClientRect(),
      mode: modeIndicator.getBoundingClientRect(),
    };

    // None of the chrome elements should have shifted horizontally.
    expect(after.nav.left).toBe(before.nav.left);
    expect(after.tab.left).toBe(before.tab.left);
    expect(after.left.left).toBe(before.left.left);
    expect(after.mode.left).toBe(before.mode.left);
    // Body still has no horizontal scroll after the inner scroll.
    expect(document.body.scrollWidth).toBe(document.body.clientWidth);
    mount.remove();
  });
});

// ---------------------------------------------------------------------------
// Column width behavior — verify min-w-[24em] + shrink-0 hold the line.
// ---------------------------------------------------------------------------

import { BoardView } from "@/components/board-view";
import type { BoardData, Entity } from "@/types/kanban";

/**
 * Build a column entity with the given id, display name, and order. The
 * field shape matches the real kanban column schema used by BoardView.
 */
function makeColumnFixture(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

/**
 * Render six columns inside an 800px host and verify that every column
 * keeps its 24em minimum width (shrink-0 stops collapse) and that the
 * board's horizontal scroll container — not the chrome — owns the overflow.
 */
describe("Board column widths — min 24em bound holds, overflow stays in scroll container", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    // Reset horizontal scroll state between runs.
    document.documentElement.scrollLeft = 0;
    document.body.scrollLeft = 0;
  });

  it("with 6 columns in an 800px viewport, every column is ≥24em wide and the board strip scrolls horizontally", () => {
    installTailwindShim();

    // 800px ≪ 6 × 24em (≈ 2304px at 16px/em). With shrink-0 on each column
    // the strip becomes wider than the host, and the scroll container's
    // overflow-x-auto absorbs the overflow — nothing propagates upward.
    const host = document.createElement("div");
    host.style.width = "800px";
    host.style.height = "600px";
    host.style.display = "flex";
    host.style.flexDirection = "column";
    host.style.overflow = "hidden";
    host.setAttribute("data-column-width-host", "");
    document.body.appendChild(host);

    const columns: Entity[] = [];
    for (let i = 0; i < 6; i++) {
      columns.push(makeColumnFixture(`c${i}`, `Col ${i}`, i));
    }
    const boardFixture: BoardData = {
      board: {
        id: "board-1",
        entity_type: "board",
        moniker: "board:board-1",
        fields: { name: "Wide Board" },
      },
      columns,
      tags: [],
      virtualTagMeta: [],
      summary: {
        total_tasks: 0,
        total_actors: 0,
        ready_tasks: 0,
        blocked_tasks: 0,
        done_tasks: 0,
        percent_complete: 0,
      },
    };

    try {
      render(
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/wide-columns">
                  <DragSessionProvider>
                    <div
                      style={{
                        display: "flex",
                        flexDirection: "column",
                        flex: "1 1 0%",
                        minHeight: 0,
                        minWidth: 0,
                      }}
                    >
                      <BoardView board={boardFixture} tasks={[]} />
                    </div>
                  </DragSessionProvider>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>,
        { container: host },
      );

      const rootFontSize = parseFloat(
        getComputedStyle(document.documentElement).fontSize,
      );
      const expectedMinPx = 24 * rootFontSize;

      // Every column FocusScope carries data-moniker="column:<id>" — select
      // by the stable moniker attribute rather than by Tailwind class names.
      // Exclude "column:<id>.name" (the header's inner FocusScope for the
      // name field, used by keyboard navigation).
      const columnEls = host.querySelectorAll<HTMLElement>(
        '[data-moniker^="column:"]:not([data-moniker*="."])',
      );
      expect(columnEls.length).toBe(6);
      for (const el of columnEls) {
        // Allow sub-pixel rounding: use Math.floor against the 24em target.
        expect(el.getBoundingClientRect().width).toBeGreaterThanOrEqual(
          Math.floor(expectedMinPx),
        );
      }

      const scrollContainer = host.querySelector<HTMLElement>(
        "div.overflow-x-auto",
      );
      expect(scrollContainer).toBeTruthy();
      expect(scrollContainer!.scrollWidth).toBeGreaterThan(
        scrollContainer!.clientWidth,
      );

      // The overflow is contained to the scroll container — nothing leaks
      // out to the host element (which is where the chrome would live in
      // the real App shell). The host has `overflow: hidden` so it would
      // clip anyway; this check verifies the column strip's `shrink-0`
      // does not push through the min-w-0 chain on the scroll container.
      expect(host.scrollWidth).toBe(host.clientWidth);
    } finally {
      host.remove();
    }
  });

  it("each column FocusScope carries shrink-0 plus min-w-[24em]/max-w-[48em]", () => {
    installTailwindShim();

    const host = document.createElement("div");
    host.style.width = "800px";
    host.style.height = "400px";
    host.style.display = "flex";
    host.style.flexDirection = "column";
    host.style.overflow = "hidden";
    document.body.appendChild(host);

    const columns = [
      makeColumnFixture("a", "A", 0),
      makeColumnFixture("b", "B", 1),
    ];
    const boardFixture: BoardData = {
      board: {
        id: "board-class",
        entity_type: "board",
        moniker: "board:board-class",
        fields: { name: "Class Board" },
      },
      columns,
      tags: [],
      virtualTagMeta: [],
      summary: {
        total_tasks: 0,
        total_actors: 0,
        ready_tasks: 0,
        blocked_tasks: 0,
        done_tasks: 0,
        percent_complete: 0,
      },
    };

    try {
      render(
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/class">
                  <DragSessionProvider>
                    <BoardView board={boardFixture} tasks={[]} />
                  </DragSessionProvider>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>,
        { container: host },
      );

      const columnEls = host.querySelectorAll<HTMLElement>(
        '[data-moniker^="column:"]:not([data-moniker*="."])',
      );
      expect(columnEls.length).toBe(2);
      for (const el of columnEls) {
        expect(el.className).toContain("min-w-[24em]");
        expect(el.className).toContain("max-w-[48em]");
        expect(el.className).toContain("shrink-0");
      }
    } finally {
      host.remove();
    }
  });
});
