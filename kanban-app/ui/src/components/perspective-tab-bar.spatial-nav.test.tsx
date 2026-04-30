/**
 * Spatial-nav integration tests for `<PerspectiveTabBar>`.
 *
 * Mounts the tab bar inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the conditional
 * spatial-nav branches light up:
 *   - the tab-bar root becomes a `<FocusZone moniker={asSegment("ui:perspective-bar")}>`
 *   - each tab becomes a `<FocusScope moniker={asSegment(`perspective_tab:${id}`)}>` leaf
 *
 * The Tauri `invoke` boundary is mocked so we can inspect the
 * `spatial_register_zone` and `spatial_register_scope` calls each
 * primitive makes on mount.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Tauri API mocks — must be set before any module that imports them.
// ---------------------------------------------------------------------------

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Perspective + view + UI mocks — match the shape used by the existing
// perspective-tab-bar.test.tsx so the tab bar renders without surprise.
// ---------------------------------------------------------------------------

/** Mutable mock perspective shape — the test toggles which tabs are present. */
type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
}));

let mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  }),
  useUIStateLoading: () => ({
    state: {
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    },
    loading: false,
  }),
}));

// Imports come after mocks
import { PerspectiveTabBar } from "./perspective-tab-bar";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  asSegment
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Render the tab bar wrapped in the production-shaped spatial-nav stack. */
function renderWithSpatialStack() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider delayDuration={100}>
          <PerspectiveTabBar />
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `spatial_register_zone` call in order. */
function registerZoneCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_register_scope` call in order. */
function registerScopeCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveTabBar (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockPerspectivesValue = {
      perspectives: [],
      activePerspective: null,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
    mockViewsValue = {
      views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
      activeView: {
        id: "board-1",
        name: "Board",
        kind: "board",
        icon: "kanban",
      },
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  it("registers a ui:perspective-bar zone on mount", async () => {
    const { unmount } = renderWithSpatialStack();
    await flushSetup();

    const calls = registerZoneCalls();
    const barZone = calls.find((c) => c.segment === "ui:perspective-bar");
    expect(barZone).toBeTruthy();
    expect(barZone?.parentZone).toBeNull();
    expect(barZone?.layerFq).toBeTruthy();

    unmount();
  });

  it("emits a wrapper element with data-moniker='ui:perspective-bar'", async () => {
    const { container, unmount } = renderWithSpatialStack();
    await flushSetup();

    const node = container.querySelector("[data-moniker='ui:perspective-bar']");
    expect(node).not.toBeNull();

    unmount();
  });

  it("preserves the tab-bar layout class on the zone wrapper", async () => {
    const { container, unmount } = renderWithSpatialStack();
    await flushSetup();

    const node = container.querySelector(
      "[data-moniker='ui:perspective-bar']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    // The h-8 shrink-0 chain keeps the tab bar a fixed-height row.
    expect(node.className).toContain("flex");
    expect(node.className).toContain("items-center");
    expect(node.className).toContain("h-8");
    expect(node.className).toContain("shrink-0");

    unmount();
  });

  it("registers a perspective_tab:{id} focusable per tab", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        { id: "p2", name: "Backlog", view: "board" },
        { id: "p3", name: "Grid Thing", view: "grid" }, // filtered out
      ],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
    };

    const { unmount } = renderWithSpatialStack();
    await flushSetup();

    const calls = registerScopeCalls();
    const monikers = calls.map((c) => c.moniker as string);
    expect(monikers).toContain("perspective_tab:p1");
    expect(monikers).toContain("perspective_tab:p2");
    // Grid perspective is filtered by view kind and must not produce a tab leaf.
    expect(monikers).not.toContain("perspective_tab:p3");

    unmount();
  });

  it("each tab focusable's parentZone is the ui:perspective-bar zone key", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
    };

    const { unmount } = renderWithSpatialStack();
    await flushSetup();

    const barZone = registerZoneCalls().find(
      (c) => c.segment === "ui:perspective-bar",
    )!;
    const tabFocusable = registerScopeCalls().find(
      (c) => c.segment === "perspective_tab:p1",
    )!;
    expect(tabFocusable.parentZone).toBe(barZone.key);
    expect(tabFocusable.layerFq).toBe(barZone.layerFq);

    unmount();
  });

  it("emits data-moniker='perspective_tab:{id}' on each tab focusable", async () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board" },
        { id: "p2", name: "Backlog", view: "board" },
      ],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
    };

    const { container, unmount } = renderWithSpatialStack();
    await flushSetup();

    expect(
      container.querySelector("[data-moniker='perspective_tab:p1']"),
    ).not.toBeNull();
    expect(
      container.querySelector("[data-moniker='perspective_tab:p2']"),
    ).not.toBeNull();

    unmount();
  });

  it("does not wrap in FocusZone when no SpatialFocusProvider is present", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint", view: "board" },
    };

    // Without the provider stack, the conditional zone falls back to a plain
    // div; there must be no `data-moniker` on the bar or tabs.
    const { container } = render(
      <TooltipProvider delayDuration={100}>
        <PerspectiveTabBar />
      </TooltipProvider>,
    );
    expect(
      container.querySelector("[data-moniker='ui:perspective-bar']"),
    ).toBeNull();
    expect(
      container.querySelector("[data-moniker='perspective_tab:p1']"),
    ).toBeNull();
  });
});
