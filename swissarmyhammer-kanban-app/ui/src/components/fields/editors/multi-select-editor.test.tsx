import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "get_entity_schema") {
    const entityType = args[1]?.entityType as string;
    return Promise.resolve(SCHEMAS[entityType] ?? DEFAULT_SCHEMA);
  }
  if (args[0] === "get_keymap_mode") return Promise.resolve("cua");
  if (args[0] === "search_mentions") return Promise.resolve([]);
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
  error: vi.fn(), warn: vi.fn(), info: vi.fn(), debug: vi.fn(), trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { MultiSelectEditor } from "./multi-select-editor";
import { KeymapProvider } from "@/lib/keymap-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { InspectProvider } from "@/lib/inspect-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity, FieldDef } from "@/types/kanban";

const TAG_SCHEMA = {
  entity: { name: "tag", fields: ["tag_name", "color"], mention_prefix: "#", mention_display_field: "tag_name" },
  fields: [
    { id: "t1", name: "tag_name", type: { kind: "text" }, section: "header" },
    { id: "t2", name: "color", type: { kind: "color" }, section: "body" },
  ],
};

const ACTOR_SCHEMA = {
  entity: { name: "actor", fields: ["name", "color"], mention_prefix: "@", mention_display_field: "name" },
  fields: [
    { id: "a1", name: "name", type: { kind: "text" }, section: "header" },
    { id: "a2", name: "color", type: { kind: "color" }, section: "body" },
  ],
};

const TASK_SCHEMA = {
  entity: { name: "task", body_field: "body", fields: ["title", "body", "assignees", "tags"] },
  fields: [
    { id: "f1", name: "title", type: { kind: "text" }, section: "header" },
    { id: "f2", name: "body", type: { kind: "markdown" }, section: "body" },
    { id: "f5", name: "assignees", type: { kind: "reference", entity: "actor", multiple: true }, section: "body" },
    { id: "f3", name: "tags", type: { kind: "computed", derive: "parse-body-tags" }, section: "header" },
  ],
};

const SCHEMAS: Record<string, unknown> = { tag: TAG_SCHEMA, actor: ACTOR_SCHEMA, task: TASK_SCHEMA };
const DEFAULT_SCHEMA = { entity: { name: "unknown", fields: [] }, fields: [] };

const ACTOR_ENTITIES: Entity[] = [
  { entity_type: "actor", id: "alice-id", fields: { name: "alice", color: "3366cc" } },
  { entity_type: "actor", id: "bob-id", fields: { name: "bob", color: "cc3366" } },
];

const TAG_ENTITIES: Entity[] = [
  { entity_type: "tag", id: "tag-bug", fields: { tag_name: "bug", color: "ff0000" } },
  { entity_type: "tag", id: "tag-feat", fields: { tag_name: "feature", color: "00ff00" } },
];

const ASSIGNEES_FIELD: FieldDef = {
  id: "f5",
  name: "assignees",
  type: { kind: "reference", entity: "actor", multiple: true },
  section: "body",
};

const TAGS_FIELD: FieldDef = {
  id: "f3",
  name: "tags",
  type: { kind: "computed", derive: "parse-body-tags" },
  section: "header",
};

function renderMultiSelect(
  props: {
    field: FieldDef;
    value: unknown;
    onCommit: (val: unknown) => void;
    onCancel: () => void;
    entity?: Entity;
  },
  entities: Record<string, Entity[]> = {},
) {
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={entities}>
          <EntityFocusProvider>
            <InspectProvider onInspect={() => {}} onDismiss={() => false}>
              <KeymapProvider>
                <MultiSelectEditor
                  field={props.field}
                  value={props.value}
                  onCommit={props.onCommit}
                  onCancel={props.onCancel}
                  entity={props.entity}
                  mode="compact"
                />
              </KeymapProvider>
            </InspectProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

/** Helper to get the CM6 EditorView from the rendered container. */
function getCmView(container: HTMLElement) {
  const cmContent = container.querySelector(".cm-content") as HTMLElement | null;
  if (!cmContent) return null;
  // @uiw/react-codemirror stores the view on the root .cm-editor element
  const cmEditor = container.querySelector(".cm-editor") as HTMLElement | null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (cmEditor as any)?.cmView?.view ?? null;
}

/** Wait for async effects (schema load, focus, etc.) */
async function settle() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

describe("MultiSelectEditor", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  describe("reference field (assignees)", () => {
    it("renders a CM6 editor", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: [], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      expect(container.querySelector(".cm-editor")).toBeTruthy();
    });

    it("shows existing selections as pills", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: ["alice-id"], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      // Should show alice's name
      expect(container.textContent).toContain("alice");
    });

    it("Enter key calls onCommit with selected IDs", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: ["alice-id"], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      expect(cmContent).toBeTruthy();

      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Enter" });
      });

      expect(onCommit).toHaveBeenCalledWith(["alice-id"]);
    });

    it("Escape key calls onCommit with selected IDs (commit, not discard)", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: ["bob-id"], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Escape" });
      });

      expect(onCommit).toHaveBeenCalledWith(["bob-id"]);
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("blur calls onCommit after timeout", async () => {
      vi.useFakeTimers();
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: ["alice-id"], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      // Settle with real timers briefly, then switch to fake
      vi.useRealTimers();
      await settle();
      vi.useFakeTimers();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.blur(cmContent);
      });

      // Blur uses setTimeout(100) before committing
      await act(async () => {
        vi.advanceTimersByTime(150);
      });

      expect(onCommit).toHaveBeenCalledWith(["alice-id"]);
      vi.useRealTimers();
    });

    it("resolves typed text in editor to entity ID on commit", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: [], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      // Type "alice" into the CM6 editor
      const view = getCmView(container);
      if (view) {
        await act(async () => {
          view.dispatch({ changes: { from: 0, to: 0, insert: "alice" } });
        });

        const cmContent = container.querySelector(".cm-content") as HTMLElement;
        await act(async () => {
          fireEvent.keyDown(cmContent, { key: "Enter" });
        });

        // Should resolve "alice" to "alice-id" and commit
        expect(onCommit).toHaveBeenCalledWith(["alice-id"]);
      }
    });

    it("actor selections render with Avatar component", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: ["alice-id"], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      // Actor pills should have the bg-muted class (actor-specific rendering)
      const pill = container.querySelector(".bg-muted");
      expect(pill).toBeTruthy();
      expect(pill!.textContent).toContain("alice");
    });

    it("remove button removes item from selection", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: ["alice-id", "bob-id"], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      // Find the × button for alice
      const pills = container.querySelectorAll(".bg-muted");
      expect(pills.length).toBe(2);

      const removeBtn = pills[0].querySelector("button");
      expect(removeBtn).toBeTruthy();

      await act(async () => {
        fireEvent.click(removeBtn!);
      });

      // After removing alice, only bob should remain
      const remainingPills = container.querySelectorAll(".bg-muted");
      expect(remainingPills.length).toBe(1);
      expect(remainingPills[0].textContent).toContain("bob");
    });
  });

  describe("computed tag field", () => {
    const taskEntity: Entity = {
      entity_type: "task",
      id: "task-1",
      fields: { title: "Test task", body: "Fix #bug issue" },
    };

    it("renders a CM6 editor for tags", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: TAGS_FIELD, value: ["tag-bug"], onCommit, onCancel, entity: taskEntity },
        { tag: TAG_ENTITIES },
      );
      await settle();

      expect(container.querySelector(".cm-editor")).toBeTruthy();
    });

    it("shows existing tag selections as colored pills", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: TAGS_FIELD, value: ["tag-bug"], onCommit, onCancel, entity: taskEntity },
        { tag: TAG_ENTITIES },
      );
      await settle();

      // Tag pills have inline style with color, not bg-muted
      expect(container.textContent).toContain("bug");
    });

    it("Escape calls onCancel for tags (already committed via body updates)", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: TAGS_FIELD, value: ["tag-bug"], onCommit, onCancel, entity: taskEntity },
        { tag: TAG_ENTITIES },
      );
      await settle();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Escape" });
      });

      // Tags commit via body updates, so on close we call onCancel (not onCommit)
      expect(onCancel).toHaveBeenCalled();
      expect(onCommit).not.toHaveBeenCalled();
    });

    it("Enter calls onCancel for tags (already committed via body updates)", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: TAGS_FIELD, value: ["tag-bug"], onCommit, onCancel, entity: taskEntity },
        { tag: TAG_ENTITIES },
      );
      await settle();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Enter" });
      });

      expect(onCancel).toHaveBeenCalled();
      expect(onCommit).not.toHaveBeenCalled();
    });

    it("remove button dispatches body update to remove tag", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: TAGS_FIELD, value: ["tag-bug"], onCommit, onCancel, entity: taskEntity },
        { tag: TAG_ENTITIES },
      );
      await settle();
      mockInvoke.mockClear();

      // Find and click the remove button
      const removeBtn = container.querySelector("button");
      expect(removeBtn).toBeTruthy();

      await act(async () => {
        fireEvent.click(removeBtn!);
      });

      // Should dispatch body update removing #bug
      const call = mockInvoke.mock.calls.find(
        (c) => c[0] === "dispatch_command" && (c[1] as Record<string, unknown>)?.cmd === "entity.update_field",
      );
      expect(call).toBeTruthy();
      const args = (call![1] as Record<string, unknown>).args as Record<string, unknown>;
      expect(args.field_name).toBe("body");
      // Body should have #bug removed
      const newBody = args.value as string;
      expect(newBody).not.toContain("#bug");
    });
  });

  describe("empty state", () => {
    it("renders placeholder text", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: [], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      // Placeholder should mention the prefix
      const placeholder = container.querySelector(".cm-placeholder");
      expect(placeholder).toBeTruthy();
    });

    it("Enter on empty editor commits empty array for reference fields", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: [], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Enter" });
      });

      expect(onCommit).toHaveBeenCalledWith([]);
    });
  });
});
