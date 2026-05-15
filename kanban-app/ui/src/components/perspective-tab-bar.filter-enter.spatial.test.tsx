/**
 * Browser-mode test: Enter on the focused Filter `<CommandButton>` leaf
 * claims spatial-nav focus on the formula bar via `nav.focus`.
 *
 * Post-migration to command-driven rendering (task 01KRE1YA65MMG29RDQDQ0VPJQG):
 * the hardcoded `<FilterFocusButton>` (and its
 * `perspective_tab.filter:{id}` leaf + local `onPress → onFocus` callback)
 * is gone. The Filter affordance is now a registry-rendered
 * tab-button leaf registered at `perspective_tab.perspective.filter.focus:{id}`.
 *
 * Post-rewire to `nav.focus` (task 01KRGZY33P99J7CGG0XRQGZ352): the
 * click site is a small `<FilterFocusCommandButton>` adapter (in
 * `perspective-tab-bar.tsx`) that mirrors `<CommandButton>`'s render
 * (icon, isActive, moniker) but overrides the dispatch — instead of
 * issuing `perspective.filter.focus` through `dispatch_command`, it
 * dispatches the frontend-only `nav.focus({ args: { fq } })` against
 * the formula bar's `filter_editor:${id}` spatial-nav scope. The
 * `nav.focus` execute closure (registered in `<SpatialFocusProvider>`)
 * then calls `actions.focus(fq)`, which issues a `spatial_focus` IPC
 * to the kernel.
 *
 * End-to-end keyboard-activation chain:
 *
 *   1. `<RegistryTabButtons>` mounts a `<FilterFocusCommandButton>`
 *      for the `perspective.filter.focus` registry entry.
 *   2. The adapter wraps the icon in `<Pressable asChild>` which
 *      mounts a `<FocusScope perspective_tab.perspective.filter.focus:{id}>`
 *      leaf with `pressable.activate` Enter/Space CommandDefs.
 *   3. Driving `focus-changed` to that leaf populates the focused-scope
 *      chain.
 *   4. `KeybindingHandler` resolves Enter through `extractScopeBindings`,
 *      dispatches `pressable.activate` → `onPress` runs → the adapter
 *      dispatches `nav.focus` (frontend execute) → `actions.focus(fq)`
 *      → `spatial_focus` IPC.
 *
 * Asserts:
 *   - The leaf registers as a `<FocusScope>` with the
 *     `perspective_tab.perspective.filter.focus:{id}` moniker.
 *   - Enter on the focused leaf produces a `spatial_focus` IPC whose
 *     `fq` ends with `filter_editor:p1` — the formula bar's scope for
 *     the active perspective.
 *   - Enter does NOT route through the deleted
 *     `dispatch_command(perspective.filter.focus)` backend path.
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

describe("PerspectiveTabBar filter button — Enter claims focus on the filter editor via nav.focus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on perspective_tab.perspective.filter.focus:p1 → Enter dispatches nav.focus to filter_editor:p1", async () => {
    await renderTabBar();
    await flushSetup();

    // The spatial-nav moniker derived from
    // `${surface}.${command.id}:${surfaceId}` — replaces the legacy
    // `perspective_tab.filter:{id}` from the deleted `<FilterFocusButton>`.
    const leaf = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab.perspective.filter.focus:p1",
    );
    expect(
      leaf,
      "perspective_tab.perspective.filter.focus:p1 must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    // Drive a focus-changed event for the leaf so the entity-focus
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

    // Snapshot the `spatial_focus` IPC call count BEFORE Enter so we
    // isolate Enter's contribution from any startup focus traffic.
    const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
    const spatialFocusBefore = mockInvoke.mock.calls.filter(
      (c: unknown[]) => c[0] === "spatial_focus",
    ).length;

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
      await Promise.resolve();
    });

    const spatialFocusAfter = mockInvoke.mock.calls.filter(
      (c: unknown[]) => c[0] === "spatial_focus",
    );
    expect(
      spatialFocusAfter.length - spatialFocusBefore,
      "Enter on the focused filter leaf must produce at least one spatial_focus IPC",
    ).toBeGreaterThan(0);

    // The last spatial_focus call carries the filter editor's FQM as
    // `fq` — `nav.focus` (the command bound to the Pressable's
    // `onPress`) resolves to `actions.focus(fq)`, which issues this
    // IPC. The path-tail must end at `filter_editor:p1` so a
    // regression that pointed `nav.focus` at the wrong scope (e.g. the
    // button leaf itself) is caught here.
    const lastSpatialCall = spatialFocusAfter[spatialFocusAfter.length - 1];
    const fq = (lastSpatialCall[1] as { fq?: string })?.fq ?? "";
    expect(
      fq.endsWith("filter_editor:p1"),
      `spatial_focus.fq must end with filter_editor:p1 (got ${fq})`,
    ).toBe(true);

    // Negative: the deleted backend channel must NOT be reached.
    // A regression that re-binds Enter to `dispatch_command(perspective.filter.focus)`
    // would fail this assertion.
    const filterFocusBackend = mockInvoke.mock.calls.filter(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as { cmd?: string })?.cmd === "perspective.filter.focus",
    );
    expect(
      filterFocusBackend,
      "Enter must not dispatch the deleted perspective.filter.focus backend command",
    ).toHaveLength(0);
  });
});
