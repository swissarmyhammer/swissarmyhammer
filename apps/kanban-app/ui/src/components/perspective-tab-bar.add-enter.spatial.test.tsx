/**
 * Browser-mode test: Enter on the focused Add Perspective leaf
 * dispatches `perspective.save` IMMEDIATELY with a generated unique
 * name — no popup (the keyboard-activation contract after card
 * `01KTYN8GB25ZFKSXWA0QA283PG`).
 *
 * # Migration history
 *
 * Card `01KQM9BGN0HFQSC168YD9G82Z2` (Add `<Pressable>` primitive)
 * first wired the hardcoded `<AddPerspectiveButton>` so keyboard users
 * could focus and activate it via Enter; activation dispatched
 * `perspective.save` directly with a frontend-computed name.
 *
 * Card `01KRE21GJMPP289N1HSTMJG5HE` (Add + Sort migration) deleted
 * `<AddPerspectiveButton>` in favour of a registry-rendered
 * `<CommandButton>` whose press opened a `<CommandPopover>` (because
 * the command has a text-shaped `name` param).
 *
 * Card `01KTYN8GB25ZFKSXWA0QA283PG` deleted that popup path: the `+`
 * is now the `<AddPerspectiveCommandButton>` adapter whose press
 * creates the perspective immediately with a generated name
 * ("Untitled" / "Untitled N" — the first free slot by exact-name match
 * against the visible list) and arms inline rename once the entity
 * appears — full-circle back to the original immediate-dispatch UX,
 * but registry-rendered and command-routed.
 *
 * # Spatial-nav moniker
 *
 * Unchanged from the CommandButton era:
 * `perspective_bar.perspective.save:<view-id>` (the
 * `${surface}.${command.id}:${surfaceId}` shape).
 *
 *   1. `<BarRegistryTabButtons>` queries the registry and renders the
 *      `<AddPerspectiveCommandButton>` adapter for `perspective.save`.
 *   2. The adapter wraps the `<button>` in a `<Pressable>` with
 *      moniker `perspective_bar.perspective.save:<view-id>` and an
 *      `onPress` that creates immediately.
 *   3. AppShell's `KeybindingHandler` resolves Enter on the focused
 *      leaf through `extractChainBindings`, dispatches
 *      `pressable.activate` → immediate `perspective.save` dispatch.
 *
 * Asserts: the focused leaf registers under the moniker AND Enter
 * dispatches `perspective.save` with the generated name without
 * opening any popover. The dedupe / rename-arming contracts are
 * covered by `perspective-tab-bar.add-create-rename.test.tsx`.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  answerListCommand,
  globalCommandsFromBindingTables,
  UI_SURFACE_PLUGIN_COMMANDS,
} from "@/test/mock-command-list";
import {
  UNHANDLED,
  emitToCallbackRecord,
  makeSpatialKernelMock,
} from "@/test/mock-spatial-kernel";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports. Mirrors
// `app-shell.test.tsx`'s kernel simulator so `spatial_focus` emits
// `focus-changed` and the entity-focus bridge populates the focused
// scope.
// ---------------------------------------------------------------------------

const listenCallbacks: Record<string, (event: unknown) => void> = {};

const { currentFocusKey, handleSpatialCommand, reset } = makeSpatialKernelMock({
  emit: emitToCallbackRecord(listenCallbacks),
});

function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
  // The pressable activation commands are DEFINED by the `app-shell-commands`
  // builtin plugin (`pressable.activate` / `pressable.activateSpace`,
  // scope ["ui:pressable"]) — their Enter / Space keys reach the keymap
  // layer only through the `useCommandList` seam, so answer `list command`
  // with the shared mock registry. Non-list `command_tool_call` ops fall
  // through to the branches below.
  const listAnswer = answerListCommand(
    cmd,
    args,
    globalCommandsFromBindingTables(),
  );
  if (listAnswer) return listAnswer;
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
  const spatial = handleSpatialCommand(cmd, args);
  if (spatial !== UNHANDLED) return Promise.resolve(spatial);
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

// `<BarRegistryTabButtons>` sources global (unscoped) tab-button commands from
// the Command registry via `useCommandList`. Return the `perspective.save`
// payload (empty `scope` = global) so its `<CommandButton>` mounts at the bar
// level and registers its spatial-nav leaf. The `UI_SURFACE_PLUGIN_COMMANDS`
// mirror rides along so the keymap layer (which reads the same registry)
// binds Enter → `pressable.activate` on the focused `<CommandButton>` leaf —
// the entries carry no `tab_button`, so the bar renders no extra buttons.
vi.mock("@/hooks/use-command-list", () => ({
  useCommandList: () => ({
    commands: [
      {
        id: "perspective.save",
        name: "Save Perspective",
        scope: [],
        tab_button: { icon: "plus" },
        params: [
          { name: "name", from: "args", shape: "text" },
          { name: "view_id", from: "scope_chain", entity_type: "view" },
        ],
        keys: {},
      },
      ...UI_SURFACE_PLUGIN_COMMANDS,
    ],
    loading: false,
    refresh: vi.fn(),
  }),
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
    reset();
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on perspective_bar.perspective.save:<view-id> → Enter creates immediately", async () => {
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
    const leaf = registerScopeArgs().find((a) => a.segment === expectedSegment);
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

    // Enter on the focused leaf must dispatch `perspective.save`
    // IMMEDIATELY with a generated unique name — the popup path for `+`
    // was deleted by card 01KTYN8GB25ZFKSXWA0QA283PG.
    //
    // Note: Radix Popover would portal its content into `document.body`,
    // NOT inside `result.container` — query at the document level so a
    // regression re-introducing the popover is visible to the assertion.
    // Voiding the unused `result` so the harness's render is still
    // required for its side-effects (mounting the bar).
    void result;
    expect(
      document.querySelector('[data-testid="command-popover"]'),
      "Enter on the focused add leaf must NOT open a popover — the + popup path is deleted",
    ).toBeNull();

    const saveCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "perspective.save",
    );
    expect(
      saveCalls.length,
      "Enter on the focused add leaf must dispatch perspective.save immediately",
    ).toBe(1);
    expect(saveCalls[0]).toMatchObject({
      args: { name: "Untitled", view: "board", view_id: "board-1" },
    });
  });
});
