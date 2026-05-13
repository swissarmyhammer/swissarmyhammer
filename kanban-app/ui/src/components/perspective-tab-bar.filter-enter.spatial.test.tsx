/**
 * Browser-mode test: Enter on the focused Filter `<CommandButton>` leaf
 * dispatches `perspective.filter.focus` through the unified command pipeline.
 *
 * Post-migration to command-driven rendering (task 01KRE1YA65MMG29RDQDQ0VPJQG):
 * the hardcoded `<FilterFocusButton>` (and its
 * `perspective_tab.filter:{id}` leaf + local `onPress → onFocus` callback)
 * is gone. The Filter affordance is now a registry-rendered
 * `<CommandButton>` for the no-arg `perspective.filter.focus` command,
 * registering its leaf as `perspective_tab.perspective.filter.focus:{id}`.
 *
 * End-to-end keyboard-activation chain mirrors the old behaviour, but
 * the side-effect now flows through dispatch:
 *
 *   1. `<RegistryTabButtons>` mounts a `<CommandButton>` for every
 *      tab-button-tagged command emitted by `list_commands_for_scope`.
 *   2. `<CommandButton>` wraps the icon in `<Pressable asChild>` which
 *      mounts a `<FocusScope perspective_tab.perspective.filter.focus:{id}>`
 *      leaf with `pressable.activate` Enter/Space CommandDefs.
 *   3. Driving `focus-changed` to that leaf populates the focused-scope
 *      chain.
 *   4. `KeybindingHandler` resolves Enter through `extractScopeBindings`,
 *      dispatches `pressable.activate` → `onPress` runs → the
 *      `<CommandButton>` dispatches `perspective.filter.focus` via
 *      `useDispatchCommand`.
 *
 * The actual editor focus is now driven by the backend's
 * `ui.focus.filter` event — the spatial half is what this test pins.
 * Editor-side focus reactivity is covered by
 * `perspective-tab-bar.filter-migration.test.tsx`.
 *
 * Asserts:
 *   - The new leaf registers as a `<FocusScope>` with the
 *     `perspective_tab.perspective.filter.focus:{id}` moniker.
 *   - Enter on the focused leaf calls `dispatch_command` with
 *     `cmd: "perspective.filter.focus"` exactly once.
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
  // The tab bar queries this on mount/effect. Returning the new
  // command exactly mirrors what the YAML produces — the
  // `<CommandButton>` only renders when this list contains a
  // `tab_button`-tagged command.
  if (cmd === "list_commands_for_scope") {
    return Promise.resolve([
      {
        id: "perspective.filter.focus",
        name: "Focus Filter",
        tab_button: { icon: "filter" },
        params: [{ name: "perspective_id", from: "scope_chain" }],
        keys: {},
      },
    ]);
  }
  if (cmd === "spatial_register_scope") {
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
// Filter-editor mock — the tab bar transitively pulls in `<FilterEditor>`
// through the formula bar. Stub it so this test does not need to set up
// the editor's full provider stack; the dispatch contract is what we
// assert here.
// ---------------------------------------------------------------------------

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
        focus() {},
        setValue() {},
        getValue() {
          return "";
        },
      }),
      [],
    );
    return <div data-testid="filter-editor-mock" />;
  });
  const FilterExpressionEditor = React.forwardRef<unknown, unknown>(
    function FilterExpressionEditor() {
      return <div data-testid="filter-expression-editor-mock" />;
    },
  );
  return {
    FilterEditor,
    FilterExpressionEditor,
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
  useBoardData: () => ({
    board: {
      entity_type: "board",
      id: "test-board",
      moniker: "board:test-board",
      fields: {},
    },
    virtualTagMeta: [],
  }),
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
  // The registry-driven button mounts inside a `useEffect` that awaits
  // `list_commands_for_scope`. Yield multiple times so the Promise
  // resolution, `setCommands`, and the inner Pressable's register
  // effects all settle before the test queries the registry.
  for (let i = 0; i < 4; i += 1) {
    // eslint-disable-next-line no-await-in-loop
    await act(async () => {
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    });
  }
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

describe("PerspectiveTabBar filter button — Enter dispatches perspective.filter.focus via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on perspective_tab.perspective.filter.focus:p1 → Enter dispatches perspective.filter.focus exactly once", async () => {
    await renderTabBar();
    await flushSetup();

    // The new spatial-nav moniker, derived by `<CommandButton>` from
    // `${surface}.${command.id}:${surfaceId}` — replaces the legacy
    // `perspective_tab.filter:{id}` from the deleted `<FilterFocusButton>`.
    const leaf = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab.perspective.filter.focus:p1",
    );
    expect(
      leaf,
      "perspective_tab.perspective.filter.focus:p1 must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    // Drive a focus-changed event for the new leaf so the entity-focus
    // bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: "perspective_tab.perspective.filter.focus:p1",
        },
      });
      currentFocusKey.key = leaf!.fq as string;
      await Promise.resolve();
    });

    // Snapshot the dispatch_command call count BEFORE the Enter press so
    // we can isolate what Enter triggers from any startup dispatches.
    const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
    const dispatchCallsBefore = mockInvoke.mock.calls.filter(
      (c: unknown[]) => c[0] === "dispatch_command",
    ).length;

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });

    const dispatchCallsAfter = mockInvoke.mock.calls.filter(
      (c: unknown[]) => c[0] === "dispatch_command",
    );
    expect(
      dispatchCallsAfter.length - dispatchCallsBefore,
      "Enter on the focused filter leaf must dispatch exactly one command",
    ).toBe(1);
    // The dispatched command must be the new `perspective.filter.focus` —
    // a regression that bound Enter to a different command (e.g. a stale
    // `perspective.filter`) would fail here.
    expect(dispatchCallsAfter[dispatchCallsAfter.length - 1][1]).toMatchObject({
      cmd: "perspective.filter.focus",
    });
  });
});
