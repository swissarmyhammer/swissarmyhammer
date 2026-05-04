/**
 * Browser-mode test pinning the **scope-is-leaf** invariant for `<NavBar>`.
 *
 * Source of truth for acceptance of card `01KQJDYJ4SDKK2G8FTAQ348ZHG`
 * (Enforce scope-is-leaf invariant in spatial-nav kernel; audit
 * nav-bar/toolbar misuse). The kernel's three peers are:
 *
 *   - `<FocusLayer>` — modal boundary
 *   - `<FocusScope>` — navigable container, can have children (other zones
 *     or scopes)
 *   - `<FocusScope>` — leaf in the spatial graph
 *
 * Wrapping a multi-leaf surface (e.g. the `<BoardSelector>` with its
 * editable name `<Field>`, dropdown trigger, and tear-off button) in a
 * `<FocusScope>` instead of a `<FocusScope>` confuses the kernel's beam
 * search and breaks zone `last_focused` memory. The Rust kernel logs
 * `scope-not-leaf` on every offending registration (see
 * `swissarmyhammer-focus/tests/scope_is_leaf.rs`); this React-side test
 * proves the navbar no longer emits the offending shape.
 *
 * Mock pattern matches `nav-bar.spatial-nav.test.tsx` — the React
 * component cannot observe the Rust-side `tracing::error!` directly,
 * but it can prove that the component dispatches
 * `spatial_register_scope` (not `spatial_register_scope`) for the
 * board-selector segment.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { BoardData, OpenBoard } from "@/types/kanban";

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
// WindowContainer + command-scope + schema mocks — same shape as
// `nav-bar.spatial-nav.test.tsx`.
// ---------------------------------------------------------------------------

const mockBoardData = vi.hoisted(() =>
  vi.fn<() => BoardData | null>(() => null),
);
const mockOpenBoards = vi.hoisted(() => vi.fn<() => OpenBoard[]>(() => []));
const mockActiveBoardPath = vi.hoisted(() =>
  vi.fn<() => string | undefined>(() => undefined),
);
const mockHandleSwitchBoard = vi.hoisted(() => vi.fn<(arg: string) => void>());

vi.mock("@/components/window-container", () => ({
  useBoardData: () => mockBoardData(),
  useOpenBoards: () => mockOpenBoards(),
  useActiveBoardPath: () => mockActiveBoardPath(),
  useHandleSwitchBoard: () => mockHandleSwitchBoard,
}));

const mockDispatchInspect = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const mockDispatchSearch = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const mockIsBusy = vi.hoisted(() => vi.fn(() => false));

vi.mock("@/lib/command-scope", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...actual,
    useDispatchCommand: (cmd: string) => {
      if (cmd === "ui.inspect") return mockDispatchInspect;
      if (cmd === "app.search") return mockDispatchSearch;
      return vi.fn(() => Promise.resolve());
    },
    useCommandBusy: () => ({ isBusy: mockIsBusy() }),
  };
});

const mockPercentFieldDef = {
  field_name: "percent_complete",
  display_name: "% Complete",
  field_type: "PercentComplete",
};

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: (_entityType: string, fieldName: string) =>
      fieldName === "percent_complete" ? mockPercentFieldDef : undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

// Mock <Field> as a plain span — this test only cares about the
// navbar's own registrations, not the Field's. Skipping the Field's
// FocusScope here narrows the test to the structural assertion: the
// navbar's `ui:navbar.board-selector` segment must be a zone.
vi.mock("@/components/fields/field", () => ({
  Field: (props: Record<string, unknown>) => (
    <span data-testid={`field-${String(props.entityId)}`} />
  ),
}));

// Mock useFieldValue used by BoardSelector for the live name display.
vi.mock("@/lib/entity-store-context", () => ({
  useFieldValue: () => "",
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { NavBar } from "./nav-bar";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WINDOW_LAYER_NAME = asSegment("window");

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

function renderNavBar() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <TooltipProvider delayDuration={100}>
          <NavBar />
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Test data — same shape as the existing nav-bar tests.
// ---------------------------------------------------------------------------

const MOCK_BOARD: BoardData = {
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
    total_tasks: 5,
    total_actors: 2,
    ready_tasks: 3,
    blocked_tasks: 1,
    done_tasks: 1,
    percent_complete: 20,
  },
};

const MOCK_OPEN_BOARDS: OpenBoard[] = [
  { path: "/boards/a/.kanban", name: "Board A", is_active: true },
  { path: "/boards/b/.kanban", name: "Board B", is_active: false },
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("NavBar — scope-is-leaf invariant", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockOpenBoards.mockReturnValue(MOCK_OPEN_BOARDS);
    mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");
    mockIsBusy.mockReturnValue(false);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("registers ui:navbar.board-selector as a zone, not a scope", async () => {
    // The kernel rejects a `<FocusScope>` whose subtree contains
    // further `<FocusScope>` or `<FocusScope>` registrations as
    // `scope-not-leaf` (see swissarmyhammer-focus/tests/scope_is_leaf.rs).
    // The board-selector houses an editable name `<Field>` (zone), a
    // dropdown trigger, and a tear-off button — three navigable
    // children — so it MUST register as a zone, never as a scope.
    const { unmount } = renderNavBar();
    await flushSetup();

    const asZone = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    );
    expect(
      asZone,
      "ui:navbar.board-selector must register as a zone",
    ).toBeDefined();

    const asScope = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.board-selector",
    );
    expect(
      asScope,
      "ui:navbar.board-selector must NOT register as a scope (scope-is-leaf invariant)",
    ).toBeUndefined();

    unmount();
  });

  it("the board-selector zone is registered under the navbar zone", async () => {
    const { unmount } = renderNavBar();
    await flushSetup();

    const zoneArgs = registerScopeArgs();
    const navbarZone = zoneArgs.find((a) => a.segment === "ui:navbar");
    expect(navbarZone, "navbar zone must register").toBeDefined();

    const boardSelectorZone = zoneArgs.find(
      (a) => a.segment === "ui:navbar.board-selector",
    );
    expect(boardSelectorZone).toBeDefined();
    expect(
      boardSelectorZone!.parentZone,
      "board-selector zone must name the navbar zone as its parent",
    ).toBe(navbarZone!.fq);

    unmount();
  });

  it("inspect and search remain leaves (single-button surfaces)", async () => {
    // The kernel accepts FocusScope leaves only when their subtree has no
    // further focus primitives. The inspect and search buttons each wrap
    // a single button inside a Tooltip — no nested zones or scopes — so
    // they correctly stay as leaves. This test pins that contract so a
    // future "promote everything to a zone" refactor does not lose the
    // leaf shape where it is the right shape.
    const { unmount } = renderNavBar();
    await flushSetup();

    const scopeArgs = registerScopeArgs();
    const inspect = scopeArgs.find((a) => a.segment === "ui:navbar.inspect");
    const search = scopeArgs.find((a) => a.segment === "ui:navbar.search");
    expect(inspect, "ui:navbar.inspect must register as a scope").toBeDefined();
    expect(search, "ui:navbar.search must register as a scope").toBeDefined();

    unmount();
  });
});
