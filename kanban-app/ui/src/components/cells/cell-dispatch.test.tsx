/**
 * Tests that CellDispatch renders the correct display component for each field type.
 * Since the inspector now uses CellDispatch too, this is THE single rendering path test.
 */
import { describe, it, expect, vi } from "vitest";
import { render, act } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "get_entity_schema") {
      const t = args?.entityType as string;
      return Promise.resolve(SCHEMAS[t] ?? DEFAULT_SCHEMA);
    }
    if (cmd === "get_keymap_mode") return Promise.resolve("cua");
    return Promise.resolve(null);
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(), warn: vi.fn(), info: vi.fn(), debug: vi.fn(), trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { CellDispatch } from "./cell-dispatch";
import { EntityInspector } from "@/components/entity-inspector";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { InspectProvider } from "@/lib/inspect-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { KeymapProvider } from "@/lib/keymap-context";
import { SchemaProvider } from "@/lib/schema-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity, FieldDef } from "@/types/kanban";

const ACTOR_SCHEMA = {
  entity: { name: "actor", fields: ["name", "color", "avatar"], mention_prefix: "@", mention_display_field: "name" },
  fields: [
    { id: "a1", name: "name", type: { kind: "text" }, section: "header" },
    { id: "a2", name: "color", type: { kind: "color" }, section: "body" },
  ],
};
const TAG_SCHEMA = {
  entity: { name: "tag", fields: ["tag_name", "color"], mention_prefix: "#", mention_display_field: "tag_name" },
  fields: [
    { id: "t1", name: "tag_name", type: { kind: "text" }, section: "header" },
    { id: "t2", name: "color", type: { kind: "color" }, section: "body" },
  ],
};
const TASK_SCHEMA = {
  entity: { name: "task", body_field: "body", fields: ["title", "tags", "body", "assignees"] },
  fields: [
    { id: "f1", name: "title", type: { kind: "markdown", single_line: true }, section: "header" },
    { id: "f3", name: "tags", type: { kind: "computed", derive: "parse-body-tags" }, display: "badge-list", section: "header" },
    { id: "f2", name: "body", type: { kind: "markdown", single_line: false }, section: "body" },
    { id: "f5", name: "assignees", type: { kind: "reference", entity: "actor", multiple: true }, display: "avatar", editor: "multi-select", section: "body" },
  ],
};
const SCHEMAS: Record<string, unknown> = { actor: ACTOR_SCHEMA, tag: TAG_SCHEMA, task: TASK_SCHEMA };
const DEFAULT_SCHEMA = { entity: { name: "unknown", fields: [] }, fields: [] };

const ACTORS: Entity[] = [
  { entity_type: "actor", id: "alice-id", fields: { name: "Alice Smith", color: "3366cc" } },
  { entity_type: "actor", id: "bob-id", fields: { name: "Bob Jones", color: "cc3366" } },
];

const TAGS: Entity[] = [
  { entity_type: "tag", id: "tag-bug", fields: { tag_name: "bug", color: "ff0000" } },
];

const TASK: Entity = {
  entity_type: "task",
  id: "task-1",
  fields: { title: "Test", assignees: ["alice-id", "bob-id"], tags: ["tag-bug"] },
};

const ASSIGNEES_FIELD: FieldDef = {
  id: "f5",
  name: "assignees",
  type: { kind: "reference", entity: "actor", multiple: true },
  display: "avatar",
  editor: "multi-select",
  section: "body",
};

const TAGS_FIELD: FieldDef = {
  id: "f3",
  name: "tags",
  type: { kind: "computed", derive: "parse-body-tags" },
  display: "badge-list",
  section: "header",
};

function renderCell(field: FieldDef, value: unknown, entities: Record<string, Entity[]> = {}) {
  return render(
    <TooltipProvider>
      <EntityStoreProvider entities={entities}>
        <EntityFocusProvider>
          <InspectProvider onInspect={() => {}} onDismiss={() => false}>
            <CellDispatch field={field} value={value} entity={TASK} />
          </InspectProvider>
        </EntityFocusProvider>
      </EntityStoreProvider>
    </TooltipProvider>,
  );
}

describe("CellDispatch", () => {
  describe("avatar display (assignees)", () => {
    it("renders Avatar circles for actor IDs", () => {
      const { container } = renderCell(ASSIGNEES_FIELD, ["alice-id", "bob-id"], { actor: ACTORS });

      // Should have exactly 2 avatar elements (rounded-full spans with initials)
      const avatars = container.querySelectorAll(".rounded-full");
      expect(avatars.length).toBeGreaterThanOrEqual(2);

      // Should show initials
      const text = container.textContent;
      expect(text).toContain("AS"); // Alice Smith
      expect(text).toContain("BJ"); // Bob Jones
    });

    it("all avatars use the same size class (md = w-7 h-7)", () => {
      const { container } = renderCell(ASSIGNEES_FIELD, ["alice-id", "bob-id"], { actor: ACTORS });

      const avatars = container.querySelectorAll(".rounded-full");
      for (const avatar of avatars) {
        expect(avatar.className).toContain("w-7");
        expect(avatar.className).toContain("h-7");
      }
    });

    it("avatars overlap with negative margin", () => {
      const { container } = renderCell(ASSIGNEES_FIELD, ["alice-id", "bob-id"], { actor: ACTORS });

      // Second avatar should have -ml-1.5 for overlap
      const avatars = container.querySelectorAll(".rounded-full");
      // The FocusScope wraps each avatar, so look for -ml-1.5 on the avatar element itself
      const hasOverlap = Array.from(avatars).some((a: Element) => a.className.includes("-ml-1.5"));
      expect(hasOverlap).toBe(true);
    });

    it("empty assignees shows dash", () => {
      const { container } = renderCell(ASSIGNEES_FIELD, [], { actor: ACTORS });
      expect(container.textContent).toBe("-");
    });

    it("single assignee has no overlap class", () => {
      const { container } = renderCell(ASSIGNEES_FIELD, ["alice-id"], { actor: ACTORS });

      const avatars = container.querySelectorAll(".rounded-full");
      // First (and only) avatar should NOT have -ml-1.5
      for (const avatar of avatars) {
        expect(avatar.className).not.toContain("-ml-1.5");
      }
    });
  });

  describe("badge-list display (tags)", () => {
    it("renders TagPill components for tag IDs", () => {
      const { container } = renderCell(TAGS_FIELD, ["tag-bug"], { tag: TAGS });
      expect(container.textContent).toContain("bug");
      expect(container.querySelector(".rounded-full")).toBeTruthy();
    });

    it("empty tags shows dash", () => {
      const { container } = renderCell(TAGS_FIELD, [], { tag: TAGS });
      expect(container.textContent).toBe("-");
    });
  });

  describe("text display fallback", () => {
    const TEXT_FIELD: FieldDef = {
      id: "f1",
      name: "title",
      type: { kind: "text", single_line: true },
      section: "header",
    };

    it("renders plain text for text fields", () => {
      const { container } = renderCell(TEXT_FIELD, "Hello world");
      expect(container.textContent).toBe("Hello world");
    });

    it("empty text shows dash", () => {
      const { container } = renderCell(TEXT_FIELD, "");
      expect(container.textContent).toBe("-");
    });
  });

  describe("grid vs inspector parity", () => {
    /**
     * Render assignees through the inspector (EntityInspector) and through
     * CellDispatch (grid path), then compare the avatar HTML. The inspector
     * wraps CellDispatch in a label div, but the avatar elements inside
     * must be identical.
     */
    it("assignees render identically in grid and inspector", async () => {
      const entity: Entity = {
        entity_type: "task",
        id: "test-id",
        fields: { title: "Test", assignees: ["alice-id", "bob-id"] },
      };

      // 1. Render via CellDispatch (grid path)
      const grid = renderCell(ASSIGNEES_FIELD, ["alice-id", "bob-id"], { actor: ACTORS });

      // 2. Render via EntityInspector (inspector path)
      const inspector = render(
        <TooltipProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ actor: ACTORS }}>
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
        </TooltipProvider>,
      );
      await act(async () => { await new Promise((r) => setTimeout(r, 50)); });

      // Extract avatar elements from both
      const gridAvatars = grid.container.querySelectorAll(".rounded-full");
      const inspectorRow = inspector.container.querySelector('[data-testid="field-row-assignees"]');
      expect(inspectorRow).toBeTruthy();
      const inspectorAvatars = inspectorRow!.querySelectorAll(".rounded-full");

      // Same number of avatars
      expect(gridAvatars.length).toBe(inspectorAvatars.length);
      expect(gridAvatars.length).toBeGreaterThanOrEqual(2);

      // Each avatar has identical classes and content
      for (let i = 0; i < gridAvatars.length; i++) {
        const g = gridAvatars[i];
        const ins = inspectorAvatars[i];
        expect(g.className, `Avatar ${i} classes differ`).toBe(ins.className);
        expect(g.textContent, `Avatar ${i} text differs`).toBe(ins.textContent);
        expect(g.tagName, `Avatar ${i} tag differs`).toBe(ins.tagName);
        // Compare inline styles (background color, etc.)
        expect(
          (g as HTMLElement).style.cssText,
          `Avatar ${i} style differs`,
        ).toBe((ins as HTMLElement).style.cssText);
      }

      inspector.unmount();
    });
  });
});
