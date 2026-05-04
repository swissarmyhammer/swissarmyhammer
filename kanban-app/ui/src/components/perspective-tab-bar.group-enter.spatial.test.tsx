/**
 * Browser-mode test: Enter on the focused `perspective_tab.group:{id}`
 * leaf opens the group popover by firing the `onOpenChange(true)`
 * callback.
 *
 * Source of truth for the GroupPopoverButton half of card
 * `01KQQSVS4EBKKFN5SS7MW5P8CN` (Migrate remaining icon-button sites part
 * 2). Pre-migration the GroupPopoverButton was a bare `<button>` inside
 * the Radix `<PopoverTrigger asChild>` slot — keyboard users could not
 * focus it via spatial-nav and Enter on the surrounding tab leaf had no
 * `pressable.activate` CommandDef.
 *
 * The migration (covered by the same PerspectiveTabFocusable
 * `<FocusScope>` → `<FocusScope>` reshape as the FilterButton path):
 *   1. Promotes `PerspectiveTabFocusable` to `<FocusScope>` with
 *      `showFocusBar={false}`.
 *   2. Adds `<Pressable asChild moniker="perspective_tab.group:{id}">`
 *      inside the existing `<PopoverTrigger asChild>` slot. Because
 *      Radix Slot composes inner-then-slot, Pressable's outer
 *      `<FocusScope>` wraps the Popover trigger button.
 *
 * End-to-end keyboard-activation chain mirrors the filter-enter test:
 *
 *   1. PerspectiveTabBar renders GroupPopoverButton via
 *      `<PopoverTrigger asChild><Pressable asChild>...</Pressable></PopoverTrigger>`.
 *   2. Pressable mounts a `<FocusScope perspective_tab.group:{id}>` leaf
 *      with two scope-level CommandDefs (Enter + Space) calling
 *      `onPress` (which calls `onOpenChange(true)`).
 *   3. Driving `focus-changed` to the leaf populates the focused-scope
 *      chain.
 *   4. Enter dispatches `pressable.activate` → `onPress` runs →
 *      `setGroupOpen(true)` flips, the Popover content mounts.
 *
 * Asserts:
 *   - The leaf registers as a `<FocusScope>` with the correct moniker.
 *   - Enter on the focused leaf opens the popover (the popover
 *     content host appears in the DOM).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

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
// Group selector mock — keeps Popover content lightweight and gives the
// test a stable testid to query for after the popover opens.
// ---------------------------------------------------------------------------

vi.mock("@/components/group-selector", () => ({
  GroupSelector: () => <div data-testid="group-selector-mock" />,
}));

// ---------------------------------------------------------------------------
// Perspective + view + UI mocks — match perspective-tab-bar.add-enter
// shape so the bar mounts without surprise.
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

describe("PerspectiveTabBar group button — Enter opens popover via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on perspective_tab.group:p1 → Enter opens the group popover", async () => {
    const { queryByTestId } = await renderTabBar();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "perspective_tab.group:p1",
    );
    expect(
      leaf,
      "perspective_tab.group:p1 must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    // Popover is closed before Enter — content is not in the DOM.
    expect(queryByTestId("group-selector-mock")).toBeNull();

    // Drive a focus-changed event for the group leaf so the entity-focus
    // bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: "perspective_tab.group:p1",
        },
      });
      currentFocusKey.key = leaf!.fq as string;
      await Promise.resolve();
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });

    // Popover content mounts when `onOpenChange(true)` fires.
    await waitFor(() => {
      expect(
        queryByTestId("group-selector-mock"),
        "Enter on the focused group leaf must open the group popover",
      ).not.toBeNull();
    });
  });
});
