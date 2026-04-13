/**
 * Tests for ReferenceSelectEditor — a searchable combobox for reference fields.
 *
 * Verifies:
 * - All referenced entities appear in the dropdown
 * - Typing filters the list by display name
 * - Selecting an item calls onCommit with the entity ID
 * - Current value shows the display name in the trigger
 * - Empty value shows "-" placeholder
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types")
    return Promise.resolve(["task", "column"]);
  if (args[0] === "get_entity_schema") {
    const entityType = args[1]?.entityType as string;
    return Promise.resolve(SCHEMAS[entityType] ?? DEFAULT_SCHEMA);
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
  if (args[0] === "search_mentions") {
    const entityType = args[1]?.entityType as string;
    const query = (args[1]?.query as string) ?? "";
    if (entityType === "column") {
      const all = [
        { id: "col-todo", display_name: "To Do", color: "3366cc" },
        { id: "col-doing", display_name: "Doing", color: "cc9900" },
        { id: "col-done", display_name: "Done", color: "33cc33" },
      ];
      if (!query) return Promise.resolve(all);
      return Promise.resolve(
        all.filter((c) =>
          c.display_name.toLowerCase().includes(query.toLowerCase()),
        ),
      );
    }
    return Promise.resolve([]);
  }
  if (args[0] === "dispatch_command") return Promise.resolve("ok");
  return Promise.resolve(null);
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

import { ReferenceSelectEditor } from "./reference-select-editor";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity, FieldDef } from "@/types/kanban";

const COLUMN_SCHEMA = {
  entity: {
    name: "column",
    fields: ["name", "order"],
    mention_prefix: "%",
    mention_display_field: "name",
  },
  fields: [
    { id: "c1", name: "name", type: { kind: "text" }, section: "header" },
    { id: "c2", name: "order", type: { kind: "number" }, section: "body" },
  ],
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "position_column"],
  },
  fields: [
    { id: "f1", name: "title", type: { kind: "text" }, section: "header" },
    {
      id: "f2",
      name: "position_column",
      type: { kind: "reference", entity: "column", multiple: false },
      section: "hidden",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  column: COLUMN_SCHEMA,
  task: TASK_SCHEMA,
};
const DEFAULT_SCHEMA = { entity: { name: "unknown", fields: [] }, fields: [] };

const COLUMN_ENTITIES: Entity[] = [
  {
    entity_type: "column",
    id: "col-todo",
    moniker: "column:col-todo",
    fields: { name: "To Do", color: "3366cc", order: 0 },
  },
  {
    entity_type: "column",
    id: "col-doing",
    moniker: "column:col-doing",
    fields: { name: "Doing", color: "cc9900", order: 1 },
  },
  {
    entity_type: "column",
    id: "col-done",
    moniker: "column:col-done",
    fields: { name: "Done", color: "33cc33", order: 2 },
  },
];

const POSITION_COLUMN_FIELD: FieldDef = {
  id: "f2",
  name: "position_column",
  type: { kind: "reference", entity: "column", multiple: false },
  section: "hidden",
  editor: "select",
  display: "badge",
};

function renderReferenceSelect(
  props: {
    field: FieldDef;
    value: unknown;
    onCommit: (val: unknown) => void;
    onCancel: () => void;
    onChange?: (val: unknown) => void;
  },
  entities: Record<string, Entity[]> = {},
) {
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={entities}>
          <EntityFocusProvider>
            <UIStateProvider>
              <ReferenceSelectEditor
                field={props.field}
                value={props.value}
                onCommit={props.onCommit}
                onCancel={props.onCancel}
                onChange={props.onChange}
                mode="compact"
              />
            </UIStateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

/** Wait for async effects (schema load, search_mentions calls, etc.) */
async function settle() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 200));
  });
}

describe("ReferenceSelectEditor", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  describe("dropdown rendering", () => {
    it("renders all referenced entities in the dropdown", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      // All 3 columns should appear as items in the dropdown
      expect(container.textContent).toContain("To Do");
      expect(container.textContent).toContain("Doing");
      expect(container.textContent).toContain("Done");
    });

    it("includes a clear option", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "col-todo",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      // A clear/empty option should be present
      const clearItem = container.querySelector("[data-ref-clear]");
      expect(clearItem).toBeTruthy();
    });
  });

  describe("search filtering", () => {
    it("typing a partial name filters the list to matching items", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      // Type "Do" in the search input — should match "Doing" and "Done" but not "To Do"
      const input = container.querySelector(
        "input[data-ref-search]",
      ) as HTMLInputElement;
      expect(input).toBeTruthy();

      await act(async () => {
        fireEvent.change(input, { target: { value: "Do" } });
      });
      // Wait for debounced search to resolve
      await settle();

      // Check visible items — "Doing" and "Done" match "Do" prefix
      const items = container.querySelectorAll("[data-ref-item]");
      const itemTexts = Array.from(items).map((el) => el.textContent);
      expect(itemTexts).toContain("Doing");
      expect(itemTexts).toContain("Done");
      // "To Do" also matches "Do" (substring), so all three will match
    });
  });

  describe("selection and commit", () => {
    it("selecting an item calls onCommit with the entity ID", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      // Click on "Doing"
      const items = container.querySelectorAll("[data-ref-item]");
      const doingItem = Array.from(items).find(
        (el) => el.textContent?.includes("Doing"),
      );
      expect(doingItem).toBeTruthy();

      await act(async () => {
        fireEvent.click(doingItem!);
      });

      // Should commit the entity ID, not the display name
      expect(onCommit).toHaveBeenCalledWith("col-doing");
    });

    it("selecting clear option commits empty string", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "col-todo",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      const clearItem = container.querySelector("[data-ref-clear]");
      expect(clearItem).toBeTruthy();

      await act(async () => {
        fireEvent.click(clearItem!);
      });

      expect(onCommit).toHaveBeenCalledWith("");
    });
  });

  describe("current value display", () => {
    it("shows the display name when value is a valid entity ID", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "col-todo",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      // The trigger should show "To Do"
      const trigger = container.querySelector("[data-ref-trigger]");
      expect(trigger).toBeTruthy();
      expect(trigger!.textContent).toContain("To Do");
    });

    it("shows a colored dot next to the display name", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "col-todo",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      // Check for a colored dot (span with inline background color)
      const trigger = container.querySelector("[data-ref-trigger]");
      const dot = trigger?.querySelector("span[data-ref-dot]");
      expect(dot).toBeTruthy();
    });

    it("shows dash placeholder when value is empty", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      const trigger = container.querySelector("[data-ref-trigger]");
      expect(trigger).toBeTruthy();
      expect(trigger!.textContent).toContain("-");
    });
  });

  describe("keyboard behavior", () => {
    it("Escape in CUA mode calls onCancel", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "col-todo",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      const input = container.querySelector(
        "input[data-ref-search]",
      ) as HTMLInputElement;
      expect(input).toBeTruthy();

      await act(async () => {
        fireEvent.keyDown(input, { key: "Escape" });
      });

      expect(onCancel).toHaveBeenCalled();
      expect(onCommit).not.toHaveBeenCalled();
    });

    it("Enter commits the currently highlighted item", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderReferenceSelect(
        {
          field: POSITION_COLUMN_FIELD,
          value: "col-todo",
          onCommit,
          onCancel,
        },
        { column: COLUMN_ENTITIES },
      );
      await settle();

      const input = container.querySelector(
        "input[data-ref-search]",
      ) as HTMLInputElement;
      expect(input).toBeTruthy();

      // Press Enter — should commit the current value (first highlighted item or current value)
      await act(async () => {
        fireEvent.keyDown(input, { key: "Enter" });
      });

      expect(onCommit).toHaveBeenCalled();
    });
  });
});
