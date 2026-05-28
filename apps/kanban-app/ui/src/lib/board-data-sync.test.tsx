/**
 * Wiring test for the store/changed → BoardData patch pipeline.
 *
 * The bug this protects against: after a column reorder + undo, the
 * backend emits N `store/changed` notifications (one per column whose
 * `order` field reverted), but `BoardData.columns` in WindowContainer
 * stayed stale because no listener applied `patchBoardData` to it.
 * Result: the board view didn't redraw, and the columns stayed visually
 * in their post-drag order.
 *
 * This test mounts the `useBoardDataSync` hook, fires a mocked MCP
 * `notifications/store/changed` notification for a column, and asserts the
 * hook called `setBoard` with the patched data — i.e. the order field of the
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

interface StoreChangePayload {
  store: string;
  item: string;
  op?: "created" | "removed" | "updated";
  changes?: Array<{ field: string; value: unknown }>;
  txn?: string | null;
  origin?: string;
}

/** Fire one `notifications/store/changed` notification at the hook. */
async function fireStoreChanged(p: StoreChangePayload) {
  await fireEvent("notifications/store/changed", {
    op: "updated",
    txn: null,
    origin: "user",
    ...p,
  });
}

/** Fire a batch of same-`txn` `store/changed` notifications, flushed together. */
async function fireStoreChangedBatch(notes: StoreChangePayload[]) {
  const cbs = listeners.get("notifications/store/changed") ?? [];
  await act(async () => {
    for (const note of notes) {
      for (const cb of cbs) {
        cb({ payload: { op: "updated", origin: "user", ...note } });
      }
    }
    // Let the txn batcher's microtask flush.
    await Promise.resolve();
  });
}

/**
 * Spin microtasks until the lazy `subscribeStoreChanged` import chain has
 * registered its listener. The first test pays the cold dynamic-import cost
 * for `@tauri-apps/api/event`, which takes more than one microtask to settle;
 * without waiting, the fired event is dropped and the patch never applies.
 */
async function waitForSubscription() {
  // Poll up to ~2s. The first test pays the cold `import("@tauri-apps/api/event")`
  // cost (a real module fetch in browser mode), which can take many ms to
  // resolve before `subscribeStoreChanged` registers its listener; without
  // waiting, the fired event has no listener and the patch is dropped.
  for (let i = 0; i < 200; i++) {
    if ((listeners.get("notifications/store/changed")?.length ?? 0) > 0) return;
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
  }
}

describe("useBoardDataSync", () => {
  beforeEach(() => {
    listeners.clear();
    mockListen.mockClear();
    latestBoard = null;
  });

  it("patches a column's order field when a column store/changed fires", async () => {
    render(<Harness initial={makeBoard()} />);

    // Wait a microtask for the subscription promise to settle.
    await waitForSubscription();

    await fireStoreChanged({
      store: "column",
      item: "todo",
      changes: [{ field: "order", value: 99 }],
    });

    const todo = latestBoard!.columns.find((c) => c.id === "todo");
    expect(todo).toBeDefined();
    expect(todo!.fields.order).toBe(99);
  });

  it("patches the board entity when a board store/changed fires", async () => {
    render(<Harness initial={makeBoard()} />);

    await waitForSubscription();

    await fireStoreChanged({
      store: "board",
      item: "board",
      changes: [{ field: "name", value: "Renamed Board" }],
    });

    expect(latestBoard!.board.fields.name).toBe("Renamed Board");
  });

  it("does not touch BoardData for non-structural stores (task)", async () => {
    const initial = makeBoard();
    render(<Harness initial={initial} />);

    await act(async () => {
      await Promise.resolve();
    });

    const before = latestBoard;
    await fireStoreChanged({
      store: "task",
      item: "some-task",
      changes: [{ field: "title", value: "renamed" }],
    });

    // BoardData reference should be the same — the hook short-circuited
    // before calling setBoard for a non-structural store.
    expect(latestBoard).toBe(before);
  });

  it("ignores reload-item stores (perspective)", async () => {
    render(<Harness initial={makeBoard()} />);

    await act(async () => {
      await Promise.resolve();
    });

    const before = latestBoard;
    await fireStoreChanged({ store: "perspective", item: "p1" });

    expect(latestBoard).toBe(before);
  });

  it("applies multiple column-order changes from one undo txn as a single batch", async () => {
    render(<Harness initial={makeBoard()} />);

    await act(async () => {
      await Promise.resolve();
    });

    // Simulate the three store/changed notifications from undoing a column
    // drag that swapped todo (0→2), doing (1→0), done (2→1) — all sharing one
    // `txn`, so they flush as one atomic board patch.
    await fireStoreChangedBatch([
      { store: "column", item: "todo", changes: [{ field: "order", value: 0 }], txn: "t1" },
      { store: "column", item: "doing", changes: [{ field: "order", value: 1 }], txn: "t1" },
      { store: "column", item: "done", changes: [{ field: "order", value: 2 }], txn: "t1" },
    ]);

    const byId = (id: string) =>
      latestBoard!.columns.find((c) => c.id === id)!.fields.order;
    expect(byId("todo")).toBe(0);
    expect(byId("doing")).toBe(1);
    expect(byId("done")).toBe(2);
  });
});
