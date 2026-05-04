/**
 * Browser-mode test that pins the single-variant `<FocusIndicator>`
 * contract end-to-end.
 *
 * The architectural rule is: one focus indicator visual (the cursor-bar),
 * rendered in one component. An earlier card slipped a second `"ring"`
 * variant past review and threaded a `focusIndicatorVariant` prop through
 * `<FocusScope>` and `<FocusScope>`. The user rejected that — every variant
 * is a chance for two consumers to pick differently and produce inconsistent
 * UX. This file pins the post-deletion state:
 *
 *   1. **Type-level** — `<FocusIndicator variant=... />` and
 *      `<FocusScope focusIndicatorVariant=... />` and
 *      `<FocusScope focusIndicatorVariant=... />` no longer compile.
 *   2. **Runtime: bar everywhere** — driving focus to each navbar leaf
 *      mounts a `<FocusIndicator>` whose className is the bar signature
 *      (`-left-2 w-1 bg-primary`), never the historic ring (`inset-0
 *      ring-2`).
 *   3. **Runtime: bar visible on a nav button** — the focused leaf's
 *      indicator has a non-zero bounding rect with `left >= 0`. This is
 *      the assertion that catches the historic "the bar lives in `gap`
 *      dead space and is invisible" failure mode that motivated the ring
 *      variant in the first place.
 *   4. **Architecture: one indicator per focused entity** — at any moment
 *      the document holds exactly one `[data-testid="focus-indicator"]`
 *      element, the runtime symmetric of the source-level guard in
 *      `focus-architecture.guards.node.test.ts`.
 *
 * Mock pattern matches `nav-bar.spatial-nav.test.tsx`:
 *   - `vi.hoisted` builds an invoke / listen mock pair the test owns.
 *   - `mockListen` records every `listen("focus-changed", cb)` callback
 *     so `fireFocusChanged(key)` drives the React tree as if the Rust
 *     kernel had emitted the event.
 *
 * Runs under the browser project (real Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";
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
// WindowContainer + command-scope + schema mocks — the navbar pulls these
// in even when we render only the navbar fragment.
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

// Mock the Field component so we don't pull in the full entity store —
// it's not load-bearing for the navbar focus-visual assertions.
vi.mock("@/components/fields/field", () => ({
  Field: (props: Record<string, unknown>) => (
    <span data-testid="field-percent">{String(props.entityId)}</span>
  ),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { NavBar } from "./nav-bar";
import { FocusLayer } from "./focus-layer";
import { FocusIndicator } from "./focus-indicator";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WINDOW_LAYER_NAME = asSegment("window");

/**
 * Tailwind utility shim — the Vitest browser harness doesn't compile
 * Tailwind CSS, so the focus indicator's `w-1`, `-left-2`, `top-0.5`,
 * `bottom-0.5`, `absolute` and `position: relative` (on the host) classes
 * resolve to no styling and `getBoundingClientRect()` returns 0×0. The
 * shim translates the handful of Tailwind utilities the bar and its host
 * actually depend on into raw CSS so the bar gets a real bounding rect.
 *
 * The shim is opt-in (called explicitly by the rect test) because adding
 * arbitrary Tailwind classes globally would taint the className-string
 * assertions in the other tests in this file.
 */
const TAILWIND_SHIM = `
.absolute { position: absolute; }
.relative { position: relative; }
.flex { display: flex; }
.items-center { align-items: center; }
.h-12 { height: 3rem; }
.px-4 { padding-left: 1rem; padding-right: 1rem; }
.gap-2 { gap: 0.5rem; }
.p-1 { padding: 0.25rem; }
.h-4 { height: 1rem; }
.w-4 { width: 1rem; }
.w-1 { width: 0.25rem; }
.-left-2 { left: -0.5rem; }
.top-0\\.5 { top: 0.125rem; }
.bottom-0\\.5 { bottom: 0.125rem; }
.ml-auto { margin-left: auto; }
`;

/**
 * Inject the Tailwind shim style sheet once per test run. Re-entry is a
 * no-op — the same `<style>` element is reused.
 */
function installTailwindShim() {
  let style = document.getElementById(
    "focus-indicator-single-variant-shim",
  ) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement("style");
    style.id = "focus-indicator-single-variant-shim";
    style.textContent = TAILWIND_SHIM;
    document.head.appendChild(style);
  }
}

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Drive a `focus-changed` event into the React tree. */
async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: null,
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/** Render `<NavBar>` inside the spatial-focus + window-root layer providers. */
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

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Test data
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

describe("FocusIndicator — single variant contract", () => {
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

  // -------------------------------------------------------------------------
  // 1. Type-level: variant prop removed
  // -------------------------------------------------------------------------

  it("the variant prop is rejected at compile time", () => {
    // `<FocusIndicator variant="ring" />` MUST NOT compile — the variant
    // prop is gone from FocusIndicatorProps. Same for the
    // `focusIndicatorVariant` prop on `<FocusScope>` and `<FocusScope>`.
    // These ts-expect-error directives are the test: TypeScript fails the
    // build if any of them stops being an error (i.e. the prop comes back).
    const _indicator = (
      <FocusIndicator
        focused
        // @ts-expect-error variant prop has been removed.
        variant="ring"
      />
    );
    const _scope = (
      <FocusScope
        moniker={asSegment("ui:test")}
        // @ts-expect-error focusIndicatorVariant prop has been removed.
        focusIndicatorVariant="ring"
      >
        <span>x</span>
      </FocusScope>
    );
    const _zone = (
      <FocusScope
        moniker={asSegment("ui:test")}
        // @ts-expect-error focusIndicatorVariant prop has been removed.
        focusIndicatorVariant="ring"
      >
        <span>x</span>
      </FocusScope>
    );
    expect(_indicator).toBeTruthy();
    expect(_scope).toBeTruthy();
    expect(_zone).toBeTruthy();
  });

  // -------------------------------------------------------------------------
  // 2. Runtime: focused indicator is the bar everywhere
  // -------------------------------------------------------------------------

  it("each navbar leaf renders the bar signature when focused — never a ring", async () => {
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    // `ui:navbar.board-selector` is a zone (multi-leaf surface), not a
    // leaf — its inner leaves (dropdown trigger, tear-off button, editable
    // name `<Field>` zone) own the visible focus signal. The kernel's
    // scope-is-leaf invariant rejects a `<FocusScope>` wrapping further
    // focus primitives — see swissarmyhammer-focus/tests/scope_is_leaf.rs.
    const monikers = [
      "ui:navbar.inspect",
      "ui:navbar.search",
    ] as const;

    for (const moniker of monikers) {
      const leaf = registerScopeArgs().find((a) => a.segment === moniker);
      expect(leaf, `expected register call for ${moniker}`).toBeDefined();

      // Focus the leaf and wait for the indicator to mount.
      await fireFocusChanged({ next_fq: leaf!.fq as FullyQualifiedMoniker });
      await waitFor(() => {
        expect(queryByTestId("focus-indicator")).not.toBeNull();
      });

      const indicator = queryByTestId("focus-indicator")!;
      const cls = indicator.className;
      // Bar signature — the only allowed visual.
      expect(cls).toContain("-left-2");
      expect(cls).toContain("top-0.5");
      expect(cls).toContain("bottom-0.5");
      expect(cls).toContain("w-1");
      expect(cls).toContain("rounded-full");
      expect(cls).toContain("bg-primary");
      // The historic ring variant is gone.
      expect(cls).not.toContain("inset-0");
      expect(cls).not.toContain("ring-2");
      expect(cls).not.toContain("ring-ring");

      // Unfocus before moving on so the next iteration starts clean.
      await fireFocusChanged({ prev_fq: leaf!.fq as FullyQualifiedMoniker });
      await waitFor(() => {
        expect(queryByTestId("focus-indicator")).toBeNull();
      });
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // 3. Runtime: bar is visible on a nav button
  // -------------------------------------------------------------------------

  it("the focused nav button's indicator has a non-zero bounding rect inside the viewport", async () => {
    // The historic failure was "the bar lives in `gap` dead space and is
    // invisible" — typically meaning either the bar's bounding rect was
    // 0×0 or its left edge fell outside the viewport. This test asserts
    // both conditions hold for each navbar leaf: width > 0, height > 0,
    // and `left >= 0`. The layout fix (bar at `-left-2`, navbar `gap-2`,
    // navbar `px-4` providing room for the leftmost leaf) must keep the
    // single cursor-bar genuinely visible without resorting to a variant.
    //
    // The Vitest browser harness doesn't compile Tailwind, so we inject a
    // tiny CSS shim that translates the bar's utility classes (and the
    // navbar layout classes the host primitives depend on) into raw
    // properties. Without it `w-1` resolves to nothing and the rect is
    // 0×0 even when the architecture is correct.
    installTailwindShim();
    const { queryByTestId, unmount } = renderNavBar();
    await flushSetup();

    // `ui:navbar.board-selector` is a zone (multi-leaf surface), not a
    // leaf — see comment above. Only the leaf monikers are exercised
    // here.
    const monikers = [
      "ui:navbar.inspect",
      "ui:navbar.search",
    ] as const;

    for (const moniker of monikers) {
      const leaf = registerScopeArgs().find((a) => a.segment === moniker)!;
      await fireFocusChanged({ next_fq: leaf.fq as FullyQualifiedMoniker });

      await waitFor(() => {
        expect(queryByTestId("focus-indicator")).not.toBeNull();
      });

      const indicator = queryByTestId("focus-indicator")!;
      const rect = indicator.getBoundingClientRect();
      expect(
        rect.width,
        `${moniker} indicator width must be > 0`,
      ).toBeGreaterThan(0);
      expect(
        rect.height,
        `${moniker} indicator height must be > 0`,
      ).toBeGreaterThan(0);
      expect(
        rect.left,
        `${moniker} indicator left edge must be inside the viewport`,
      ).toBeGreaterThanOrEqual(0);

      await fireFocusChanged({ prev_fq: leaf.fq as FullyQualifiedMoniker });
      await waitFor(() => {
        expect(queryByTestId("focus-indicator")).toBeNull();
      });
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // 4. Architecture: only one indicator per focused entity
  // -------------------------------------------------------------------------

  it("the document holds exactly one focus-indicator element when a leaf is focused", async () => {
    // Runtime symmetric of the source-level guard test
    // (`focus-architecture.guards.node.test.ts`) — the bar lives in one
    // component and one place. Mounting the entire navbar with one leaf
    // focused must yield a single `[data-testid="focus-indicator"]`.
    const { container, unmount } = renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    )!;
    await fireFocusChanged({ next_fq: leaf.fq as FullyQualifiedMoniker });

    await waitFor(() => {
      expect(
        container.querySelectorAll('[data-testid="focus-indicator"]').length,
      ).toBe(1);
    });

    unmount();
  });
});
