import { describe, it, expect } from "vitest";
import { arrayMove } from "@dnd-kit/sortable";
import { neighborIds } from "./neighbor-ids";

describe("neighborIds", () => {
  // -----------------------------------------------------------------------
  // Single-item list
  // -----------------------------------------------------------------------
  it("single item: both neighbors are null", () => {
    const result = neighborIds(["a"], 0, "a");
    expect(result).toEqual({ beforeId: null, afterId: null });
  });

  // -----------------------------------------------------------------------
  // Two-item swap cases
  // -----------------------------------------------------------------------
  it("two items, move index-0 to index-1: beforeId is the other item, afterId is null", () => {
    // ids after arrayMove: ["b", "a"]  →  a is now at index 1
    const result = neighborIds(["b", "a"], 1, "a");
    expect(result).toEqual({ beforeId: "b", afterId: null });
  });

  it("two items, move index-1 to index-0: beforeId is null, afterId is the other item", () => {
    // ids after arrayMove: ["b", "a"]  →  b is now at index 0
    const result = neighborIds(["b", "a"], 0, "b");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });

  // -----------------------------------------------------------------------
  // Three-item cases
  // -----------------------------------------------------------------------
  it("3 items, move first item to last position: beforeId is middle item, afterId is null", () => {
    // Original: [a, b, c] — move a from index 0 to index 2
    // After arrayMove: [b, c, a]  →  a is at index 2
    const result = neighborIds(["b", "c", "a"], 2, "a");
    expect(result).toEqual({ beforeId: "c", afterId: null });
  });

  it("3 items, move last item to first position: beforeId is null, afterId is original first item", () => {
    // Original: [a, b, c] — move c from index 2 to index 0
    // After arrayMove: [c, a, b]  →  c is at index 0
    const result = neighborIds(["c", "a", "b"], 0, "c");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });

  it("3 items, move first item to middle: beforeId and afterId both populated", () => {
    // Original: [a, b, c] — move a from index 0 to index 1
    // After arrayMove: [b, a, c]  →  a is at index 1
    const result = neighborIds(["b", "a", "c"], 1, "a");
    expect(result).toEqual({ beforeId: "b", afterId: "c" });
  });

  it("3 items, move last item to middle: beforeId and afterId both populated", () => {
    // Original: [a, b, c] — move c from index 2 to index 1
    // After arrayMove: [a, c, b]  →  c is at index 1
    const result = neighborIds(["a", "c", "b"], 1, "c");
    expect(result).toEqual({ beforeId: "a", afterId: "b" });
  });

  it("3 items, item already in middle: beforeId is first, afterId is last", () => {
    // No movement — item b at index 1, no-op but still a valid call
    const result = neighborIds(["a", "b", "c"], 1, "b");
    expect(result).toEqual({ beforeId: "a", afterId: "c" });
  });

  // -----------------------------------------------------------------------
  // Cross-column drop (selfId appears in the list once)
  // -----------------------------------------------------------------------
  it("item inserted at start of a foreign column's list: beforeId null, afterId is first existing", () => {
    // Dropping task-1 into column that had [task-2, task-3]; result list [task-1, task-2, task-3]
    const result = neighborIds(["task-1", "task-2", "task-3"], 0, "task-1");
    expect(result).toEqual({ beforeId: null, afterId: "task-2" });
  });

  it("item inserted at end of a foreign column's list: beforeId is last existing, afterId null", () => {
    // Dropping task-3 into column that had [task-1, task-2]; result list [task-1, task-2, task-3]
    const result = neighborIds(["task-1", "task-2", "task-3"], 2, "task-3");
    expect(result).toEqual({ beforeId: "task-2", afterId: null });
  });

  it("item inserted in the middle of a foreign column's list", () => {
    // Dropping task-2 into column that had [task-1, task-3]; result list [task-1, task-2, task-3]
    const result = neighborIds(["task-1", "task-2", "task-3"], 1, "task-2");
    expect(result).toEqual({ beforeId: "task-1", afterId: "task-3" });
  });

  // -----------------------------------------------------------------------
  // Regression: drag to first slot must produce beforeId=null, afterId=second task
  // This is the exact scenario that fails in the app
  // -----------------------------------------------------------------------
  it("5 items, drag last to first: beforeId null, afterId is the item that was first", () => {
    // Original: [a, b, c, d, e] — drag e to index 0
    // After arrayMove([a,b,c,d,e], 4, 0) → [e, a, b, c, d]
    const reordered = ["e", "a", "b", "c", "d"];
    const result = neighborIds(reordered, 0, "e");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });

  it("5 items, drag second-to-last to first: beforeId null, afterId is first", () => {
    // Original: [a, b, c, d, e] — drag d to index 0
    // After arrayMove([a,b,c,d,e], 3, 0) → [d, a, b, c, e]
    const reordered = ["d", "a", "b", "c", "e"];
    const result = neighborIds(reordered, 0, "d");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });

  it("4 items, drag index 1 to index 0: beforeId null, afterId is old first", () => {
    // Original: [a, b, c, d] — drag b to index 0
    // After arrayMove: [b, a, c, d]
    const reordered = ["b", "a", "c", "d"];
    const result = neighborIds(reordered, 0, "b");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });
});

// ---------------------------------------------------------------------------
// End-to-end drag simulation: arrayMove + neighborIds together
// This mirrors the exact logic in board-view.tsx handleDragEnd
// ---------------------------------------------------------------------------
describe("drag simulation (arrayMove + neighborIds)", () => {
  function simulateSameColumnDrag(
    ids: string[],
    activeId: string,
    overId: string,
  ): { beforeId: string | null; afterId: string | null } | "no-op" {
    const oldIndex = ids.indexOf(activeId);
    const newIndex = ids.indexOf(overId);
    if (oldIndex === -1 || newIndex === -1 || oldIndex === newIndex)
      return "no-op";
    const reordered = arrayMove(ids, oldIndex, newIndex);
    return neighborIds(reordered, newIndex, activeId);
  }

  it("drag last card onto first card → card moves to index 0", () => {
    const result = simulateSameColumnDrag(["a", "b", "c"], "c", "a");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });

  it("drag third card onto first card in 5-card list → card moves to index 0", () => {
    const result = simulateSameColumnDrag(["a", "b", "c", "d", "e"], "c", "a");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });

  it("drag second card onto first → moves to index 0", () => {
    const result = simulateSameColumnDrag(["a", "b", "c"], "b", "a");
    expect(result).toEqual({ beforeId: null, afterId: "a" });
  });

  it("drag first card onto second → moves to index 1", () => {
    const result = simulateSameColumnDrag(["a", "b", "c"], "a", "b");
    expect(result).toEqual({ beforeId: "b", afterId: "c" });
  });

  it("drag first card onto last → moves to end", () => {
    // arrayMove([a,b,c], 0, 2) → [b, c, a] — a is after c
    const result = simulateSameColumnDrag(["a", "b", "c"], "a", "c");
    expect(result).toEqual({ beforeId: "c", afterId: null });
  });

  it("drag onto self is no-op", () => {
    const result = simulateSameColumnDrag(["a", "b", "c"], "b", "b");
    expect(result).toBe("no-op");
  });
});
