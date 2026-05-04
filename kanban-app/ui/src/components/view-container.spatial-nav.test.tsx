/**
 * Spatial-nav regression tests for `<ViewContainer>`.
 *
 * The `ui:view` `<FocusScope>` wrapper was deleted because its rect exactly
 * overlapped the inner view's own zone (`ui:board` for `<BoardView>`,
 * `ui:grid` for `<GridView>`). It added no semantic value to the spatial
 * graph and just inserted an extra hop the cascade had to traverse. These
 * tests pin its absence so a future refactor cannot reintroduce the
 * redundant wrapper.
 *
 * Mounts the container inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`). The Tauri
 * `invoke` boundary is mocked at the module level so we can inspect every
 * `spatial_register_scope` call ViewContainer's subtree makes on mount.
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
import {
  asSegment
} from "@/types/spatial";

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
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <ViewContainer />
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `spatial_register_scope` call in the order they happened. */
function registerScopeCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
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

  it("does NOT register a ui:view zone when wrapped in SpatialFocusProvider + FocusLayer", async () => {
    const { unmount } = renderWithSpatialStack();
    await flushSetup();

    // The redundant `ui:view` chrome zone was deleted — it overlapped the
    // inner view's own zone (`ui:board` / `ui:grid`) for the same rect.
    // Nothing in `view-container.tsx`'s subtree (the bare ViewContainer
    // itself, not the inner view bodies that are mocked here) should call
    // `spatial_register_scope` with `segment === "ui:view"`.
    const calls = registerScopeCalls();
    const viewZone = calls.find((c) => c.segment === "ui:view");
    expect(viewZone).toBeUndefined();

    unmount();
  });

  it("does NOT emit a wrapper element with data-segment='ui:view'", async () => {
    const { container, unmount } = renderWithSpatialStack();
    await flushSetup();

    // The wrapper DOM element is gone too — no `[data-segment='ui:view']`
    // node should exist anywhere in the rendered tree.
    const node = container.querySelector("[data-segment='ui:view']");
    expect(node).toBeNull();

    unmount();
  });

  it("does NOT emit a ui:view wrapper when no SpatialFocusProvider is present", () => {
    // Without the provider stack, the ViewContainer renders bare children.
    // The absence assertion holds in this configuration too — the zone is
    // gone for good, not just conditional on the providers.
    const { container } = render(
      <EntityFocusProvider>
        <ViewContainer />
      </EntityFocusProvider>,
    );
    expect(container.querySelector("[data-segment='ui:view']")).toBeNull();
  });
});
