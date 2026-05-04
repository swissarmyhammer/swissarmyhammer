/**
 * Browser-mode test: Enter on the focused `ui:leftnav.view:{id}` leaf
 * dispatches `view.set` exactly once with `view_id` in `args`.
 *
 * Source of truth for the left-nav `ScopedViewButton` half of card
 * `01KQQSVS4EBKKFN5SS7MW5P8CN` (Migrate remaining icon-button sites part
 * 2: left-nav + perspective-tab-bar Filter/Group). Pre-migration each
 * view button was wrapped in a hand-rolled `<FocusScope view:{id}>`
 * with a manually-built `view.activate` CommandDef. The Pressable
 * migration replaces the inner FocusScope+button with
 * `<Pressable asChild moniker="ui:leftnav.view:{id}">`, which mounts
 * the leaf and registers its own `pressable.activate` (Enter/Space)
 * CommandDefs under the leaf's scope. The outer
 * `<CommandScopeProvider moniker="view:{id}">` stays so the right-click
 * context menu still resolves the view scope chain.
 *
 * End-to-end keyboard-activation chain:
 *
 *   1. LeftNav renders each view button via `<Pressable asChild>`.
 *   2. Pressable mounts a `<FocusScope moniker="ui:leftnav.view:{id}">`
 *      leaf with two scope-level CommandDefs: vim/cua Enter and cua
 *      Space, both calling `onPress` (which dispatches `view.set`).
 *   3. The kernel emits `focus-changed` with the leaf's FQM →
 *      EntityFocusProvider mirrors that into the focused-scope store.
 *   4. AppShell's `KeybindingHandler` reads the focused scope's
 *      bindings via `extractScopeBindings(focusedScope)`, sees
 *      `pressable.activate` bound to Enter, dispatches it through
 *      `useDispatchCommand`.
 *   5. The CommandDef's `execute` fires `onPress` → which dispatches
 *      `view.set` with `args: { view_id }` via the real dispatch
 *      chain (no mock layer in between).
 *
 * Asserts: `dispatch_command` is invoked exactly once with
 * `cmd: "view.set"` and `args: { view_id: "v1" }`.
 *
 * Mirrors `nav-bar.inspect-enter.spatial.test.tsx` exactly except for
 * the host component (`<LeftNav>`), the moniker
 * (`ui:leftnav.view:{id}`), and the dispatched command (`view.set`).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import type { ViewDef } from "@/types/kanban";

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
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_scope") {
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
// Views-context mock — must point at the same module bindings the
// component imports. We do NOT mock `@/lib/command-scope` — the test
// wants the real dispatch path.
// ---------------------------------------------------------------------------

const V1: ViewDef = { id: "v1", name: "View 1", kind: "board", icon: "kanban" };
const V2: ViewDef = { id: "v2", name: "View 2", kind: "grid", icon: "table" };

let mockViewsValue: {
  views: ViewDef[];
  activeView: ViewDef | null;
  setActiveViewId: (id: string) => void;
  refresh: () => Promise<void>;
} = {
  views: [V1, V2],
  activeView: V1,
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
  useFieldValue: () => "",
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
  useOpenBoards: () => [],
  useActiveBoardPath: () => undefined,
  useHandleSwitchBoard: () => vi.fn(),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

// Imports after mocks
import { LeftNav } from "./left-nav";
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

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Render `<LeftNav>` inside the production-shaped provider stack. */
async function renderLeftNav() {
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
                      <LeftNav />
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

describe("LeftNav view button — Enter activates view.set via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
    mockViewsValue = {
      views: [V1, V2],
      activeView: V1,
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  it("seeds focus on ui:leftnav.view:v1 → Enter dispatches view.set with view_id v1", async () => {
    await renderLeftNav();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:leftnav.view:v1",
    );
    expect(
      leaf,
      "ui:leftnav.view:v1 must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    // Drive a focus-changed event for the v1 leaf so the entity-focus
    // bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: "ui:leftnav.view:v1",
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

    const setCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "view.set",
    );
    expect(
      setCalls.length,
      "Enter on the focused view leaf must dispatch view.set exactly once",
    ).toBe(1);
    expect(
      setCalls[0].args,
      "view.set must carry view_id in args",
    ).toEqual({ view_id: "v1" });
  });
});
