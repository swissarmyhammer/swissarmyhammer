import { describe, it, expect } from "vitest";
import { computeDropZones } from "./drop-zones";

describe("computeDropZones", () => {
  const columnId = "col-1";

  // -----------------------------------------------------------------------
  // Empty column
  // -----------------------------------------------------------------------
  it("empty column returns a single zone with no before/after", () => {
    const zones = computeDropZones([], columnId);
    expect(zones).toEqual([{ key: "empty", columnId }]);
    // Verify no placement properties
    expect(zones[0]).not.toHaveProperty("beforeId");
    expect(zones[0]).not.toHaveProperty("afterId");
  });

  // -----------------------------------------------------------------------
  // Single task
  // -----------------------------------------------------------------------
  it("single task produces 2 zones (before + after)", () => {
    const zones = computeDropZones(["A"], columnId);
    expect(zones).toHaveLength(2);
    expect(zones).toEqual([
      { key: "before-A", columnId, beforeId: "A" },
      { key: "after-A", columnId, afterId: "A" },
    ]);
  });

  // -----------------------------------------------------------------------
  // Three tasks — the canonical case
  // -----------------------------------------------------------------------
  it("3 tasks produce 4 zones with correct before/after IDs", () => {
    const zones = computeDropZones(["A", "B", "C"], columnId);
    expect(zones).toHaveLength(4);
    expect(zones).toEqual([
      { key: "before-A", columnId, beforeId: "A" },
      { key: "before-B", columnId, beforeId: "B" },
      { key: "before-C", columnId, beforeId: "C" },
      { key: "after-C", columnId, afterId: "C" },
    ]);
  });

  // -----------------------------------------------------------------------
  // Two tasks
  // -----------------------------------------------------------------------
  it("2 tasks produce 3 zones", () => {
    const zones = computeDropZones(["X", "Y"], columnId);
    expect(zones).toHaveLength(3);
    expect(zones[0]).toMatchObject({ key: "before-X", beforeId: "X" });
    expect(zones[1]).toMatchObject({ key: "before-Y", beforeId: "Y" });
    expect(zones[2]).toMatchObject({ key: "after-Y", afterId: "Y" });
  });

  // -----------------------------------------------------------------------
  // All zones carry columnId (no boardPath)
  // -----------------------------------------------------------------------
  it("all zones carry columnId", () => {
    const zones = computeDropZones(["A", "B", "C"], columnId);
    for (const zone of zones) {
      expect(zone.columnId).toBe(columnId);
    }
  });

  // -----------------------------------------------------------------------
  // Keys are unique
  // -----------------------------------------------------------------------
  it("all zone keys are unique", () => {
    const zones = computeDropZones(["A", "B", "C", "D", "E"], columnId);
    const keys = zones.map((z) => z.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  // -----------------------------------------------------------------------
  // before/after are mutually exclusive on each descriptor
  // -----------------------------------------------------------------------
  it("each zone has at most one of beforeId or afterId", () => {
    const zones = computeDropZones(["A", "B", "C"], columnId);
    for (const zone of zones) {
      const hasBefore = "beforeId" in zone;
      const hasAfter = "afterId" in zone;
      expect(hasBefore && hasAfter).toBe(false);
    }
  });
});
