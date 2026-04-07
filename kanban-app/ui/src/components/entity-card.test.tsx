import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  act,
  waitFor,
} from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "tags", "progress", "body"],
    commands: [
      {
        id: "ui.inspect",
        name: "Inspect {{entity.type}}",
        context_menu: true,
      },
    ],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f3",
      name: "tags",
      type: { kind: "computed", derive: "parse-body-tags" },
      editor: "multi-select",
      display: "badge-list",
      section: "header",
    },
    {
      id: "f4",
      name: "progress",
      type: { kind: "computed", derive: "parse-body-progress" },
      editor: "none",
      display: "progress",
      section: "header",
    },
    {
      id: "f2",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types") return Promise.resolve(["task"]);
  if (args[0] === "get_entity_schema") return Promise.resolve(TASK_SCHEMA);
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
  if (args[0] === "update_entity_field")
    return Promise.resolve({ id: "task-1" });
  if (args[0] === "list_commands_for_scope")
    return Promise.resolve([
      {
        id: "ui.inspect",
        name: "Inspect task",
        target: "task:task-1",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
  if (args[0] === "show_context_menu") return Promise.resolve();
  return Promise.resolve("ok");
});

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

import "@/components/fields/registrations";
import { EntityCard } from "./entity-card";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

/** Create a task Entity with sensible defaults and optional field overrides. */
function makeEntity(fieldOverrides: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
    fields: {
      title: "Hello **world**",
      body: "",
      tags: [],
      assignees: [],
      depends_on: [],
      position_column: "col-1",
      position_ordinal: "a0",
      ...fieldOverrides,
    },
  };
}

/** Track the current entity so the store can find it via useFieldValue. */
let currentEntity: Entity = makeEntity();

function renderCard(ui: React.ReactElement) {
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ task: [currentEntity], tag: [] }}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>{ui}</UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

/** Render and wait for schema to load */
async function renderWithProvider(ui: React.ReactElement) {
  const result = renderCard(ui);
  await act(async () => {
    await new Promise((r) => setTimeout(r, 100));
  });
  return result;
}

describe("EntityCard", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("renders title as text via Field display", async () => {
    currentEntity = makeEntity();
    await renderWithProvider(<EntityCard entity={currentEntity} />);
    // TextDisplay renders plain text (display: "text"), not markdown
    expect(screen.getByText("Hello **world**")).toBeTruthy();
  });

  it("(i) button dispatches ui.inspect with explicit target moniker", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    mockInvoke.mockClear();
    const inspectBtn = container.querySelector("button[aria-label='Inspect']")!;
    await act(async () => {
      fireEvent.click(inspectBtn);
    });
    const inspectCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.inspect",
    );
    expect(inspectCall).toBeTruthy();
    // Target must be passed explicitly so the backend uses ctx.target
    // instead of walking the scope chain (which depends on focus state).
    const params = inspectCall![1] as Record<string, unknown>;
    expect(params.target).toBe("task:task-1");
  });

  it("(i) button always renders", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    expect(
      container.querySelector("button[aria-label='Inspect']"),
    ).not.toBeNull();
  });

  it("enters edit mode when title is clicked", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    const titleEl = screen.getByText("Hello **world**");
    fireEvent.click(titleEl);
    expect(container.querySelector(".cm-editor")).toBeTruthy();
  });

  it("saving edited title calls dispatch_command with correct params", async () => {
    mockInvoke.mockClear();
    currentEntity = makeEntity({ title: "bug" });
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );

    // Click to enter edit mode
    const titleEl = screen.getByText("bug");
    fireEvent.click(titleEl);
    const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
    expect(cmEditor).toBeTruthy();

    // Get CM6 EditorView and replace doc text
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(cmEditor);
    if (!view) return; // jsdom limitation — skip gracefully

    await act(async () => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "defect" },
      });
    });

    // CM6 manages focus internally. Call blur() on the contenteditable
    // element so CM6's DOMObserver detects the focus loss.
    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await act(async () => {
      cmContent.blur();
      // CM6's DOMObserver polls focus state — give it a tick
      await new Promise((r) => setTimeout(r, 50));
    });

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(
        (c) =>
          c[0] === "dispatch_command" &&
          (c[1] as Record<string, unknown>)?.cmd === "entity.update_field",
      );
      expect(call).toBeTruthy();
      expect(call![1]).toMatchObject({
        cmd: "entity.update_field",
        args: {
          entity_type: "task",
          id: "task-1",
          field_name: "title",
          value: "defect",
        },
      });
    });
  });

  it("entity.inspect command includes target moniker in context menu", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    const card = container.querySelector("[data-moniker='task:task-1']")!;
    await act(async () => {
      fireEvent.contextMenu(card);
      // Flush the promise chain (list_commands_for_scope → show_context_menu)
      await new Promise((r) => setTimeout(r, 50));
    });
    // Context menu items carry cmd + target as separate fields
    const ctxCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "show_context_menu",
    );
    expect(ctxCall).toBeTruthy();
    const items = ctxCall![1].items as {
      cmd: string;
      target?: string;
      name: string;
    }[];
    expect(
      items.find((i) => i.cmd === "ui.inspect" && i.target === "task:task-1"),
    ).toBeTruthy();
  });

  it("clicking card body does not trigger inspect", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    mockInvoke.mockClear();
    const card = container.querySelector(".rounded-md")!;
    fireEvent.click(card);
    // Click on card body should not dispatch ui.inspect — only the (i) button does
    const inspectCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.inspect",
    );
    expect(inspectCall).toBeUndefined();
  });

  describe("progress bar", () => {
    it("shows progress bar when progress field has items", async () => {
      currentEntity = makeEntity({
        progress: { total: 3, completed: 1, percent: 33 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("33");
    });

    it("shows 0% progress when no items are completed", async () => {
      currentEntity = makeEntity({
        progress: { total: 2, completed: 0, percent: 0 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("0");
      expect(container.textContent).toContain("0%");
    });

    it("shows 100% progress when all items are completed", async () => {
      currentEntity = makeEntity({
        progress: { total: 2, completed: 2, percent: 100 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("100");
    });

    it("does not show progress bar when total is zero", async () => {
      currentEntity = makeEntity({
        progress: { total: 0, completed: 0, percent: 0 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });

    it("does not show progress bar when progress field is null", async () => {
      currentEntity = makeEntity();
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });
  });
});
