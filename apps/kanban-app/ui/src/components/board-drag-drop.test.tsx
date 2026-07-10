/**
 * End-to-end drag reorder tests.
 *
 * These tests verify the pure-function pipeline that drives drag-and-drop
 * reordering: computeDropZones produces the right descriptors, and those
 * descriptors carry the correct before_id / after_id for the backend
 * move-task command.
 *
 * Full React rendering of BoardView is intentionally avoided — it requires
 * Tauri providers, entity stores, and other infrastructure. Instead we test
 * the data-flow layer that BoardView delegates to.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { computeDropZones, type DropZoneDescriptor } from "@/lib/drop-zones";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/mcp-transport", async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  callCommandTool: vi.fn().mockResolvedValue({ ok: true }),
  subscribeCommandsChanged: vi.fn().mockResolvedValue(() => {}),
}));

import { usePersistTaskMove } from "@/components/board-view";
import { callCommandTool } from "@/lib/mcp-transport";

const mockCallCommandTool = vi.mocked(callCommandTool);

// ---------------------------------------------------------------------------
// computeDropZones — zone count and descriptor shape
// ---------------------------------------------------------------------------

describe("computeDropZones", () => {
  it("empty column produces a single zone with no anchors", () => {
    const zones = computeDropZones([], "doing");
    expect(zones).toHaveLength(1);
    expect(zones[0]).toEqual({
      key: "empty",
      columnId: "doing",
    });
    // No beforeId or afterId — backend interprets this as "append"
    expect(zones[0].beforeId).toBeUndefined();
    expect(zones[0].afterId).toBeUndefined();
  });

  it("single card produces 2 zones: before-A and after-A", () => {
    const zones = computeDropZones(["A"], "todo");
    expect(zones).toHaveLength(2);
    expect(zones[0]).toMatchObject({ key: "before-A", beforeId: "A" });
    expect(zones[1]).toMatchObject({ key: "after-A", afterId: "A" });
  });

  it("N cards produce N+1 zones", () => {
    const zones = computeDropZones(["A", "B", "C"], "todo");
    expect(zones).toHaveLength(4);
  });

  it("all zones carry the column ID", () => {
    const zones = computeDropZones(["A", "B"], "doing");
    for (const z of zones) {
      expect(z.columnId).toBe("doing");
    }
  });
});

// ---------------------------------------------------------------------------
// Scenario: move 3rd card to 2nd position
// ---------------------------------------------------------------------------

describe("move 3rd card to 2nd position", () => {
  // Board has [A, B, C] in "todo". User drags C and drops on zone before-B.
  const zones = computeDropZones(["A", "B", "C"], "todo");

  it("zone at index 1 targets before-B", () => {
    const zone = zones[1];
    expect(zone.key).toBe("before-B");
    expect(zone.beforeId).toBe("B");
    expect(zone.afterId).toBeUndefined();
  });

  it("dropping C on zone before-B produces move args { before_id: B }", () => {
    // Simulate what the UI does: pick the zone descriptor, build move args
    const zone = zones[1];
    const moveArgs = buildMoveArgs("C", zone);
    expect(moveArgs).toEqual({
      op: "move task",
      id: "C",
      column: "todo",
      before_id: "B",
    });
  });
});

// ---------------------------------------------------------------------------
// Scenario: move 1st card to last position
// ---------------------------------------------------------------------------

describe("move 1st card to last position", () => {
  const zones = computeDropZones(["A", "B", "C"], "todo");

  it("zone at index 3 (after-C) is the trailing zone", () => {
    const zone = zones[3];
    expect(zone.key).toBe("after-C");
    expect(zone.afterId).toBe("C");
    expect(zone.beforeId).toBeUndefined();
  });

  it("dropping A on zone after-C produces move args { after_id: C }", () => {
    const zone = zones[3];
    const moveArgs = buildMoveArgs("A", zone);
    expect(moveArgs).toEqual({
      op: "move task",
      id: "A",
      column: "todo",
      after_id: "C",
    });
  });
});

// ---------------------------------------------------------------------------
// Scenario: move card to a different (empty) column
// ---------------------------------------------------------------------------

describe("move card to empty column", () => {
  const zones = computeDropZones([], "doing");

  it("empty column zone produces move args with column only", () => {
    const zone = zones[0];
    const moveArgs = buildMoveArgs("A", zone);
    expect(moveArgs).toEqual({
      op: "move task",
      id: "A",
      column: "doing",
    });
  });
});

// ---------------------------------------------------------------------------
// Scenario: cross-column move to non-empty column
// ---------------------------------------------------------------------------

describe("cross-column move to non-empty column", () => {
  // "doing" column already has [X, Y]. User drops A before X.
  const zones = computeDropZones(["X", "Y"], "doing");

  it("dropping on zone before-X includes target column and before_id", () => {
    const zone = zones[0];
    const moveArgs = buildMoveArgs("A", zone);
    expect(moveArgs).toEqual({
      op: "move task",
      id: "A",
      column: "doing",
      before_id: "X",
    });
  });
});

// ---------------------------------------------------------------------------
// usePersistTaskMove — the REAL drop-dispatch wire shape
// ---------------------------------------------------------------------------

// These tests pin what the actual drop hook sends over the wire — not a local
// mirror of it. The `task-commands` plugin's `task.move` accepts exactly this
// shape (`target: "task:<id>"`, args `{ id, column, before_id | after_id }`);
// the backend half is pinned by
// `swissarmyhammer-command-service/tests/integration/builtin_task_commands_e2e.rs`
// (`task_move_with_drop_dispatch_shape_*`). The two layers drifting apart is
// the bug that silently broke every internal drag drop.
describe("usePersistTaskMove dispatch wire shape", () => {
  beforeEach(() => {
    mockCallCommandTool.mockClear();
    mockCallCommandTool.mockResolvedValue({ ok: true });
  });

  it("same-column reorder dispatches task.move with before_id and target task:<id>", async () => {
    const { result } = renderHook(() => usePersistTaskMove());
    // Board has [A, B, C] in "todo"; C is dropped on the before-B zone.
    const zones = computeDropZones(["A", "B", "C"], "todo");
    await act(async () => {
      await result.current(zones[1], "C");
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith(
      "execute command",
      expect.objectContaining({
        id: "task.move",
        ctx: expect.objectContaining({
          target: "task:C",
          args: { id: "C", column: "todo", before_id: "B" },
        }),
      }),
    );
  });

  it("cross-column drop dispatches task.move with the target column and after_id", async () => {
    const { result } = renderHook(() => usePersistTaskMove());
    // "doing" has [X, Y]; the dragged task A is dropped on the trailing
    // after-Y zone.
    const zones = computeDropZones(["X", "Y"], "doing");
    await act(async () => {
      await result.current(zones[2], "A");
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith(
      "execute command",
      expect.objectContaining({
        id: "task.move",
        ctx: expect.objectContaining({
          target: "task:A",
          args: { id: "A", column: "doing", after_id: "Y" },
        }),
      }),
    );
  });

  it("empty-column drop dispatches task.move with column only (append)", async () => {
    const { result } = renderHook(() => usePersistTaskMove());
    const zones = computeDropZones([], "doing");
    await act(async () => {
      await result.current(zones[0], "A");
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith(
      "execute command",
      expect.objectContaining({
        id: "task.move",
        ctx: expect.objectContaining({
          target: "task:A",
          args: { id: "A", column: "doing" },
        }),
      }),
    );
  });
});

// ---------------------------------------------------------------------------
// Helper: build the move-task command args from a zone descriptor
// ---------------------------------------------------------------------------

/**
 * Build the JSON args that would be sent to the Rust backend's move-task
 * dispatch, given the dragged task ID and the drop-zone descriptor.
 *
 * This mirrors the logic the UI uses when converting a drop event into
 * a backend command.
 *
 * @param taskId - The ID of the task being dragged.
 * @param zone - The drop-zone descriptor where the task was dropped.
 * @returns The move-task command args.
 */
function buildMoveArgs(
  taskId: string,
  zone: DropZoneDescriptor,
): Record<string, string> {
  const args: Record<string, string> = {
    op: "move task",
    id: taskId,
    column: zone.columnId,
  };
  if (zone.beforeId) {
    args.before_id = zone.beforeId;
  }
  if (zone.afterId) {
    args.after_id = zone.afterId;
  }
  return args;
}
