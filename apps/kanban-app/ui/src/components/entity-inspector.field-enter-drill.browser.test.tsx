/**
 * Browser-mode tests pinning the "Enter on a focused inspector field
 * zone drills into pills first, falls through to edit mode only when
 * there are no pills" contract.
 *
 * Source of truth for card `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (bug 3). The
 * field-zone scope-level `field.edit` `CommandDef` (keys: vim Enter /
 * cua Enter) was extended to:
 *
 *   1. Read the focused field-zone `FullyQualifiedMoniker` from the spatial
 *      provider.
 *   2. `await actions.drillIn(key)` — kernel returns the first
 *      spatial child's moniker (e.g. a pill in a badge-list field) or
 *      null when the field has no children.
 *   3. On a non-null result: `setFocus(moniker)` and return — the
 *      user lands on the first pill, ready to arrow-key among them.
 *   4. On null: fall through to `onEdit?.()` — opens the editor for
 *      editable fields, no-op for read-only fields.
 *
 * The six tests below mirror the acceptance criteria from the card.
 *
 * Mock pattern follows `inspectors-container.enter-drill-in.browser.test.tsx`
 * — `drillInResponses` map lets each test seed the kernel's answer per
 * field-zone key.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen, listeners } = await vi.hoisted(async () => {
  const { setupSpatialMocks } = await import("@/test/spatial-nav-harness");
  return setupSpatialMocks();
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
import { EntityInspector } from "./entity-inspector";
import { AppShell } from "./app-shell";
import { wrapMcpDispatch } from "@/test/mcp-invoke-translator";
import {
  answerListCommand,
  globalCommandsFromBindingTables,
  navDispatchCmds,
} from "@/test/mock-command-list";
import {
  UNHANDLED,
  emitToListenerMap,
  makeSpatialKernelMock,
} from "@/test/mock-spatial-kernel";
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
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds — task with two fields:
//   - `tags`: badge-list display, multi-select editor → has pill children
//   - `name`: text display, markdown editor → editable, no pill children
//   - `id`:   text display, editor "none" → non-editable, no children
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    fields: ["name", "tags", "id"],
  },
  fields: [
    {
      id: "fn",
      name: "name",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "type",
      section: "header",
    },
    {
      id: "ft",
      name: "tags",
      type: {
        kind: "reference",
        entity: "tag",
        multiple: true,
        commit_display_names: true,
      },
      editor: "multi-select",
      display: "badge-list",
      icon: "tag",
      section: "header",
    },
    {
      id: "fid",
      name: "id",
      type: { kind: "text", single_line: true },
      editor: "none",
      display: "text",
      icon: "hash",
      section: "header",
    },
  ],
};

const TAG_SCHEMA = {
  entity: {
    name: "tag",
    fields: ["tag_name", "color"],
    mention_prefix: "#",
    mention_display_field: "tag_name",
  },
  fields: [
    {
      id: "tn",
      name: "tag_name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = { task: TASK_SCHEMA, tag: TAG_SCHEMA };

/**
 * Shared spatial-kernel mock: the `monikerToKey` projection, the
 * `currentFocusKey` slot, the per-test `drillInResponses` override map, and
 * the `handleSpatialCommand` dispatcher implementing the no-silent-dropout
 * echo contract. Tests seed `drillInResponses` to steer drill-in answers.
 */
const { drillInResponses, handleSpatialCommand, reset: resetSpatialKernel } =
  makeSpatialKernelMock({ emit: emitToListenerMap(listeners) });

/**
 * Answer the entity- and UI-shell IPCs the providers fire at mount
 * (entity types, schema lookup, UI/undo state, command-scope listing,
 * raw `dispatch_command`). Returns {@link UNHANDLED} when `command` is not
 * one of these so the caller can fall through to other handlers.
 */
function handleEntityCommand(command: string, commandArgs?: unknown): unknown {
  if (command === "list_entity_types") return ["task", "tag"];
  if (command === "get_entity_schema") {
    const entityType = (commandArgs as { entityType?: string })?.entityType;
    return SCHEMAS[entityType ?? ""] ?? TASK_SCHEMA;
  }
  if (command === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (command === "get_undo_state") return { can_undo: false, can_redo: false };
  if (command === "list_commands_for_scope") return [];
  if (command === "dispatch_command") return undefined;
  return UNHANDLED;
}

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  command: string,
  commandArgs?: unknown,
): Promise<unknown> {
  // The field-edit commands are DEFINED by the `app-shell-commands` builtin plugin
  // (`field.edit` / `field.editEnter`, scope ["ui:field"]) — their Enter /
  // `i` keys reach the keymap layer only through the `useCommandList` seam,
  // so answer `list command` with the shared mock registry. Non-list
  // `command_tool_call` ops fall through to the branches below.
  const listAnswer = answerListCommand(
    command,
    commandArgs,
    globalCommandsFromBindingTables(),
  );
  if (listAnswer) return listAnswer;

  const entityAnswer = handleEntityCommand(command, commandArgs);
  if (entityAnswer !== UNHANDLED) return entityAnswer;

  const spatialAnswer = handleSpatialCommand(command, commandArgs);
  if (spatialAnswer !== UNHANDLED) return spatialAnswer;

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

function makeTags(): Entity[] {
  return [
    {
      entity_type: "tag",
      id: "tag-bug",
      moniker: "tag:tag-bug",
      fields: { tag_name: "bug", color: "ff0000" },
    },
    {
      entity_type: "tag",
      id: "tag-ui",
      moniker: "tag:tag-ui",
      fields: { tag_name: "ui", color: "0000ff" },
    },
  ];
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

/**
 * Collect every `spatial_focus` invocation. Under the production
 * pathway (`SpatialFocusProvider` mounted), `FocusActions.setFocus(fq)`
 * routes through `spatial.focus(fq)` → `invoke("spatial_focus", { fq })`
 * rather than dispatching a `app.setFocus` command. The kernel echoes
 * a `focus-changed` event the bridge mirrors into the entity-focus
 * store. Tests that observe a drill / setFocus fanout assert on this
 * IPC, not on a `dispatch_command(app.setFocus, ...)` call.
 */
function spatialFocusCalls(): Array<{ fq?: string }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      const args = (outer?.params ?? outer) as { fq?: string };
      return args;
    });
}

/** Filter `dispatch_command` calls down to those for `app.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "app.inspect");
}

/**
 * Collect every IPC that would carry a host-driven nav effect for the
 * given kernel command — either a direct client-side `cmd` invocation
 * (e.g. `spatial_navigate`) or its wrapped `command_tool_call` form
 * (`{ tool: "focus", op }`). Host-driven nav asserts this list is empty,
 * proving the effect ran kernel-side rather than leaving the webview.
 */
function filterIpcCalls(cmd: string, op: string): unknown[][] {
  return mockInvoke.mock.calls.filter(
    (ipcCall) =>
      ipcCall[0] === cmd ||
      (ipcCall[0] === "command_tool_call" &&
        (ipcCall[1] as { tool?: string; op?: string } | undefined)?.tool ===
          "focus" &&
        (ipcCall[1] as { tool?: string; op?: string } | undefined)?.op === op),
  );
}

/** Filter `spatial_drill_in` calls. */
function drillInCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_drill_in")
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      const args = (outer?.params ?? outer) as { fq: FullyQualifiedMoniker };
      return args;
    });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one for the current window.
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
  const handlers = listeners.get("notifications/focus/changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render the inspector for a given task entity inside the production-
 * shaped spatial-nav stack PLUS `<AppShell>` so the global keymap
 * handler is mounted (Enter resolves through the field-zone scope to
 * `field.edit`'s execute closure).
 */
function renderInspector(entity: Entity, tagEntities: Entity[] = []) {
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
                      entities={{ task: [entity], tag: tagEntities }}
                    >
                      <FieldUpdateProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <AppShell>
                            <EntityInspector entity={entity} />
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

describe("EntityInspector — Enter on a focused field zone (drill-in vs. edit)", () => {
  beforeEach(() => {
    resetSpatialKernel();
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, defaultInvokeImpl) as (
        cmd: string,
        args?: unknown,
      ) => Promise<unknown>,
    );
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // #1: Pill field — Enter drills into the first pill.
  //
  // The tags field renders two pills via `MentionView` (each registers
  // as a `<FocusScope>` leaf with moniker `tag:<id>`). Stub
  // `spatial_drill_in(tagsKey) → "tag:tag-bug"`. After Enter, the
  // entity-focus moniker store records `tag:tag-bug` (so
  // `useFocusedScope()` reports it), and the field is still in display
  // mode — no editor mounts.
  // -------------------------------------------------------------------------

  it("enter_on_pill_field_drills_into_first_pill", async () => {
    const { container, unmount } = renderInspector(
      makeTask({ name: "Hello", tags: ["bug", "ui"], id: "T1" }),
      makeTags(),
    );
    await flushSetup();

    const tagsZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(tagsZone, "tags field zone must register").toBeTruthy();

    // Stub the kernel so drill-in on the tags zone returns the first
    // pill's moniker.
    drillInResponses.set(tagsZone!.fq as string, "tag:tag-bug");

    // Seed focus on the tags field zone.
    await fireFocusChanged({
      next_fq: tagsZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, defaultInvokeImpl) as (
        cmd: string,
        args?: unknown,
      ) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // The field-zone scope-level `field.edit` command's closure
    // dispatched `spatial_drill_in` for the tags zone.
    const drillCalls = drillInCalls();
    expect(
      drillCalls.length,
      "Enter on a focused field zone must dispatch spatial_drill_in once",
    ).toBe(1);
    expect(drillCalls[0].fq).toBe(tagsZone!.fq);

    // The closure's success branch forwards the kernel-returned
    // moniker through `FocusActions.setFocus`, which under the
    // production `SpatialFocusProvider` path invokes
    // `spatial_focus({ fq: "tag:tag-bug" })`. Confirm that fanout fires.
    const focusCalls = spatialFocusCalls();
    expect(focusCalls.length).toBeGreaterThanOrEqual(1);
    const targetCall = focusCalls.find((c) => c.fq === "tag:tag-bug");
    expect(
      targetCall,
      "spatial_focus must carry the first pill's moniker as fq",
    ).toBeTruthy();

    // The field stayed in display mode. `BadgeListDisplay` renders
    // pills via `MentionView` → `TextViewer` (a *readonly* CM6 mount),
    // so `.cm-editor` is always present for badge-list fields. To
    // distinguish display vs. edit, check the `.cm-content` node's
    // `aria-readonly` and `contenteditable` attributes — both flip
    // when the multi-select editor mounts in their place.
    const cmContent = container.querySelector(
      '[data-segment="field:task:T1.tags"] .cm-content',
    );
    expect(cmContent).not.toBeNull();
    expect(
      cmContent!.getAttribute("aria-readonly"),
      "tags field must NOT enter edit mode when Enter drills into pills — display-mode CM6 is readonly",
    ).toBe("true");
    expect(
      cmContent!.getAttribute("contenteditable"),
      "tags field must NOT enter edit mode when Enter drills into pills — display-mode CM6 is non-editable",
    ).toBe("false");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: ArrowRight from first pill lands on second pill (in-zone beam
  // search).
  //
  // After drilling into the first pill, the user arrow-keys through
  // siblings. Cardinal nav is HOST-DRIVEN (commit f6a56d7c1): the global
  // `nav.right` command resolves through the keymap and dispatches the
  // command id to the backend (`dispatch_command nav.right`), where the
  // `nav-commands` builtin plugin runs the kernel `navigate focus` op —
  // it resolves the move's origin from its own focus slot, so NO
  // client-side `spatial_navigate` IPC leaves the webview (the same
  // contract `entity-inspector.field-vertical-nav.browser.test.tsx`
  // pins for ArrowUp/ArrowDown). We then synthesize the kernel's
  // response with a focus-changed event for the second pill's key.
  // -------------------------------------------------------------------------

  it("right_from_first_pill_lands_on_second_pill", async () => {
    const { container, unmount } = renderInspector(
      makeTask({ name: "Hello", tags: ["bug", "ui"], id: "T1" }),
      makeTags(),
    );
    await flushSetup();

    // Find both pill scopes. `MentionView` registers each as a
    // FocusScope with moniker `tag:<id>`.
    const registeredScopes = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_scope")
      .map((c) => c[1] as Record<string, unknown>);
    const bugPill = registeredScopes.find((s) => s.segment === "tag:tag-bug");
    const uiPill = registeredScopes.find((s) => s.segment === "tag:tag-ui");
    expect(bugPill, "first pill must register").toBeTruthy();
    expect(uiPill, "second pill must register").toBeTruthy();

    // Seed the bug pill as the focused entity (mid-drill state).
    await fireFocusChanged({
      next_fq: bugPill!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("tag:tag-bug"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, defaultInvokeImpl) as (
        cmd: string,
        args?: unknown,
      ) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowRight", code: "ArrowRight" });
      await Promise.resolve();
    });
    await flushSetup();

    // ArrowRight on the focused pill routes the global `nav.right`
    // command id to the backend — the kernel `navigate focus` executes
    // host-side in the `nav-commands` builtin plugin (it resolves the
    // move's origin from its own focus slot), so the webview dispatches
    // the command id and NO client-side `spatial_navigate` IPC leaves
    // the webview.
    expect(
      navDispatchCmds(mockInvoke),
      "ArrowRight on a focused pill must dispatch nav.right to the backend",
    ).toEqual(["nav.right"]);
    const navigateIpcCalls = filterIpcCalls(
      "spatial_navigate",
      "navigate focus",
    );
    expect(
      navigateIpcCalls.length,
      "host-driven nav must NOT emit a client-side spatial_navigate IPC",
    ).toBe(0);

    // Synthesize the kernel's response: focus advances to the ui pill.
    await fireFocusChanged({
      next_fq: uiPill!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("tag:tag-ui"),
    });
    await flushSetup();

    // The ui pill's `<FocusScope>` wrapper flips data-focused="true".
    // Pills render as MentionView spans with `data-moniker="tag:..."`.
    const uiPillNode = container.querySelector(
      '[data-segment="tag:tag-ui"]',
    ) as HTMLElement | null;
    expect(uiPillNode).not.toBeNull();
    expect(
      uiPillNode!.getAttribute("data-focused"),
      "after the kernel reports ui pill as focused, its scope must mark data-focused=true",
    ).toBe("true");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: Escape from focused pill drills back to the field zone.
  //
  // Drill-out is HOST-DRIVEN (commit f6a56d7c1): Escape resolves through
  // the global `nav.drillOut` command, which dispatches the command id to
  // the backend (`dispatch_command nav.drillOut`). The `nav-commands`
  // builtin plugin runs the kernel `drill_out layer` op — it resolves the
  // focused scope from its own slot, COMMITS focus to the parent zone, and
  // emits `focus-changed` itself; on a layer-root edge it falls through to
  // `ui_state dismiss`. So the webview dispatches the command id and emits
  // NO client-side `spatial_drill_out` / `spatial_focus` IPC (symmetric
  // with the host-driven cardinal-nav contract in
  // `entity-inspector.field-vertical-nav.browser.test.tsx`). We then
  // synthesize the kernel's response with a focus-changed event for the
  // field zone.
  // -------------------------------------------------------------------------

  it("escape_from_pill_drills_back_to_field_zone", async () => {
    const { container, unmount } = renderInspector(
      makeTask({ name: "Hello", tags: ["bug", "ui"], id: "T1" }),
      makeTags(),
    );
    await flushSetup();

    const registeredScopes = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_scope")
      .map((c) => c[1] as Record<string, unknown>);
    const bugPill = registeredScopes.find((s) => s.segment === "tag:tag-bug");
    const tagsZone = registeredScopes.find(
      (s) => s.segment === "field:task:T1.tags",
    );
    expect(bugPill).toBeTruthy();
    expect(tagsZone, "tags field zone must register").toBeTruthy();

    // Seed the bug pill as the focused entity (mid-drill state).
    await fireFocusChanged({
      next_fq: bugPill!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("tag:tag-bug"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, defaultInvokeImpl) as (
        cmd: string,
        args?: unknown,
      ) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    // Escape on the focused pill routes the global `nav.drillOut` command
    // id to the backend — the kernel `drill_out layer` executes host-side
    // in the `nav-commands` builtin plugin (it resolves the focused scope
    // and commits the move from its own slot), so the webview dispatches
    // the command id and NO client-side `spatial_drill_out` IPC leaves the
    // webview.
    expect(
      navDispatchCmds(mockInvoke),
      "Escape on a focused pill must dispatch nav.drillOut to the backend",
    ).toEqual(["nav.drillOut"]);
    const drillOutIpcCalls = filterIpcCalls(
      "spatial_drill_out",
      "drill_out layer",
    );
    expect(
      drillOutIpcCalls.length,
      "host-driven drill-out must NOT emit a client-side spatial_drill_out IPC",
    ).toBe(0);
    // No client-side focus commit either — the kernel emits focus-changed.
    expect(
      spatialFocusCalls().length,
      "host-driven drill-out must NOT emit a client-side spatial_focus IPC",
    ).toBe(0);

    // Synthesize the kernel's response: focus returns to the tags field
    // zone. The entity-focus bridge mirrors the moniker into the store and
    // the zone's wrapper flips `data-focused="true"`.
    await fireFocusChanged({
      next_fq: tagsZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    const tagsNode = container.querySelector(
      '[data-segment="field:task:T1.tags"]',
    ) as HTMLElement | null;
    expect(tagsNode).not.toBeNull();
    expect(
      tagsNode!.getAttribute("data-focused"),
      "after the kernel reports the tags zone as focused, its scope must mark data-focused=true",
    ).toBe("true");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: Editable scalar field — Enter enters edit mode, DOM focus
  // lands on the editor input.
  //
  // The name field has `editor: "markdown"`, `display: "text"`, no
  // pills. `spatial_drill_in(nameKey)` returns null (default mock),
  // so the field.edit closure falls through to `onEdit?.()` — which
  // the inspector wires to flip its row's editing state.
  // -------------------------------------------------------------------------

  it("enter_on_editable_scalar_field_enters_edit_mode", async () => {
    const { container, unmount } = renderInspector(
      makeTask({ name: "Hello", tags: [], id: "T1" }),
    );
    await flushSetup();

    const nameZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.name",
    );
    expect(nameZone).toBeTruthy();

    // Seed focus on the name field zone (default drill-in returns
    // null — no pills).
    await fireFocusChanged({
      next_fq: nameZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.name"),
    });
    await flushSetup();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();
    // CM6 mount is async — let it settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 200));
    });

    await waitFor(() => {
      const editor = container.querySelector(
        '[data-segment="field:task:T1.name"] .cm-editor',
      );
      expect(
        editor,
        "after Enter on a focused editable field with no pills, the editor must mount",
      ).not.toBeNull();
    });

    // DOM focus lands inside the field zone (CM6's `.cm-content` is a
    // contenteditable child).
    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.name"]',
    );
    expect(fieldZone).not.toBeNull();
    const active = document.activeElement;
    expect(
      active,
      "an active element must exist after Enter mounts the editor",
    ).not.toBeNull();
    expect(
      fieldZone!.contains(active),
      "DOM focus must land somewhere inside the field zone after Enter",
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #5: Non-editable, no-pill field — Enter is a no-op.
  //
  // The id field has `editor: "none"` (so the inspector wires no
  // `onEdit`) and `display: "text"` (no pills).
  // `spatial_drill_in(idKey)` returns null. The field.edit closure
  // falls through to `onEdit?.()` which is undefined → silently
  // returns. No editor mounts; no `app.inspect` dispatch fires.
  // -------------------------------------------------------------------------

  it("enter_on_non_editable_field_with_no_pills_is_noop", async () => {
    const { container, unmount } = renderInspector(
      makeTask({ name: "Hello", tags: [], id: "T1" }),
    );
    await flushSetup();

    const idZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.id",
    );
    expect(idZone).toBeTruthy();

    // Seed focus on the id field zone.
    await fireFocusChanged({
      next_fq: idZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.id"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(
      wrapMcpDispatch(mockInvoke, defaultInvokeImpl) as (
        cmd: string,
        args?: unknown,
      ) => Promise<unknown>,
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // No editor mounts on the id field.
    expect(
      container.querySelector('[data-segment="field:task:T1.id"] .cm-editor'),
      "no editor must mount on a non-editable field after Enter",
    ).toBeNull();
    // No inspect dispatch fires.
    expect(
      inspectDispatches().length,
      "Enter on a non-editable field must NOT dispatch app.inspect",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #6: Pill field with zero pills — Enter falls through to edit mode.
  //
  // When the editable pill field's value is empty, no pills are
  // rendered → the kernel returns null from drill-in → `field.edit`
  // falls through to `onEdit?.()` and the inspector flips the row to
  // edit mode. Pin this contract under test so a future implementer
  // doesn't accidentally re-introduce a pill-only short-circuit that
  // makes empty-pill fields uneditable.
  // -------------------------------------------------------------------------

  it("enter_on_pill_field_with_zero_pills_falls_through_to_edit_or_noop", async () => {
    const { container, unmount } = renderInspector(
      makeTask({ name: "Hello", tags: [], id: "T1" }),
    );
    await flushSetup();

    const tagsZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(tagsZone, "tags field zone must register").toBeTruthy();

    // Default drill-in returns null (no pills registered for an empty
    // tags value). Seed focus on the tags field zone.
    await fireFocusChanged({
      next_fq: tagsZone!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 200));
    });

    // The tags field is editable (`editor: "multi-select"`). Without
    // pills the closure falls through to `onEdit?.()`, which the
    // inspector wires to enter edit mode — the multi-select editor
    // renders a CM6 chip picker. Distinguishing display-mode CM6
    // (readonly TextViewer for an empty list — actually no display CM6
    // for an empty list, since `EmptyBadgeList` renders a plain
    // `<span>None</span>`) from edit-mode CM6 (the multi-select
    // editor) is straightforward: the empty display has NO `.cm-editor`
    // at all, while edit mode mounts one with `contenteditable="true"`.
    await waitFor(() => {
      const cmContent = container.querySelector(
        '[data-segment="field:task:T1.tags"] .cm-content',
      );
      expect(
        cmContent,
        "after Enter on an empty editable pill field, the multi-select editor must mount its CM6 content",
      ).not.toBeNull();
      expect(
        cmContent!.getAttribute("contenteditable"),
        "the multi-select editor's CM6 content must be editable (contenteditable=true)",
      ).toBe("true");
    });

    // `app.inspect` must NOT fire — Enter is for drill-in/edit, not
    // for inspecting.
    expect(
      inspectDispatches().length,
      "Enter on an empty pill field must NOT dispatch app.inspect",
    ).toBe(0);

    unmount();
  });
});
