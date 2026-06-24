/**
 * Browser-mode tests pinning the "read-only/computed fields can never be
 * entered or blanked" contract (card `01KTCRZA9M1F1GCK378J8AD7RW`).
 *
 * A field whose metadata declares no editor (`editor: "none"`, the YAML
 * default for computed fields like `status_date` and `virtual_tags`) must:
 *
 *   1. NOT enter edit mode on Enter/drill-in — even when a caller wires an
 *      `onEdit` callback unconditionally (as `entity-card.tsx`'s
 *      `CardFields` does for every card field).
 *   2. NEVER blank its displayed value — even when a caller arms
 *      `editing={true}` directly. Pre-fix, an armed read-only field mounted
 *      `FieldEditor`, which resolved no registered editor and rendered
 *      `null`: the value disappeared and, with no editor to fire
 *      onDone/onCancel, never came back.
 *
 * Editability is metadata-driven (single source of truth:
 * `resolveEditor(field) !== "none"`), checked once inside the `<Field>`
 * interpreter — not per-call-site.
 *
 * Mock pattern matches `field.enter-edit.browser.test.tsx`. Additionally
 * stubs `useBoardData` (à la `displays/virtual-tag-display.test.tsx`) so the
 * `virtual-badge-list` display can resolve READY/BLOCKED/BLOCKING pill
 * metadata without the full `WindowContainer` chain.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, screen } from "@testing-library/react";
import { useState } from "react";

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

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
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

// Stub `useBoardData` so `virtual-tag-display.tsx` resolves pill metadata.
// Values mirror the Rust `DEFAULT_REGISTRY` in `virtual_tags.rs`. The full
// `window-container.tsx` chain (RustEngine, CommandScope, …) is not needed —
// `VirtualTagDisplay` only ever touches `useBoardData`.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({
    board: {
      id: "stub-board",
      entity_type: "board",
      moniker: "board:stub",
      fields: {},
    },
    columns: [],
    tags: [],
    virtualTagMeta: [
      {
        slug: "READY",
        color: "0e8a16",
        description: "Task has no unmet dependencies",
      },
      {
        slug: "BLOCKED",
        color: "e36209",
        description: "Task has at least one unmet dependency",
      },
      {
        slug: "BLOCKING",
        color: "d73a4a",
        description: "Other tasks depend on this one",
      },
    ],
    summary: { total: 0, by_column: {}, by_tag: {} },
  }),
}));

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { Field } from "@/components/fields/field";
import { AppShell } from "@/components/app-shell";
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
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel,
} from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds — computed/read-only fields mirroring the builtin YAML
// definitions (`status_date.yaml`, `virtual_tags.yaml`): `editor: none`.
// ---------------------------------------------------------------------------

const STATUS_DATE_FIELD: FieldDef = {
  id: "f1",
  name: "status_date",
  type: { kind: "computed", derive: "derive-status-date" },
  editor: "none",
  display: "status-date",
  icon: "target",
  section: "header",
};

const VIRTUAL_TAGS_FIELD: FieldDef = {
  id: "f2",
  name: "virtual_tags",
  type: { kind: "computed", derive: "compute-virtual-tags" },
  editor: "none",
  display: "virtual-badge-list",
  icon: "zap",
  section: "header",
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    fields: ["status_date", "virtual_tags"],
  },
  fields: [STATUS_DATE_FIELD, VIRTUAL_TAGS_FIELD],
};

const STATUS_DATE_TIMESTAMP = "2026-01-01T00:00:00Z";
const STATUS_DATE_VALUE = { kind: "created", timestamp: STATUS_DATE_TIMESTAMP };

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return TASK_SCHEMA;
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
  if (cmd === "spatial_drill_in") {
    // Echo the focused FQM — "no spatial children to descend into", so the
    // `field.edit` closure falls through to its `onEdit` branch.
    return (args as { focusedFq?: string } | undefined)?.focusedFq ?? null;
  }
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

/** Wait for register effects scheduled in `useEffect` to flush. */
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

/** Drive a `focus-changed` event as if the Rust kernel had emitted one. */
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
  const handlers = listeners.get("notifications/focus/changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render a `<Field>` inside the production-shaped provider stack with
 * `<AppShell>` so the global keydown handler is mounted.
 *
 * Mirrors the worst-case production caller (`entity-card.tsx`'s
 * `CardFields`): `onEdit` is wired UNCONDITIONALLY — no per-call-site
 * editability check — so the metadata gate inside `<Field>` is the only
 * thing standing between Enter and a blanked field.
 */
function renderFieldHarness(props: {
  field: FieldDef;
  entity: Entity;
  initialEditing?: boolean;
  onEditSpy?: () => void;
}) {
  function Harness() {
    const [editing, setEditing] = useState(props.initialEditing ?? false);
    return (
      <Field
        fieldDef={props.field}
        entityType={props.entity.entity_type}
        entityId={props.entity.id}
        mode="full"
        editing={editing}
        onEdit={() => {
          props.onEditSpy?.();
          setEditing(true);
        }}
        onDone={() => setEditing(false)}
        onCancel={() => setEditing(false)}
      />
    );
  }

  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <TooltipProvider delayDuration={100}>
                  <SchemaProvider>
                    <EntityStoreProvider entities={{ task: [props.entity] }}>
                      <FieldUpdateProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <AppShell>
                            <Harness />
                          </AppShell>
                        </ActiveBoardPathProvider>
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

/** Focus the given field zone via a kernel-shaped focus-changed event. */
async function focusFieldZone(segment: string) {
  const zone = registerScopeArgs().find((a) => a.segment === segment);
  expect(
    zone,
    `the field must register a spatial zone with segment ${segment}`,
  ).toBeTruthy();
  await fireFocusChanged({
    next_fq: zone!.fq as FullyQualifiedMoniker,
    next_segment: asSegment(segment),
  });
  await flushSetup();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Field — read-only/computed fields cannot be entered or blanked", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Case 1: computed status-date — Enter must not arm editing and the
  // displayed phrase must survive the gesture.
  // -------------------------------------------------------------------------

  it("enter_on_computed_status_date_field_does_not_enter_edit_and_preserves_value", async () => {
    const onEditSpy = vi.fn();
    const { container, unmount } = renderFieldHarness({
      field: STATUS_DATE_FIELD,
      entity: makeTask({ status_date: STATUS_DATE_VALUE }),
      onEditSpy,
    });
    await flushSetup();

    const phraseSelector = `[data-segment='field:task:T1.status_date'] span[title='${STATUS_DATE_TIMESTAMP}']`;
    expect(
      container.querySelector(phraseSelector),
      "the status-date phrase must render before Enter",
    ).not.toBeNull();

    await focusFieldZone("field:task:T1.status_date");

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    expect(
      onEditSpy,
      "Enter on a read-only/computed field must NOT arm edit mode, even " +
        "when the caller wires onEdit unconditionally",
    ).not.toHaveBeenCalled();
    expect(
      container.querySelector(phraseSelector),
      "the status-date phrase must still render after Enter — the value " +
        "must never blank",
    ).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 2: computed virtual tags — Enter must keep the READY/BLOCKING
  // pills rendered (no missing-editor blank).
  // -------------------------------------------------------------------------

  it("enter_on_virtual_badge_list_field_keeps_pills_rendered", async () => {
    const onEditSpy = vi.fn();
    const { unmount } = renderFieldHarness({
      field: VIRTUAL_TAGS_FIELD,
      entity: makeTask({ virtual_tags: ["READY", "BLOCKING"] }),
      onEditSpy,
    });
    await flushSetup();

    expect(
      screen.getByText("#READY"),
      "the READY pill must render before Enter",
    ).toBeTruthy();
    expect(screen.getByText("#BLOCKING")).toBeTruthy();

    await focusFieldZone("field:task:T1.virtual_tags");

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    expect(
      onEditSpy,
      "Enter on the virtual_tags field must NOT arm edit mode",
    ).not.toHaveBeenCalled();
    expect(
      screen.queryByText("#READY"),
      "the READY pill must still render after Enter — drill-in must never " +
        "flip a display-only field into a missing/empty editor",
    ).toBeTruthy();
    expect(screen.queryByText("#BLOCKING")).toBeTruthy();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 3: editing armed directly by a caller — the metadata gate inside
  // <Field> must keep rendering the display, never a null editor.
  // -------------------------------------------------------------------------

  it("read_only_field_with_editing_armed_still_renders_its_display_value", async () => {
    const { container, unmount } = renderFieldHarness({
      field: STATUS_DATE_FIELD,
      entity: makeTask({ status_date: STATUS_DATE_VALUE }),
      initialEditing: true,
    });
    await flushSetup();

    expect(
      container.querySelector(
        `[data-segment='field:task:T1.status_date'] span[title='${STATUS_DATE_TIMESTAMP}']`,
      ),
      "a read-only/computed field must render its display even when a " +
        "caller arms editing — there is no editor to mount, so falling " +
        "into edit mode blanks the value",
    ).not.toBeNull();

    unmount();
  });
});
