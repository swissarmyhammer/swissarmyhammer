/**
 * Browser-mode test: Enter on the focused Add Perspective leaf opens
 * the registry-rendered `<CommandPopover>` (the new keyboard-activation
 * contract after card `01KRE21GJMPP289N1HSTMJG5HE`).
 *
 * # Migration history
 *
 * Card `01KQM9BGN0HFQSC168YD9G82Z2` (Add `<Pressable>` primitive)
 * first wired the hardcoded `<AddPerspectiveButton>` so keyboard users
 * could focus and activate it via Enter; activation dispatched
 * `perspective.save` directly with a frontend-computed name.
 *
 * Card `01KRE21GJMPP289N1HSTMJG5HE` (Add + Sort migration) deleted
 * `<AddPerspectiveButton>`. The `+` affordance is now a
 * registry-rendered `<CommandButton>` for `perspective.save` rendered
 * by `<BarRegistryTabButtons>` at the bar level. The CommandButton's
 * `handlePress` opens a `<CommandPopover>` (because the command has a
 * pickable `name` param) instead of dispatching directly — the user
 * fills in the form and submits to actually save the perspective.
 *
 * # Spatial-nav moniker
 *
 * Changed from `ui:perspective-bar.add` (the legacy Pressable's
 * deliberately-chosen segment) to
 * `perspective_bar.perspective.save:<view-id>` (the moniker
 * `<CommandButton>` builds from `${surface}.${command.id}:${surfaceId}`).
 * Same migration pattern as the Filter / Group buttons before it.
 *
 *   1. `<BarRegistryTabButtons>` queries the registry and renders one
 *      `<CommandButton>` per global tab-button command.
 *   2. `<CommandButton>` wraps the `<button>` in a `<Pressable>` with
 *      moniker `perspective_bar.perspective.save:<view-id>` and an
 *      `onPress` that opens the popover.
 *   3. AppShell's `KeybindingHandler` resolves Enter on the focused
 *      leaf through `extractScopeBindings`, dispatches
 *      `pressable.activate` → opens the popover.
 *
 * Asserts: the focused leaf registers under the new moniker AND Enter
 * opens the popover (`<CommandPopover>` mounts under the
 * `data-testid="command-popover"` attribute). The submit → dispatch
 * chain is covered by `perspective-tab-bar.add-and-sort-migration.test.tsx`
 * — pinning it here too would couple this spatial-nav test to the
 * picker-pipeline render path it doesn't own.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

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
  if (cmd === "list_commands_for_scope") {
    // After the Add migration, `<BarRegistryTabButtons>` queries the
    // registry for global tab-button commands. Return the
    // `perspective.save` payload so the registry-rendered
    // `<CommandButton>` mounts at the bar level and registers its
    // spatial-nav leaf.
    return Promise.resolve([
      {
        id: "perspective.save",
        name: "Save Perspective",
        group: "global",
        context_menu: false,
        available: true,
        tab_button: { icon: "plus" },
        params: [
          { name: "name", from: "args", shape: "text" },
          {
            name: "view_id",
            from: "scope_chain",
            entity_type: "view",
          },
        ],
        keys: {},
      },
    ]);
  }
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
// Perspective + view + UI mocks — match perspective-tab-bar.spatial-nav
// shape so the bar mounts without surprise. We do NOT mock
// `@/lib/command-scope` — the test wants the real dispatch path.
// ---------------------------------------------------------------------------

const mockPerspectivesValue = {
  perspectives: [] as Array<{ id: string; name: string; view: string }>,
  activePerspective: null,
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

function dispatchCommandCalls(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "dispatch_command")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

describe("PerspectiveTabBar add button — Enter activates the registry-rendered <CommandButton>", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on perspective_bar.perspective.save:<view-id> → Enter opens the popover", async () => {
    const result = await renderTabBar();
    await flushSetup();
    // Two extra microtask flushes cover the registry's async resolve →
    // setState → `<CommandButton>` mount → Pressable register chain.
    // The bar's `list_commands_for_scope` is the slow path here.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // The new moniker shape is `${surface}.${command.id}:${surfaceId}`
    // where `<BarRegistryTabButtons>` uses surface `perspective_bar` and
    // the active view id as the suffix.
    const expectedSegment = "perspective_bar.perspective.save:board-1";
    const leaf = registerScopeArgs().find(
      (a) => a.segment === expectedSegment,
    );
    expect(
      leaf,
      `${expectedSegment} must register as a FocusScope leaf via Pressable`,
    ).toBeDefined();

    // Drive a focus-changed event for the add leaf so the entity-focus
    // bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: expectedSegment,
        },
      });
      currentFocusKey.key = leaf!.fq as string;
      await Promise.resolve();
    });

    const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });

    // Enter on the focused leaf must open the popover — the new
    // contract for the registry-rendered tab button is "click /
    // keyboard-activate opens the popover", NOT immediate dispatch.
    // The popover-submit → `perspective.save` dispatch chain is
    // covered by `perspective-tab-bar.add-and-sort-migration.test.tsx`.
    //
    // Note: Radix Popover portals its content into `document.body`, NOT
    // inside `result.container` — query at the document level so the
    // portalled popover surface is visible to the assertion. Voiding
    // the unused `result` so the harness's render is still required for
    // its side-effects (mounting the bar + the popover root).
    void result;
    const popover = document.querySelector(
      '[data-testid="command-popover"]',
    );
    expect(
      popover,
      "Enter on the focused add leaf must open <CommandPopover>",
    ).toBeTruthy();

    // No `perspective.save` dispatch fires until the user submits the
    // popover — guards against a regression that wires Enter to both
    // the popover open AND a direct dispatch.
    const saveCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "perspective.save",
    );
    expect(
      saveCalls.length,
      "Enter on the focused add leaf must NOT dispatch perspective.save directly — it opens the popover first",
    ).toBe(0);
  });
});
