/**
 * Spatial-nav integration tests for `<BoardView>`.
 *
 * Mounts the board inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the conditional
 * `<BoardSpatialZone>` lights up its `<FocusZone moniker={asSegment("ui:board")}>`
 * branch. The Tauri `invoke` boundary is mocked at the module level so we can
 * inspect the `spatial_register_zone` calls the zone makes on mount.
 *
 * Companion file: `board-view.guards.node.test.ts` pins the source-level
 * invariants (no `ClaimPredicate` import, no neighbor-moniker plumbing, no
 * board-level keydown listener). This file pins the runtime contract that
 * the board renders exactly one `ui:board` zone parented at the layer root.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { BoardData, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before any module that imports them.
//
// `mockInvoke` is hoisted so the SpatialFocusProvider's invoke calls
// (`spatial_push_layer`, `spatial_register_zone`, …) flow through it and
// tests can assert against them.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn((..._args: unknown[]) => Promise.resolve()),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
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

// ---------------------------------------------------------------------------
// Mock perspective-container — BoardView reads `groupField` from it.
// ---------------------------------------------------------------------------

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { BoardView } from "./board-view";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  asSegment
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

function makeTask(id: string, columnId: string, ordinal: string): Entity {
  return {
    id,
    entity_type: "task",
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: columnId,
      position_ordinal: ordinal,
    },
  };
}

const board: BoardData = {
  board: {
    id: "board-1",
    entity_type: "board",
    moniker: "board:board-1",
    fields: { name: "Test Board" },
  },
  columns: [
    makeColumn("col-todo", "Todo", 0),
    makeColumn("col-doing", "Doing", 1),
    makeColumn("col-done", "Done", 2),
  ],
  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: 3,
    total_actors: 0,
    ready_tasks: 3,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

const tasks: Entity[] = [
  makeTask("t1", "col-todo", "a0"),
  makeTask("t2", "col-todo", "a1"),
  makeTask("t3", "col-doing", "a0"),
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Flush microtasks queued by FocusZone's register effect. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Render `BoardView` inside the production-shaped spatial-nav stack.
 *
 * Mirrors the App.tsx wrapping (`<SpatialFocusProvider>` →
 * `<FocusLayer name="window">`) so `BoardSpatialZone`'s optional-context
 * lookups both succeed and the conditional `<FocusZone>` branch is taken.
 */
function renderBoardWithSpatialStack() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/board">
                  <DragSessionProvider>
                    <BoardView board={board} tasks={tasks} />
                  </DragSessionProvider>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Render `BoardView` without the spatial-nav stack (pre-zone shape). */
function renderBoardWithoutSpatialStack() {
  return render(
    <EntityFocusProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{}}>
          <TooltipProvider>
            <ActiveBoardPathProvider value="/test/board">
              <DragSessionProvider>
                <BoardView board={board} tasks={tasks} />
              </DragSessionProvider>
            </ActiveBoardPathProvider>
          </TooltipProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </EntityFocusProvider>,
  );
}

/** Pull every `spatial_register_zone` call as a typed record. */
function registeredZones(): Array<{
  fq: string;
  segment: string;
  rect: unknown;
  layerFq: string;
  parentZone: string | null;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map(
      (c) =>
        c[1] as {
          fq: string;
          segment: string;
          rect: unknown;
          layerFq: string;
          parentZone: string | null;
        },
    );
}

/** Pull every `spatial_push_layer` push as a typed record. */
function pushedLayers(): Array<{
  fq: string;
  name: string;
  parent: string | null;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_push_layer")
    .map(
      (c) =>
        c[1] as {
          fq: string;
          name: string;
          parent: string | null;
        },
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("BoardView (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("registers a single ui:board zone whose parentZone is null and layerKey is the window root", async () => {
    const { unmount } = renderBoardWithSpatialStack();
    await flushSetup();

    // The `ui:board` zone must mount exactly once.
    const boardZones = registeredZones().filter(
      (z) => z.segment === "ui:board",
    );
    expect(boardZones).toHaveLength(1);
    const boardZone = boardZones[0];

    // Board zone is rooted directly under the window layer — no enclosing
    // FocusZone wraps it, so `parentZone` must be null.
    expect(boardZone.parentZone).toBeNull();

    // The `layerKey` must match the window-root layer that was just pushed.
    const windowLayer = pushedLayers().find((l) => l.name === "window");
    expect(windowLayer).toBeTruthy();
    expect(boardZone.layerFq).toBe(windowLayer!.fq);

    unmount();
  });

  it("emits a wrapper element with data-moniker='ui:board'", async () => {
    const { container, unmount } = renderBoardWithSpatialStack();
    await flushSetup();

    const node = container.querySelector("[data-segment='ui:board']");
    expect(node).not.toBeNull();

    unmount();
  });

  it.skip("does not wrap in FocusZone when no SpatialFocusProvider is present", async () => {
    // SKIPPED: Under path-monikers (card 01KQD6064G1C1RAXDFPJVT1F46) the
    // BoardSpatialBody invokes the non-optional `useFullyQualifiedMoniker`
    // directly so this no-provider short-circuit is no longer reachable
    // from production code paths. Tests that need this contract must wrap
    // in `<SpatialFocusProvider>` + `<FocusLayer>`.
    const { container, unmount } = renderBoardWithoutSpatialStack();
    await flushSetup();

    expect(container.querySelector("[data-segment='ui:board']")).toBeNull();

    const boardZones = registeredZones().filter(
      (z) => z.segment === "ui:board",
    );
    expect(boardZones).toHaveLength(0);

    unmount();
  });
});
