/**
 * Browser-mode test: Enter on the focused `board-selector.tear-off` leaf
 * dispatches `window.new` exactly once.
 *
 * Source of truth for the tear-off-button half of card
 * `01KQPZAFSPJEMHMKRSQGPD0JM6` (Migrate remaining icon-button sites to
 * `<Pressable>`). Pre-migration this site was a `<FocusScope>` wrapping a
 * `<Tooltip><Button onClick={dispatchNewWindow}>` chain — keyboard users
 * could focus the leaf but Enter was a no-op (`nav.drillIn` returns the
 * focused FQM, `setFocus` is idempotent, the visible effect was
 * nothing). The Pressable migration adds the two scope-level CommandDefs
 * (vim/cua Enter and cua Space) so Enter on the focused leaf invokes
 * the same `window.new` dispatch as the click.
 *
 * Mirrors `nav-bar.inspect-enter.spatial.test.tsx` exactly except for
 * the moniker (`board-selector.tear-off`), the host component
 * (`<BoardSelector showTearOff>`), and the dispatched command
 * (`window.new`).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  answerListCommand,
  globalCommandsFromBindingTables,
} from "@/test/mock-command-list";
import {
  UNHANDLED,
  emitToCallbackRecord,
  makeSpatialKernelMock,
} from "@/test/mock-spatial-kernel";
import { render, fireEvent, act } from "@testing-library/react";
import type { OpenBoard } from "@/types/kanban";

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
// Schema + entity-store mocks — keep BoardSelector mounting in the
// narrow provider tree without surprise. The schema returns a minimal
// board shape so the editable name `<Field>` does not need to resolve.
// ---------------------------------------------------------------------------

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
  SchemaProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useFieldValue: () => "",
  useEntityStore: () => ({ getEntities: () => [] }),
  EntityStoreProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

// Imports after mocks
import { BoardSelector } from "./board-selector";
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

const MOCK_OPEN_BOARDS: OpenBoard[] = [
  { path: "/boards/a/.kanban", name: "Board A", is_active: true },
];

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Render `<BoardSelector showTearOff>` inside the production-shaped
 * provider stack so the `window.new` dispatch routes through the real
 * command-scope chain (no mocked `useDispatchCommand`).
 */
async function renderBoardSelector() {
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
                      <BoardSelector
                        boards={MOCK_OPEN_BOARDS}
                        selectedPath={MOCK_OPEN_BOARDS[0].path}
                        onSelect={() => {}}
                        showTearOff
                      />
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

describe("BoardSelector tear-off button — Enter activates window.new via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    reset();
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on board-selector.tear-off → Enter dispatches window.new once", async () => {
    await renderBoardSelector();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "board-selector.tear-off",
    );
    expect(
      leaf,
      "board-selector.tear-off must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    // Drive a focus-changed event for the tear-off leaf so the
    // entity-focus bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: "board-selector.tear-off",
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

    const newWindowCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "window.new",
    );
    expect(
      newWindowCalls.length,
      "Enter on the focused tear-off leaf must dispatch window.new exactly once",
    ).toBe(1);
    expect(
      (newWindowCalls[0].args as Record<string, unknown>).board_path,
      "window.new must carry the selected board path",
    ).toBe("/boards/a/.kanban");
  });
});
