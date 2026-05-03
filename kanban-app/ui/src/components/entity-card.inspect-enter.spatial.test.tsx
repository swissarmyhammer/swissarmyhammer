/**
 * Spatial-nav test: Enter on the focused `card.inspect:{id}` leaf
 * dispatches `ui.inspect` exactly once with the card's moniker as
 * target — and clicking the (i) button does the same thing without
 * also triggering the parent card's click handler.
 *
 * Source of truth for the entity-card half of the reopen-scope expansion
 * on card `01KQM9BGN0HFQSC168YD9G82Z2` (Add `<Pressable>` primitive).
 * Pre-migration, the card's (i) button was wrapped in a bare
 * `<FocusScope>` with an inner `<button onClick={…}>` — keyboard focus
 * landed on the leaf but Enter did NOTHING (the kernel's drillIn echoes
 * the focused FQM for a leaf, `setFocus` is idempotent, the visible
 * effect is a no-op). The Pressable migration adds the missing
 * scope-level CommandDef so Enter / Space dispatches `ui.inspect`.
 *
 * Two parallel paths must both fire `ui.inspect` exactly once and
 * neither must propagate to the parent card's click handler:
 *
 *   1. Keyboard: focus the leaf, press Enter.
 *   2. Pointer: click the rendered (i) button.
 *
 * Mock pattern mirrors `nav-bar.inspect-enter.spatial.test.tsx` —
 * Tauri kernel simulator + real dispatch path (no `useDispatchCommand`
 * mocking) so we watch `dispatch_command` IPC calls directly.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports. Mirrors
// `app-shell.test.tsx`'s kernel simulator so `spatial_focus` emits
// `focus-changed` and the entity-focus bridge populates the focused
// scope.
// ---------------------------------------------------------------------------

const monikerToKey = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };
const listenCallbacks: Record<string, (event: unknown) => void> = {};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    body_field: "body",
    fields: ["title", "status", "body"],
    sections: [{ id: "header", on_card: true }, { id: "body" }],
  },
  fields: [
    {
      id: "f-title",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f-status",
      name: "status",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f-body",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
} as unknown as EntitySchema;

function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
  if (cmd === "list_entity_types") return Promise.resolve(["task"]);
  if (cmd === "get_entity_schema") return Promise.resolve(TASK_SCHEMA);
  if (cmd === "get_undo_state")
    return Promise.resolve({ can_undo: false, can_redo: false });
  if (cmd === "list_commands_for_scope") return Promise.resolve([]);
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
  emit: vi.fn(() => Promise.resolve()),
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
// Imports come after mocks. Crucially we do NOT mock
// `@/lib/command-scope` — the test wants the real dispatch path so we
// can prove keyboard activation drives the same `dispatch_command` IPC
// the click handler does.
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { EntityCard } from "./entity-card";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
import { invoke } from "@tauri-apps/api/core";

const WINDOW_LAYER_NAME = asSegment("window");

function makeTask(): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
    fields: {
      title: "Hello",
      status: "todo",
      body: "",
      depends_on: [],
      position_column: "col-1",
      position_ordinal: "a0",
    },
  };
}

async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/** Render an `<EntityCard>` inside the production-shaped provider stack. */
async function renderCard() {
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
                      <SchemaProvider>
                        <EntityStoreProvider entities={{ task: [makeTask()] }}>
                          <FieldUpdateProvider>
                            <EntityCard entity={makeTask()} />
                          </FieldUpdateProvider>
                        </EntityStoreProvider>
                      </SchemaProvider>
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

describe("EntityCard inspect button — Enter activates ui.inspect via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on card.inspect:{id} → Enter dispatches ui.inspect once with card moniker", async () => {
    await renderCard();
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "card.inspect:task-1",
    );
    expect(
      leaf,
      "card.inspect:task-1 must register as a FocusScope leaf via Pressable",
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
          next_segment: "card.inspect:task-1",
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
      "ui.inspect must carry the card's moniker as target",
    ).toBe("task:task-1");
  });

  it("clicking the (i) button dispatches ui.inspect once and does NOT bubble to the card zone's spatial_focus", async () => {
    // The card body is wrapped in a `<FocusZone>` whose onClick handler
    // calls `focus(cardFq)` to make the card the spatial focus. If a
    // click on the (i) button bubbled up, the card zone's onClick would
    // fire `spatial_focus(cardFq)` — moving the focus to the card
    // instead of the inspect leaf, the wrong UX. The migration to
    // `<Pressable asChild>` keeps the inner button's
    // `onClick={(e) => e.stopPropagation()}` so propagation stops at
    // the (i) button. Radix Slot's `mergeProps` runs the child's
    // `onClick` first, then the slot's — so `e.stopPropagation()` lands
    // BEFORE Pressable's `handleClick` triggers `onPress` (dispatch).
    // Both behaviours hold in the correct order.
    const { container } = await renderCard();
    await flushSetup();

    // Discover the card zone's FQM from the registered zones — it's
    // what would be passed to `spatial_focus` if the click bubbled.
    const cardZoneFq = monikerToKey.get("task:task-1");
    expect(cardZoneFq, "card zone must register").toBeTruthy();

    const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
    mockInvoke.mockClear();

    const inspectButton = container.querySelector(
      'button[aria-label="Inspect"]',
    ) as HTMLButtonElement | null;
    expect(inspectButton, "(i) button must render with aria-label").toBeTruthy();

    await act(async () => {
      fireEvent.click(inspectButton!);
      await Promise.resolve();
    });

    const inspectCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "ui.inspect",
    );
    expect(
      inspectCalls.length,
      "Clicking (i) must dispatch ui.inspect exactly once",
    ).toBe(1);
    expect(inspectCalls[0].target).toBe("task:task-1");

    // `spatial_focus(cardFq)` would only be called if the click bubbled
    // to the card zone's onClick handler. With e.stopPropagation
    // preserved on the inner button, it must not.
    const focusToCardCalls = mockInvoke.mock.calls.filter(
      (c: unknown[]) =>
        c[0] === "spatial_focus" &&
        (c[1] as { fq?: string })?.fq === cardZoneFq,
    );
    expect(
      focusToCardCalls.length,
      "click on (i) must NOT bubble into the card zone's spatial_focus",
    ).toBe(0);
  });
});
