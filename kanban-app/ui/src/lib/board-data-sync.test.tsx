/**
 * Wiring test for the entity-field-changed → BoardData patch pipeline.
 *
 * The bug this protects against: after a column reorder + undo, the
 * backend emits N `entity-field-changed` events (one per column whose
 * `order` field reverted), but `BoardData.columns` in WindowContainer
 * stayed stale because no listener applied `patchBoardData` to it.
 * Result: the board view didn't redraw, and the columns stayed visually
 * in their post-drag order.
 *
 * This test mounts the `useBoardDataSync` hook, fires a mocked Tauri
 * `entity-field-changed` event for a column, and asserts that the hook
 * called `setBoard` with the patched data — i.e. the order field of the
 * named column was updated.
 */
// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";

type ListenCallback = (event: { payload: unknown }) => void;

const { mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockListen, listeners };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { useBoardDataSync } from "./board-data-sync";
import type { BoardData, Entity } from "@/types/kanban";
import { useRef, useState } from "react";

function makeEntity(
  type: string,
  id: string,
  fields: Record<string, unknown> = {},
): Entity {
  return { entity_type: type, id, moniker: `${type}:${id}`, fields };
}

function makeBoard(): BoardData {
  return {
    board: makeEntity("board", "board", { name: "Test" }),
    columns: [
      makeEntity("column", "todo", { name: "To Do", order: 0 }),
      makeEntity("column", "doing", { name: "Doing", order: 1 }),
      makeEntity("column", "done", { name: "Done", order: 2 }),
    ],
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
}

let latestBoard: BoardData | null = null;
function Harness({
  initial,
  activeBoardPath,
}: {
  initial: BoardData;
  activeBoardPath?: string;
}) {
  const [board, setBoard] = useState<BoardData | null>(initial);
  const pathRef = useRef<string | undefined>(activeBoardPath);
  pathRef.current = activeBoardPath;
  useBoardDataSync(setBoard, pathRef);
  latestBoard = board;
  return null;
}

async function fireEvent(eventName: string, payload: unknown) {
  const cbs = listeners.get(eventName) ?? [];
  await act(async () => {
    for (const cb of cbs) cb({ payload });
  });
}

describe("useBoardDataSync", () => {
  beforeEach(() => {
    listeners.clear();
    mockListen.mockClear();
    latestBoard = null;
  });

  it("patches a column's order field when entity-field-changed fires", async () => {
    render(<Harness initial={makeBoard()} />);

    // Wait a microtask for the listener registration promise to settle.
    await act(async () => {
      await Promise.resolve();
    });

    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "column",
      id: "todo",
      changes: [{ field: "order", value: 99 }],
    });

    const todo = latestBoard!.columns.find((c) => c.id === "todo");
    expect(todo).toBeDefined();
    expect(todo!.fields.order).toBe(99);
  });

  it("patches the board entity when entity-field-changed fires for the board", async () => {
    render(<Harness initial={makeBoard()} />);

    await act(async () => {
      await Promise.resolve();
    });

    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "board",
      id: "board",
      changes: [{ field: "name", value: "Renamed Board" }],
    });

    expect(latestBoard!.board.fields.name).toBe("Renamed Board");
  });

  it("does not touch BoardData for non-structural entity types (task)", async () => {
    const initial = makeBoard();
    render(<Harness initial={initial} />);

    await act(async () => {
      await Promise.resolve();
    });

    const before = latestBoard;
    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "task",
      id: "some-task",
      changes: [{ field: "title", value: "renamed" }],
    });

    // BoardData reference should be the same — patchBoardData returned null
    // and the hook short-circuited.
    expect(latestBoard).toBe(before);
  });

  it("ignores events tagged with a different board_path", async () => {
    render(
      <Harness initial={makeBoard()} activeBoardPath="/Users/me/boardA" />,
    );

    await act(async () => {
      await Promise.resolve();
    });

    const before = latestBoard;
    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "column",
      id: "todo",
      changes: [{ field: "order", value: 99 }],
      board_path: "/Users/me/boardB",
    });

    // Reference unchanged — the patch was filtered out.
    expect(latestBoard).toBe(before);
  });

  it("applies events whose board_path matches the active board", async () => {
    render(
      <Harness initial={makeBoard()} activeBoardPath="/Users/me/boardA" />,
    );

    await act(async () => {
      await Promise.resolve();
    });

    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "column",
      id: "todo",
      changes: [{ field: "order", value: 99 }],
      board_path: "/Users/me/boardA",
    });

    const todo = latestBoard!.columns.find((c) => c.id === "todo");
    expect(todo!.fields.order).toBe(99);
  });

  it("applies multiple column-order events from one undo group", async () => {
    render(<Harness initial={makeBoard()} />);

    await act(async () => {
      await Promise.resolve();
    });

    // Simulate the three field-changed events from undoing a column drag
    // that swapped todo (0→2), doing (1→0), done (2→1) — i.e. revert each
    // column's `order` back to its pre-drag value.
    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "column",
      id: "todo",
      changes: [{ field: "order", value: 0 }],
    });
    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "column",
      id: "doing",
      changes: [{ field: "order", value: 1 }],
    });
    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "column",
      id: "done",
      changes: [{ field: "order", value: 2 }],
    });

    const byId = (id: string) =>
      latestBoard!.columns.find((c) => c.id === id)!.fields.order;
    expect(byId("todo")).toBe(0);
    expect(byId("doing")).toBe(1);
    expect(byId("done")).toBe(2);
  });
});
