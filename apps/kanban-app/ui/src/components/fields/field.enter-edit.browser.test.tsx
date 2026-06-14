/**
 * Browser-mode tests pinning the "Enter on a focused field zone enters edit
 * mode" contract.
 *
 * Source of truth for card `01KQ9X3A9NMRYK50GWP4S4ZMJ4`. The fix wires a
 * scope-level `field.edit` `CommandDef` (keys: vim Enter / cua Enter)
 * onto each field zone's `<CommandScope>` when the field is in display
 * mode AND has an `onEdit` callback. The field-zone scope is closer than
 * the global root scope, so `extractChainBindings` claims Enter for
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
 *      a no-op — `editing` stays false and no `app.inspect` dispatch
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
import { commandToolCall } from "@/test/mock-command-list";
import {
  getWebviewCommandHandler,
  hasWebviewCommandHandler,
  resetWebviewCommandBusForTest,
} from "@/lib/webview-command-bus";
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

const EDITABLE_NOTES_FIELD: FieldDef = {
  id: "f4",
  name: "notes",
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

/**
 * Production-shaped `tags` field (mirrors
 * `crates/swissarmyhammer-kanban/builtin/definitions/tags.yaml`): a
 * computed multi-value field whose display renders one `<FocusScope>`
 * pill per tag inside the field zone — the shape the pill-focused
 * Enter test below depends on.
 */
const TAGS_FIELD: FieldDef = {
  id: "f3",
  name: "tags",
  type: {
    kind: "computed",
    derive: "parse-body-tags",
    entity: "tag",
    commit_display_names: true,
  },
  editor: "multi-select",
  display: "badge-list",
  icon: "tag",
  section: "header",
} as unknown as FieldDef;

const TASK_SCHEMA = {
  entity: {
    name: "task",
    fields: ["title", "notes", "id", "tags"],
  },
  fields: [
    EDITABLE_TITLE_FIELD,
    EDITABLE_NOTES_FIELD,
    READ_ONLY_FIELD,
    TAGS_FIELD,
  ],
};

/** Tag schema so `MentionView` resolves tag pills (mention prefix + display field). */
const TAG_SCHEMA = {
  entity: {
    name: "tag",
    fields: ["tag_name", "color"],
    mention_prefix: "#",
    mention_display_field: "tag_name",
  },
  fields: [
    {
      id: "tf1",
      name: "tag_name",
      type: { kind: "text", single_line: true },
      editor: "text",
      display: "text",
      icon: "tag",
      section: "header",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  task: TASK_SCHEMA,
  tag: TAG_SCHEMA,
};

/** Tag entities backing the `tags` slugs used by the pill tests. */
const TAG_ENTITIES: Entity[] = [
  {
    id: "tag-1",
    entity_type: "tag",
    moniker: "tag:tag-1",
    fields: { tag_name: "bugfix", color: "ff0000" },
  },
  {
    id: "tag-2",
    entity_type: "tag",
    moniker: "tag:tag-2",
    fields: { tag_name: "feature", color: "00ff00" },
  },
];

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  // The field-edit commands are DEFINED by the `app-shell-commands` builtin plugin
  // (`field.edit` / `field.editEnter`, scope `["ui:field"]`) — in production
  // their keys reach the keymap layer through the CommandService registry,
  // so the harness publishes the same metadata through the `useCommandList`
  // seam.
  if (cmd === "command_tool_call") return commandToolCall(args);
  if (cmd === "list_entity_types") return ["task", "tag"];
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
  if (cmd === "spatial_drill_in") {
    // Under the no-silent-dropout contract the kernel echoes the
    // focused FQM when there's nothing to descend into. The field
    // tests below never set up children for the field zones they
    // exercise — drill-in must echo the focused FQM so the closure's
    // compare-to-focused fall-through opens the editor.
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

/** Filter `dispatch_command` calls down to those for `app.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "app.inspect");
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one. The bridge in `<EntityFocusProvider>`
 * mirrors `payload.next_segment` into the entity-focus store; the
 * focused entity scope becomes the head of the chain that
 * `extractChainBindings` walks on the next keydown.
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
 * Render a `<Field>` inside the production-shaped provider stack with
 * `<AppShell>` so the global keydown handler is mounted. The harness
 * controls `editing` itself so the test can assert the flip after
 * Enter, mirroring the production container's responsibility (e.g.
 * `EntityInspector`'s `editingFieldName` state).
 */
function renderFieldHarness(props: {
  field: FieldDef;
  entity: Entity;
  /** Extra tag entities for badge-list pill resolution. */
  tags?: Entity[];
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
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <TooltipProvider delayDuration={100}>
                  <SchemaProvider>
                    <EntityStoreProvider
                      entities={{
                        task: [props.entity],
                        tag: props.tags ?? [],
                      }}
                    >
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

/**
 * Render TWO editable fields of the same task, each managing its own `editing`
 * state, inside the production-shaped stack. Used by the context-menu-target
 * test: the dispatch names one field as its `target` while the OTHER is the
 * spatially-focused field, so the closure must focus + edit the TARGET, not
 * the field that happened to be focused.
 */
function renderTwoFieldHarness(props: {
  fieldA: FieldDef;
  fieldB: FieldDef;
  entity: Entity;
}) {
  function OneField({ fieldDef }: { fieldDef: FieldDef }) {
    const [editing, setEditing] = useState(false);
    return (
      <Field
        fieldDef={fieldDef}
        entityType={props.entity.entity_type}
        entityId={props.entity.id}
        mode="full"
        editing={editing}
        onEdit={() => setEditing(true)}
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
                    <EntityStoreProvider
                      entities={{ task: [props.entity], tag: [] }}
                    >
                      <FieldUpdateProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <AppShell>
                            <OneField fieldDef={props.fieldA} />
                            <OneField fieldDef={props.fieldB} />
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
    resetWebviewCommandBusForTest();
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

    const titleZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    expect(
      titleZone,
      "the title field row must register a spatial zone with the field moniker",
    ).toBeTruthy();

    // Before Enter: no CM6 editor mounted, the field is in display
    // mode (no `.cm-editor` inside the field zone).
    expect(
      container.querySelector(
        "[data-segment='field:task:T1.title'] .cm-editor",
      ),
      "no editor mounts before Enter — field starts in display mode",
    ).toBeNull();

    // Drive a focus-changed event for the field zone.
    await fireFocusChanged({
      next_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
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
        "[data-segment='field:task:T1.title'] .cm-editor",
      );
      expect(
        editor,
        "after Enter on a focused editable field, the editor must be mounted",
      ).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // #1b: The edit behavior routes through the webview command bus (Card D) —
  // the `field.edit` / `field.editEnter` DEFINITIONS live in the `app-shell-commands`
  // plugin; the Field registers the live handlers only while its zone is the
  // spatial focus, so a dispatched id always reaches the focused field.
  // -------------------------------------------------------------------------

  it("registers field.edit/field.editEnter bus handlers only while the field zone is focused", async () => {
    const { unmount } = renderFieldHarness({
      field: EDITABLE_TITLE_FIELD,
      entity: makeTask({ title: "Hello" }),
    });
    await flushSetup();

    // Before focus: no handler — a dispatch would fall through to the
    // plugin's inert host execute.
    expect(hasWebviewCommandHandler("field.edit")).toBe(false);
    expect(hasWebviewCommandHandler("field.editEnter")).toBe(false);

    const titleZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    expect(titleZone).toBeTruthy();

    await fireFocusChanged({
      next_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
    });
    await flushSetup();

    // Focused: both plugin ids resolve to this field's live closure.
    expect(hasWebviewCommandHandler("field.edit")).toBe(true);
    expect(hasWebviewCommandHandler("field.editEnter")).toBe(true);

    // Focus away again: the slots are released.
    await fireFocusChanged({
      prev_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_fq: "/window/elsewhere" as FullyQualifiedMoniker,
      next_segment: asSegment("ui:elsewhere"),
    });
    await flushSetup();
    expect(hasWebviewCommandHandler("field.edit")).toBe(false);
    expect(hasWebviewCommandHandler("field.editEnter")).toBe(false);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #1c: Enter on a PILL inside a multi-value (tags) field opens the field's
  // editor. This is the keyboard path `field.edit`'s own drill-in produces:
  // Enter on the field zone focuses the first pill; a second Enter must fall
  // through the pill (a spatial leaf — the kernel echoes) and open the
  // editor. The keymap binds Enter to `field.edit` whenever the `ui:field`
  // marker appears ANYWHERE in the focused chain, so the bus handler must be
  // registered while focus is anywhere WITHIN the field zone's subtree — not
  // only while the zone itself is the direct focus. Regression test for the
  // Card D review blocker (pill-focused Enter was a dead binding: the keymap
  // resolved `field.edit`, the bus slot was empty, and the dispatch died on
  // the plugin's inert host execute while still preventDefault-ing).
  // -------------------------------------------------------------------------

  it("enter_on_pill_inside_tags_field_opens_the_field_editor", async () => {
    const onEditSpy = vi.fn();
    const { unmount } = renderFieldHarness({
      field: TAGS_FIELD,
      entity: makeTask({ tags: ["bugfix", "feature"] }),
      tags: TAG_ENTITIES,
      onEditSpy,
    });
    await flushSetup();

    const tagsZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(
      tagsZone,
      "the tags field row must register a spatial zone with the field moniker",
    ).toBeTruthy();
    const pill = registerScopeArgs().find((a) => a.segment === "tag:tag-1");
    expect(
      pill,
      "each tag pill must register its own FocusScope leaf",
    ).toBeTruthy();
    // Structural sanity: the pill is spatially INSIDE the field zone.
    expect(String(pill!.fq).startsWith(`${tagsZone!.fq}/`)).toBe(true);

    // Focus the PILL — the exact state `field.edit`'s drill-in puts the
    // user in.
    await fireFocusChanged({
      next_fq: pill!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("tag:tag-1"),
    });
    await flushSetup();

    // The field's bus handlers must be live while focus sits on a pill
    // within its subtree (matching the keymap's marker-in-chain gate).
    expect(
      hasWebviewCommandHandler("field.edit"),
      "field.edit must have a live bus handler while a pill inside the field is focused",
    ).toBe(true);

    // Press Enter. The keymap resolves it to `field.edit` (the `ui:field`
    // marker is in the focused chain); the bus handler drills into the
    // pill, the kernel echoes (leaf), and the closure falls through to
    // `onEdit` — opening the editor.
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });
    await flushSetup();

    expect(
      onEditSpy,
      "Enter on a pill inside the tags field must open the field's editor",
    ).toHaveBeenCalledTimes(1);

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

    const titleZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    expect(titleZone).toBeTruthy();

    await fireFocusChanged({
      next_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
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
        "[data-segment='field:task:T1.title']",
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
      "[data-segment='field:task:T1.title'] .cm-editor",
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

    const idZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.id",
    );
    expect(idZone).toBeTruthy();

    await fireFocusChanged({
      next_fq: idZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.id"),
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
      container.querySelector("[data-segment='field:task:T1.id'] .cm-editor"),
      "no editor must mount on a non-editable field after Enter",
    ).toBeNull();
    expect(onEditSpy).not.toHaveBeenCalled();
    expect(
      inspectDispatches().length,
      "Enter on a non-editable field must NOT dispatch app.inspect",
    ).toBe(0);

    unmount();
  });
});

// ---------------------------------------------------------------------------
// Palette / context-menu surface dispatch.
//
// Card `01KV30ZXHWPS4FZK9WEH4DMMZY`: "Edit Field" surfaces on the command
// palette and context menu, not just the keymap. Picking it must produce the
// EXACT same outcome as pressing Enter — focus + activate the target field's
// editor (or drill into its first pill) — reusing the one `editClosure`. These
// tests dispatch `field.edit` the way the palette / context-menu surface does:
// by invoking the registered webview-bus handler directly (the dispatch path
// `useDispatchCommand` takes when an id has a bus handler), NOT through the
// keymap. They fail before the metadata + dispatch change and pass after.
// ---------------------------------------------------------------------------

describe("Field — palette/context-menu field.edit dispatch", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    resetWebviewCommandBusForTest();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // A palette/context-menu dispatch (via the bus, not the keymap) at a focused
  // non-pill text field enters edit mode — identical to the Enter test.
  // -------------------------------------------------------------------------

  it("bus dispatch of field.edit on a focused text field enters edit mode", async () => {
    const { container, unmount } = renderFieldHarness({
      field: EDITABLE_TITLE_FIELD,
      entity: makeTask({ title: "Hello" }),
    });
    await flushSetup();

    const titleZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    expect(titleZone).toBeTruthy();

    await fireFocusChanged({
      next_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
    });
    await flushSetup();

    // The focused field registers the `field.edit` bus handler. Dispatch it
    // exactly as the palette/context-menu would — through the bus, with the
    // field's own moniker as the explicit target, NOT via a keystroke.
    const handler = getWebviewCommandHandler("field.edit");
    expect(
      handler,
      "the focused field must register a field.edit bus handler",
    ).toBeTruthy();
    await act(async () => {
      await handler!({ target: "field:task:T1.title" });
    });
    await flushSetup();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    await waitFor(() => {
      const editor = container.querySelector(
        "[data-segment='field:task:T1.title'] .cm-editor",
      );
      expect(
        editor,
        "a palette/context-menu field.edit dispatch must open the editor, same as Enter",
      ).not.toBeNull();
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // A pill field still drills into the first pill on a bus dispatch — the
  // two-outcome behavior is preserved for the new surfaces, not just the
  // keybinding.
  // -------------------------------------------------------------------------

  it("bus dispatch of field.edit on a pill field drills into the first pill", async () => {
    const onEditSpy = vi.fn();
    const { unmount } = renderFieldHarness({
      field: TAGS_FIELD,
      entity: makeTask({ tags: ["bugfix", "feature"] }),
      tags: TAG_ENTITIES,
      onEditSpy,
    });
    await flushSetup();

    const tagsZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(tagsZone).toBeTruthy();
    const firstPill = registerScopeArgs().find(
      (a) => a.segment === "tag:tag-1",
    );
    expect(firstPill).toBeTruthy();

    // A field WITH pills drills into the first pill: program the kernel so a
    // `drill_in layer` from the tags zone descends to the first pill (≠ the
    // focused FQM), the structural answer the closure reads to choose "drill
    // in" over "open editor". The drill-in lowers onto `command_tool_call`
    // (op `drill_in layer`); the default mock returns null for it (the
    // no-children echo case the text-field test above exercises).
    mockInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "command_tool_call") {
        const a = args as { op?: string; params?: { focused_fq?: string } };
        if (
          a?.op === "drill_in layer" &&
          a.params?.focused_fq === tagsZone!.fq
        ) {
          return { next_fq: firstPill!.fq };
        }
      }
      return defaultInvokeImpl(cmd, args);
    });

    // Focus the field zone itself (not a pill) — the state a palette pick on
    // the field leaves the user in.
    await fireFocusChanged({
      next_fq: tagsZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    // A focus claim to the first pill (`nav.focus` lowers onto the focus
    // kernel's `set focus`) is the observable signal of a drill-in.
    const focusClaims = () =>
      mockInvoke.mock.calls.filter(
        (c) =>
          c[0] === "command_tool_call" &&
          (c[1] as { op?: string })?.op === "set focus",
      ).length;
    const focusClaimsBefore = focusClaims();

    const handler = getWebviewCommandHandler("field.edit");
    expect(handler).toBeTruthy();
    await act(async () => {
      await handler!({ target: "field:task:T1.tags" });
    });
    await flushSetup();

    // Drill-in path: focus moved to the first pill, and the editor did NOT
    // open (onEdit not called) — the two-outcome behavior preserved for the
    // palette/context-menu surface, not just the keybinding.
    expect(
      focusClaims(),
      "a bus field.edit on a pill field must drill into the first pill (focus moves)",
    ).toBeGreaterThan(focusClaimsBefore);
    expect(
      onEditSpy,
      "a pill field must drill in, not open the editor, on a bus dispatch",
    ).not.toHaveBeenCalled();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Context-menu target case: a dispatch carrying an explicit `field:` target
  // that is NOT the currently-focused field must put the TARGET field into
  // edit mode (focus moves to it, its editor mounts), not the field that
  // happened to be focused. A single code path serves Enter, palette, and
  // context-menu.
  // -------------------------------------------------------------------------

  it("bus dispatch of field.edit with an unfocused field target focuses + re-dispatches to the target", async () => {
    // Capture `nav.focus` claims and `field.edit` re-dispatches at the
    // backend boundary so the test sees the closure's target-routing contract
    // without depending on the kernel actually relocating focus in the
    // lightweight harness.
    const navFocusCalls: unknown[] = [];
    mockInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "command_tool_call") {
        const a = args as { op?: string; params?: { fq?: string } };
        if (a?.op === "set focus") navFocusCalls.push(a.params?.fq);
      }
      if (cmd === "dispatch_command") {
        const a = args as { cmd?: string; args?: { fq?: string } };
        if (a?.cmd === "nav.focus") navFocusCalls.push(a.args?.fq);
      }
      return defaultInvokeImpl(cmd, args);
    });

    const { unmount } = renderTwoFieldHarness({
      fieldA: EDITABLE_TITLE_FIELD,
      fieldB: EDITABLE_NOTES_FIELD,
      entity: makeTask({ title: "Hello", notes: "World" }),
    });
    await flushSetup();

    const titleZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    expect(titleZone).toBeTruthy();

    // Focus field A (title) — the previously-focused field. Field B (notes)
    // is the context-menu TARGET and is NOT spatially focused.
    await fireFocusChanged({
      next_fq: titleZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
    });
    await flushSetup();

    // The dispatch the bus path takes when a context-menu selection over B's
    // row carries B as its explicit target. Only the focused field (A) has a
    // live `field.edit` handler, so A's closure must route to the target:
    // claim focus for B's moniker and re-dispatch `field.edit`, so once B is
    // focused its OWN handler opens B's editor (a single code path for Enter,
    // palette, and context-menu).
    const handler = getWebviewCommandHandler("field.edit");
    expect(handler).toBeTruthy();
    await act(async () => {
      await handler!({ target: "field:task:T1.notes" });
    });
    await flushSetup();

    // The closure claimed focus for the TARGET field's moniker — the first
    // half of "operate on the targeted field, not the focused one".
    expect(
      navFocusCalls.some((c) =>
        JSON.stringify(c).includes("field:task:T1.notes"),
      ),
      `field.edit with a non-focused field target must claim focus for that target; saw ${JSON.stringify(navFocusCalls)}`,
    ).toBe(true);

    unmount();
  });
});
