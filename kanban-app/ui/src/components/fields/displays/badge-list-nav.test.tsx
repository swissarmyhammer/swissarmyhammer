/**
 * Structural tests for pill navigation after the spatial-nav zone migration.
 *
 * Before the migration, pill nav was driven by `claimWhen` predicates that
 * the pills registered with the entity-focus context. After the migration,
 * within-field pill navigation flows from beam-search rule 1 in the Rust
 * spatial graph (the field row is a `<FocusScope kind="zone">` and the
 * pills are leaves inside it). The actual navigation is therefore driven
 * by the spatial focus state on the Rust side; these tests verify the
 * React-side structural surface that the navigator relies on.
 *
 * Concretely:
 *   - Each pill renders as its own `FocusScope` whose `data-moniker`
 *     follows `${entityType}:${entityId}` (computed-tag fields) or
 *     `${entityType}:${rawId}` (reference fields with no slug
 *     resolution).
 *   - The pills are descendants of the parent field row scope, so the
 *     spatial-nav graph treats them as in-zone candidates of that zone.
 *   - No `claimWhen` predicates remain on the pills.
 *
 * Cross-zone behaviour (rule 2) is exercised by the Rust spatial-nav
 * unit tests, not here.
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
// store and fall back to the raw entity id, which is exactly what these
// tests need to verify (moniker = buildMoniker(entityType, rawId)).
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
    mentionableTypes: [
      { entityType: "tag", prefix: "#", displayField: "tag_name" },
      { entityType: "task", prefix: "^", displayField: "title" },
    ],
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
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
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asMoniker } from "@/types/spatial";

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

/**
 * Full provider tree with a parent FocusScope (simulating a field row)
 * containing BadgeListDisplay. The parent is `kind="zone"` to mirror the
 * real `FieldRow`, which registers as a navigable zone in the spatial-nav
 * graph.
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
        <FocusScope
          moniker={asMoniker(parentMoniker)}
          kind="zone"
          commands={[]}
        >
          <BadgeListDisplay
            field={tagField}
            value={values}
            entity={taskEntity}
            mode="full"
          />
        </FocusScope>
      </TooltipProvider>
    </EntityFocusProvider>
  );
}

/**
 * Harness for reference-field navigation (entity ID values, no slug resolution).
 * The parent is `kind="zone"` to mirror the real `FieldRow`.
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
        <FocusScope
          moniker={asMoniker(parentMoniker)}
          kind="zone"
          commands={[]}
        >
          <BadgeListDisplay
            field={refField}
            value={values}
            entity={refEntity}
            mode="full"
          />
        </FocusScope>
      </TooltipProvider>
    </EntityFocusProvider>
  );
}

describe("BadgeListDisplay pill scope structure", () => {
  it("each tag pill renders as a FocusScope nested inside the parent field row", async () => {
    const { container } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Parent field row scope
    const fieldRow = container.querySelector('[data-moniker="field:tags"]');
    expect(fieldRow).toBeTruthy();

    // Each pill is its own FocusScope nested inside the field row.
    const pills = fieldRow!.querySelectorAll(
      '[data-moniker^="tag:"]',
    ) as NodeListOf<HTMLElement>;
    expect(pills.length).toBe(3);
    const monikers = Array.from(pills).map((p) =>
      p.getAttribute("data-moniker"),
    );
    expect(monikers).toEqual(["tag:tag-1", "tag:tag-2", "tag:tag-3"]);
  });

  it("renders the expected scope tree: parent field row plus one scope per pill", async () => {
    // Smoke check: rendering the harness produces the parent field-row
    // scope and exactly N pill scopes nested under it. This is the
    // structural shape the spatial navigator relies on (zone + leaves).
    const { container } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    expect(container.querySelectorAll("[data-moniker]").length).toBe(4); // field row + 3 pills
  });
});

describe("BadgeListDisplay reference-field pill structure", () => {
  it("monikers use buildMoniker(entityType, entityId) directly — no slug resolution", async () => {
    const { container } = render(
      <RefNavHarness
        values={["task-dep-A", "task-dep-B"]}
        parentMoniker="field:depends_on"
      />,
    );
    await flush();

    // The parent field row scope is present.
    const fieldRow = container.querySelector(
      '[data-moniker="field:depends_on"]',
    );
    expect(fieldRow).toBeTruthy();

    // Two pills, each carrying the raw task id as its moniker tail.
    const pills = fieldRow!.querySelectorAll(
      '[data-moniker^="task:"]',
    ) as NodeListOf<HTMLElement>;
    expect(pills.length).toBe(2);
    const monikers = Array.from(pills).map((p) =>
      p.getAttribute("data-moniker"),
    );
    expect(monikers).toEqual(["task:task-dep-A", "task:task-dep-B"]);
  });
});
