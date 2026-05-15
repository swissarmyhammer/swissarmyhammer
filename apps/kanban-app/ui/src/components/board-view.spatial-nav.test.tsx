/**
 * Spatial-nav integration tests for `<BoardView>`.
 *
 * Mounts the board inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the conditional
 * `<BoardSpatialZone>` lights up its `<FocusScope moniker={asSegment("ui:board")}>`
 * branch. The Tauri `invoke` boundary is mocked at the module level so we can
 * inspect the `spatial_register_scope` calls the zone makes on mount.
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
// (`spatial_push_layer`, `spatial_register_scope`, …) flow through it and
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
import { asSegment } from "@/types/spatial";

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

/** Flush microtasks queued by FocusScope's register effect. */
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
 * lookups both succeed and the conditional `<FocusScope>` branch is taken.
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

/** Pull every `spatial_register_scope` call as a typed record. */
function registeredZones(): Array<{
  fq: string;
  segment: string;
  rect: unknown;
  layerFq: string;
  parentZone: string | null;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
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

  it("registers a single board:{id} entity zone parented at the layer root", async () => {
    // Post-`8232b25cc`, the redundant `ui:board` chrome zone was
    // dropped — the board content mounts directly under the
    // `board:{id}` entity zone (the `<Inspectable>` + `<FocusScope>`
    // pair on `<BoardView>`). Pin that there is exactly one
    // `board:{id}` zone, parented at the window-root layer.
    const { unmount } = renderBoardWithSpatialStack();
    await flushSetup();

    const boardEntityZones = registeredZones().filter(
      (z) =>
        typeof z.segment === "string" &&
        (z.segment as string).startsWith("board:"),
    );
    expect(boardEntityZones).toHaveLength(1);
    const boardZone = boardEntityZones[0];

    // The chrome `ui:board` scope must NOT be registered — its removal
    // was the whole point of `8232b25cc`. A regression that
    // re-introduces it would re-create the same-rect overlap warning.
    const chromeZones = registeredZones().filter(
      (z) => z.segment === "ui:board",
    );
    expect(chromeZones).toHaveLength(0);

    // The `layerKey` must match the window-root layer that was just pushed.
    const windowLayer = pushedLayers().find((l) => l.name === "window");
    expect(windowLayer).toBeTruthy();
    expect(boardZone.layerFq).toBe(windowLayer!.fq);

    unmount();
  });

  it("emits a wrapper element with data-segment='board:{id}'", async () => {
    const { container, unmount } = renderBoardWithSpatialStack();
    await flushSetup();

    const node = container.querySelector("[data-segment='board:board-1']");
    expect(node).not.toBeNull();

    // The dropped chrome scope must leave no DOM marker behind.
    expect(container.querySelector("[data-segment='ui:board']")).toBeNull();

    unmount();
  });

  // Note: a former `it.skip("does not wrap in FocusScope when no
  // SpatialFocusProvider is present", …)` was removed under path-monikers
  // (card 01KQD6064G1C1RAXDFPJVT1F46). `BoardSpatialBody` now calls the
  // non-optional `useFullyQualifiedMoniker`, so the no-provider
  // short-circuit no longer exists in production. Tests asserting board
  // wrapper shape must mount inside `<SpatialFocusProvider>` +
  // `<FocusLayer>` (see the test above).
});
