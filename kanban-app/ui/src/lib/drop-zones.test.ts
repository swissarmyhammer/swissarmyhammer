import { describe, it, expect } from "vitest";
import { computeDropZones } from "./drop-zones";

describe("computeDropZones", () => {
  const boardPath = "/boards/test";
  const columnId = "col-1";

  // -----------------------------------------------------------------------
  // Empty column
  // -----------------------------------------------------------------------
  it("empty column returns a single zone with no before/after", () => {
    const zones = computeDropZones([], columnId, boardPath);
    expect(zones).toEqual([{ key: "empty", boardPath, columnId }]);
    // Verify no placement properties
    expect(zones[0]).not.toHaveProperty("beforeId");
    expect(zones[0]).not.toHaveProperty("afterId");
  });

  // -----------------------------------------------------------------------
  // Single task
  // -----------------------------------------------------------------------
  it("single task produces 2 zones (before + after)", () => {
    const zones = computeDropZones(["A"], columnId, boardPath);
    expect(zones).toHaveLength(2);
    expect(zones).toEqual([
      { key: "before-A", boardPath, columnId, beforeId: "A" },
      { key: "after-A", boardPath, columnId, afterId: "A" },
    ]);
  });

  // -----------------------------------------------------------------------
  // Three tasks — the canonical case
  // -----------------------------------------------------------------------
  it("3 tasks produce 4 zones with correct before/after IDs", () => {
    const zones = computeDropZones(["A", "B", "C"], columnId, boardPath);
    expect(zones).toHaveLength(4);
    expect(zones).toEqual([
      { key: "before-A", boardPath, columnId, beforeId: "A" },
      { key: "before-B", boardPath, columnId, beforeId: "B" },
      { key: "before-C", boardPath, columnId, beforeId: "C" },
      { key: "after-C", boardPath, columnId, afterId: "C" },
    ]);
  });

  // -----------------------------------------------------------------------
  // Two tasks
  // -----------------------------------------------------------------------
  it("2 tasks produce 3 zones", () => {
    const zones = computeDropZones(["X", "Y"], columnId, boardPath);
    expect(zones).toHaveLength(3);
    expect(zones[0]).toMatchObject({ key: "before-X", beforeId: "X" });
    expect(zones[1]).toMatchObject({ key: "before-Y", beforeId: "Y" });
    expect(zones[2]).toMatchObject({ key: "after-Y", afterId: "Y" });
  });

  // -----------------------------------------------------------------------
  // All zones carry boardPath and columnId
  // -----------------------------------------------------------------------
  it("all zones carry boardPath and columnId", () => {
    const zones = computeDropZones(["A", "B", "C"], columnId, boardPath);
    for (const zone of zones) {
      expect(zone.boardPath).toBe(boardPath);
      expect(zone.columnId).toBe(columnId);
    }
  });

  // -----------------------------------------------------------------------
  // Keys are unique
  // -----------------------------------------------------------------------
  it("all zone keys are unique", () => {
    const zones = computeDropZones(
      ["A", "B", "C", "D", "E"],
      columnId,
      boardPath,
    );
    const keys = zones.map((z) => z.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  // -----------------------------------------------------------------------
  // before/after are mutually exclusive on each descriptor
  // -----------------------------------------------------------------------
  it("each zone has at most one of beforeId or afterId", () => {
    const zones = computeDropZones(["A", "B", "C"], columnId, boardPath);
    for (const zone of zones) {
      const hasBefore = "beforeId" in zone;
      const hasAfter = "afterId" in zone;
      expect(hasBefore && hasAfter).toBe(false);
    }
  });
});
