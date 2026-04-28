/**
 * Browser-mode tests pinning the "Enter on a focused field zone enters edit
 * mode" contract.
 *
 * Source of truth for card `01KQ9X3A9NMRYK50GWP4S4ZMJ4`. The fix wires a
 * scope-level `field.edit` `CommandDef` (keys: vim Enter / cua Enter)
 * onto each field zone's `<CommandScope>` when the field is in display
 * mode AND has an `onEdit` callback. The field-zone scope is closer than
 * the global root scope, so `extractScopeBindings` claims Enter for
 * `field.edit` only when the focused entity is an editable field zone —
 * shadowing the global `nav.drillIn: Enter` precisely there. In edit
 * mode the binding is NOT registered: the editor element holds DOM
 * focus and owns Enter via its own keymap (commit / newline).
 *
 * The four cases below mirror the acceptance criteria in the card's
 * description:
 *
 *   1. Enter on a focused editable field zone (display mode) flips
 *      `editing` to true and mounts the editor element.
 *   2. After Enter, DOM focus lands on the editor input.
 *   3. Enter inside the editor (already editing) does NOT call
 *      `onEdit` again — the editor's keymap owns Enter.
 *   4. Enter on a non-editable field zone (no `onEdit` provided) is
 *      a no-op — `editing` stays false and no `ui.inspect` dispatch
 *      fires.
 *
 * Mock pattern matches `inspector-field.space-inspect.browser.test.tsx`
 * and `field.spatial-nav.test.tsx` so the file integrates with the
 * existing field-side test suite.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
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
  asLayerName,
  type FocusChangedPayload,
  type SpatialKey,
  type WindowLabel,
} from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds — minimal task schema with one editable field and one
// read-only field (`editor: "none"`).
// ---------------------------------------------------------------------------

const EDITABLE_TITLE_FIELD: FieldDef = {
  id: "f1",
  name: "title",
  type: { kind: "markdown", single_line: true },
  editor: "markdown",
  display: "text",
  icon: "type",
  section: "header",
};

const READ_ONLY_FIELD: FieldDef = {
  id: "f2",
  name: "id",
  type: { kind: "text", single_line: true },
  editor: "none",
  display: "text",
  icon: "hash",
  section: "header",
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    fields: ["title", "id"],
  },
  fields: [EDITABLE_TITLE_FIELD, READ_ONLY_FIELD],
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
  if (cmd === "spatial_drill_in") return null;
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

/** Collect every `spatial_register_zone` invocation argument bag. */
function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
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
 * kernel had emitted one. The bridge in `<EntityFocusProvider>`
 * mirrors `payload.next_moniker` into the entity-focus store; the
 * focused entity scope becomes the head of the chain that
 * `extractScopeBindings` walks on the next keydown.
 */
async function fireFocusChanged({
  prev_key = null,
  next_key = null,
  next_moniker = null,
}: {
  prev_key?: SpatialKey | null;
  next_key?: SpatialKey | null;
  next_moniker?: string | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_key,
    next_key,
    next_moniker: next_moniker as FocusChangedPayload["next_moniker"],
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render a `<Field>` inside the production-shaped provider stack with
 * `<AppShell>` so the global keydown handler is mounted. The harness
 * controls `editing` itself so the test can assert the flip after
 * Enter, mirroring the production container's responsibility (e.g.
 * `EntityInspector`'s `editingFieldName` state).
 */
function renderFieldHarness(props: {
  field: FieldDef;
  entity: Entity;
  initialEditing?: boolean;
  onEditSpy?: () => void;
  forceNoOnEdit?: boolean;
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
        onEdit={
          props.forceNoOnEdit
            ? undefined
            : () => {
                props.onEditSpy?.();
                setEditing(true);
              }
        }
        onDone={() => setEditing(false)}
        onCancel={() => setEditing(false)}
      />
    );
  }

  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asLayerName("window")}>
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Field — Enter on focused field zone enters edit mode", () => {
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
  // #1: Display-mode editable field — Enter flips editing to true.
  // -------------------------------------------------------------------------

  it("enter_on_field_zone_in_display_mode_enters_edit_mode", async () => {
    const { container, unmount } = renderFieldHarness({
      field: EDITABLE_TITLE_FIELD,
      entity: makeTask({ title: "Hello" }),
    });
    await flushSetup();

    const titleZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:T1.title",
    );
    expect(
      titleZone,
      "the title field row must register a spatial zone with the field moniker",
    ).toBeTruthy();

    // Before Enter: no CM6 editor mounted, the field is in display
    // mode (no `.cm-editor` inside the field zone).
    expect(
      container.querySelector(
        "[data-moniker='field:task:T1.title'] .cm-editor",
      ),
      "no editor mounts before Enter — field starts in display mode",
    ).toBeNull();

    // Drive a focus-changed event for the field zone.
    await fireFocusChanged({
      next_key: titleZone!.key as SpatialKey,
      next_moniker: "field:task:T1.title",
    });
    await flushSetup();

    // Press Enter. The field's scope-level `field.edit` command
    // (registered when `editing === false` and `onEdit` is defined)
    // wins over the global `nav.drillIn: Enter`. Its `execute`
    // closure calls `onEdit()`, which the harness wires to flip
    // `editing` to `true`.
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });
    await flushSetup();
    // CM6 mount is async — let it settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    await waitFor(() => {
      const editor = container.querySelector(
        "[data-moniker='field:task:T1.title'] .cm-editor",
      );
      expect(
        editor,
        "after Enter on a focused editable field, the editor must be mounted",
      ).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: After Enter, DOM focus lands on the editor input.
  // -------------------------------------------------------------------------

  it("enter_on_field_zone_in_display_mode_focuses_the_editor_input", async () => {
    const { container, unmount } = renderFieldHarness({
      field: EDITABLE_TITLE_FIELD,
      entity: makeTask({ title: "Hello" }),
    });
    await flushSetup();

    const titleZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:T1.title",
    );
    expect(titleZone).toBeTruthy();

    await fireFocusChanged({
      next_key: titleZone!.key as SpatialKey,
      next_moniker: "field:task:T1.title",
    });
    await flushSetup();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });
    await flushSetup();
    // CM6's autofocus runs after a microtask + a small delay. Wait
    // for the editor and then for `document.activeElement` to settle
    // inside the field zone.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 200));
    });

    await waitFor(() => {
      const fieldZone = container.querySelector(
        "[data-moniker='field:task:T1.title']",
      );
      expect(fieldZone).not.toBeNull();
      const active = document.activeElement;
      expect(
        active,
        "an active element must exist after Enter mounts the editor",
      ).not.toBeNull();
      // CM6's editable element is `.cm-content` (a contenteditable
      // div). Single-line markdown editors otherwise expose a
      // contenteditable host; either way the active element is
      // somewhere inside the field zone.
      expect(
        fieldZone!.contains(active),
        "DOM focus must land somewhere inside the field zone after Enter",
      ).toBe(true);
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: Already in edit mode — Enter must NOT call onEdit again.
  // -------------------------------------------------------------------------

  it("enter_on_field_zone_already_in_edit_mode_does_not_call_onEdit_again", async () => {
    const onEditSpy = vi.fn();
    const { container, unmount } = renderFieldHarness({
      field: EDITABLE_TITLE_FIELD,
      entity: makeTask({ title: "Hello" }),
      initialEditing: true,
      onEditSpy,
    });
    await flushSetup();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    // Sanity: the editor mounted.
    const editor = container.querySelector(
      "[data-moniker='field:task:T1.title'] .cm-editor",
    );
    expect(
      editor,
      "the editor must be mounted when initialEditing is true",
    ).not.toBeNull();

    // Fire Enter on the editor's content element. The global keymap
    // handler's `isEditableTarget` short-circuits on `.cm-editor`
    // descendants, so neither `field.edit` nor `nav.drillIn`
    // resolves. The CM6 editor owns Enter via its own keymap.
    const cmContent = editor!.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).not.toBeNull();
    cmContent.focus();
    await act(async () => {
      fireEvent.keyDown(cmContent, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    expect(
      onEditSpy,
      "onEdit must NOT be called while editing — the editor owns Enter",
    ).not.toHaveBeenCalled();

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: Non-editable field — Enter is a no-op (no edit, no inspect).
  // -------------------------------------------------------------------------

  it("enter_on_non_editable_field_zone_is_noop", async () => {
    const onEditSpy = vi.fn();
    const { container, unmount } = renderFieldHarness({
      field: READ_ONLY_FIELD,
      entity: makeTask({ id: "T1" }),
      forceNoOnEdit: true,
      onEditSpy,
    });
    await flushSetup();

    const idZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:T1.id",
    );
    expect(idZone).toBeTruthy();

    await fireFocusChanged({
      next_key: idZone!.key as SpatialKey,
      next_moniker: "field:task:T1.id",
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // No editor mounts — the field stays in display mode. The
    // read-only field has `editor: "none"`, so even if `onEdit` were
    // ever wired the click-to-edit surface would skip wrapping.
    // After `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` extended `field.edit` to
    // drill into spatial children first, the command IS registered
    // for any focused field zone (even read-only) — but the kernel
    // returns null for a leaf with no children, the closure falls
    // through to `onEdit?.()`, and `onEdit` is undefined here, so
    // nothing happens. The observable outcome is identical to the
    // pre-drill-in behaviour: no editor, no inspect dispatch.
    expect(
      container.querySelector("[data-moniker='field:task:T1.id'] .cm-editor"),
      "no editor must mount on a non-editable field after Enter",
    ).toBeNull();
    expect(onEditSpy).not.toHaveBeenCalled();
    expect(
      inspectDispatches().length,
      "Enter on a non-editable field must NOT dispatch ui.inspect",
    ).toBe(0);

    unmount();
  });
});
