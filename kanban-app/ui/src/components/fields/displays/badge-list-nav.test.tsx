/**
 * Integration tests for pill navigation via claimWhen predicates.
 *
 * Uses the real EntityFocusProvider and FocusScope (no mocking of
 * entity-focus-context) so broadcastNavCommand actually evaluates
 * registered claim predicates.
 */
import { describe, it, expect, vi } from "vitest";
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
    fields: { tag_name: "bugfix", color: "ff0000" },
  },
  {
    id: "tag-2",
    entity_type: "tag",
    fields: { tag_name: "feature", color: "00ff00" },
  },
  {
    id: "tag-3",
    entity_type: "tag",
    fields: { tag_name: "docs", color: "0000ff" },
  },
];

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => mockTags, getEntity: vi.fn() }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: [
      { entityType: "tag", prefix: "#", displayField: "tag_name" },
    ],
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  }),
}));

// ---------------------------------------------------------------------------

import { BadgeListDisplay } from "./badge-list-display";
import { FocusScope } from "@/components/focus-scope";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { InspectProvider } from "@/lib/inspect-context";
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
  fields: { tags: ["bugfix", "feature", "docs"] },
};

const refEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  fields: { depends_on: ["task-dep-A", "task-dep-B"] },
};

/** Flush microtasks and pending effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

/** Reads focusedMoniker from context and renders it as text. */
function FocusMonitor() {
  const { focusedMoniker } = useEntityFocus();
  return <span data-testid="focus-monitor">{focusedMoniker ?? "null"}</span>;
}

/** Button to set focus imperatively. */
function SetFocusButton({ moniker }: { moniker: string }) {
  const { setFocus } = useEntityFocus();
  return <button data-testid="set-focus" onClick={() => setFocus(moniker)} />;
}

/** Button to call broadcastNavCommand. */
function BroadcastButton({
  commandId,
  testId,
}: {
  commandId: string;
  testId?: string;
}) {
  const { broadcastNavCommand } = useEntityFocus();
  return (
    <button
      data-testid={testId ?? "broadcast"}
      onClick={() => broadcastNavCommand(commandId)}
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
        <InspectProvider onInspect={() => {}} onDismiss={() => false}>
          <FocusScope moniker={parentMoniker} commands={[]}>
            <BadgeListDisplay
              field={tagField}
              value={values}
              entity={taskEntity}
              mode="full"
            />
          </FocusScope>
        </InspectProvider>
      </TooltipProvider>
      <FocusMonitor />
      <SetFocusButton moniker={parentMoniker} />
      <BroadcastButton commandId="nav.right" testId="nav-right" />
      <BroadcastButton commandId="nav.left" testId="nav-left" />
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
        <InspectProvider onInspect={() => {}} onDismiss={() => false}>
          <FocusScope moniker={parentMoniker} commands={[]}>
            <BadgeListDisplay
              field={refField}
              value={values}
              entity={refEntity}
              mode="full"
            />
          </FocusScope>
        </InspectProvider>
      </TooltipProvider>
      <FocusMonitor />
      <SetFocusButton moniker={parentMoniker} />
      <BroadcastButton commandId="nav.right" testId="nav-right" />
      <BroadcastButton commandId="nav.left" testId="nav-left" />
    </EntityFocusProvider>
  );
}

describe("BadgeListDisplay pill navigation", () => {
  it("nav.right from field moniker focuses first pill", async () => {
    const { getByTestId } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Focus the parent field
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("field:tags");

    // nav.right → first pill (tag:tag-1)
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-1",
    );
  });

  it("nav.right from first pill focuses second pill", async () => {
    const { getByTestId } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Focus the parent field, then nav.right twice
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-1",
    );

    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-2",
    );
  });

  it("nav.right from last pill leaves focus unchanged (clamp)", async () => {
    const { getByTestId } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Navigate to the last pill (tag:tag-3)
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-3",
    );

    // One more nav.right — should stay on last pill
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-3",
    );
  });

  it("nav.left from second pill focuses first pill", async () => {
    const { getByTestId } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Navigate to second pill
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-2",
    );

    // nav.left → first pill
    await act(async () => {
      getByTestId("nav-left").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-1",
    );
  });

  it("nav.left from first pill leaves focus unchanged (clamp)", async () => {
    const { getByTestId } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Navigate to first pill
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-1",
    );

    // nav.left — first pill has no nav.left predicate, so focus stays
    await act(async () => {
      getByTestId("nav-left").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "tag:tag-1",
    );
  });
});

describe("BadgeListDisplay reference-field pill navigation", () => {
  it("monikers use buildMoniker(entityType, entityId) directly — no slug resolution", async () => {
    const { getByTestId } = render(
      <RefNavHarness
        values={["task-dep-A", "task-dep-B"]}
        parentMoniker="field:depends_on"
      />,
    );
    await flush();

    // Focus the parent field
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("field:depends_on");

    // nav.right → first pill uses entity ID directly: task:task-dep-A
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "task:task-dep-A",
    );

    // nav.right → second pill: task:task-dep-B
    await act(async () => {
      getByTestId("nav-right").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "task:task-dep-B",
    );

    // nav.left → back to first pill
    await act(async () => {
      getByTestId("nav-left").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe(
      "task:task-dep-A",
    );
  });
});
