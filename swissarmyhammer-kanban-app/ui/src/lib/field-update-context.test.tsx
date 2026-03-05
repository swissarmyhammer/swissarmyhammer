import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve({ id: "t1", entity_type: "task", title: "Updated" }));

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { FieldUpdateProvider, useFieldUpdate } from "./field-update-context";

describe("FieldUpdateProvider", () => {
  const mockRefresh = vi.fn(() => Promise.resolve());

  function wrapper({ children }: { children: ReactNode }) {
    return (
      <FieldUpdateProvider onRefresh={mockRefresh}>
        {children}
      </FieldUpdateProvider>
    );
  }

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("calls invoke with camelCase params (entityType, fieldName)", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("task", "t1", "title", "New Title");
    });

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith("update_entity_field", {
      entityType: "task",
      id: "t1",
      fieldName: "title",
      value: "New Title",
    });
  });

  it("does NOT use snake_case params", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("task", "t1", "title", "X");
    });

    const args = mockInvoke.mock.calls[0][1] as Record<string, unknown>;
    expect(args).not.toHaveProperty("entity_type");
    expect(args).not.toHaveProperty("field_name");
    expect(args).toHaveProperty("entityType");
    expect(args).toHaveProperty("fieldName");
  });

  it("calls onRefresh after successful update", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("task", "t1", "body", "New body");
    });

    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("does NOT call onRefresh when invoke fails", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("network error"));
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await expect(
        result.current.updateField("task", "t1", "title", "X"),
      ).rejects.toThrow("network error");
    });

    expect(mockRefresh).not.toHaveBeenCalled();
  });

  it("passes correct entity type for tag updates", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("tag", "tag-1", "color", "ff0000");
    });

    expect(mockInvoke).toHaveBeenCalledWith("update_entity_field", {
      entityType: "tag",
      id: "tag-1",
      fieldName: "color",
      value: "ff0000",
    });
  });

  it("passes correct entity type for column updates", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("column", "col-1", "name", "In Progress");
    });

    expect(mockInvoke).toHaveBeenCalledWith("update_entity_field", {
      entityType: "column",
      id: "col-1",
      fieldName: "name",
      value: "In Progress",
    });
  });
});
