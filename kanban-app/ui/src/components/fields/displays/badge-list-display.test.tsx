/**
 * Tests for BadgeListDisplay after the MentionView migration.
 *
 * BadgeListDisplay is now a thin adapter: it inspects the field type, builds
 * an `items` array of `{entityType, id | slug}` references, and delegates
 * rendering to `MentionView`. Pills are CM6 widget spans, not React-rendered
 * pill divs, so DOM assertions look for `.cm-mention-pill` elements rather
 * than `span.rounded-full`.
 */

import { describe, it, expect, vi } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — must be declared before importing the component under test
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ label: "main" })),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

const mockTags = [
  {
    id: "tag-1",
    entity_type: "tag",
    moniker: "tag:tag-1",
    fields: { tag_name: "bugfix", color: "ff0000" },
  },
  {
    id: "tag-2",
    entity_type: "tag",
    moniker: "tag:tag-2",
    fields: { tag_name: "feature", color: "00ff00" },
  },
];

const mockTasks = [
  {
    id: "task-dep-1",
    entity_type: "task",
    moniker: "task:task-dep-1",
    fields: { title: "Refactor login flow", color: "3366ff" },
  },
  {
    id: "task-dep-2",
    entity_type: "task",
    moniker: "task:task-dep-2",
    fields: { title: "Add password reset", color: "33ccff" },
  },
  {
    id: "task-dep-3",
    entity_type: "task",
    moniker: "task:task-dep-3",
    fields: { title: "Document auth module", color: "33ff66" },
  },
];

const mockEntitiesByType: Record<string, unknown[]> = {
  tag: mockTags,
  task: mockTasks,
};

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: (type: string) => mockEntitiesByType[type] ?? [],
    getEntity: vi.fn(),
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: [
      { entityType: "tag", prefix: "#", displayField: "tag_name" },
      { entityType: "task", prefix: "^", displayField: "title" },
    ],
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => null,
}));

// ---------------------------------------------------------------------------

import { BadgeListDisplay } from "./badge-list-display";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import type { Entity, FieldDef } from "@/types/kanban";

const tagField: FieldDef = {
  name: "tags",
  display: "badge-list",
  type: { entity: "tag", commit_display_names: true },
} as unknown as FieldDef;

const dependsOnField: FieldDef = {
  name: "depends_on",
  display: "badge-list",
  type: { entity: "task", kind: "reference", multiple: true },
} as unknown as FieldDef;

const taskEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: { tags: ["bugfix", "feature"] },
};

/** Wrap children in the providers MentionView requires. */
function Providers({ children }: { children: React.ReactNode }) {
  return (
    <EntityFocusProvider>
      <TooltipProvider>{children}</TooltipProvider>
    </EntityFocusProvider>
  );
}

function renderDisplay(
  overrides: {
    value?: unknown;
    field?: FieldDef;
    entity?: Entity;
    mode?: "full" | "compact";
  } = {},
) {
  return render(
    <Providers>
      <BadgeListDisplay
        field={overrides.field ?? tagField}
        value={overrides.value ?? ["bugfix", "feature"]}
        entity={overrides.entity ?? taskEntity}
        mode={overrides.mode ?? "full"}
      />
    </Providers>,
  );
}

/** Flush microtasks and pending effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

/** Get all FocusScope wrappers (elements with a data-moniker attribute). */
function getScopes(container: HTMLElement) {
  return Array.from(container.querySelectorAll("[data-moniker]"));
}

/** Get all CM6 mention-pill widget spans. */
function getPillWidgets(container: HTMLElement) {
  return Array.from(container.querySelectorAll(".cm-mention-pill"));
}

describe("BadgeListDisplay", () => {
  it("renders one pill per tag value via MentionView", async () => {
    const { container } = renderDisplay();
    await flush();

    const widgets = getPillWidgets(container);
    expect(widgets.length).toBe(2);
    const texts = widgets.map((w) => w.textContent ?? "");
    expect(texts).toContain("#bugfix");
    expect(texts).toContain("#feature");
  });

  it("renders one FocusScope per pill with tag monikers", async () => {
    const { container } = renderDisplay();
    await flush();

    const scopes = getScopes(container);
    expect(scopes.length).toBe(2);
    const monikers = scopes.map((s) => s.getAttribute("data-moniker"));
    expect(monikers).toContain("tag:tag-1");
    expect(monikers).toContain("tag:tag-2");
  });

  it("renders a dash in compact mode when values are empty", async () => {
    const { container } = renderDisplay({ value: [], mode: "compact" });
    await flush();

    expect(getScopes(container).length).toBe(0);
    expect(container.textContent).toContain("-");
  });

  it("renders italic None in full mode when values are empty", async () => {
    const { container } = renderDisplay({ value: [], mode: "full" });
    await flush();

    expect(getScopes(container).length).toBe(0);
    const none = container.querySelector("span.italic");
    expect(none).toBeTruthy();
    expect(none?.textContent).toBe("None");
  });

  it("renders the configured placeholder in full mode when values array is empty", async () => {
    const tagFieldWithPlaceholder: FieldDef = {
      ...tagField,
      placeholder: "Add tags",
    } as unknown as FieldDef;
    const { container } = renderDisplay({
      field: tagFieldWithPlaceholder,
      value: [],
      mode: "full",
    });
    await flush();

    expect(getScopes(container).length).toBe(0);
    const hint = container.querySelector("span.italic");
    expect(hint).toBeTruthy();
    // Keeps the muted/italic wrapper, swaps the hardcoded "None" text
    // for the YAML-configured placeholder string.
    expect(hint?.textContent).toBe("Add tags");
  });

  it("renders the configured placeholder in compact mode when values array is empty", async () => {
    const tagFieldWithPlaceholder: FieldDef = {
      ...tagField,
      placeholder: "Add tags",
    } as unknown as FieldDef;
    const { container } = renderDisplay({
      field: tagFieldWithPlaceholder,
      value: [],
      mode: "compact",
    });
    await flush();

    expect(getScopes(container).length).toBe(0);
    const hint = container.querySelector("span.text-muted-foreground\\/50");
    expect(hint).toBeTruthy();
    // Compact mode still uses the muted class, but the "-" fallback is
    // replaced by the YAML placeholder.
    expect(hint?.textContent).toBe("Add tags");
  });

  it("renders depends_on task IDs as CM6 pills with clipped display names", async () => {
    const dependsEntity: Entity = {
      id: "task-parent",
      entity_type: "task",
      moniker: "task:task-parent",
      fields: { depends_on: ["task-dep-1", "task-dep-2", "task-dep-3"] },
    };

    const { container } = renderDisplay({
      field: dependsOnField,
      value: ["task-dep-1", "task-dep-2", "task-dep-3"],
      entity: dependsEntity,
      mode: "full",
    });
    await flush();

    const widgets = getPillWidgets(container);
    expect(widgets.length).toBe(3);
    const texts = widgets.map((w) => w.textContent ?? "");
    // Each pill shows the task's title (prefixed by "^") — the CM6 widget
    // pipeline produces the display name, not the slug.
    expect(texts).toContain("^Refactor login flow");
    expect(texts).toContain("^Add password reset");
    expect(texts).toContain("^Document auth module");

    // And one FocusScope per item with a task moniker (derived from entity id).
    const scopes = getScopes(container);
    expect(scopes.length).toBe(3);
    const monikers = scopes.map((s) => s.getAttribute("data-moniker"));
    expect(monikers).toContain("task:task-dep-1");
    expect(monikers).toContain("task:task-dep-2");
    expect(monikers).toContain("task:task-dep-3");
  });
});
