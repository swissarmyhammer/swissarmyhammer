/**
 * Browser test for card `01KR7CDEFWWVF4WH0BCHE8Y21J`'s `app.dismiss`
 * topmost-layer-aware contract.
 *
 * `app.dismiss` is a single command that closes whichever modal layer
 * is currently topmost — palette > inspector > no-op. The backend's
 * `DismissCmd::execute` (in
 * `swissarmyhammer-kanban/src/commands/app_commands.rs`) implements
 * that ordering: it checks `palette_open` first, then `inspector_stack`,
 * then falls through to a no-op when only the window layer is mounted.
 *
 * This test pins the frontend dispatch path: dispatching `app.dismiss`
 * always reaches the backend via `dispatch_command`, regardless of
 * which layer is currently topmost. The backend's topmost-layer
 * decision is covered by Rust integration tests in
 * `swissarmyhammer-kanban/tests/dismiss_inspector_integration.rs`.
 *
 * The matching frontend invariant tested here:
 *   - Backdrop click on the inspector (a "click outside the active
 *     layer" gesture) dispatches `app.dismiss`, NOT a hard-coded
 *     `ui.inspector.close_all`. This is the load-bearing change in
 *     card `01KR7CDEFWWVF4WH0BCHE8Y21J`: the backdrop's intent is
 *     "dismiss the topmost layer", and that decision belongs to the
 *     backend's `DismissCmd`, not the frontend.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import * as React from "react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API mocks.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen } = vi.hoisted(() => {
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
  return { mockInvoke, mockListen };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
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

const mockEntitiesByType = vi.hoisted(() =>
  vi.fn<() => Record<string, unknown[]>>(() => ({})),
);
const mockUIState = vi.hoisted(() =>
  vi.fn(() => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  })),
);

vi.mock("@/components/rust-engine-container", () => ({
  useEntitiesByType: () => mockEntitiesByType(),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => null,
    schemas: {},
    loading: false,
  }),
  useSchemaOptional: () => undefined,
}));

vi.mock("@/lib/entity-focus-context", () => {
  const actions = {
    setFocus: vi.fn(),
    registerScope: vi.fn(),
    unregisterScope: vi.fn(),
    getScope: vi.fn(),
  };
  return {
    useEntityFocus: () => ({
      focusedFq: null,
      setFocusedMoniker: vi.fn(),
    }),
    useFocusActions: () => actions,
    useOptionalFocusActions: () => actions,
    useEntityScopeRegistration: () => {},
    useFocusedMoniker: () => null,
    useFocusedMonikerRef: () => ({ current: null }),
    useIsFocused: () => false,
    useIsDirectFocus: () => false,
    useOptionalIsDirectFocus: () => false,
  };
});

// ---------------------------------------------------------------------------
// Imports — after mocks.
// ---------------------------------------------------------------------------

import { InspectorsContainer } from "./inspectors-container";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function uiStateWithStack(stack: string[]) {
  return {
    keymap_mode: "cua" as const,
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {
      main: {
        board_path: "/test",
        inspector_stack: stack,
        active_view_id: "",
        active_perspective_id: "",
        palette_open: false,
        palette_mode: "command" as const,
      },
    },
    recent_boards: [],
  };
}

async function flush() {
  await act(async () => {
    await Promise.resolve();
  });
}

const WINDOW_LAYER_NAME = asSegment("window");

function renderInspectors(extraChildren?: React.ReactNode) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <InspectorsContainer />
        {extraChildren}
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `dispatch_command` invocation, in order. */
function backendDispatchCalls(): Array<{ cmd: string }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as { cmd: string });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("app.dismiss — topmost-layer aware (frontend dispatch path)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    });
    mockEntitiesByType.mockReturnValue({});
  });

  it("backdrop click with an inspector open dispatches app.dismiss (NOT ui.inspector.close_all)", async () => {
    // Inspector panel open — backdrop is rendered.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));
    const { container, unmount } = renderInspectors();
    await flush();

    const backdrop = container.querySelector(".fixed.inset-0");
    expect(backdrop).not.toBeNull();

    fireEvent.click(backdrop!);
    await flush();

    // The frontend dispatched `app.dismiss` to the backend — NOT
    // `ui.inspector.close_all`. The backend's `DismissCmd` decides
    // which layer to close based on the topmost-layer rule.
    const calls = backendDispatchCalls();
    expect(calls.find((c) => c.cmd === "app.dismiss")).toBeDefined();
    expect(
      calls.find((c) => c.cmd === "ui.inspector.close_all"),
    ).toBeUndefined();

    unmount();
  });

  it("with no inspector open, no backdrop is rendered (so no app.dismiss can fire from the backdrop)", async () => {
    // Inspector stack is empty — backdrop must not be in the DOM.
    // This corresponds to the "only window layer mounted, app.dismiss
    // is a no-op" behavior on the frontend side: there's no surface
    // that would dispatch `app.dismiss` for a click-outside gesture.
    mockUIState.mockReturnValue(uiStateWithStack([]));
    const { container, unmount } = renderInspectors();
    await flush();

    const backdrop = container.querySelector(".fixed.inset-0");
    expect(backdrop).toBeNull();

    // No backend dispatches at all from the empty render.
    const dismissCalls = backendDispatchCalls().filter(
      (c) => c.cmd === "app.dismiss",
    );
    expect(dismissCalls).toEqual([]);

    unmount();
  });

  it("opening multiple inspectors keeps the same backdrop semantics — backdrop click still dispatches app.dismiss", async () => {
    // Multiple panels stacked. The backdrop is still the
    // click-outside surface; clicking it must still dispatch
    // `app.dismiss` (not, say, a per-panel close).
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1", "task:t2"]));
    const { container, unmount } = renderInspectors();
    await flush();

    const backdrop = container.querySelector(".fixed.inset-0");
    expect(backdrop).not.toBeNull();

    fireEvent.click(backdrop!);
    await flush();

    const calls = backendDispatchCalls();
    expect(calls.find((c) => c.cmd === "app.dismiss")).toBeDefined();
    expect(
      calls.find((c) => c.cmd === "ui.inspector.close_all"),
    ).toBeUndefined();

    unmount();
  });
});
