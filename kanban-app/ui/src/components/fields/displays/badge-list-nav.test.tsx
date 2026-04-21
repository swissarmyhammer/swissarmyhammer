/**
 * Integration tests for pill FocusScope registration.
 *
 * After the spatial nav migration, directional navigation between pills
 * (nav.left/nav.right) is handled by Rust via DOM rect geometry. These
 * tests verify that each pill renders a FocusScope with the correct
 * moniker so spatial nav can discover them, and that setFocus can target
 * individual pills.
 */
import { describe, it, expect, vi } from "vitest";
import type { ReactNode } from "react";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs and heavy dependencies that aren't relevant to nav
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
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
  {
    id: "tag-3",
    entity_type: "tag",
    moniker: "tag:tag-3",
    fields: { tag_name: "docs", color: "0000ff" },
  },
];

// Task store is empty — reference-field pills in RefNavHarness miss the
// store and fall back to the raw entity id, which is exactly what the
// nav tests need to verify (moniker = buildMoniker(entityType, rawId)).
const mockEntitiesByType: Record<string, unknown[]> = {
  tag: mockTags,
  task: [],
};

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: (type: string) => mockEntitiesByType[type] ?? [],
    getEntity: vi.fn(),
  }),
  // Passthrough provider — the test provides its own EntityFocusProvider,
  // but MentionView's upstream imports may transitively reference this.
  EntityStoreProvider: ({ children }: { children: ReactNode }) => children,
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

// window-container's useBoardData is referenced by cm mention extension
// hooks; MentionView itself doesn't use it, but a transitive import
// drags it in. Stub it so the module loads without a real Tauri engine.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => null,
}));

// ---------------------------------------------------------------------------

import { BadgeListDisplay } from "./badge-list-display";
import { FocusScope } from "@/components/focus-scope";
import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedMoniker,
} from "@/lib/entity-focus-context";
import { TooltipProvider } from "@/components/ui/tooltip";

import type { Entity, FieldDef } from "@/types/kanban";

const tagField: FieldDef = {
  name: "tags",
  display: "badge-list",
  type: { entity: "tag", commit_display_names: true },
} as unknown as FieldDef;

const refField: FieldDef = {
  name: "depends_on",
  display: "badge-list",
  type: { entity: "task" },
} as unknown as FieldDef;

const taskEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: { tags: ["bugfix", "feature", "docs"] },
};

const refEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: { depends_on: ["task-dep-A", "task-dep-B"] },
};

/** Flush microtasks and pending effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

/** Reads the focused moniker from the focus store and renders it as text. */
function FocusMonitor() {
  const focusedMoniker = useFocusedMoniker();
  return <span data-testid="focus-monitor">{focusedMoniker ?? "null"}</span>;
}

/** Button to set focus imperatively. */
function SetFocusButton({ moniker }: { moniker: string }) {
  const { setFocus } = useEntityFocus();
  return (
    <button
      data-testid={`set-focus-${moniker}`}
      onClick={() => setFocus(moniker)}
    />
  );
}

/**
 * Full provider tree with a parent FocusScope (simulating a field row)
 * containing BadgeListDisplay.
 */
function NavHarness({
  values,
  parentMoniker,
}: {
  values: string[];
  parentMoniker: string;
}) {
  return (
    <EntityFocusProvider>
      <TooltipProvider>
        <FocusScope moniker={parentMoniker} commands={[]}>
          <BadgeListDisplay
            field={tagField}
            value={values}
            entity={taskEntity}
            mode="full"
          />
        </FocusScope>
      </TooltipProvider>
      <FocusMonitor />
      <SetFocusButton moniker={parentMoniker} />
      <SetFocusButton moniker="tag:tag-1" />
      <SetFocusButton moniker="tag:tag-2" />
      <SetFocusButton moniker="tag:tag-3" />
    </EntityFocusProvider>
  );
}

/**
 * Harness for reference-field navigation (entity ID values, no slug resolution).
 */
function RefNavHarness({
  values,
  parentMoniker,
}: {
  values: string[];
  parentMoniker: string;
}) {
  return (
    <EntityFocusProvider>
      <TooltipProvider>
        <FocusScope moniker={parentMoniker} commands={[]}>
          <BadgeListDisplay
            field={refField}
            value={values}
            entity={refEntity}
            mode="full"
          />
        </FocusScope>
      </TooltipProvider>
      <FocusMonitor />
      <SetFocusButton moniker={parentMoniker} />
      <SetFocusButton moniker="task:task-dep-A" />
      <SetFocusButton moniker="task:task-dep-B" />
    </EntityFocusProvider>
  );
}

describe("BadgeListDisplay pill FocusScope registration", () => {
  it("each pill renders a FocusScope with the correct tag moniker", async () => {
    const { container } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    const scopes = Array.from(container.querySelectorAll("[data-moniker]"));
    const monikers = scopes.map((s) => s.getAttribute("data-moniker"));
    expect(monikers).toContain("tag:tag-1");
    expect(monikers).toContain("tag:tag-2");
    expect(monikers).toContain("tag:tag-3");
  });

  it("setFocus can target individual pills directly", async () => {
    const { getByTestId } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Focus first pill directly
    await act(async () => {
      getByTestId("set-focus-tag:tag-1").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("tag:tag-1");

    // Focus second pill directly
    await act(async () => {
      getByTestId("set-focus-tag:tag-2").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("tag:tag-2");

    // Focus third pill directly
    await act(async () => {
      getByTestId("set-focus-tag:tag-3").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("tag:tag-3");
  });

  it("parent field and pills have distinct monikers", async () => {
    const { getByTestId } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Focus parent
    await act(async () => {
      getByTestId("set-focus-field:tags").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("field:tags");

    // Focus a pill — moves away from parent
    await act(async () => {
      getByTestId("set-focus-tag:tag-1").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("tag:tag-1");
  });
});

describe("BadgeListDisplay reference-field pill registration", () => {
  it("reference pills use entity ID monikers directly (no slug resolution)", async () => {
    const { container, getByTestId } = render(
      <RefNavHarness
        values={["task-dep-A", "task-dep-B"]}
        parentMoniker="field:depends_on"
      />,
    );
    await flush();

    // Verify monikers are based on entity IDs, not slugs
    const scopes = Array.from(container.querySelectorAll("[data-moniker]"));
    const monikers = scopes.map((s) => s.getAttribute("data-moniker"));
    expect(monikers).toContain("task:task-dep-A");
    expect(monikers).toContain("task:task-dep-B");

    // setFocus can target each pill
    await act(async () => {
      getByTestId("set-focus-task:task-dep-A").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("task:task-dep-A");

    await act(async () => {
      getByTestId("set-focus-task:task-dep-B").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("task:task-dep-B");
  });
});
