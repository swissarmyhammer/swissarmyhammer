/**
 * Spatial-nav integration tests for `<ViewContainer>`.
 *
 * Mounts the container inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the conditional
 * `<ViewSpatialZone>` lights up its `<FocusZone moniker={asMoniker("ui:view")}>`
 * branch. The Tauri `invoke` boundary is mocked at the module level so we can
 * inspect the `spatial_register_zone` calls the zone makes on mount.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { ViewDef, BoardData, Entity } from "@/types/kanban";

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
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// ---------------------------------------------------------------------------
// Mock views-context — control the active view from each test.
// ---------------------------------------------------------------------------

const mockViews = vi.hoisted(() =>
  vi.fn(() => ({
    views: [] as ViewDef[],
    activeView: null as ViewDef | null,
    setActiveViewId: vi.fn(),
    refresh: vi.fn(),
  })),
);

vi.mock("@/lib/views-context", () => ({
  ViewsProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  useViews: () => mockViews(),
}));

// ---------------------------------------------------------------------------
// Mock window-container hooks so ViewContainer's data dependencies resolve.
// ---------------------------------------------------------------------------

const mockBoardData = vi.hoisted(() =>
  vi.fn<() => BoardData | null>(() => null),
);
const mockEntitiesByType = vi.hoisted(() =>
  vi.fn<() => Record<string, Entity[]>>(() => ({})),
);

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
}));

vi.mock("@/components/rust-engine-container", () => ({
  useEntitiesByType: () => mockEntitiesByType(),
}));

// Mock ui-state-context for transitive dependencies.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
}));

// Mock view components — we verify zone wrapping, not their internals.
vi.mock("@/components/grouped-board-view", () => ({
  GroupedBoardView: () => <div data-testid="board-view">BoardView</div>,
}));

vi.mock("@/components/grid-view", () => ({
  GridView: () => <div data-testid="grid-view">GridView</div>,
}));

// Imports come after mocks
import { ViewContainer } from "./view-container";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { asLayerName } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const BOARD_VIEW: ViewDef = {
  id: "board-default",
  name: "Board",
  kind: "board",
  icon: "kanban",
};

const MOCK_BOARD: BoardData = {
  board: {
    entity_type: "board",
    id: "b1",
    moniker: "board:b1",
    fields: { name: { String: "Test Board" } },
  },
  columns: [],
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Render ViewContainer wrapped in the production-shaped spatial-nav stack. */
function renderWithSpatialStack() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asLayerName("window")}>
        <EntityFocusProvider>
          <ViewContainer />
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `spatial_register_zone` call in the order they happened. */
function registerZoneCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ViewContainer (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockViews.mockReturnValue({
      views: [BOARD_VIEW],
      activeView: BOARD_VIEW,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    });
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockEntitiesByType.mockReturnValue({});
  });

  it("registers a ui:view zone when wrapped in SpatialFocusProvider + FocusLayer", async () => {
    const { unmount } = renderWithSpatialStack();
    await flushSetup();

    const calls = registerZoneCalls();
    const viewZone = calls.find((c) => c.moniker === "ui:view");
    expect(viewZone).toBeTruthy();
    expect(viewZone?.parentZone).toBeNull();
    expect(viewZone?.layerKey).toBeTruthy();

    unmount();
  });

  it("emits a wrapper element with data-moniker='ui:view'", async () => {
    const { container, unmount } = renderWithSpatialStack();
    await flushSetup();

    const node = container.querySelector("[data-moniker='ui:view']");
    expect(node).not.toBeNull();

    unmount();
  });

  it("preserves the flex chain className on the view zone wrapper", async () => {
    const { container, unmount } = renderWithSpatialStack();
    await flushSetup();

    const node = container.querySelector(
      "[data-moniker='ui:view']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    // The zone className must keep the BoardView / GridView chain alive.
    expect(node.className).toContain("flex-1");
    expect(node.className).toContain("flex");
    expect(node.className).toContain("flex-col");
    expect(node.className).toContain("min-h-0");
    expect(node.className).toContain("min-w-0");

    unmount();
  });

  it("does not wrap in FocusZone when no SpatialFocusProvider is present", () => {
    // Without the provider stack, the conditional zone must short-circuit and
    // render children directly so existing tests stay unaffected.
    const { container } = render(
      <EntityFocusProvider>
        <ViewContainer />
      </EntityFocusProvider>,
    );
    expect(container.querySelector("[data-moniker='ui:view']")).toBeNull();
  });
});
