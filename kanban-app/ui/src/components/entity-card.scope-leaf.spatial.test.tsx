/**
 * Browser-mode test pinning the **scope-is-leaf** invariant for `<EntityCard>`.
 *
 * Source of truth for the second iteration of card
 * `01KQJDYJ4SDKK2G8FTAQ348ZHG` (scope-is-leaf invariant — task card
 * audit). The kernel's three peers are:
 *
 *   - `<FocusLayer>` — modal boundary
 *   - `<FocusZone>` — navigable container, can have children (other zones
 *     or scopes)
 *   - `<FocusScope>` — leaf in the spatial graph
 *
 * The previous card-as-`<FocusScope>` shape was a kernel "leaf" with no
 * tracked children — but the React tree composed `<Field>` zones inside
 * it whose `parent_zone` (read via `useParentZoneFq()`) skipped the
 * Scope and pointed at the column zone, because `<FocusScope>` does not
 * push `FocusZoneContext.Provider`. The kernel saw fields as siblings
 * of cards under the column rather than as descendants of cards. The
 * path-prefix branch of `swissarmyhammer-focus`'s
 * `warn_forward_scope_ancestors` (in `registry.rs`) catches this shape
 * directly: any registered Zone whose FQM is a strict path-descendant
 * of a registered Scope's FQM is a `scope-not-leaf` violation.
 *
 * This file pins:
 *   1. The card body registers as a `<FocusZone>` (segment matches
 *      `^task:` and the call lands on `spatial_register_zone`, NOT on
 *      `spatial_register_scope`).
 *   2. The inspect-button is an inner leaf scope
 *      (`card.inspect:{id}` segment, registered via
 *      `spatial_register_scope`). The drag-handle is intentionally NOT
 *      a leaf scope — it has no keyboard activation story (dnd-kit on
 *      the board uses `PointerSensor` only, no `KeyboardSensor`), so
 *      registering it would create a tab-stop trap with no action.
 *   3. The inspect-button leaf registers its `parentZone` as the card
 *      zone's FQM — so the card zone has the inspect chrome leaf as a
 *      direct path-descendant `parent_zone`-wise.
 *   4. The `<Field>` zones inside the card register their `parentZone`
 *      as the card zone's FQM — fields are children of cards, not
 *      siblings.
 *
 * Mock pattern matches `nav-bar.scope-leaf.spatial.test.tsx` and
 * `entity-card.spatial.test.tsx`.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
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

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { EntityCard } from "./entity-card";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Schema + fixture data
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    body_field: "body",
    fields: ["title", "status", "body"],
    sections: [{ id: "header", on_card: true }, { id: "body" }],
  },
  fields: [
    {
      id: "f-title",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f-status",
      name: "status",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f-body",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
} as unknown as EntitySchema;

async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    return entityType === "task" ? TASK_SCHEMA : TASK_SCHEMA;
  }
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  return undefined;
}

function makeTask(): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
    fields: {
      title: "Hello",
      status: "todo",
      body: "",
      depends_on: [],
      position_column: "col-1",
      position_ordinal: "a0",
    },
  };
}

async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

function renderCard() {
  return render(
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <TooltipProvider delayDuration={100}>
            <SchemaProvider>
              <EntityStoreProvider entities={{ task: [makeTask()] }}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <EntityCard entity={makeTask()} />
                    </UIStateProvider>
                  </FieldUpdateProvider>
                </EntityFocusProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </CommandBusyProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("EntityCard — scope-is-leaf invariant", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("registers task:{id} as a zone, never as a scope", async () => {
    // The kernel's path-prefix scope-is-leaf check fires on any Zone
    // whose FQM is a strict path-descendant of a registered Scope's FQM.
    // Because the card's React subtree contains <Field> zones whose FQMs
    // are composed under the card's FQM, the card MUST register as a
    // zone — registering as a scope would make every inner field a
    // scope-not-leaf offender.
    const { unmount } = renderCard();
    await flushSetup();

    const asZone = registerZoneArgs().find(
      (a) => a.segment === "task:task-1",
    );
    expect(
      asZone,
      "task:{id} must register as a zone",
    ).toBeDefined();

    const asScope = registerScopeArgs().find(
      (a) => a.segment === "task:task-1",
    );
    expect(
      asScope,
      "task:{id} must NOT register as a scope (scope-is-leaf invariant)",
    ).toBeUndefined();

    unmount();
  });

  it("the drag-handle does NOT register as a scope", async () => {
    // The drag-handle is mouse-only — `@dnd-kit` on the board uses
    // `useSensor(PointerSensor, …)` with no `KeyboardSensor` (see
    // `board-view.tsx`). A focusable element with no keyboard action
    // would be a tab-stop trap, so the drag-handle is intentionally
    // NOT wrapped in a `<FocusScope>`. Assert that no
    // `spatial_register_scope` call carries a `card.drag-handle:` segment.
    const { unmount } = renderCard();
    await flushSetup();

    const dragHandleScopes = registerScopeArgs().filter((a) =>
      typeof a.segment === "string" &&
      /^card\.drag-handle:/.test(a.segment),
    );
    expect(
      dragHandleScopes,
      "no card.drag-handle:* segment should be registered as a scope",
    ).toEqual([]);

    unmount();
  });

  it("the inspect-button leaf registers as a scope under the card zone", async () => {
    const { unmount } = renderCard();
    await flushSetup();

    const cardZone = registerZoneArgs().find(
      (a) => a.segment === "task:task-1",
    )!;

    const inspectLeaf = registerScopeArgs().find(
      (a) => a.segment === "card.inspect:task-1",
    );
    expect(
      inspectLeaf,
      "card.inspect:{id} must register as a leaf scope",
    ).toBeDefined();
    expect(
      inspectLeaf!.parentZone,
      "inspect leaf's parentZone must point at the card zone",
    ).toBe(cardZone.fq);

    unmount();
  });

  it("inner <Field> zones nest under the card zone (parent_zone = card)", async () => {
    // Field zones inside the card body register their parent_zone via
    // useParentZoneFq(), which walks FocusZoneContext. With the card now
    // a <FocusZone>, that context push lands on the card's FQM — fields
    // are children of the card in the spatial graph, not siblings.
    const { unmount } = renderCard();
    await flushSetup();

    const cardZone = registerZoneArgs().find(
      (a) => a.segment === "task:task-1",
    )!;

    const titleZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:task-1.title",
    );
    expect(titleZone, "title field must register as a zone").toBeDefined();
    expect(
      titleZone!.parentZone,
      "title field's parentZone must point at the card zone, not the column",
    ).toBe(cardZone.fq);

    unmount();
  });
});
