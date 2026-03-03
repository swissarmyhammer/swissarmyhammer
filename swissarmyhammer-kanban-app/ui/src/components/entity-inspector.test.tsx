import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("cua")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { EntityInspector } from "./entity-inspector";
import { KeymapProvider } from "@/lib/keymap-context";
import type { FieldDef, Entity } from "@/types/kanban";

function renderWithProvider(ui: React.ReactElement) {
  return render(<KeymapProvider>{ui}</KeymapProvider>);
}

function makeField(name: string, kind: string, extras: Record<string, unknown> = {}): FieldDef {
  const base: Record<string, unknown> = { kind };
  if (kind === "text" || kind === "markdown") base.single_line = false;
  if (kind === "number") { base.min = undefined; base.max = undefined; }
  if (kind === "reference") { base.entity = "task"; base.multiple = false; }
  if (kind === "computed") base.derive = "test-derive";
  if (kind === "select" || kind === "multi-select") base.options = [];
  Object.assign(base, extras);

  return {
    id: `field-${name}`,
    name,
    type: base as FieldDef["type"],
  };
}

function makeEntity(fields: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "test-id",
    fields,
  };
}

describe("EntityInspector", () => {
  it("renders a field row for each field definition", () => {
    const fields = [
      makeField("title", "text"),
      makeField("body", "markdown"),
      makeField("due", "date"),
    ];
    const entity = makeEntity({ title: "My Task", body: "Description" });
    const onUpdateField = vi.fn();

    renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    expect(screen.getByTestId("field-row-title")).toBeDefined();
    expect(screen.getByTestId("field-row-body")).toBeDefined();
    expect(screen.getByTestId("field-row-due")).toBeDefined();
  });

  it("displays field labels as humanized names", () => {
    const fields = [makeField("depends_on", "reference", { multiple: true })];
    const entity = makeEntity({});
    const onUpdateField = vi.fn();

    renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    expect(screen.getByText("depends on")).toBeDefined();
  });

  it("hides fields listed in hideFields", () => {
    const fields = [
      makeField("title", "text"),
      makeField("body", "markdown"),
    ];
    const entity = makeEntity({ title: "Test", body: "Content" });
    const onUpdateField = vi.fn();

    renderWithProvider(
      <EntityInspector
        entity={entity}
        fields={fields}
        hideFields={["body"]}
        onUpdateField={onUpdateField}
      />,
    );

    expect(screen.getByTestId("field-row-title")).toBeDefined();
    expect(screen.queryByTestId("field-row-body")).toBeNull();
  });

  it("displays field values as markdown", () => {
    const fields = [makeField("title", "text")];
    const entity = makeEntity({ title: "Hello World" });
    const onUpdateField = vi.fn();

    renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    expect(screen.getByText("Hello World")).toBeDefined();
  });

  it("enters edit mode on click — shows CodeMirror editor", () => {
    const fields = [makeField("title", "text")];
    const entity = makeEntity({ title: "Original" });
    const onUpdateField = vi.fn();

    const { container } = renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    // Click to edit
    fireEvent.click(screen.getByText("Original"));

    // Should now show a CodeMirror editor
    const editor = container.querySelector(".cm-editor");
    expect(editor).toBeTruthy();
  });

  it("commits on blur", () => {
    const fields = [makeField("title", "text")];
    const entity = makeEntity({ title: "Original" });
    const onUpdateField = vi.fn();

    const { container } = renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    fireEvent.click(screen.getByText("Original"));
    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();

    // Blur commits the current value
    fireEvent.blur(cmContent);
    expect(onUpdateField).toHaveBeenCalledWith("title", "Original");
  });

  it("does not allow editing computed fields", () => {
    const fields = [makeField("tags", "computed")];
    const entity = makeEntity({ tags: ["bug", "feature"] });
    const onUpdateField = vi.fn();

    const { container } = renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    expect(screen.getByTestId("field-row-tags")).toBeDefined();

    // Click should not enter edit mode (no CodeMirror editor)
    fireEvent.click(screen.getByText("bug, feature"));
    expect(container.querySelector(".cm-editor")).toBeNull();
  });

  it("shows 'Empty' for missing field values", () => {
    const fields = [makeField("due", "date")];
    const entity = makeEntity({});
    const onUpdateField = vi.fn();

    renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    expect(screen.getByText("Empty")).toBeDefined();
  });

  it("only allows one field to be edited at a time", () => {
    const fields = [
      makeField("title", "text"),
      makeField("name", "text"),
    ];
    const entity = makeEntity({ title: "A", name: "B" });
    const onUpdateField = vi.fn();

    const { container } = renderWithProvider(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    // Edit first field
    fireEvent.click(screen.getByText("A"));
    // Should have exactly one CodeMirror editor
    const editors = container.querySelectorAll(".cm-editor");
    expect(editors.length).toBe(1);
    // Second field should still be in display mode (rendered as markdown)
    expect(screen.getByText("B")).toBeDefined();
  });
});
