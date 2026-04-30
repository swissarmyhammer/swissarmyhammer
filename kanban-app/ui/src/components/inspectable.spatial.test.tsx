/**
 * Browser-mode tests for the `<Inspectable>` wrapper and its sibling
 * `useInspectOnDoubleClick` hook (see `inspectable.tsx`).
 *
 * `<Inspectable>` is the single source of the double-click → `ui.inspect`
 * dispatch in the codebase. After card 01KQ7K7KZNR3EHS9SY0XY79NYE the
 * spatial-nav primitives `<FocusScope>` and `<FocusZone>` are pure
 * spatial: they no longer carry an `inspectOnDoubleClick` prop and never
 * call `useDispatchCommand("ui.inspect")`. Inspect lives here.
 *
 * This file pins the wrapper's contract end-to-end:
 *
 *   1. `<Inspectable>` alone fires inspect on dblclick.
 *   2. Inputs / textareas / contenteditable are skipped.
 *   3. `<Inspectable>` composes around `<FocusScope>` cleanly — exactly
 *      one dispatch fires (the wrapper handles it; the scope no longer
 *      does).
 *   4. `<FocusScope>` alone does NOT register a `ui.inspect` dispatch
 *      handler (regression guard for the dispatch hook leaking back
 *      into the primitive).
 *   5. Symmetric for `<FocusZone>`.
 *   6. Real-world `<EntityCard>` opt-in.
 *   7. Real-world `<ColumnView>` opt-in.
 *   8. Inner button stops propagation contract.
 *
 * Mock pattern matches `column-view.spatial.test.tsx` /
 * `perspective-bar.spatial.test.tsx`. Runs under
 * `kanban-app/ui/vite.config.ts`'s browser project (real Chromium via
 * Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

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
// Spy patch for `useDispatchCommand` — capture which preset command IDs
// are registered. Tests #4 and #5 rely on this to assert that
// `<FocusScope>` / `<FocusZone>` (used outside of an `<Inspectable>`)
// never register `useDispatchCommand("ui.inspect")`.
//
// We swap the real module for a thin wrapper that records every call's
// preset cmd id, then re-exports everything else untouched. The spy
// list is reset before each test.
// ---------------------------------------------------------------------------

const useDispatchSpy: { calls: (string | undefined)[] } = { calls: [] };

vi.mock("@/lib/command-scope", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...actual,
    useDispatchCommand: (presetCmd?: string) => {
      useDispatchSpy.calls.push(presetCmd);
      return actual.useDispatchCommand(presetCmd as string);
    },
  };
});

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { Inspectable } from "./inspectable";
import { FocusScope } from "./focus-scope";
import { FocusZone } from "./focus-zone";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EntityCard } from "./entity-card";
import { ColumnView } from "./column-view";
import {
  asSegment
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Default `invoke` implementation covering the IPCs the provider stack
 * fires on mount. Keeps the EntityCard / ColumnView tests stable while
 * leaving every spatial-nav IPC available for assertion.
 */
async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return [];
  if (cmd === "get_entity_schema") return null;
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

/** Render `ui` inside the production-shaped spatial-nav stack. */
function withSpatialStack(ui: React.ReactElement): React.ReactElement {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/board">
                  {ui}
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

/** Collect every `dispatch_command` call's args, in order. */
function dispatchCommandCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Filter `dispatch_command` calls down to those for `ui.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return dispatchCommandCalls().filter((c) => c.cmd === "ui.inspect");
}

// ---------------------------------------------------------------------------
// Tests — `<Inspectable>` core contract
// ---------------------------------------------------------------------------

describe("Inspectable — core dispatch contract", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    useDispatchSpy.calls = [];
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // #1: <Inspectable> alone fires inspect on dblclick
  // -------------------------------------------------------------------------

  it("dblclick inside <Inspectable> dispatches ui.inspect with target=moniker", async () => {
    const { getByTestId, unmount } = render(
      withSpatialStack(
        <Inspectable moniker={asSegment("task:fake")}>
          <div data-testid="x">x</div>
        </Inspectable>,
      ),
    );
    await flushSetup();

    mockInvoke.mockClear();
    fireEvent.doubleClick(getByTestId("x"));

    const dispatches = inspectDispatches();
    expect(dispatches).toHaveLength(1);
    expect(dispatches[0].target).toBe("task:fake");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: <Inspectable> skips inputs / textareas / contenteditable
  // -------------------------------------------------------------------------

  it("dblclick on input / textarea / contenteditable inside <Inspectable> does NOT dispatch", async () => {
    const { getByTestId, unmount } = render(
      withSpatialStack(
        <Inspectable moniker={asSegment("task:fake")}>
          <input data-testid="input" type="text" />
          <textarea data-testid="textarea" />
          <div data-testid="ce" contentEditable suppressContentEditableWarning>
            <span data-testid="ce-inner">inner</span>
          </div>
        </Inspectable>,
      ),
    );
    await flushSetup();

    mockInvoke.mockClear();

    // Each editable surface should be skipped; no dispatch fires.
    fireEvent.doubleClick(getByTestId("input"));
    fireEvent.doubleClick(getByTestId("textarea"));
    fireEvent.doubleClick(getByTestId("ce-inner"));

    expect(inspectDispatches()).toHaveLength(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: <Inspectable> composes around <FocusScope> cleanly
  // -------------------------------------------------------------------------

  it("<Inspectable> wrapping <FocusScope> dispatches exactly once", async () => {
    const { getByTestId, unmount } = render(
      withSpatialStack(
        <Inspectable moniker={asSegment("task:fake")}>
          <FocusScope moniker={asSegment("task:fake")}>
            <div data-testid="x">x</div>
          </FocusScope>
        </Inspectable>,
      ),
    );
    await flushSetup();

    mockInvoke.mockClear();
    fireEvent.doubleClick(getByTestId("x"));

    const dispatches = inspectDispatches();
    expect(dispatches).toHaveLength(1);
    expect(dispatches[0].target).toBe("task:fake");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: <FocusScope> alone does NOT register a `ui.inspect` dispatch handler
  // -------------------------------------------------------------------------

  it("<FocusScope> outside any <Inspectable> never registers useDispatchCommand(ui.inspect)", async () => {
    useDispatchSpy.calls = [];

    const { unmount } = render(
      withSpatialStack(
        <FocusScope moniker={asSegment("ui:chrome")}>
          <div>x</div>
        </FocusScope>,
      ),
    );
    await flushSetup();

    expect(useDispatchSpy.calls).not.toContain("ui.inspect");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #5: Symmetric — <FocusZone> alone does NOT register either
  // -------------------------------------------------------------------------

  it("<FocusZone> outside any <Inspectable> never registers useDispatchCommand(ui.inspect)", async () => {
    useDispatchSpy.calls = [];

    const { unmount } = render(
      withSpatialStack(
        <FocusZone moniker={asSegment("ui:perspective-bar")}>
          <div>x</div>
        </FocusZone>,
      ),
    );
    await flushSetup();

    expect(useDispatchSpy.calls).not.toContain("ui.inspect");

    unmount();
  });
});

// ---------------------------------------------------------------------------
// Tests — real-world entity wrappers
// ---------------------------------------------------------------------------

describe("Inspectable — real-world entity wrappers", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    useDispatchSpy.calls = [];
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // #6: real-world card → inspect fires with target = task moniker
  // -------------------------------------------------------------------------

  it("EntityCard — dblclick dispatches ui.inspect with target=task:<id>", async () => {
    const taskEntity: Entity = {
      entity_type: "task",
      id: "01TASK001",
      moniker: "task:01TASK001",
      fields: { title: "Card title" },
    };

    const { container, unmount } = render(
      withSpatialStack(<EntityCard entity={taskEntity} />),
    );
    await flushSetup();

    const node = container.querySelector(
      "[data-segment='task:01TASK001']",
    ) as HTMLElement;
    expect(node).not.toBeNull();

    mockInvoke.mockClear();
    fireEvent.doubleClick(node);
    const dispatches = inspectDispatches();
    expect(dispatches).toHaveLength(1);
    expect(dispatches[0].target).toBe("task:01TASK001");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #7: real-world column → inspect fires with target = column moniker
  // -------------------------------------------------------------------------

  it("ColumnView — dblclick on column body dispatches ui.inspect with target=column:<id>", async () => {
    const column: Entity = {
      entity_type: "column",
      id: "01ABCDEFGHJKMNPQRSTVWXYZ01",
      moniker: "column:01ABCDEFGHJKMNPQRSTVWXYZ01",
      fields: { name: "To Do" },
    };

    const { container, unmount } = render(
      withSpatialStack(<ColumnView column={column} tasks={[]} />),
    );
    await flushSetup();

    const node = container.querySelector(
      "[data-segment='column:01ABCDEFGHJKMNPQRSTVWXYZ01']",
    ) as HTMLElement;
    expect(node).not.toBeNull();

    mockInvoke.mockClear();
    fireEvent.doubleClick(node);
    const dispatches = inspectDispatches();
    expect(dispatches.length).toBeGreaterThanOrEqual(1);
    // Find the dispatch whose target is the column moniker. The column
    // body wraps internal field zones; double-click on the column body
    // (the data-moniker node itself) should fire the column dispatch.
    const columnDispatch = dispatches.find(
      (d) => d.target === "column:01ABCDEFGHJKMNPQRSTVWXYZ01",
    );
    expect(columnDispatch).toBeTruthy();

    unmount();
  });

  // -------------------------------------------------------------------------
  // #8: inner button stops propagation → no dispatch at the wrapper
  // -------------------------------------------------------------------------

  it("inner button onDoubleClick stopPropagation — wrapping <Inspectable> does NOT dispatch", async () => {
    const innerHandler = vi.fn((e: React.MouseEvent) => {
      e.stopPropagation();
    });

    const { getByText, unmount } = render(
      withSpatialStack(
        <Inspectable moniker={asSegment("task:withbutton")}>
          <button type="button" onDoubleClick={innerHandler}>
            X
          </button>
        </Inspectable>,
      ),
    );
    await flushSetup();

    mockInvoke.mockClear();
    fireEvent.doubleClick(getByText("X"));

    // The inner handler ran exactly once.
    expect(innerHandler).toHaveBeenCalledTimes(1);

    // No `ui.inspect` dispatch at the wrapping `<Inspectable>` — the
    // inner button's `e.stopPropagation()` killed the bubbling gesture
    // before the wrapper's `onDoubleClick` could see it.
    expect(inspectDispatches()).toHaveLength(0);

    unmount();
  });
});
