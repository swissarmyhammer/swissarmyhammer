import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "get_entity_schema") {
    const entityType = args[1]?.entityType as string;
    return Promise.resolve(SCHEMAS[entityType] ?? DEFAULT_SCHEMA);
  }
  if (args[0] === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
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
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { EditorView } from "@codemirror/view";
import { MultiSelectEditor } from "./multi-select-editor";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { InspectProvider } from "@/lib/inspect-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity, FieldDef } from "@/types/kanban";

const TAG_SCHEMA = {
  entity: {
    name: "tag",
    fields: ["tag_name", "color"],
    mention_prefix: "#",
    mention_display_field: "tag_name",
  },
  fields: [
    { id: "t1", name: "tag_name", type: { kind: "text" }, section: "header" },
    { id: "t2", name: "color", type: { kind: "color" }, section: "body" },
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
    { id: "a1", name: "name", type: { kind: "text" }, section: "header" },
    { id: "a2", name: "color", type: { kind: "color" }, section: "body" },
  ],
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "body", "assignees", "tags"],
  },
  fields: [
    { id: "f1", name: "title", type: { kind: "text" }, section: "header" },
    { id: "f2", name: "body", type: { kind: "markdown" }, section: "body" },
    {
      id: "f5",
      name: "assignees",
      type: { kind: "reference", entity: "actor", multiple: true },
      section: "body",
    },
    {
      id: "f3",
      name: "tags",
      type: { kind: "computed", derive: "parse-body-tags" },
      section: "header",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  tag: TAG_SCHEMA,
  actor: ACTOR_SCHEMA,
  task: TASK_SCHEMA,
};
const DEFAULT_SCHEMA = { entity: { name: "unknown", fields: [] }, fields: [] };

const ACTOR_ENTITIES: Entity[] = [
  {
    entity_type: "actor",
    id: "alice-id",
    fields: { name: "alice", color: "3366cc" },
  },
  {
    entity_type: "actor",
    id: "bob-id",
    fields: { name: "bob", color: "cc3366" },
  },
];

const TAG_ENTITIES: Entity[] = [
  {
    entity_type: "tag",
    id: "tag-bug",
    fields: { tag_name: "bug", color: "ff0000" },
  },
  {
    entity_type: "tag",
    id: "tag-feat",
    fields: { tag_name: "feature", color: "00ff00" },
  },
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
              <UIStateProvider>
                <MultiSelectEditor
                  field={props.field}
                  value={props.value}
                  onCommit={props.onCommit}
                  onCancel={props.onCancel}
                  entity={props.entity}
                  mode="compact"
                />
              </UIStateProvider>
            </InspectProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

/** Get the CM6 EditorView from the rendered container using the official API. */
function getCmView(container: HTMLElement): EditorView {
  const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
  const view = EditorView.findFromDOM(cmEditor);
  if (!view) throw new Error("EditorView not found — CM6 did not initialize");
  return view;
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

    it("shows existing selections as prefixed tokens in the doc", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        { field: ASSIGNEES_FIELD, value: ["alice-id"], onCommit, onCancel },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      const view = getCmView(container);
      expect(view.state.doc.toString()).toContain("@alice");
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

    it("Escape in CUA mode calls onCancel (discard)", async () => {
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

      expect(onCancel).toHaveBeenCalled();
      expect(onCommit).not.toHaveBeenCalled();
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

      // Type "alice" into the CM6 editor via EditorView.findFromDOM
      const view = getCmView(container);
      await act(async () => {
        view.dispatch({ changes: { from: 0, to: 0, insert: "alice" } });
      });

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Enter" });
      });

      // Should resolve "alice" to "alice-id" and commit
      expect(onCommit).toHaveBeenCalledWith(["alice-id"]);
    });

    it("multiple selections appear as separate tokens in the doc", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        {
          field: ASSIGNEES_FIELD,
          value: ["alice-id", "bob-id"],
          onCommit,
          onCancel,
        },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      const view = getCmView(container);
      const doc = view.state.doc.toString();
      expect(doc).toContain("@alice");
      expect(doc).toContain("@bob");
    });

    it("deleting a token from the doc removes it from committed value", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        {
          field: ASSIGNEES_FIELD,
          value: ["alice-id", "bob-id"],
          onCommit,
          onCancel,
        },
        { actor: ACTOR_ENTITIES },
      );
      await settle();

      // Remove @alice from the doc, keep @bob
      const view = getCmView(container);
      const doc = view.state.doc.toString();
      const aliceStart = doc.indexOf("@alice");
      expect(aliceStart).toBeGreaterThanOrEqual(0);
      await act(async () => {
        view.dispatch({
          changes: { from: aliceStart, to: aliceStart + "@alice ".length },
        });
      });

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Enter" });
      });

      expect(onCommit).toHaveBeenCalledWith(["bob-id"]);
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
        {
          field: TAGS_FIELD,
          value: ["tag-bug"],
          onCommit,
          onCancel,
          entity: taskEntity,
        },
        { tag: TAG_ENTITIES },
      );
      await settle();

      expect(container.querySelector(".cm-editor")).toBeTruthy();
    });

    it("shows existing tag selections as colored pills", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        {
          field: TAGS_FIELD,
          value: ["tag-bug"],
          onCommit,
          onCancel,
          entity: taskEntity,
        },
        { tag: TAG_ENTITIES },
      );
      await settle();

      // Tag pills have inline style with color, not bg-muted
      expect(container.textContent).toContain("bug");
    });

    it("Escape in CUA mode calls onCancel (discard)", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        {
          field: TAGS_FIELD,
          value: ["tag-bug"],
          onCommit,
          onCancel,
          entity: taskEntity,
        },
        { tag: TAG_ENTITIES },
      );
      await settle();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Escape" });
      });

      expect(onCancel).toHaveBeenCalled();
      expect(onCommit).not.toHaveBeenCalled();
    });

    it("Enter commits tag slugs via onCommit", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        {
          field: TAGS_FIELD,
          value: ["tag-bug"],
          onCommit,
          onCancel,
          entity: taskEntity,
        },
        { tag: TAG_ENTITIES },
      );
      await settle();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Enter" });
      });

      expect(onCommit).toHaveBeenCalledWith(["bug"]);
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("deleting a tag token from the doc removes it from committed value", async () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { container } = renderMultiSelect(
        {
          field: TAGS_FIELD,
          value: ["tag-bug", "tag-feat"],
          onCommit,
          onCancel,
          entity: taskEntity,
        },
        { tag: TAG_ENTITIES },
      );
      await settle();

      // Doc should contain both tags
      const view = getCmView(container);
      const doc = view.state.doc.toString();
      expect(doc).toContain("#bug");
      expect(doc).toContain("#feature");

      // Remove #bug from the doc
      const bugStart = doc.indexOf("#bug");
      await act(async () => {
        view.dispatch({
          changes: { from: bugStart, to: bugStart + "#bug ".length },
        });
      });

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        fireEvent.keyDown(cmContent, { key: "Enter" });
      });

      // Only feature should remain
      expect(onCommit).toHaveBeenCalledWith(["feature"]);
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
