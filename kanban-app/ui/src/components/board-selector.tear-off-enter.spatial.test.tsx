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
import { render, fireEvent, act } from "@testing-library/react";
import type { OpenBoard } from "@/types/kanban";

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
    monikerToKey.clear();
    currentFocusKey.key = null;
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
