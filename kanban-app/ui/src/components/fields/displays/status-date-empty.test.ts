import { describe, it, expect } from "vitest";
import { isStatusDateEmpty } from "./status-date-empty";

describe("isStatusDateEmpty", () => {
  it("treats null as empty", () => {
    expect(isStatusDateEmpty(null)).toBe(true);
  });

  it("treats undefined as empty", () => {
    expect(isStatusDateEmpty(undefined)).toBe(true);
  });

  it("treats non-object primitives as empty", () => {
    expect(isStatusDateEmpty(42)).toBe(true);
    expect(isStatusDateEmpty("2026-04-10T00:00:00Z")).toBe(true);
    expect(isStatusDateEmpty(true)).toBe(true);
  });

  it("treats an array as empty", () => {
    expect(
      isStatusDateEmpty([
        { kind: "created", timestamp: "2026-04-10T00:00:00Z" },
      ]),
    ).toBe(true);
  });

  it("treats an object missing kind as empty", () => {
    expect(isStatusDateEmpty({ timestamp: "2026-04-10T00:00:00Z" })).toBe(true);
  });

  it("treats an object missing timestamp as empty", () => {
    expect(isStatusDateEmpty({ kind: "created" })).toBe(true);
  });

  it("treats an object with non-string kind as empty", () => {
    expect(isStatusDateEmpty({ kind: 1, timestamp: "2026-04-10" })).toBe(true);
  });

  it("treats an object with non-string timestamp as empty", () => {
    expect(isStatusDateEmpty({ kind: "created", timestamp: 12345 })).toBe(true);
  });

  it("treats an unknown kind as empty", () => {
    expect(
      isStatusDateEmpty({ kind: "archived", timestamp: "2026-04-10T00:00:00Z" }),
    ).toBe(true);
    expect(
      isStatusDateEmpty({ kind: "updated", timestamp: "2026-04-10T00:00:00Z" }),
    ).toBe(true);
    expect(
      isStatusDateEmpty({ kind: "", timestamp: "2026-04-10T00:00:00Z" }),
    ).toBe(true);
  });

  it("treats an unparseable timestamp as empty", () => {
    expect(isStatusDateEmpty({ kind: "created", timestamp: "" })).toBe(true);
    expect(
      isStatusDateEmpty({ kind: "created", timestamp: "not-a-date" }),
    ).toBe(true);
    expect(isStatusDateEmpty({ kind: "created", timestamp: "2026-99-99" })).toBe(
      true,
    );
  });

  it("treats a well-formed RFC 3339 datetime as non-empty", () => {
    for (const kind of [
      "completed",
      "overdue",
      "started",
      "scheduled",
      "created",
    ]) {
      expect(
        isStatusDateEmpty({ kind, timestamp: "2026-04-10T00:00:00Z" }),
      ).toBe(false);
    }
  });

  it("treats a bare YYYY-MM-DD date as non-empty", () => {
    expect(isStatusDateEmpty({ kind: "created", timestamp: "2026-04-10" })).toBe(
      false,
    );
  });

  it("ignores extra fields on an otherwise valid payload", () => {
    expect(
      isStatusDateEmpty({
        kind: "completed",
        timestamp: "2026-04-10T00:00:00Z",
        extra: "ignored",
      }),
    ).toBe(false);
  });
});
