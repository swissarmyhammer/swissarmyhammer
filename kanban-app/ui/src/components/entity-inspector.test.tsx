import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Schema with sections matching the new YAML definitions
const TASK_SCHEMA = {
  entity: { name: "task", body_field: "body", fields: ["title", "tags", "progress", "body", "assignees", "depends_on", "position_column"] },
  fields: [
    { id: "f1", name: "title", type: { kind: "markdown", single_line: true }, section: "header" },
    { id: "f3", name: "tags", type: { kind: "computed", derive: "parse-body-tags" }, section: "header" },
    { id: "f4", name: "progress", type: { kind: "computed", derive: "parse-body-progress" }, section: "header" },
    { id: "f2", name: "body", type: { kind: "markdown", single_line: false }, section: "body" },
    { id: "f5", name: "assignees", type: { kind: "reference", entity: "actor", multiple: true }, section: "body" },
    { id: "f7", name: "depends_on", type: { kind: "reference", entity: "task", multiple: true }, section: "body" },
    { id: "f8", name: "position_column", type: { kind: "reference", entity: "column", multiple: false }, section: "hidden" },
  ],
};

const TAG_SCHEMA = {
  entity: { name: "tag", fields: ["tag_name", "color", "description"], mention_prefix: "#", mention_display_field: "tag_name" },
  fields: [
    { id: "t1", name: "tag_name", type: { kind: "text", single_line: true }, section: "header" },
    { id: "t2", name: "color", type: { kind: "color" }, section: "body" },
    { id: "t3", name: "description", type: { kind: "markdown" }, section: "body" },
  ],
};

const ACTOR_SCHEMA = {
  entity: { name: "actor", fields: ["name", "color"], mention_prefix: "@", mention_display_field: "name" },
  fields: [
    { id: "a1", name: "name", type: { kind: "text", single_line: true }, section: "header" },
    { id: "a2", name: "color", type: { kind: "color" }, section: "body" },
  ],
};

const SCHEMAS: Record<string, unknown> = { task: TASK_SCHEMA, tag: TAG_SCHEMA, actor: ACTOR_SCHEMA };

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "get_entity_schema") {
    const entityType = args[1]?.entityType as string;
    return Promise.resolve(SCHEMAS[entityType] ?? TASK_SCHEMA);
  }
  if (args[0] === "get_keymap_mode") return Promise.resolve("cua");
  if (args[0] === "update_entity_field") return Promise.resolve({ id: "test-id" });
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
  error: vi.fn(), warn: vi.fn(), info: vi.fn(), debug: vi.fn(), trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { EntityInspector } from "./entity-inspector";
import { KeymapProvider } from "@/lib/keymap-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { InspectProvider } from "@/lib/inspect-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

function makeEntity(fields: Record<string, unknown> = {}): Entity {
  return { entity_type: "task", id: "test-id", fields };
}

async function renderInspector(entity: Entity, tagEntities: Entity[] = []) {
  const result = render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ tag: tagEntities }}>
          <EntityFocusProvider>
            <InspectProvider onInspect={() => {}} onDismiss={() => false}>
              <FieldUpdateProvider>
                <KeymapProvider>
                  <EntityInspector entity={entity} />
                </KeymapProvider>
              </FieldUpdateProvider>
            </InspectProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>
  );
  // Wait for async schema load
  await act(async () => { await new Promise((r) => setTimeout(r, 50)); });
  return result;
}

describe("EntityInspector", () => {
  it("renders fields from schema in section order (header, body)", async () => {
    await renderInspector(makeEntity({ title: "My Task", body: "Description", tags: [] }));
    expect(screen.getByTestId("field-row-title")).toBeTruthy();
    expect(screen.getByTestId("field-row-body")).toBeTruthy();
    expect(screen.getByTestId("field-row-tags")).toBeTruthy();
  });

  it("does not render fields with section: hidden", async () => {
    const { container } = await renderInspector(makeEntity({ position_column: "todo" }));
    expect(container.querySelector('[data-testid="field-row-position_column"]')).toBeNull();
  });

  it("groups fields into header and body sections", async () => {
    const { container } = await renderInspector(makeEntity({ title: "T", body: "B", tags: [] }));
    const header = container.querySelector('[data-testid="inspector-header"]');
    const body = container.querySelector('[data-testid="inspector-body"]');
    expect(header).toBeTruthy();
    expect(body).toBeTruthy();
    // title is in header, body is in body section
    expect(header!.querySelector('[data-testid="field-row-title"]')).toBeTruthy();
    expect(body!.querySelector('[data-testid="field-row-body"]')).toBeTruthy();
  });

  it("renders markdown fields with EditableMarkdown (click enters edit mode)", async () => {
    const { container } = await renderInspector(makeEntity({ title: "Click me" }));
    fireEvent.click(screen.getByText("Click me"));
    expect(container.querySelector(".cm-editor")).toBeTruthy();
  });

  it("saves field on blur via FieldUpdateContext with correct params", async () => {
    mockInvoke.mockClear();
    const { container } = await renderInspector(makeEntity({ title: "Old" }));

    fireEvent.click(screen.getByText("Old"));
    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    if (!cmContent) return;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const view = (cmContent as any).cmTile?.view;
    if (!view?.dispatch) return;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: "New" } });
    await act(async () => { fireEvent.blur(cmContent); });

    const call = mockInvoke.mock.calls.find(
      (c) => c[0] === "dispatch_command" && (c[1] as Record<string, unknown>)?.cmd === "entity.update_field",
    );
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({ cmd: "entity.update_field", args: { entity_type: "task", id: "test-id", field_name: "title", value: "New" } });
  });

  it("allows editing computed tag fields via multi-select", async () => {
    const { container } = await renderInspector(makeEntity({ tags: ["bug"] }));
    const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
    expect(tagsRow).toBeTruthy();
    // Click should produce a CM6 editor (tags are editable via tag/untag commands)
    await act(async () => {
      fireEvent.click(tagsRow!.querySelector(".cursor-text, .min-h-\\[1\\.25rem\\]")!);
    });
    expect(tagsRow!.querySelector(".cm-editor")).toBeTruthy();
  });

  it("body_field renders #tag as a styled pill when tag entity exists", async () => {
    const tags = [
      { entity_type: "tag", id: "tag-ui", fields: { tag_name: "ui", color: "1d76db", description: "UI" } },
    ];
    const { container } = await renderInspector(makeEntity({ body: "Fix #ui bug" }), tags);

    const bodyRow = container.querySelector('[data-testid="field-row-body"]');
    expect(bodyRow).toBeTruthy();
    const pill = Array.from(bodyRow!.querySelectorAll("span")).find(
      (s: Element) => s.textContent === "#ui" && s.classList.contains("rounded-full"),
    );
    expect(pill, `Expected #ui pill. HTML: ${bodyRow!.innerHTML}`).toBeTruthy();
  });

  it("non-body markdown fields do NOT get tag pills", async () => {
    const tags = [
      { entity_type: "tag", id: "tag-ui", fields: { tag_name: "ui", color: "1d76db", description: "" } },
    ];
    const { container } = await renderInspector(makeEntity({ title: "Fix #ui" }), tags);

    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    const pill = Array.from(titleRow!.querySelectorAll("span")).find(
      (s: Element) => s.textContent === "#ui" && s.classList.contains("rounded-full"),
    );
    expect(pill, "Title should NOT have tag pills").toBeFalsy();
  });
});
