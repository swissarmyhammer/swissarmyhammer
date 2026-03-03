import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { EntityInspector } from "./entity-inspector";
import type { FieldDef, Entity } from "@/types/kanban";

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

    render(
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

    render(
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

    render(
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

  it("displays field values from entity", () => {
    const fields = [makeField("title", "text")];
    const entity = makeEntity({ title: "Hello World" });
    const onUpdateField = vi.fn();

    render(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    expect(screen.getByText("Hello World")).toBeDefined();
  });

  it("enters edit mode on click and commits on Enter", () => {
    const fields = [makeField("title", "text")];
    const entity = makeEntity({ title: "Original" });
    const onUpdateField = vi.fn();

    render(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    // Click to edit
    fireEvent.click(screen.getByText("Original"));

    // Should now show an input
    const input = screen.getByDisplayValue("Original") as HTMLInputElement;
    expect(input).toBeDefined();

    // Change value and press Enter
    fireEvent.change(input, { target: { value: "Updated" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onUpdateField).toHaveBeenCalledWith("title", "Updated");
  });

  it("cancels editing on Escape", () => {
    const fields = [makeField("title", "text")];
    const entity = makeEntity({ title: "Original" });
    const onUpdateField = vi.fn();

    render(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    fireEvent.click(screen.getByText("Original"));
    const input = screen.getByDisplayValue("Original");
    fireEvent.keyDown(input, { key: "Escape" });

    expect(onUpdateField).not.toHaveBeenCalled();
  });

  it("does not allow editing computed fields", () => {
    const fields = [makeField("tags", "computed")];
    const entity = makeEntity({ tags: ["bug", "feature"] });
    const onUpdateField = vi.fn();

    render(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    // Computed field should display value
    expect(screen.getByTestId("field-row-tags")).toBeDefined();

    // Click should not enter edit mode (no input appears)
    fireEvent.click(screen.getByText("bug, feature"));
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("shows 'Empty' for missing field values", () => {
    const fields = [makeField("due", "date")];
    const entity = makeEntity({});
    const onUpdateField = vi.fn();

    render(
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

    render(
      <EntityInspector entity={entity} fields={fields} onUpdateField={onUpdateField} />,
    );

    // Edit first field
    fireEvent.click(screen.getByText("A"));
    expect(screen.getByDisplayValue("A")).toBeDefined();
    // Second field should still be in display mode
    expect(screen.getByText("B")).toBeDefined();
  });
});
