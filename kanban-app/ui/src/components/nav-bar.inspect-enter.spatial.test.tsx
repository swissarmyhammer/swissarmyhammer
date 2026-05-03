/**
 * Browser-mode test: Enter on the focused `ui:navbar.inspect` leaf
 * dispatches `ui.inspect` exactly once.
 *
 * Source of truth for the inspect-button half of card
 * `01KQM9BGN0HFQSC168YD9G82Z2` (Add `<Pressable>` primitive). Pins the
 * end-to-end keyboard-activation chain:
 *
 *   1. NavBar renders the Info button via `<Pressable asChild>`.
 *   2. Pressable mounts a `<FocusScope>` leaf with two scope-level
 *      CommandDefs: vim/cua Enter and cua Space, both calling onPress.
 *   3. The kernel emits `focus-changed` with the leaf's FQM →
 *      EntityFocusProvider mirrors that into the focused-scope store.
 *   4. AppShell's `KeybindingHandler` reads the focused scope's bindings
 *      via `extractScopeBindings(focusedScope)`, sees `pressable.activate`
 *      bound to Enter, dispatches it through `useDispatchCommand`.
 *   5. The CommandDef's `execute` fires `onPress` → which dispatches
 *      `ui.inspect` against the board's moniker via the real dispatch
 *      chain (no mock layer in between).
 *
 * Assertions: `dispatch_command` is invoked exactly once with
 * `cmd: "ui.inspect"` and `target` set to the board's moniker.
 *
 * Distinct from `nav-bar.spatial-nav.test.tsx`: that file mocks
 * `useDispatchCommand`, so it cannot prove keyboard activation drives
 * the same dispatch path the click handler does. This file does NOT
 * mock the dispatch hook — it watches `dispatch_command` IPC calls.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import type { BoardData, OpenBoard } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports. Mirrors
// `app-shell.test.tsx`'s kernel simulator so `spatial_focus` emits
// `focus-changed` and the entity-focus bridge populates the focused
// scope.
// ---------------------------------------------------------------------------

const monikerToKey = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };
const listenCallbacks: Record<string, (event: unknown) => void> = {};

function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
  if (cmd === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_zone") {
    const a = (args ?? {}) as { fq?: string; segment?: string };
    if (a.fq && a.segment) monikerToKey.set(a.segment, a.fq);
    return Promise.resolve(null);
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { fq?: string };
    if (a.fq) {
      for (const [m, k] of monikerToKey.entries()) {
        if (k === a.fq) {
          monikerToKey.delete(m);
          break;
        }
      }
    }
    return Promise.resolve(null);
  }
  if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
    const a = (args ?? {}) as { focusedFq?: string };
    return Promise.resolve(a.focusedFq ?? null);
  }
  if (cmd === "spatial_focus") {
    const a = (args ?? {}) as { fq?: string };
    const fq = a.fq ?? null;
    let moniker: string | null = null;
    for (const [s, k] of monikerToKey.entries()) {
      if (k === fq) {
        moniker = s;
        break;
      }
    }
    if (fq) {
      const prev = currentFocusKey.key;
      currentFocusKey.key = fq;
      queueMicrotask(() => {
        const cb = listenCallbacks["focus-changed"];
        if (cb) {
          cb({
            payload: {
              window_label: "main",
              prev_fq: prev,
              next_fq: fq,
              next_segment: moniker,
            },
          });
        }
      });
    }
    return Promise.resolve(null);
  }
  return Promise.resolve(null);
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: unknown) => defaultInvoke(cmd, args)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, cb: (event: unknown) => void) => {
    listenCallbacks[eventName] = cb;
    return Promise.resolve(() => {});
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

// ---------------------------------------------------------------------------
// WindowContainer + schema mocks — mirror nav-bar.spatial-nav.test.tsx
// so the bar mounts without surprise. Crucially we do NOT mock
// `@/lib/command-scope` — the test wants the real dispatch path.
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

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/components/fields/field", () => ({
  Field: (props: Record<string, unknown>) => (
    <span data-testid={`field-${String(props.entityId)}`} />
  ),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useFieldValue: () => "",
  useEntityStore: () => ({ getEntities: () => [] }),
}));

// Imports after mocks
import { NavBar } from "./nav-bar";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
import { invoke } from "@tauri-apps/api/core";

const WINDOW_LAYER_NAME = asSegment("window");

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
];

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Render `<NavBar>` inside the production-shaped provider stack. */
async function renderNavBar() {
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <EntityFocusProvider>
            <UIStateProvider>
              <AppModeProvider>
                <UndoProvider>
                  <AppShell>
                    <TooltipProvider delayDuration={100}>
                      <NavBar />
                    </TooltipProvider>
                  </AppShell>
                </UndoProvider>
              </AppModeProvider>
            </UIStateProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await Promise.resolve();
  });
  return result;
}

/** Collect every `spatial_register_scope` call's args. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "spatial_register_scope")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

/** Collect every `dispatch_command` call's args. */
function dispatchCommandCalls(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "dispatch_command")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

describe("NavBar inspect button — Enter activates ui.inspect via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
    mockBoardData.mockReturnValue(MOCK_BOARD);
    mockOpenBoards.mockReturnValue(MOCK_OPEN_BOARDS);
    mockActiveBoardPath.mockReturnValue("/boards/a/.kanban");
  });

  it("seeds focus on ui:navbar.inspect → Enter dispatches ui.inspect once", async () => {
    await renderNavBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:navbar.inspect",
    );
    expect(
      leaf,
      "ui:navbar.inspect must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    // Drive a focus-changed event for the inspect leaf so the
    // entity-focus bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: "ui:navbar.inspect",
        },
      });
      currentFocusKey.key = leaf!.fq as string;
      await Promise.resolve();
    });

    // Clear prior IPC noise; we only care about what Enter triggers.
    const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });

    const inspectCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "ui.inspect",
    );
    expect(
      inspectCalls.length,
      "Enter on the focused inspect leaf must dispatch ui.inspect exactly once",
    ).toBe(1);
    expect(
      inspectCalls[0].target,
      "ui.inspect must carry the board's moniker as target",
    ).toBe("board:b1");
  });
});
