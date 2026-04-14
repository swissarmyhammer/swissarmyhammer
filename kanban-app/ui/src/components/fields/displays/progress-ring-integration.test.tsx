/**
 * Integration test: Field component renders progress-ring display
 * for a board entity's percent_complete field, using the same data
 * shapes the backend actually returns.
 */
import { describe, it, expect, vi } from "vitest";
import { render, act } from "@testing-library/react";

const BOARD_SCHEMA = {
  entity: {
    name: "board",
    search_display_field: "name",
    fields: ["name", "description", "percent_complete"],
  },
  fields: [
    {
      id: "b1",
      name: "name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "b2",
      name: "description",
      type: { kind: "markdown" },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
    {
      id: "b3",
      name: "percent_complete",
      type: { kind: "computed", derive: "board-percent-complete" },
      editor: "none",
      display: "progress-ring",
      icon: "percent",
      section: "header",
    },
  ],
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "progress", "body"],
  },
  fields: [
    {
      id: "t1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "t2",
      name: "progress",
      type: { kind: "computed", derive: "parse-body-progress" },
      editor: "none",
      display: "progress",
      icon: "bar-chart",
      section: "header",
    },
    {
      id: "t3",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  board: BOARD_SCHEMA,
  task: TASK_SCHEMA,
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types")
    return Promise.resolve(["board", "task"]);
  if (args[0] === "get_entity_schema") {
    const entityType = args[1]?.entityType as string;
    return Promise.resolve(SCHEMAS[entityType] ?? TASK_SCHEMA);
  }
  if (args[0] === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  return Promise.resolve("ok");
});

// Preserve real exports (SERIALIZE_TO_IPC_FN, Resource, Channel, TauriEvent,
// …) so transitively-imported submodules like `window.js` / `dpi.js` can
// still resolve their re-exports. Only override `invoke` / `listen`.
vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  return {
    ...actual,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    invoke: (...args: any[]) => mockInvoke(...args),
  };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  return {
    ...actual,
    listen: vi.fn(() => Promise.resolve(() => {})),
  };
});
// `window-container.tsx` calls `getCurrentWindow()` at module-load time.
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
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

import "@/components/fields/registrations";
import { Field } from "@/components/fields/field";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

function Providers({
  children,
  entities,
}: {
  children: React.ReactNode;
  entities: Record<string, Entity[]>;
}) {
  return (
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={entities}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>{children}</UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>
  );
}

describe("progress-ring integration", () => {
  it("renders progress ring for board percent_complete with real backend data shape", async () => {
    // This is the exact shape the backend returns: { done, total, percent }
    const boardEntity: Entity = {
      entity_type: "board",
      id: "board",
      moniker: "board:board",
      fields: {
        name: "My Board",
        percent_complete: { done: 3, total: 10, percent: 30 },
      },
    };

    const { container } = render(
      <Providers entities={{ board: [boardEntity] }}>
        <Field
          fieldDef={BOARD_SCHEMA.fields[2] as any}
          entityType="board"
          entityId="board"
          mode="compact"
          editing={false}
        />
      </Providers>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const ring = container.querySelector("[role='progressbar']");
    expect(ring, "Expected progress ring to render").toBeTruthy();
    expect(container.textContent).toContain("30%");
  });

  it("renders progress bar for task progress with real backend data shape", async () => {
    // This is the exact shape parse-body-progress returns: { total, completed, percent }
    const taskEntity: Entity = {
      entity_type: "task",
      id: "task-1",
      moniker: "task:task-1",
      fields: {
        title: "My Task",
        body: "- [x] done\n- [ ] todo",
        progress: { total: 2, completed: 1, percent: 50 },
      },
    };

    const { container } = render(
      <Providers entities={{ task: [taskEntity] }}>
        <Field
          fieldDef={TASK_SCHEMA.fields[1] as any}
          entityType="task"
          entityId="task-1"
          mode="compact"
          editing={false}
        />
      </Providers>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const bar = container.querySelector("[role='progressbar']");
    expect(bar, "Expected progress bar to render").toBeTruthy();
    expect(container.textContent).toContain("50%");
  });

  it("returns null for board with zero tasks", async () => {
    const boardEntity: Entity = {
      entity_type: "board",
      id: "board",
      moniker: "board:board",
      fields: {
        name: "Empty Board",
        percent_complete: { done: 0, total: 0, percent: 0 },
      },
    };

    const { container } = render(
      <Providers entities={{ board: [boardEntity] }}>
        <Field
          fieldDef={BOARD_SCHEMA.fields[2] as any}
          entityType="board"
          entityId="board"
          mode="compact"
          editing={false}
        />
      </Providers>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(container.querySelector("[role='progressbar']")).toBeNull();
  });
});
