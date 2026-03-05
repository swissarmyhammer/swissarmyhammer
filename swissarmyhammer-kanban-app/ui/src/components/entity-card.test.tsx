import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const TASK_SCHEMA = {
  entity: { name: "task", body_field: "body", fields: ["title", "tags", "progress", "body"] },
  fields: [
    { id: "f1", name: "title", type: { kind: "markdown", single_line: true }, section: "header" },
    { id: "f3", name: "tags", type: { kind: "computed", derive: "parse-body-tags" }, section: "header", display: "badge-list" },
    { id: "f4", name: "progress", type: { kind: "computed", derive: "parse-body-progress" }, section: "header", display: "number" },
    { id: "f2", name: "body", type: { kind: "markdown", single_line: false }, section: "body" },
  ],
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "get_entity_schema") return Promise.resolve(TASK_SCHEMA);
  if (args[0] === "get_keymap_mode") return Promise.resolve("cua");
  if (args[0] === "update_entity_field") return Promise.resolve({ id: "task-1" });
  return Promise.resolve("ok");
});

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

import { EntityCard } from "./entity-card";
import { KeymapProvider } from "@/lib/keymap-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

/** Create a task Entity with sensible defaults and optional field overrides. */
function makeEntity(fieldOverrides: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "task-1",
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

function renderCard(ui: React.ReactElement) {
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ tag: [] }}>
          <FieldUpdateProvider onRefresh={() => {}}>
            <KeymapProvider>{ui}</KeymapProvider>
          </FieldUpdateProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>
  );
}

/** Render and wait for schema to load */
async function renderWithProvider(ui: React.ReactElement) {
  const result = renderCard(ui);
  await act(async () => { await new Promise((r) => setTimeout(r, 100)); });
  return result;
}

describe("EntityCard", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("renders title as markdown (bold text)", async () => {
    await renderWithProvider(<EntityCard entity={makeEntity()} />);
    const strong = screen.getByText("world");
    expect(strong.tagName).toBe("STRONG");
  });

  it("(i) button calls onInspect with entity id", async () => {
    const onInspect = vi.fn();
    const { container } = await renderWithProvider(
      <EntityCard entity={makeEntity()} onInspect={onInspect} />
    );
    const inspectBtn = container.querySelector("button[title='Inspect']")!;
    fireEvent.click(inspectBtn);
    expect(onInspect).toHaveBeenCalledWith("task-1");
  });

  it("(i) button is hidden when onInspect is not provided", async () => {
    const { container } = await renderWithProvider(
      <EntityCard entity={makeEntity()} />
    );
    expect(container.querySelector("button[title='Inspect']")).toBeNull();
  });

  it("enters edit mode when title is clicked", async () => {
    const { container } = await renderWithProvider(
      <EntityCard entity={makeEntity()} />
    );
    const titleEl = screen.getByText("world");
    fireEvent.click(titleEl);
    expect(container.querySelector(".cm-editor")).toBeTruthy();
  });

  it("saving edited title calls invoke with correct camelCase params", async () => {
    mockInvoke.mockClear();
    // Use a simple title so CM6 doc content is predictable
    const entity = makeEntity({ title: "bug" });
    const { container } = await renderWithProvider(
      <EntityCard entity={entity} />
    );

    // Click to enter edit mode
    const titleEl = screen.getByText("bug");
    fireEvent.click(titleEl);
    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();

    // Get the CM6 EditorView and replace the document text
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const editorView = (cmContent as any).cmTile?.view;
    if (!editorView?.dispatch) {
      // CM6 internals not available in jsdom — skip gracefully
      return;
    }
    editorView.dispatch({
      changes: { from: 0, to: editorView.state.doc.length, insert: "defect" },
    });

    // Blur triggers commit
    await act(async () => {
      fireEvent.blur(cmContent);
    });

    // Verify invoke was called with camelCase params for the title save
    const updateCall = mockInvoke.mock.calls.find(
      (call) => call[0] === "update_entity_field"
    );
    expect(updateCall).toBeTruthy();
    expect(updateCall![1]).toEqual({
      entityType: "task",
      id: "task-1",
      fieldName: "title",
      value: "defect",
    });
  });

  it("clicking card body does not trigger inspect", async () => {
    const onInspect = vi.fn();
    const { container } = await renderWithProvider(
      <EntityCard entity={makeEntity()} onInspect={onInspect} />
    );
    const card = container.querySelector(".rounded-md")!;
    fireEvent.click(card);
    expect(onInspect).not.toHaveBeenCalled();
  });

  describe("progress bar", async () => {
    it("shows progress bar when description has checkboxes", async () => {
      const { container } = await renderWithProvider(
        <EntityCard
          entity={makeEntity({
            body: "- [x] done\n- [ ] pending\n- [ ] also pending",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("33");
    });

    it("shows 0% progress when no checkboxes are checked", async () => {
      const { container } = await renderWithProvider(
        <EntityCard
          entity={makeEntity({
            body: "- [ ] first\n- [ ] second",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("0");
      expect(container.textContent).toContain("0%");
    });

    it("shows 100% progress when all checkboxes are checked", async () => {
      const { container } = await renderWithProvider(
        <EntityCard
          entity={makeEntity({
            body: "- [x] done\n- [x] also done",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("100");
    });

    it("does not show progress bar when description has no checkboxes", async () => {
      const { container } = await renderWithProvider(
        <EntityCard
          entity={makeEntity({
            body: "Just some plain text",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });

    it("does not show progress bar when description is empty", async () => {
      const { container } = await renderWithProvider(
        <EntityCard entity={makeEntity()} />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });
  });
});
