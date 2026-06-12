/**
 * Browser-mode test pinning the bug fix: pressing Space on a focused
 * inspector field zone dispatches the inspect with the field-led scope
 * chain — regardless of which layer the field is rendered in.
 *
 * Before card 01KQ9XJ4XGKVW24EZSQCA6K3E2, Space ownership lived on
 * `board.inspect` registered at the BoardView's `<CommandScopeProvider>`.
 * The inspector layer mounts as a sibling of the BoardView (not a
 * descendant), so a focused field zone's scope chain never reached the
 * board scope — Space did nothing on a focused field.
 *
 * After Card G the Space owner is the SINGLE plugin-owned `entity.inspect`
 * (`builtin/plugins/app-shell-commands/commands/ui.ts`): a global binding whose dispatch
 * carries the focused scope chain to the backend, where the plugin resolves
 * the innermost inspectable-ENTITY moniker. A focused field zone leads its
 * own chain, so the dispatch fires regardless of layer; server-side the
 * `field:{type}:{id}.{name}` projection moniker is skipped and the
 * CONTAINING entity wins (kanban card 01KTY6XTJQFCG9ENKTAMC6N3JV) — the
 * webview's job here is only the dispatch shape, which is what this test
 * pins.
 *
 * The test below mounts a real `<EntityInspector>`, simulates the
 * spatial kernel emitting a `focus-changed` event for the title field,
 * fires `keydown { key: " " }` at the document, and asserts exactly one
 * `dispatch_command` IPC fires for `entity.inspect` whose scope chain is
 * led by the field moniker (and zero webview-side `app.inspect`).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  return {
    ...actual,
    invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
  };
});

vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  return {
    ...actual,
    listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  };
});

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
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
// Imports come after mocks.
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { EntityInspector } from "./entity-inspector";
import { AppShell } from "./app-shell";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { commandToolCall } from "@/test/mock-command-list";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel,
} from "@/types/spatial";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds — minimal task schema with one editable field.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "body"],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "type",
      section: "header",
    },
    {
      id: "f2",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      icon: "file-text",
      section: "body",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  task: TASK_SCHEMA,
};

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  // The global keybinding table is registry-driven: `useCommandList`
  // fetches `list command` through `command_tool_call`, and Space resolves
  // from the plugin-owned `entity.inspect`'s `keys` (Card G). The shared
  // mock registry synthesizes that global set from `BINDING_TABLES`.
  if (cmd === "command_tool_call") return commandToolCall(args);
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    return SCHEMAS[entityType ?? ""] ?? TASK_SCHEMA;
  }
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

function makeTask(fields: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "T1",
    moniker: "task:T1",
    fields,
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait two ticks so mount-time effects flush before assertions. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Filter `dispatch_command` calls down to those for `app.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "app.inspect");
}

/**
 * Filter `dispatch_command` calls down to those for the plugin-owned
 * `entity.inspect` (Card G). Space routes this id to the BACKEND with the
 * focused scope chain; the plugin resolves the field moniker server-side
 * from the chain's leaf-first head.
 */
function entityInspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "entity.inspect");
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one. Mirrors the helper shape used in
 * `entity-inspector.spatial-nav.test.tsx` so the two test files stay
 * in sync.
 */
async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
  next_segment = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
  next_segment?: string | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: next_segment as FocusChangedPayload["next_segment"],
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render the inspector inside the production-shaped provider stack
 * with `<AppShell>` so the global keydown listener (the one
 * `<KeybindingHandler>` mounts) is active. The inspector mounts as a
 * descendant of `<AppShell>`, mirroring App.tsx's production tree —
 * the inspector layer is a sibling of any board view, but both share
 * the same window-root focus layer and the same keydown listener.
 */
function renderInspector(entity: Entity = makeTask({ title: "Hello" })) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <TooltipProvider delayDuration={100}>
                  <SchemaProvider>
                    <EntityStoreProvider entities={{ task: [entity] }}>
                      <FieldUpdateProvider>
                        <AppShell>
                          <EntityInspector entity={entity} />
                        </AppShell>
                      </FieldUpdateProvider>
                    </EntityStoreProvider>
                  </SchemaProvider>
                </TooltipProvider>
              </UndoProvider>
            </AppModeProvider>
          </UIStateProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Inspector field — Space → app.inspect", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("space_on_focused_inspector_field_dispatches_inspect_with_field_moniker", async () => {
    const { unmount } = renderInspector();
    await flushSetup();
    // Second flush to drain the LayerScopeRegistry → mockInvoke mirror
    // queue (the setup hook resolves the dynamic import asynchronously
    // on the first scope mount per test file).
    await flushSetup();

    // Find the title field's `<FocusScope>` registration so we know
    // the FullyQualifiedMoniker to drive into `focus-changed`.
    const titleZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    expect(
      titleZone,
      "the title field row must register a spatial zone with the field moniker",
    ).toBeTruthy();

    // Drive a focus-changed event from the kernel. This mirrors the
    // production flow when the user clicks (or arrow-keys to) the
    // field row — `useFocusClaim` flips, `useFocusedScope` updates,
    // and `extractChainBindings` will see the new chain on the next
    // keydown.
    await fireFocusChanged({
      next_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
    });
    await flushSetup();

    mockInvoke.mockClear();

    // Press Space — the GLOBAL `entity.inspect` binding (plugin-owned,
    // Card G) resolves and routes ONE dispatch to the backend with the
    // focused scope chain; the plugin resolves the field's moniker
    // server-side from the chain's leaf-first head.
    await act(async () => {
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });
    await flushSetup();

    const dispatches = entityInspectDispatches();
    expect(
      dispatches.length,
      "Space on a focused inspector field zone must dispatch entity.inspect exactly once",
    ).toBe(1);
    expect(
      (dispatches[0].scopeChain as string[] | undefined)?.[0],
      "the dispatched chain's head must be the focused field's moniker",
    ).toBe("field:task:T1.title");
    expect(
      inspectDispatches().length,
      "Space must not synthesize a webview-side app.inspect — the backend owns the inspect",
    ).toBe(0);

    unmount();
  });
});
