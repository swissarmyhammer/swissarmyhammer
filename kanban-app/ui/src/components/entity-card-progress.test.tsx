/**
 * Integration test: verifies a progress bar on an entity card correctly
 * reflects the computed progress value in the entity store, both for the
 * initial render and after an `entity-field-changed` event patches the
 * field in place.
 *
 * Context: the bug being regressed here reported that cards showed an empty
 * progress bar even though the entity store carried the correct computed
 * value (verified via clipboard). The existing `entity-card.test.tsx` only
 * renders the card against a static store snapshot, so event-driven
 * regressions slipped through. This suite exercises the reactive path.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { useCallback, useEffect, useState, type ReactNode } from "react";

// ── Schema + mocks ──────────────────────────────────────────────────────────

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "progress", "body"],
    commands: [
      {
        id: "ui.inspect",
        name: "Inspect {{entity.type}}",
        context_menu: true,
      },
    ],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f2",
      name: "progress",
      type: { kind: "computed", derive: "parse-body-progress" },
      editor: "none",
      display: "progress",
      icon: "clock",
      section: "header",
    },
    {
      id: "f3",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types") return Promise.resolve(["task"]);
  if (args[0] === "get_entity_schema") return Promise.resolve(TASK_SCHEMA);
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
  if (args[0] === "list_commands_for_scope") return Promise.resolve([]);
  return Promise.resolve("ok");
});

// Capture `listen` callbacks so tests can fire synthetic Tauri events.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const listenHandlers: Record<string, Array<(event: any) => void>> = {};
const mockListen = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (eventName: string, handler: (event: any) => void) => {
    (listenHandlers[eventName] ??= []).push(handler);
    return Promise.resolve(() => {
      listenHandlers[eventName] = (listenHandlers[eventName] ?? []).filter(
        (h) => h !== handler,
      );
    });
  },
);

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: Parameters<typeof mockListen>) => mockListen(...args),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import "@/components/fields/registrations";
import { EntityCard } from "./entity-card";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

/**
 * Fire a simulated Tauri event to all registered handlers for `eventName`.
 * Wraps the call in `act()` so React state updates are flushed.
 */
async function fireTauriEvent(
  eventName: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  payload: Record<string, any>,
) {
  const handlers = listenHandlers[eventName] ?? [];
  await act(async () => {
    for (const handler of handlers) {
      handler({ payload });
    }
  });
}

/**
 * Minimal hook that replicates the entity-field-changed path from
 * `RustEngineContainer`. Listens once, patches entities in place from the
 * `changes` array, and re-renders children via `EntityStoreProvider`.
 */
function useMinimalEventStore(initial: Record<string, Entity[]>) {
  const [entitiesByType, setEntitiesByType] = useState(initial);

  const setEntitiesFor = useCallback(
    (type: string, updater: (prev: Entity[]) => Entity[]) =>
      setEntitiesByType((prev) => ({
        ...prev,
        [type]: updater(prev[type] ?? []),
      })),
    [],
  );

  useEffect(() => {
    const unlisteners = [
      mockListen(
        "entity-field-changed",
        (e: {
          payload: {
            entity_type: string;
            id: string;
            changes: Array<{ field: string; value: unknown }>;
          };
        }) => {
          const { entity_type, id, changes } = e.payload;
          if (!changes || changes.length === 0) return;
          setEntitiesFor(entity_type, (prev) =>
            prev.map((ent) => {
              if (ent.id !== id) return ent;
              const patched = { ...ent.fields };
              for (const { field, value } of changes) patched[field] = value;
              return { ...ent, fields: patched };
            }),
          );
        },
      ),
    ];
    return () => {
      Promise.all(unlisteners).then((fns) => fns.forEach((fn) => fn()));
    };
  }, [setEntitiesFor]);

  return entitiesByType;
}

/**
 * Wrap the card in the same provider stack used in production, but sourced
 * from `useMinimalEventStore` so we can fire `entity-field-changed` events.
 */
function ReactiveCardHarness({
  initial,
  children,
}: {
  initial: Record<string, Entity[]>;
  children: ReactNode;
}) {
  const entities = useMinimalEventStore(initial);
  return (
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={entities}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>{children}</UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>
  );
}

/** Build a task Entity with overrideable fields. */
function makeTask(fieldOverrides: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
    fields: {
      title: "Production task",
      body: "- [x] one\n- [x] two",
      ...fieldOverrides,
    },
  };
}

describe("EntityCard progress integration", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    for (const key of Object.keys(listenHandlers)) {
      delete listenHandlers[key];
    }
  });

  // Direct regression for the bug report: the entity store carries the
  // production shape `{completed, percent, total}` with all numbers, the
  // card should render the bar at aria-valuenow=100.
  it("renders 100% bar for exact production shape {completed: 14, percent: 100, total: 14}", async () => {
    const task = makeTask({
      progress: { completed: 14, percent: 100, total: 14 },
    });

    const { container } = render(
      <ReactiveCardHarness initial={{ task: [task], tag: [] }}>
        <EntityCard entity={task} />
      </ReactiveCardHarness>,
    );
    // Let the schema provider finish loading.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    const bar = container.querySelector('[role="progressbar"]');
    expect(bar, "Expected progress bar to render").toBeTruthy();
    expect(bar!.getAttribute("aria-valuenow")).toBe("100");
    expect(container.textContent).toContain("100%");
  });

  // The live scenario: entity starts without a `progress` field (the watcher
  // emits the raw on-disk snapshot), then the enriched event patches it in.
  // The card must re-render with the new bar value.
  it("re-renders the bar when entity-field-changed patches progress in", async () => {
    const task = makeTask({ progress: undefined });
    // Remove the `progress` key entirely so the initial payload matches the
    // pre-enrichment snapshot a fresh task would have.
    delete task.fields.progress;

    const { container } = render(
      <ReactiveCardHarness initial={{ task: [task], tag: [] }}>
        <EntityCard entity={task} />
      </ReactiveCardHarness>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    // Initially no bar (no progress field).
    expect(container.querySelector('[role="progressbar"]')).toBeNull();

    // Backend enrichment arrives as a field-changed event.
    await fireTauriEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "task",
      id: "task-1",
      changes: [
        {
          field: "progress",
          value: { completed: 14, total: 14, percent: 100 },
        },
      ],
    });

    const bar = container.querySelector('[role="progressbar"]');
    expect(
      bar,
      "Progress bar should appear after enrichment event",
    ).toBeTruthy();
    expect(bar!.getAttribute("aria-valuenow")).toBe("100");
  });

  // Guards against a stale snapshot-cache bug: once the bar has rendered a
  // non-zero value, a follow-up event with a *different* non-zero value must
  // update the bar. If `fieldValuesEqual` incorrectly matched, the snapshot
  // cache would hand back the old object and the bar would freeze.
  it("updates the bar when progress changes from one non-zero value to another", async () => {
    const task = makeTask({
      progress: { completed: 1, total: 4, percent: 25 },
    });

    const { container } = render(
      <ReactiveCardHarness initial={{ task: [task], tag: [] }}>
        <EntityCard entity={task} />
      </ReactiveCardHarness>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    let bar = container.querySelector('[role="progressbar"]');
    expect(bar!.getAttribute("aria-valuenow")).toBe("25");

    await fireTauriEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "task",
      id: "task-1",
      changes: [
        { field: "progress", value: { completed: 4, total: 4, percent: 100 } },
      ],
    });

    bar = container.querySelector('[role="progressbar"]');
    expect(bar!.getAttribute("aria-valuenow")).toBe("100");
    expect(container.textContent).toContain("100%");
  });

  // Acceptance criterion from the card: the title still renders correctly.
  // Including this so the regression catches if progress rendering somehow
  // disrupts sibling fields in the header section.
  it("renders the title alongside the progress bar", async () => {
    const task = makeTask({
      progress: { completed: 14, total: 14, percent: 100 },
    });

    render(
      <ReactiveCardHarness initial={{ task: [task], tag: [] }}>
        <EntityCard entity={task} />
      </ReactiveCardHarness>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    expect(screen.getByText("Production task")).toBeTruthy();
  });
});
