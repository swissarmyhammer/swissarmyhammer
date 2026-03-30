import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Schema with sections matching the new YAML definitions
const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: [
      "title",
      "tags",
      "progress",
      "body",
      "assignees",
      "depends_on",
      "position_column",
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
      icon: "tag",
      section: "header",
    },
    {
      id: "f4",
      name: "progress",
      type: { kind: "computed", derive: "parse-body-progress" },
      editor: "none",
      display: "number",
      icon: "bar-chart",
      section: "header",
    },
    {
      id: "f2",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      icon: "file-text",
      section: "body",
    },
    {
      id: "f5",
      name: "assignees",
      type: { kind: "reference", entity: "actor", multiple: true },
      editor: "multi-select",
      display: "avatar",
      icon: "users",
      section: "body",
    },
    {
      id: "f7",
      name: "depends_on",
      type: { kind: "reference", entity: "task", multiple: true },
      editor: "multi-select",
      display: "badge-list",
      icon: "workflow",
      section: "body",
    },
    {
      id: "f8",
      name: "position_column",
      type: { kind: "reference", entity: "column", multiple: false },
      editor: "select",
      display: "badge",
      section: "hidden",
    },
  ],
};

const TAG_SCHEMA = {
  entity: {
    name: "tag",
    fields: ["tag_name", "color", "description"],
    mention_prefix: "#",
    mention_display_field: "tag_name",
  },
  fields: [
    {
      id: "t1",
      name: "tag_name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "tag",
      section: "header",
    },
    {
      id: "t2",
      name: "color",
      type: { kind: "color" },
      editor: "color-palette",
      display: "color-swatch",
      icon: "palette",
      section: "body",
    },
    {
      id: "t3",
      name: "description",
      type: { kind: "markdown" },
      editor: "markdown",
      display: "markdown",
      icon: "align-left",
      section: "body",
    },
  ],
};

const ACTOR_SCHEMA = {
  entity: {
    name: "actor",
    fields: ["name", "color"],
    mention_prefix: "@",
    mention_display_field: "name",
  },
  fields: [
    {
      id: "a1",
      name: "name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "type",
      section: "header",
    },
    {
      id: "a2",
      name: "color",
      type: { kind: "color" },
      editor: "color-palette",
      display: "color-swatch",
      icon: "palette",
      section: "body",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  task: TASK_SCHEMA,
  tag: TAG_SCHEMA,
  actor: ACTOR_SCHEMA,
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types")
    return Promise.resolve(["task", "tag", "actor"]);
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
  if (args[0] === "update_entity_field")
    return Promise.resolve({ id: "test-id" });
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

import "@/components/fields/registrations";
import { EntityInspector } from "./entity-inspector";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { InspectProvider } from "@/lib/inspect-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandScopeProvider } from "@/lib/command-scope";
import type { Entity } from "@/types/kanban";

function makeEntity(fields: Record<string, unknown> = {}): Entity {
  return { entity_type: "task", id: "test-id", fields };
}

async function renderInspector(entity: Entity, tagEntities: Entity[] = []) {
  const result = render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ task: [entity], tag: tagEntities }}>
          <EntityFocusProvider>
            <InspectProvider onInspect={() => {}} onDismiss={() => false}>
              <FieldUpdateProvider>
                <UIStateProvider>
                  <CommandScopeProvider commands={[]}>
                    <EntityInspector entity={entity} />
                  </CommandScopeProvider>
                </UIStateProvider>
              </FieldUpdateProvider>
            </InspectProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
  // Wait for async schema load
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  return result;
}

describe("EntityInspector", () => {
  it("renders fields from schema in section order (header, body)", async () => {
    await renderInspector(
      makeEntity({ title: "My Task", body: "Description", tags: [] }),
    );
    expect(screen.getByTestId("field-row-title")).toBeTruthy();
    expect(screen.getByTestId("field-row-body")).toBeTruthy();
    expect(screen.getByTestId("field-row-tags")).toBeTruthy();
  });

  it("does not render fields with section: hidden", async () => {
    const { container } = await renderInspector(
      makeEntity({ position_column: "todo" }),
    );
    expect(
      container.querySelector('[data-testid="field-row-position_column"]'),
    ).toBeNull();
  });

  it("groups fields into header and body sections", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "B", tags: [] }),
    );
    const header = container.querySelector('[data-testid="inspector-header"]');
    const body = container.querySelector('[data-testid="inspector-body"]');
    expect(header).toBeTruthy();
    expect(body).toBeTruthy();
    // title is in header, body is in body section
    expect(
      header!.querySelector('[data-testid="field-row-title"]'),
    ).toBeTruthy();
    expect(body!.querySelector('[data-testid="field-row-body"]')).toBeTruthy();
  });

  it("renders markdown fields via Field (click enters edit mode)", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "Click me" }),
    );
    // TextDisplay renders plain text; click on it enters edit mode via Field
    const titleText = screen.getByText("Click me");
    fireEvent.click(titleText);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    expect(titleRow!.querySelector(".cm-editor")).toBeTruthy();
  });

  // Container no longer calls updateField — editors save themselves.
  // Save behavior is tested in editor-save.test.tsx matrix.

  it("allows editing computed tag fields via multi-select", async () => {
    const { container } = await renderInspector(makeEntity({ tags: ["bug"] }), [
      {
        entity_type: "tag",
        id: "tag-bug",
        fields: { tag_name: "bug", color: "ff0000" },
      },
    ]);
    const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
    expect(tagsRow).toBeTruthy();
    // Click the display area to enter edit mode
    const clickTarget =
      tagsRow!.querySelector(".cursor-text") ??
      tagsRow!.querySelector(".min-h-\\[1\\.25rem\\]");
    expect(clickTarget).toBeTruthy();
    await act(async () => {
      fireEvent.click(clickTarget!);
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(tagsRow!.querySelector(".cm-editor")).toBeTruthy();
  });

  it("body_field renders #tag as a styled pill when tag entity exists", async () => {
    const tags = [
      {
        entity_type: "tag",
        id: "tag-ui",
        fields: { tag_name: "ui", color: "1d76db", description: "UI" },
      },
    ];
    const { container } = await renderInspector(
      makeEntity({ body: "Fix #ui bug" }),
      tags,
    );

    const bodyRow = container.querySelector('[data-testid="field-row-body"]');
    expect(bodyRow).toBeTruthy();
    const pill = Array.from(bodyRow!.querySelectorAll("span")).find(
      (s: Element) =>
        s.textContent === "#ui" && s.classList.contains("rounded-full"),
    );
    expect(pill, `Expected #ui pill. HTML: ${bodyRow!.innerHTML}`).toBeTruthy();
  });

  it("non-body markdown fields do NOT get tag pills", async () => {
    const tags = [
      {
        entity_type: "tag",
        id: "tag-ui",
        fields: { tag_name: "ui", color: "1d76db", description: "" },
      },
    ];
    const { container } = await renderInspector(
      makeEntity({ title: "Fix #ui" }),
      tags,
    );

    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    const pill = Array.from(titleRow!.querySelectorAll("span")).find(
      (s: Element) =>
        s.textContent === "#ui" && s.classList.contains("rounded-full"),
    );
    expect(pill, "Title should NOT have tag pills").toBeFalsy();
  });

  it("first visible field has data-focused attribute by default", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "B", tags: [] }),
    );
    // First navigable field (title, in header) should be focused
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    expect(titleRow!.getAttribute("data-focused")).toBe("true");
    // Second field should not be focused
    const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
    expect(tagsRow!.getAttribute("data-focused")).toBeNull();
  });

  it("clicking a field syncs the inspector nav cursor to that field", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "Click me", tags: [] }),
    );
    // Initially first field (title) is focused
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    expect(titleRow!.getAttribute("data-focused")).toBe("true");

    // Click the body field text to enter edit mode
    const bodyText = screen.getByText("Click me");
    await act(async () => {
      fireEvent.click(bodyText);
      await new Promise((r) => setTimeout(r, 50));
    });

    // Body field (index 3: title=0, tags=1, progress=2, body=3) should now be focused
    const bodyRow = container.querySelector('[data-testid="field-row-body"]');
    expect(bodyRow!.getAttribute("data-focused")).toBe("true");
    // Title should no longer be focused
    expect(titleRow!.getAttribute("data-focused")).toBeNull();
  });

  it("only one field has data-focused at a time", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "B", tags: [], assignees: [] }),
    );
    const focused = container.querySelectorAll("[data-focused]");
    expect(focused.length).toBe(1);
  });
});
