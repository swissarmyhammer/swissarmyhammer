/**
 * Browser-mode test: Enter on the focused `perspective_tab.filter:{id}`
 * leaf moves keyboard focus into the formula bar by firing the
 * `onFilterFocus` callback.
 *
 * Source of truth for the FilterButton half of card
 * `01KQQSVS4EBKKFN5SS7MW5P8CN` (Migrate remaining icon-button sites part
 * 2). Pre-migration the FilterButton was a bare `<button>` rendered
 * inside the per-tab `<FocusScope perspective_tab:{id}>` leaf — there
 * was no separate spatial leaf for the icon, and Enter on the tab leaf
 * fired no `onPress` because the tab leaf carried no
 * `pressable.activate` CommandDef.
 *
 * The migration:
 *   1. Promotes `PerspectiveTabFocusable` from `<FocusScope>` to
 *      `<FocusZone>` with `showFocusBar={false}` (entity-card
 *      iteration-2 reshape precedent).
 *   2. Wraps the existing `<TabButton>` in an inner
 *      `<FocusScope perspective_tab.name:{id}>` leaf so the name button
 *      stays focusable.
 *   3. Replaces the FilterButton bare `<button>` with
 *      `<Pressable asChild moniker="perspective_tab.filter:{id}">` which
 *      mounts its own leaf with `pressable.activate` Enter/Space
 *      CommandDefs.
 *
 * End-to-end keyboard-activation chain mirrors
 * `nav-bar.inspect-enter.spatial.test.tsx`:
 *
 *   1. PerspectiveTabBar renders FilterButton via `<Pressable asChild>`.
 *   2. Pressable mounts a `<FocusScope perspective_tab.filter:{id}>`
 *      leaf with two scope-level CommandDefs (Enter + Space) calling
 *      `onPress` (which calls `onFocus` → `filterEditorRef.current?.focus()`).
 *   3. Driving `focus-changed` to the leaf populates the focused-scope
 *      chain.
 *   4. `KeybindingHandler` resolves Enter through `extractScopeBindings`,
 *      dispatches `pressable.activate` → `onPress` runs → the formula
 *      bar's `FilterEditor` ref's `.focus()` is called.
 *
 * Asserts:
 *   - The leaf registers as a `<FocusScope>` with the correct moniker.
 *   - Enter on the focused leaf invokes `FilterEditorHandle.focus()`
 *     exactly once.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
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
// Filter-editor mock — captures `.focus()` calls so the test can assert
// that Enter on the focused filter leaf moves keyboard focus into the
// formula bar via the `onFilterFocus` callback.
// ---------------------------------------------------------------------------

const filterEditorFocusSpy = vi.fn();

vi.mock("@/components/filter-editor", async () => {
  const React = await import("react");
  type FilterEditorHandle = {
    focus(): void;
    setValue(text: string): void;
    getValue(): string;
  };
  const FilterEditor = React.forwardRef<
    FilterEditorHandle,
    { filter: string; perspectiveId: string }
  >(function FilterEditor(_props, ref) {
    React.useImperativeHandle(
      ref,
      () => ({
        focus() {
          filterEditorFocusSpy();
        },
        setValue() {},
        getValue() {
          return "";
        },
      }),
      [],
    );
    return <div data-testid="filter-editor-mock" />;
  });
  return {
    FilterEditor,
  };
});

// ---------------------------------------------------------------------------
// Perspective + view + UI mocks — match perspective-tab-bar.add-enter
// shape so the bar mounts without surprise. We do NOT mock
// `@/lib/command-scope` — the test wants the real dispatch path.
// ---------------------------------------------------------------------------

const mockPerspectivesValue = {
  perspectives: [{ id: "p1", name: "Sprint", view: "board" }],
  activePerspective: { id: "p1", name: "Sprint", view: "board" },
  setActivePerspectiveId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
}));

const mockViewsValue = {
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
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

// Imports after mocks
import { PerspectiveTabBar } from "./perspective-tab-bar";
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

async function renderTabBar() {
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
                      <PerspectiveTabBar />
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

function registerScopeArgs(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "spatial_register_scope")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

describe("PerspectiveTabBar filter button — Enter focuses formula bar via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    filterEditorFocusSpy.mockClear();
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on perspective_tab.filter:p1 → Enter calls FilterEditorHandle.focus() once", async () => {
    await renderTabBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab.filter:p1",
    );
    expect(
      leaf,
      "perspective_tab.filter:p1 must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    // Drive a focus-changed event for the filter leaf so the entity-focus
    // bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: "perspective_tab.filter:p1",
        },
      });
      currentFocusKey.key = leaf!.fq as string;
      await Promise.resolve();
    });

    // Reset the spy so we measure only what Enter triggers.
    filterEditorFocusSpy.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });

    expect(
      filterEditorFocusSpy.mock.calls.length,
      "Enter on the focused filter leaf must call FilterEditor.focus() exactly once",
    ).toBe(1);
  });
});
