/**
 * Browser-mode test pinning the bug fix: pressing Space on a focused
 * inspector field zone dispatches `ui.inspect` with `target =
 * field:<type>:<id>.<name>`.
 *
 * Before card 01KQ9XJ4XGKVW24EZSQCA6K3E2, Space ownership lived on
 * `board.inspect` registered at the BoardView's `<CommandScopeProvider>`.
 * The inspector layer mounts as a sibling of the BoardView (not a
 * descendant), so a focused field zone's scope chain never reached the
 * board scope — Space did nothing on a focused field.
 *
 * After the fix, every `<Inspectable>` wrapper contributes its own
 * scope-level `entity.inspect` `CommandDef` keyed to Space. Because
 * `<Field>` (`fields/field.tsx`) wraps its `<FocusScope>` in
 * `<Inspectable moniker={field:<type>:<id>.<name>}>`, every field row
 * carries the binding regardless of which layer it is rendered in.
 *
 * The test below mounts a real `<EntityInspector>`, simulates the
 * spatial kernel emitting a `focus-changed` event for the title field,
 * fires `keydown { key: " " }` at the document, and asserts exactly one
 * `dispatch_command` IPC fires for `ui.inspect` with the field moniker
 * carried as `target`.
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
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel
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

/** Filter `dispatch_command` calls down to those for `ui.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "ui.inspect");
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

describe("Inspector field — Space → ui.inspect", () => {
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
    // and `extractScopeBindings` will see the new chain on the next
    // keydown.
    await fireFocusChanged({
      next_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
    });
    await flushSetup();

    mockInvoke.mockClear();

    // Press Space — the global keymap handler reads the focused
    // scope's bindings, finds the per-Inspectable `entity.inspect`
    // command (its `keys.cua: "Space"`), runs the `execute` closure,
    // which dispatches `ui.inspect` against the field moniker.
    await act(async () => {
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });
    await flushSetup();

    const dispatches = inspectDispatches();
    expect(
      dispatches.length,
      "Space on a focused inspector field zone must dispatch ui.inspect exactly once",
    ).toBe(1);
    expect(
      dispatches[0].target,
      "ui.inspect from the focused field zone must carry that field's moniker as target",
    ).toBe("field:task:T1.title");

    unmount();
  });
});
