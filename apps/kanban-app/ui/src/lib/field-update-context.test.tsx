import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) =>
  Promise.resolve({ id: "t1", entity_type: "task", title: "Updated" }),
);

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
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
  function wrapper({ children }: { children: ReactNode }) {
    return <FieldUpdateProvider>{children}</FieldUpdateProvider>;
  }

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("calls dispatch_command with entity.update_field", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("task", "t1", "title", "New Title");
    });

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "entity.update_field",
      args: {
        entity_type: "task",
        id: "t1",
        field_name: "title",
        value: "New Title",
      },
      scopeChain: [],
    });
  });

  it("uses snake_case keys in args", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("task", "t1", "title", "X");
    });

    const payload = mockInvoke.mock.calls[0][1] as Record<string, unknown>;
    const args = payload.args as Record<string, unknown>;
    expect(args).toHaveProperty("entity_type");
    expect(args).toHaveProperty("field_name");
    expect(args).not.toHaveProperty("entityType");
    expect(args).not.toHaveProperty("fieldName");
  });

  it("re-throws when invoke fails", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("network error"));
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await expect(
        result.current.updateField("task", "t1", "title", "X"),
      ).rejects.toThrow("network error");
    });
  });

  it("passes correct entity type for tag updates", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField("tag", "tag-1", "color", "ff0000");
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "entity.update_field",
      args: {
        entity_type: "tag",
        id: "tag-1",
        field_name: "color",
        value: "ff0000",
      },
      scopeChain: [],
    });
  });

  it("passes correct entity type for column updates", async () => {
    const { result } = renderHook(() => useFieldUpdate(), { wrapper });

    await act(async () => {
      await result.current.updateField(
        "column",
        "col-1",
        "name",
        "In Progress",
      );
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "entity.update_field",
      args: {
        entity_type: "column",
        id: "col-1",
        field_name: "name",
        value: "In Progress",
      },
      scopeChain: [],
    });
  });
});
