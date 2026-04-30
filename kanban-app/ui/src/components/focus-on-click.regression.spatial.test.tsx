/**
 * Regression suite: clicking every component class in the production app
 * produces a visible focus indicator.
 *
 * Source of truth for acceptance of card `01KQ7SPWRGG4AHTQ3RBNMPMG97`.
 *
 * Background. The user has reported click-related focus failures three
 * times across the project. Each time we closed a per-component card
 * (column, perspective tab, nav bar) without an integration test that
 * pinned the click → indicator chain across every component class. This
 * file is that integration test, gathered in one place so any future
 * regression on any leaf surfaces immediately. The most recently reported
 * regression — clicking a perspective tab not producing a visible focus
 * indicator — is the first test in the suite and gates the rest.
 *
 * For each component class the test:
 *
 *   1. Mounts the component inside the production-shaped spatial-nav
 *      provider stack: `<SpatialFocusProvider>` + `<FocusLayer name="window">`
 *      plus whatever per-component contexts the component reads. The
 *      `<EntityFocusProvider>` is omitted (as it is in the canonical
 *      `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx`)
 *      because the spatial-focus path is the load-bearing one for visible
 *      indicator rendering — the entity-focus chrome silently degrades
 *      when missing per `<FocusScope>`'s contract.
 *   2. Captures the registered `FullyQualifiedMoniker` from the corresponding
 *      `mockInvoke("spatial_register_*", ...)` call so the test fires
 *      `focus-changed` against the right key.
 *   3. Clicks the `[data-segment="<expected>"]` element via
 *      `fireEvent.click()`.
 *   4. Asserts exactly one `mockInvoke("spatial_focus", { key })` call
 *      whose `key` matches the captured registered key.
 *   5. Fires `focus-changed` with `next_fq = capturedKey` and asserts
 *      `[data-segment="<expected>"]` carries `data-focused="true"` and
 *      contains a `[data-testid="focus-indicator"]` descendant.
 *   6. Negative guard: after the click, no parent zone's `spatial_focus`
 *      was dispatched — the leaf's own `e.stopPropagation()` call must
 *      keep the click from bubbling to its enclosing zone.
 *
 * # Components in scope (one named test per class)
 *
 *   - **Task / tag card** (`task:<id>`, `tag:<id>`).
 *   - **Column body** (`column:<id>`) — clicking column whitespace, NOT a card.
 *   - **Column name leaf** (`column:<id>.name`).
 *   - **Perspective tab** (`perspective_tab:<id>`) — the user-reported bug.
 *   - **Perspective bar background** (`ui:perspective-bar`).
 *   - **Nav bar buttons** (`ui:navbar.search`, `ui:navbar.inspect`,
 *     `ui:navbar.board-selector`) — three named tests, one per leaf.
 *   - **Toolbar action** (`ui:toolbar.*`) — skipped: production has no
 *     toolbar component today; see `it.skip` note for the linked card to
 *     file when one ships.
 *   - **Inspector field row** (`field:task:<id>.<name>`).
 *   - ~~Inspector panel background~~ — removed in card
 *     `01KQCTJY1QZ710A05SE975GHNR`. The `<FocusZone moniker="panel:*">`
 *     wrap was deleted; the inspector body renders directly inside
 *     `<SlidePanel>` and field zones at the layer root carry their own
 *     click contracts.
 *
 * # Why a single regression suite, not per-component cards
 *
 * Filing yet another per-component card for the perspective tab does not
 * close the systemic gap: there is no test that proves clicks work
 * everywhere. Each per-component card pins its own leaf's contract; this
 * suite pins the union of those contracts so a regression in any leaf —
 * past, present, or future — surfaces in one place. The suite ships
 * independently of the unified-policy work and the `<Inspectable>` /
 * `<FocusScope>` refactors.
 *
 * # Click target architecture
 *
 * The card description deliberately does NOT assert anything about which
 * component "should" be focused when a click lands on a Field zone inside
 * a card — that is the click-target rule, separate from "does click
 * produce ANY focus indicator". This suite asserts whatever the
 * click-target rule is, the chosen target's indicator becomes visible.
 * For the column-body test we click on the bordered column wrapper itself
 * (the registered zone host) so the click target is unambiguous. For the
 * card test we click `[data-entity-card="<id>"]` so the click lands on
 * the card body, not on a descendant Field zone.
 *
 * Mock pattern matches the canonical `vi.hoisted` setup in
 * `grid-view.nav-is-eventdriven.test.tsx` —
 * `mockInvoke` / `mockListen` / `listeners` triple captured in `vi.hoisted`
 * so they are available before any module that mocks `@tauri-apps/api/*`
 * imports them.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright). Files matching `*.test.tsx` outside
 * `*.node.test.ts` land there.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
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
  emit: vi.fn(() => Promise.resolve()),
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
// Per-component context mocks
//
// Each component reads from its own peer context (perspective-context,
// views-context, window-container, schema-context, ui-state-context). The
// canonical pattern in this repo is to mock those contexts with
// `vi.hoisted` factories so the test can swap in a fixture for each
// describe block without unmounting the suite-wide Tauri mocks. This is
// the same pattern `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx`
// uses.
// ---------------------------------------------------------------------------

/** Mutable mock perspective shape — the perspective-bar fixture toggles which tabs are present. */
type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
}));

let mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

// Mutable hoisted board fixtures — the nav-bar tests swap these in their
// `beforeEach` so the inspect leaf (which only mounts when `useBoardData()`
// returns a board) is present. The default values keep the perspective-bar
// / column-view tests deterministic regardless of nav-bar fixture state.
const mockBoardData = vi.hoisted(() => vi.fn<() => unknown>(() => null));
const mockOpenBoards = vi.hoisted(() => vi.fn<() => unknown[]>(() => []));
const mockActiveBoardPath = vi.hoisted(() =>
  vi.fn<() => string | undefined>(() => undefined),
);

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
  useOpenBoards: () => mockOpenBoards(),
  useActiveBoardPath: () => mockActiveBoardPath(),
  useHandleSwitchBoard: () => vi.fn(),
}));

// `@/lib/entity-store-context`, `@/lib/schema-context`, and
// `@/lib/ui-state-context` are intentionally NOT mocked at module scope.
// Their `*Provider` components are imported below and used in the render
// helpers — mocking the modules would replace those exports too. The
// provider tree itself is the test fixture (matches
// `entity-card.spatial.test.tsx`); the providers' mount-time IPCs flow
// through `mockInvoke`, so the real implementations work fine in this
// harness.

// ---------------------------------------------------------------------------
// Component imports — after mocks
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { PerspectiveTabBar } from "./perspective-tab-bar";
import { ColumnView } from "./column-view";
import { NavBar } from "./nav-bar";
import { EntityCard } from "./entity-card";
import { Field } from "./fields/field";
import { FocusLayer } from "./focus-layer";
import { FocusZone } from "./focus-zone";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { CommandBusyProvider } from "@/lib/command-scope";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

/** Two ULID-shaped column ids for the suite's small board fixture. */
const COLUMN_ID_A = "01ABCDEFGHJKMNPQRSTVWXYZ01";
/** Task ids used by the card / inspector tests. */
const TASK_ID_A = "01TASKAAAAAAAAAAAAAAAAAAAA";
const TASK_ID_B = "01TASKBBBBBBBBBBBBBBBBBBBB";
/** Tag id used by the tag-card test. */
const TAG_ID = "01TAGAAAAAAAAAAAAAAAAAAAAA";

/** Build a column entity seed. */
function makeColumn(id = COLUMN_ID_A, name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

/** Build a task entity seed bound to a column. */
function makeTask(id: string, columnId: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id.slice(0, 6)}`,
      position_column: columnId,
      position_ordinal: "a0",
    },
  };
}

/** Build a tag entity seed. */
function makeTag(id: string, name: string): Entity {
  return {
    entity_type: "tag",
    id,
    moniker: `tag:${id}`,
    fields: { name, tag_name: name },
  };
}

/**
 * Minimal task schema for the card / field tests.
 *
 * The schema only needs to enumerate `title` so `<EntityCard>`'s
 * schema-driven render produces a single visible field. Full coverage of
 * the multi-value display path lives in `entity-card.spatial.test.tsx`.
 */
const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    fields: ["title"],
    sections: [{ id: "header", on_card: true }],
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
  ],
} as unknown as EntitySchema;

/** Tag schema — same minimal shape so tag cards render their `name`. */
const TAG_SCHEMA = {
  entity: {
    name: "tag",
    entity_type: "tag",
    fields: ["name"],
    sections: [{ id: "header", on_card: true }],
  },
  fields: [
    {
      id: "f-name",
      name: "name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
} as unknown as EntitySchema;

const SCHEMAS: Record<string, EntitySchema> = {
  task: TASK_SCHEMA,
  tag: TAG_SCHEMA,
};

/**
 * Default invoke responses for the mount-time IPCs the providers fire.
 *
 * Each describe block resets `mockInvoke.mockImplementation` to this so
 * the schema seeds the providers ask for at mount don't return undefined.
 * The spatial-focus IPCs (`spatial_register_*`, `spatial_focus`,
 * `spatial_unregister_scope`) all fall through to `undefined` per the
 * default behaviour — that's fine, the test reads them out of the mock's
 * call log directly.
 */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task", "tag", "column"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    return SCHEMAS[entityType ?? ""] ?? TASK_SCHEMA;
  }
  if (cmd === "get_ui_state") {
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  }
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "show_context_menu") return undefined;
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Wait for register effects scheduled in `useEffect` to flush.
 *
 * Two ticks: first runs the registration `useEffect`, second lets any
 * promise-resolved follow-on (e.g. listener registration) settle.
 */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the active window. Wraps the dispatch in `act()` so
 * React state updates flush before the caller asserts against post-update
 * DOM in the next tick.
 */
async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
  next_segment = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
  next_segment?: string | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: next_segment as FocusChangedPayload["next_segment"],
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/** Collect every `spatial_register_zone` invocation argument bag. */
function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/**
 * Find a registered scope or zone by moniker. Throws on miss with a
 * helpful list of every moniker the test saw, so a misspelled fixture
 * fails with actionable context instead of a cryptic `undefined`.
 */
function findRegistration(moniker: string): Record<string, unknown> {
  const all = [...registerZoneArgs(), ...registerScopeArgs()];
  const reg = all.find((r) => r.segment === moniker);
  if (!reg) {
    const seen = all.map((r) => String(r.moniker)).join(", ");
    throw new Error(
      `expected registration for moniker "${moniker}"; saw [${seen || "<none>"}]`,
    );
  }
  return reg;
}

/**
 * Common click-and-assert chain shared by every test.
 *
 * Captures every assertion the regression suite enforces in one helper:
 *
 *   1. Resolve the `[data-segment="<expected>"]` DOM node.
 *   2. Find the registration call for that moniker; capture its key.
 *   3. Clear the invoke mock so we measure only the click's IPCs.
 *   4. `fireEvent.click(node)`.
 *   5. Assert exactly one `spatial_focus` call whose `key` equals the
 *      captured registered key.
 *   6. Negative guard: zero focus calls against any of `parentMonikers`.
 *   7. Fire `focus-changed { next_fq: capturedKey }`.
 *   8. Assert the node carries `data-focused="true"` and a
 *      `[data-testid="focus-indicator"]` descendant.
 *
 * `parentMonikers` lets each test point at its parent zones explicitly so
 * the negative guard catches a regression where the leaf forgot to call
 * `e.stopPropagation()` in its click handler.
 */
async function assertClickProducesIndicator({
  container,
  moniker,
  parentMonikers,
}: {
  container: HTMLElement;
  moniker: string;
  parentMonikers: readonly string[];
}) {
  const node = container.querySelector(
    `[data-segment='${moniker}']`,
  ) as HTMLElement | null;
  expect(node, `[data-segment='${moniker}'] must be in the DOM`).not.toBeNull();

  const registration = findRegistration(moniker);
  const capturedKey = registration.fq as FullyQualifiedMoniker;
  expect(typeof capturedKey).toBe("string");

  // Capture the parent zones' registered keys before the click so we can
  // assert exactly that NO focus call landed against them. We resolve
  // them first because mockInvoke.mockClear() below wipes the history.
  const parentKeys = parentMonikers.map((m) => {
    const reg = findRegistration(m);
    return reg.fq as FullyQualifiedMoniker;
  });

  // Clear so the assertion measures only the click's IPC.
  mockInvoke.mockClear();
  mockInvoke.mockImplementation(defaultInvokeImpl);

  fireEvent.click(node!);

  const focusCalls = spatialFocusCalls();
  expect(
    focusCalls.length,
    `expected exactly one spatial_focus call for moniker '${moniker}'`,
  ).toBe(1);
  expect(
    focusCalls[0].fq,
    `spatial_focus key must match the registered key for '${moniker}'`,
  ).toBe(capturedKey);

  // Negative guard for stopPropagation: no parent zone may have received
  // its own spatial_focus call from this single click.
  for (const parentKey of parentKeys) {
    expect(
      focusCalls.find((c) => c.fq === parentKey),
      `click on '${moniker}' must NOT bubble to any parent zone`,
    ).toBeUndefined();
  }

  // Drive the focus-changed event the kernel would emit after spatial_focus.
  await fireFocusChanged({ next_fq: capturedKey, next_segment: moniker });

  // Re-resolve the node — `<FocusScope>`'s body re-renders once when its
  // claim listener fires, but `data-moniker` stays stable on the same
  // div so the original reference is still valid. We assert through it
  // directly to keep the failure message pointed at the right element.
  await waitFor(() => {
    expect(node!.getAttribute("data-focused")).toBe("true");
  });

  const indicator = node!.querySelector(
    "[data-testid='focus-indicator']",
  ) as HTMLElement | null;
  expect(
    indicator,
    `<FocusIndicator> must mount inside '${moniker}' when claimed`,
  ).not.toBeNull();
  expect(node!.contains(indicator!)).toBe(true);
}

// ---------------------------------------------------------------------------
// Provider-stack helpers
//
// Each describe block has its own renderer because the provider tree is
// component-specific. The helpers concentrate the wiring so the test
// bodies stay focused on the click → indicator chain.
// ---------------------------------------------------------------------------

/** Wrap content in the spatial-nav stack the production tree mounts. */
function withSpatialStack(content: ReactNode) {
  return (
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <TooltipProvider delayDuration={100}>{content}</TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </CommandBusyProvider>
  );
}

/**
 * Render an `<EntityCard>` for the given entity inside the provider stack
 * production uses. `<EntityFocusProvider>` is mounted because the
 * inspector dispatch (`<Inspectable>`) reads from it; the schema /
 * entity-store / field-update / UI-state providers are the same ones
 * `entity-card.spatial.test.tsx` mounts.
 */
function renderCard(entity: Entity, allEntities: Record<string, Entity[]>) {
  return render(
    withSpatialStack(
      <SchemaProvider>
        <EntityStoreProvider entities={allEntities}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>
                <EntityCard entity={entity} />
              </UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>,
    ),
  );
}

/**
 * Render a `<ColumnView>` inside a surrounding `ui:board` zone. The
 * surrounding zone gives the column a parent zone (matching production)
 * so the negative guard can assert click on the column body does NOT
 * bubble to the board zone.
 */
function renderColumnInBoard(column: Entity, tasks: Entity[]) {
  return render(
    withSpatialStack(
      <SchemaProvider>
        <EntityStoreProvider entities={{ task: tasks }}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>
                <FocusZone moniker={asSegment("ui:board")}>
                  <ColumnView column={column} tasks={tasks} />
                </FocusZone>
              </UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>,
    ),
  );
}

/** Render the perspective tab bar inside the spatial stack.
 *
 * `<PerspectiveTabBar>` reads `useSchema()` (for the active view's entity
 * type field defs the `GroupPopoverButton` mounts) and `useUIState()`
 * (for the keymap mode that drives the inline rename keymap), so the
 * suite mounts the real `<SchemaProvider>` and `<UIStateProvider>`. The
 * providers' mount-time IPCs flow through `mockInvoke` exactly the way
 * the production tree does in `App.tsx`.
 */
function renderPerspectiveBar() {
  return render(
    withSpatialStack(
      <SchemaProvider>
        <UIStateProvider>
          <PerspectiveTabBar />
        </UIStateProvider>
      </SchemaProvider>,
    ),
  );
}

/**
 * Render the nav bar inside the spatial stack.
 *
 * `<NavBar>` reads `useSchema()` to look up the percent-complete field
 * def, so the suite mounts the real `<SchemaProvider>`. Same provider
 * shape as `App.tsx` would mount.
 */
function renderNavBar() {
  return render(
    withSpatialStack(
      <SchemaProvider>
        <NavBar />
      </SchemaProvider>,
    ),
  );
}

/**
 * Render a single `<Field>` inside an inspector-shaped provider stack.
 *
 * We render the bare `<Field>` rather than mounting `<InspectorsContainer>`
 * because the panel zone test (further down) covers the panel-level
 * registration end-to-end. This keeps the field-row test focused on the
 * `field:{type}:{id}.{name}` zone's click contract — which is the user-
 * visible regression scope.
 */
function renderInspectorFieldRow(entity: Entity, fieldName: string) {
  // Build the FieldDef from the schema so the fixture mirrors how
  // `<EntityInspector>` constructs it in production.
  const fieldDef = TASK_SCHEMA.fields.find((f) => f.name === fieldName)!;
  return render(
    withSpatialStack(
      <SchemaProvider>
        <EntityStoreProvider entities={{ task: [entity] }}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>
                <Field
                  fieldDef={fieldDef}
                  entityType={entity.entity_type}
                  entityId={entity.id}
                  mode="full"
                  editing={false}
                  showFocusBar
                />
              </UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>,
    ),
  );
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("focus-on-click regression suite (every component class)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    // Reset per-component context fixtures to a known baseline. Each
    // describe block tweaks these before its render.
    mockPerspectivesValue = {
      perspectives: [],
      activePerspective: null,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
    mockViewsValue = {
      views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
      activeView: {
        id: "board-1",
        name: "Board",
        kind: "board",
        icon: "kanban",
      },
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Perspective tab — the user-reported bug. Runs first so a regression
  // here surfaces immediately.
  //
  // The user clicks on the visible tab text — i.e. on the inner `<button>`
  // element rendered by `TabButton`. The button has its own `onClick`
  // (`setActivePerspectiveId`) and does NOT call `e.stopPropagation()`,
  // so the click must bubble to the surrounding `<FocusScope>` wrapper
  // which dispatches `spatial_focus`. We click
  // the inner button (not the wrapper div) on purpose: a regression that
  // adds `e.stopPropagation()` to the button's handler — or anything
  // else that swallows the click before it reaches the spatial wrapper —
  // would silently fail this assertion. Clicking the wrapper div directly
  // would not catch that class of regression.
  // -------------------------------------------------------------------------
  describe("perspective tab", () => {
    it("clicking a perspective tab focuses it and renders the indicator", async () => {
      mockPerspectivesValue = {
        perspectives: [
          { id: "p1", name: "Sprint", view: "board" },
          { id: "p2", name: "Backlog", view: "board" },
        ],
        activePerspective: { id: "p1", name: "Sprint", view: "board" },
        setActivePerspectiveId: vi.fn(),
        refresh: vi.fn(() => Promise.resolve()),
      };

      const { container, unmount } = renderPerspectiveBar();
      await flushSetup();

      // Capture the registered key for the leaf BEFORE we clear the
      // invoke mock — `findRegistration` reads from the mock's call log.
      const moniker = "perspective_tab:p1";
      const tabRegistration = findRegistration(moniker);
      const tabKey = tabRegistration.fq as FullyQualifiedMoniker;

      const tabNode = container.querySelector(
        `[data-segment='${moniker}']`,
      ) as HTMLElement | null;
      expect(tabNode).not.toBeNull();

      // Capture the bar zone's key for the negative-guard assertion.
      const barRegistration = findRegistration("ui:perspective-bar");
      const barKey = barRegistration.fq as FullyQualifiedMoniker;

      // Click the INNER button — the user's click target. This exercises
      // the full `<button onClick={...}>` → bubble → `<FocusScope>
      // onClick={spatial_focus}` chain, NOT the wrapper-div shortcut.
      const innerButton = tabNode!.querySelector(
        "button",
      ) as HTMLButtonElement | null;
      expect(
        innerButton,
        "perspective tab must render an inner <button> for click handling",
      ).not.toBeNull();

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      fireEvent.click(innerButton!);

      const focusCalls = spatialFocusCalls();
      expect(
        focusCalls.length,
        "exactly one spatial_focus must fire on inner-button click",
      ).toBe(1);
      expect(
        focusCalls[0].fq,
        "spatial_focus key must match the perspective tab's registered key",
      ).toBe(tabKey);

      // Negative guard: the click must not bubble to the perspective bar
      // zone and re-fire spatial_focus against the bar's key. This pins
      // `<FocusScope>`'s `e.stopPropagation()` contract.
      expect(
        focusCalls.find((c) => c.fq === barKey),
        "click on a perspective tab must NOT bubble to the bar zone",
      ).toBeUndefined();

      // Drive the focus-changed event the kernel would emit.
      await fireFocusChanged({ next_fq: tabKey, next_segment: moniker });

      await waitFor(() => {
        expect(tabNode!.getAttribute("data-focused")).toBe("true");
      });

      const indicator = tabNode!.querySelector(
        "[data-testid='focus-indicator']",
      ) as HTMLElement | null;
      expect(
        indicator,
        "<FocusIndicator> must mount inside the focused perspective tab",
      ).not.toBeNull();
      expect(tabNode!.contains(indicator!)).toBe(true);

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Perspective bar background
  // -------------------------------------------------------------------------
  describe("perspective bar background", () => {
    it("clicking the bar whitespace focuses the bar zone and renders the indicator", async () => {
      mockPerspectivesValue = {
        perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
        activePerspective: { id: "p1", name: "Sprint", view: "board" },
        setActivePerspectiveId: vi.fn(),
        refresh: vi.fn(() => Promise.resolve()),
      };

      const { container, unmount } = renderPerspectiveBar();
      await flushSetup();

      const barNode = container.querySelector(
        "[data-segment='ui:perspective-bar']",
      ) as HTMLElement | null;
      expect(barNode).not.toBeNull();

      const barRegistration = findRegistration("ui:perspective-bar");
      const barKey = barRegistration.fq as FullyQualifiedMoniker;

      // The bar carries `showFocusBar={false}` (it's viewport-spanning
      // chrome — see `PerspectiveBarSpatialZone` in
      // `perspective-tab-bar.tsx`), so we only assert the click → focus
      // dispatch and `data-focused` flip. The visible indicator lives on
      // the leaves; the bar's own claim still rides through `data-focused`
      // for e2e selectors.
      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      // Synthesize a click whose target is the bar wrapper itself, NOT a
      // tab inside it. Dispatching directly on the bar node sets the
      // event's target to the bar — `<FocusZone>`'s click handler reads
      // `e.target` only to skip editable surfaces; everything else flows
      // through to `focus(barKey)`.
      fireEvent.click(barNode!);

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].fq).toBe(barKey);

      await fireFocusChanged({
        next_fq: barKey,
        next_segment: asSegment("ui:perspective-bar"),
      });

      await waitFor(() => {
        expect(barNode!.getAttribute("data-focused")).toBe("true");
      });

      // Per `PerspectiveBarSpatialZone`'s `showFocusBar={false}`, no
      // visible bar mounts on the bar wrapper itself — its focus is
      // signalled to e2e selectors via `data-focused` only and the
      // visible signal lives on the focused tab leaf instead.
      const indicator = barNode!.querySelector(
        "[data-testid='focus-indicator']",
      );
      expect(
        indicator,
        "ui:perspective-bar opts out of the visible bar (showFocusBar=false)",
      ).toBeNull();

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Nav bar buttons — three named tests, one per leaf.
  // -------------------------------------------------------------------------
  describe("nav bar buttons", () => {
    /**
     * Build a board fixture for the nav bar tests so both `inspect` and
     * `board-selector` leaves render. `<NavBar>` only mounts the inspect
     * leaf when `useBoardData()` returns a board; the board-selector
     * leaf is always present.
     */
    const NAV_BOARD = {
      board: {
        entity_type: "board",
        id: "b1",
        moniker: "board:b1",
        fields: { name: { String: "Test Board" } },
      },
      columns: [],
      tags: [],
      virtualTagMeta: [],
      summary: {
        total_tasks: 0,
        total_actors: 0,
        ready_tasks: 0,
        blocked_tasks: 0,
        done_tasks: 0,
        percent_complete: 0,
      },
    };

    /**
     * Swap the hoisted board-fixture closures so `<NavBar>` sees a
     * board, which is required for the `ui:navbar.inspect` leaf to mount.
     * The hoisted closures are read by the module-scope `vi.mock(…)`
     * factory above; reassigning them here takes effect for any
     * `useBoardData()` call after this point in the test.
     */
    beforeEach(() => {
      mockBoardData.mockReturnValue(NAV_BOARD);
      mockOpenBoards.mockReturnValue([
        { path: "/boards/a/.kanban", name: "Board A", is_active: true },
      ]);
      mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");
    });

    it("clicking ui:navbar.search focuses the search button and renders the indicator", async () => {
      const { container, unmount } = renderNavBar();
      await flushSetup();

      await assertClickProducesIndicator({
        container,
        moniker: "ui:navbar.search",
        parentMonikers: ["ui:navbar"],
      });

      unmount();
    });

    it("clicking ui:navbar.inspect focuses the inspect button and renders the indicator", async () => {
      const { container, unmount } = renderNavBar();
      await flushSetup();

      // The inspect leaf only mounts when `useBoardData()` returns a
      // board. The hoisted `mockBoardData.mockReturnValue(NAV_BOARD)`
      // above guarantees that for every test in this describe block, so
      // the leaf is reliably present.
      await assertClickProducesIndicator({
        container,
        moniker: "ui:navbar.inspect",
        parentMonikers: ["ui:navbar"],
      });

      unmount();
    });

    it("clicking ui:navbar.board-selector focuses it and renders the indicator", async () => {
      const { container, unmount } = renderNavBar();
      await flushSetup();

      await assertClickProducesIndicator({
        container,
        moniker: "ui:navbar.board-selector",
        parentMonikers: ["ui:navbar"],
      });

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Toolbar action — production has no toolbar component today. The card
  // description lists `ui:toolbar.*` as a target class so the suite stays
  // exhaustive when one ships.
  // -------------------------------------------------------------------------
  describe("toolbar action", () => {
    it.skip("clicking a toolbar action focuses it and renders the indicator — production has no toolbar component today", () => {
      // No toolbar component exists in production yet. When one lands,
      // unskip and replace with the same `assertClickProducesIndicator`
      // chain pointed at the toolbar's leaf monikers (`ui:toolbar.<id>`).
      // The architectural guard
      // (`focus-architecture.guards.node.test.ts`, Guard A) requires every
      // `ui:` prefix to register as a `<FocusZone>` or `<FocusScope>`,
      // so the registration shape is already pinned — only the click side
      // remains to be tested here.
    });
  });

  // -------------------------------------------------------------------------
  // Task card
  // -------------------------------------------------------------------------
  describe("task card", () => {
    it("clicking a task card focuses it and renders the indicator", async () => {
      const task = makeTask(TASK_ID_A, COLUMN_ID_A);
      const { container, unmount } = renderCard(task, { task: [task] });
      await flushSetup();

      // Click the card body's chrome — the `[data-entity-card]` div sits
      // INSIDE the `<FocusScope>` so the click bubbles through the scope's
      // outer div. The scope is the moniker host (`task:<id>`), which is
      // what we assert against.
      const moniker = `task:${TASK_ID_A}`;
      const node = container.querySelector(
        `[data-segment='${moniker}']`,
      ) as HTMLElement | null;
      expect(node).not.toBeNull();

      const registration = findRegistration(moniker);
      const cardKey = registration.fq as FullyQualifiedMoniker;

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      // Click the inner `[data-entity-card]` div so the event bubbles up
      // through the card body to the `<FocusScope>` root. Clicking the
      // scope div directly would also work but is less faithful to how
      // a user actually clicks: their click lands on visible chrome.
      const cardBody = container.querySelector(
        `[data-entity-card='${TASK_ID_A}']`,
      ) as HTMLElement | null;
      expect(cardBody).not.toBeNull();
      fireEvent.click(cardBody!);

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].fq).toBe(cardKey);

      // Negative guard: in this isolated harness the card has no
      // surrounding zone, but `data-entity-card` may itself be matched
      // by a per-leaf field selector elsewhere — we re-check focusCalls
      // length above so any extra hop would already have failed.

      await fireFocusChanged({ next_fq: cardKey, next_segment: moniker });

      await waitFor(() => {
        expect(node!.getAttribute("data-focused")).toBe("true");
      });

      const indicator = node!.querySelector(
        "[data-testid='focus-indicator']",
      ) as HTMLElement | null;
      expect(indicator).not.toBeNull();
      expect(node!.contains(indicator!)).toBe(true);

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Tag card — uses the same `<EntityCard>` shape, different moniker.
  // -------------------------------------------------------------------------
  describe("tag card", () => {
    it("clicking a tag card focuses it and renders the indicator", async () => {
      const tag = makeTag(TAG_ID, "bug");
      const { container, unmount } = renderCard(tag, { tag: [tag] });
      await flushSetup();

      const moniker = `tag:${TAG_ID}`;
      const node = container.querySelector(
        `[data-segment='${moniker}']`,
      ) as HTMLElement | null;
      expect(node).not.toBeNull();

      const registration = findRegistration(moniker);
      const cardKey = registration.fq as FullyQualifiedMoniker;

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      const cardBody = container.querySelector(
        `[data-entity-card='${TAG_ID}']`,
      ) as HTMLElement | null;
      expect(cardBody).not.toBeNull();
      fireEvent.click(cardBody!);

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].fq).toBe(cardKey);

      await fireFocusChanged({ next_fq: cardKey, next_segment: moniker });

      await waitFor(() => {
        expect(node!.getAttribute("data-focused")).toBe("true");
      });

      const indicator = node!.querySelector("[data-testid='focus-indicator']");
      expect(indicator).not.toBeNull();

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Column body — clicking on whitespace inside the column, NOT a card.
  // -------------------------------------------------------------------------
  describe("column body", () => {
    it("clicking column whitespace focuses the column zone and renders the indicator", async () => {
      const column = makeColumn(COLUMN_ID_A);
      const tasks = [
        makeTask(TASK_ID_A, COLUMN_ID_A),
        makeTask(TASK_ID_B, COLUMN_ID_A),
      ];
      const { container, unmount } = renderColumnInBoard(column, tasks);
      await flushSetup();

      // The column body is the registered `<FocusZone>` host; clicking
      // it directly lands the event on the zone's outer div.
      const moniker = `column:${COLUMN_ID_A}`;
      await assertClickProducesIndicator({
        container,
        moniker,
        parentMonikers: ["ui:board"],
      });

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Column name leaf — moniker `column:<id>.name`.
  // -------------------------------------------------------------------------
  describe("column name leaf", () => {
    it("clicking the column name focuses the name leaf and renders the indicator", async () => {
      const column = makeColumn(COLUMN_ID_A);
      const tasks = [makeTask(TASK_ID_A, COLUMN_ID_A)];
      const { container, unmount } = renderColumnInBoard(column, tasks);
      await flushSetup();

      const moniker = `column:${COLUMN_ID_A}.name`;
      await assertClickProducesIndicator({
        container,
        moniker,
        parentMonikers: ["ui:board"],
      });

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Inspector field row — `field:task:<id>.<name>`.
  // -------------------------------------------------------------------------
  describe("inspector field row", () => {
    it("clicking a field row focuses it and renders the indicator", async () => {
      const task = makeTask(TASK_ID_A, COLUMN_ID_A);
      const { container, unmount } = renderInspectorFieldRow(task, "title");
      await flushSetup();

      // The field zone has no enclosing card-zone in this fixture, so
      // there is no parent zone to negative-guard against. The
      // `parentMonikers: []` slot still pins the assertion shape so a
      // future regression that introduces a new ancestor will fail loudly.
      const moniker = `field:task:${TASK_ID_A}.title`;
      await assertClickProducesIndicator({
        container,
        moniker,
        parentMonikers: [],
      });

      unmount();
    });
  });

  // -------------------------------------------------------------------------
  // Inspector panel background was deleted in card
  // `01KQCTJY1QZ710A05SE975GHNR` — the `<FocusZone moniker="panel:*">`
  // wrap is gone and the inspector body renders directly inside
  // `<SlidePanel>`. Field zones at the inspector layer root carry their
  // own click contracts (covered by the `<Field>` test above and by
  // `inspector.layer-shape.browser.test.tsx`).
  // -------------------------------------------------------------------------
});
