import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — must be declared before importing the component under test
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mutable references so each test can swap the entity-store and schema state.
let mockEntities: Record<string, unknown[]> = {};
let mockMentionableTypes: Array<{
  entityType: string;
  prefix: string;
  displayField: string;
}> = [];

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: (type: string) => mockEntities[type] ?? [],
    getEntity: () => undefined,
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: mockMentionableTypes,
    loading: false,
  }),
}));

// ---------------------------------------------------------------------------

import { BadgeDisplay } from "./badge-display";
import type { Entity, FieldDef } from "@/types/kanban";

const taskEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: {},
};

/** Helper: locate the rendered rounded-full badge span. */
function getBadge(container: HTMLElement): HTMLElement | null {
  return container.querySelector("span.rounded-full") as HTMLElement | null;
}

describe("BadgeDisplay", () => {
  describe("reference field resolution", () => {
    it("renders the target entity's display name and color when matched", () => {
      mockEntities = {
        project: [
          {
            id: "spatial-nav",
            entity_type: "project",
            moniker: "project:spatial-nav",
            fields: {
              name: "Spatial Focus Navigation",
              color: "6366f1",
            },
          },
        ],
      };
      mockMentionableTypes = [
        { entityType: "project", prefix: "$", displayField: "name" },
      ];

      const field: FieldDef = {
        id: "00000000000000000000000010",
        name: "project",
        type: { kind: "reference", entity: "project", multiple: false },
      } as unknown as FieldDef;

      const { container } = render(
        <BadgeDisplay
          field={field}
          value="spatial-nav"
          entity={taskEntity}
          mode="full"
        />,
      );

      const badge = getBadge(container);
      expect(badge).not.toBeNull();
      expect(badge!.textContent).toContain("Spatial Focus Navigation");
      expect(badge!.textContent).not.toContain("spatial-nav");
      // Inline style mirrors the select-options color path:
      // backgroundColor `#<hex>20` (browser normalises to rgba with alpha)
      // and color `#<hex>` (browser normalises to rgb).
      expect(badge!.style.backgroundColor).toBe("rgba(99, 102, 241, 0.125)");
      expect(badge!.style.color).toBe("rgb(99, 102, 241)");
    });

    it("falls back to the raw value when the target entity is missing", () => {
      mockEntities = { project: [] };
      mockMentionableTypes = [
        { entityType: "project", prefix: "$", displayField: "name" },
      ];

      const field: FieldDef = {
        id: "00000000000000000000000010",
        name: "project",
        type: { kind: "reference", entity: "project", multiple: false },
      } as unknown as FieldDef;

      const { container } = render(
        <BadgeDisplay
          field={field}
          value="spatial-nav"
          entity={taskEntity}
          mode="full"
        />,
      );

      const badge = getBadge(container);
      expect(badge).not.toBeNull();
      expect(badge!.textContent).toContain("spatial-nav");
    });
  });

  describe("select-options field", () => {
    it("renders the option label and color from field.type.options", () => {
      mockEntities = {};
      mockMentionableTypes = [];

      const field: FieldDef = {
        id: "00000000000000000000000099",
        name: "status",
        type: {
          kind: "select",
          options: [
            { value: "todo", label: "To Do", color: "0066ff", order: 0 },
            { value: "done", label: "Done", color: "00aa00", order: 1 },
          ],
        },
      } as unknown as FieldDef;

      const { container } = render(
        <BadgeDisplay
          field={field}
          value="todo"
          entity={taskEntity}
          mode="full"
        />,
      );

      const badge = getBadge(container);
      expect(badge).not.toBeNull();
      expect(badge!.textContent).toContain("To Do");
      expect(badge!.style.backgroundColor).toBe("rgba(0, 102, 255, 0.125)");
      expect(badge!.style.color).toBe("rgb(0, 102, 255)");
    });
  });
});
