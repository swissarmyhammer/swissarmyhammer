import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { resolveCommandName, useEntityCommands } from "./entity-commands";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mocks for Tauri + providers
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    commands: [
      {
        id: "entity.inspect",
        name: "Inspect {{entity.type}}",
        context_menu: true,
      },
      {
        id: "entity.archive",
        name: "Archive {{entity.type}}",
        context_menu: true,
      },
    ],
  },
  fields: [],
};

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: vi.fn((...args: any[]) => {
    if (args[0] === "list_entity_types") return Promise.resolve(["task"]);
    if (args[0] === "get_entity_schema") return Promise.resolve(TASK_SCHEMA);
    return Promise.resolve(null);
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Import providers after mocks are set up
import { SchemaProvider } from "@/lib/schema-context";
import { InspectProvider } from "@/lib/inspect-context";

/** Render a hook inside SchemaProvider + InspectProvider. */
function makeWrapper(onInspect = vi.fn()) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(
      SchemaProvider,
      null,
      createElement(InspectProvider, {
        onInspect,
        onDismiss: () => false,
        children,
      }),
    );
  };
}

const makeEntity = (fields: Record<string, unknown>): Entity => ({
  entity_type: "task",
  id: "test-id",
  fields,
});

describe("resolveCommandName", () => {
  it("resolves {{entity.type}} with 'task' to capitalized 'Task'", () => {
    expect(resolveCommandName("Inspect {{entity.type}}", "task")).toBe(
      "Inspect Task",
    );
  });

  it("resolves {{entity.type}} with 'column' to capitalized 'Column'", () => {
    expect(resolveCommandName("Inspect {{entity.type}}", "column")).toBe(
      "Inspect Column",
    );
  });

  it("resolves {{entity.name}} from entity field value", () => {
    const entity = makeEntity({ name: "Backlog" });
    expect(resolveCommandName("Rename {{entity.name}}", "column", entity)).toBe(
      "Rename Backlog",
    );
  });

  it("resolves {{entity.title}} from entity field value", () => {
    const entity = makeEntity({ title: "Fix the bug" });
    expect(resolveCommandName("View {{entity.title}}", "task", entity)).toBe(
      "View Fix the bug",
    );
  });

  it("resolves missing field to empty string", () => {
    const entity = makeEntity({});
    expect(
      resolveCommandName("Edit {{entity.nonexistent}}", "task", entity),
    ).toBe("Edit ");
  });

  it("returns string unchanged when there are no template variables", () => {
    expect(resolveCommandName("Delete", "task")).toBe("Delete");
  });

  it("resolves multiple template variables in one string", () => {
    const entity = makeEntity({ name: "Sprint 1" });
    expect(
      resolveCommandName(
        "Move {{entity.type}} to {{entity.name}}",
        "task",
        entity,
      ),
    ).toBe("Move Task to Sprint 1");
  });

  it("resolves field template to empty string when entity is not provided", () => {
    expect(resolveCommandName("Edit {{entity.name}}", "task")).toBe("Edit ");
  });
});

describe("useEntityCommands", () => {
  it("returns CommandDefs with resolved names from schema", async () => {
    const { result } = renderHook(() => useEntityCommands("task", "task-1"), {
      wrapper: makeWrapper(),
    });

    // Initially empty while schema loads
    expect(result.current).toEqual([]);

    // Wait for schema to load
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(result.current.length).toBeGreaterThan(0);
    const inspectCmd = result.current.find((c) => c.id === "entity.inspect");
    expect(inspectCmd).toBeDefined();
    expect(inspectCmd!.name).toBe("Inspect Task");
    expect(inspectCmd!.target).toBe("task:task-1");
    expect(inspectCmd!.contextMenu).toBe(true);
  });

  it("resolves template name using entity field values", async () => {
    const entity: Entity = {
      entity_type: "task",
      id: "task-1",
      fields: { title: "Fix bug" },
    };
    const { result } = renderHook(
      () => useEntityCommands("task", "task-1", entity),
      { wrapper: makeWrapper() },
    );

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const archiveCmd = result.current.find((c) => c.id === "entity.archive");
    expect(archiveCmd).toBeDefined();
    // "Archive {{entity.type}}" resolves to "Archive Task"
    expect(archiveCmd!.name).toBe("Archive Task");
  });

  it("appends extraCommands after schema commands", async () => {
    const extra = [
      {
        id: "task.untag",
        name: "Remove Tag",
        contextMenu: true,
        args: { id: "t1", tag: "foo" },
      },
    ];
    const { result } = renderHook(
      () => useEntityCommands("task", "task-1", undefined, extra),
      { wrapper: makeWrapper() },
    );

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const untag = result.current.find((c) => c.id === "task.untag");
    expect(untag).toBeDefined();
    expect(untag!.name).toBe("Remove Tag");
    // Schema commands come first
    expect(result.current[0].id).toBe("entity.inspect");
  });

  it("entity.inspect execute calls the inspect function", async () => {
    const onInspect = vi.fn();
    const { result } = renderHook(() => useEntityCommands("task", "task-42"), {
      wrapper: makeWrapper(onInspect),
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const inspectCmd = result.current.find((c) => c.id === "entity.inspect");
    expect(inspectCmd).toBeDefined();
    inspectCmd!.execute!();
    expect(onInspect).toHaveBeenCalledWith("task", "task-42");
  });
});
