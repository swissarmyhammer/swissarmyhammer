/**
 * Production-path test for the `group.toggleCollapse` command (vim `z o`).
 *
 * `group.toggleCollapse` is DEFINED by the `board-commands` builtin plugin
 * (`builtin/plugins/board-commands/index.ts`) with `keys: { vim: "z o" }` and
 * `scope: ["ui:board"]`; its live BEHAVIOR is a webview-bus handler each
 * `<GroupSection>` registers via `useFocusedWebviewCommandHandlers` — so the
 * handler is live ONLY while spatial focus is within that group's subtree.
 * Dispatching the command therefore flips the collapse state of exactly the
 * group section that currently holds focus.
 *
 * This test exercises the real wiring end-to-end with no mock at the seam
 * under test:
 *
 *   1. The real `<GroupedBoardView>` renders the grouped board with one
 *      `group:<value>` `<FocusScope>` per bucket (the collapse state lives in
 *      `<GroupedBoardBody>`, keyed by `bucket.value`, starting collapsed).
 *   2. Focus is committed inside ONE group's subtree through the real focus
 *      store (`store.set(fq)` — the exact write the kernel `focus-changed`
 *      bridge performs). That lights the focused group's bus handler via
 *      `useFocusedWebviewCommandHandlers`.
 *   3. `useDispatchCommand("group.toggleCollapse")` runs — the real dispatcher
 *      consults the webview command bus first, finds the focused group's
 *      handler, and runs it (no backend call).
 *   4. The focused group expands (its `<BoardView>` body mounts) while every
 *      other group stays collapsed — proving the dispatch hit exactly the
 *      focused group.
 *
 * Browser project (real Chromium) so `<FocusScope>` registers real rects and
 * the focus store behaves as in production.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { useEffect } from "react";
import type { BoardData, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// Grouping is active: group tasks by their `project` field.
vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: "project",
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: (type: string) => (type === "task" ? { fields: [] } : undefined),
    getFieldDef: () => undefined,
    loading: false,
    mentionableTypes: [],
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
  }),
  SchemaProvider: ({ children }: { children: React.ReactNode }) => children,
}));

// The inner board content is irrelevant to the collapse-command behavior.
// A lightweight `<BoardView>` mock keeps the test focused on the group
// scope + bus + dispatch wiring, and — by rendering NO inner `<FocusScope>`
// — keeps each group's `<FocusScope>` a valid leaf.
vi.mock("@/components/board-view", () => ({
  BoardView: ({ tasks }: { board: BoardData; tasks: Entity[] }) => (
    <div data-testid="board-view">
      {tasks.map((t) => (
        <div key={t.id} data-testid={`task-${t.id}`} />
      ))}
    </div>
  ),
}));

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { GroupedBoardView } from "./grouped-board-view";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider, useFocusStore } from "@/lib/entity-focus-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useDispatchCommand } from "@/lib/command-scope";
import { resetWebviewCommandBusForTest } from "@/lib/webview-command-bus";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Fixture — two groups (`alpha`, `beta`), one task each, single column.
// ---------------------------------------------------------------------------

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

function makeTask(id: string, project?: string): Entity {
  const fields: Record<string, unknown> = {
    title: `Task ${id}`,
    position_column: "col-todo",
    position_ordinal: "a0",
  };
  // A task with no `project` lands in the empty-string ungrouped bucket
  // (`computeGroups` maps null/empty to `value: ""`).
  if (project !== undefined) fields.project = project;
  return { id, entity_type: "task", moniker: `task:${id}`, fields };
}

const board: BoardData = {
  board: {
    id: "board-1",
    entity_type: "board",
    moniker: "board:board-1",
    fields: { name: "Test Board" },
  },
  columns: [makeColumn("col-todo", "Todo", 0)],
  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: 2,
    total_actors: 0,
    ready_tasks: 2,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

const tasks: Entity[] = [
  makeTask("t-alpha", "alpha"),
  makeTask("t-beta", "beta"),
  // No `project` → the ungrouped bucket (`value: ""`, segment `group:`).
  makeTask("t-ungrouped"),
];

// ---------------------------------------------------------------------------
// Harness — exposes the real focus store and a `group.toggleCollapse`
// dispatcher to the test body.
// ---------------------------------------------------------------------------

interface Handles {
  setFocus: (fq: string) => void;
  dispatchToggle: () => Promise<unknown>;
}

function Harness({ onReady }: { onReady: (h: Handles) => void }) {
  const store = useFocusStore();
  const dispatch = useDispatchCommand("group.toggleCollapse");
  useEffect(() => {
    onReady({
      // The exact write the kernel `focus-changed` bridge performs.
      setFocus: (fq: string) => store.set(fq),
      dispatchToggle: () => dispatch(),
    });
  }, [store, dispatch, onReady]);
  return null;
}

function renderGrouped(onReady: (h: Handles) => void) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider>
            <ActiveBoardPathProvider value="/test/board">
              <DragSessionProvider>
                <Harness onReady={onReady} />
                <GroupedBoardView board={board} tasks={tasks} />
              </DragSessionProvider>
            </ActiveBoardPathProvider>
          </TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Whether the group section for `value` currently renders its expanded body. */
function isExpanded(container: HTMLElement, value: string): boolean {
  const section = container.querySelector(
    `[data-group-section][data-group-value="${value}"]`,
  );
  return section?.querySelector('[data-testid="group-section-body"]') != null;
}

/** Read the `group:<value>` `<FocusScope>`'s full FQM off its `data-moniker`. */
function groupFq(container: HTMLElement, value: string): string {
  const el = container.querySelector(
    `[data-segment="group:${value}"]`,
  ) as HTMLElement | null;
  const fq = el?.getAttribute("data-moniker");
  if (!fq) throw new Error(`no group FocusScope rendered for value=${value}`);
  return fq;
}

describe("group.toggleCollapse command (vim z o)", () => {
  beforeEach(() => {
    resetWebviewCommandBusForTest();
  });

  it("expands exactly the focused group when dispatched", async () => {
    let handles!: Handles;
    const { container } = renderGrouped((h) => {
      handles = h;
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 20));
    });

    // Both groups start collapsed (the lazy initializer seeds every bucket).
    expect(isExpanded(container, "alpha")).toBe(false);
    expect(isExpanded(container, "beta")).toBe(false);

    // Commit focus inside the `alpha` group's subtree.
    const alphaFq = groupFq(container, "alpha");
    await act(async () => {
      handles.setFocus(alphaFq);
      await Promise.resolve();
    });

    // Dispatch the command — the real dispatcher routes through the webview
    // command bus to the focused group's handler.
    await act(async () => {
      await handles.dispatchToggle();
      await new Promise((r) => setTimeout(r, 20));
    });

    // Only the focused group flipped.
    expect(isExpanded(container, "alpha")).toBe(true);
    expect(isExpanded(container, "beta")).toBe(false);
  });

  it("toggles the empty-string ungrouped bucket (segment `group:`)", async () => {
    // The ungrouped bucket carries `value: ""`, so its scope segment is the
    // bare `group:`. Prove `composeFq` + focus-within prefix matching handle
    // the empty value, and dispatching while focused inside it flips ONLY it.
    let handles!: Handles;
    const { container } = renderGrouped((h) => {
      handles = h;
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(isExpanded(container, "")).toBe(false);
    expect(isExpanded(container, "alpha")).toBe(false);

    const ungroupedFq = groupFq(container, "");
    await act(async () => {
      handles.setFocus(ungroupedFq);
      await Promise.resolve();
    });
    await act(async () => {
      await handles.dispatchToggle();
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(isExpanded(container, "")).toBe(true);
    expect(isExpanded(container, "alpha")).toBe(false);
    expect(isExpanded(container, "beta")).toBe(false);
  });

  it("is a no-op when focus is outside every group (empty handler slot)", async () => {
    let handles!: Handles;
    const { container } = renderGrouped((h) => {
      handles = h;
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(isExpanded(container, "alpha")).toBe(false);
    expect(isExpanded(container, "beta")).toBe(false);

    // Focus somewhere with no registered group handler.
    await act(async () => {
      handles.setFocus("/window/somewhere-else");
      await Promise.resolve();
    });

    await act(async () => {
      await handles.dispatchToggle();
      await new Promise((r) => setTimeout(r, 20));
    });

    // No group's collapse state changed.
    expect(isExpanded(container, "alpha")).toBe(false);
    expect(isExpanded(container, "beta")).toBe(false);
  });
});
