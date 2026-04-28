/**
 * Browser-mode tests pinning the "Enter on a focused inspector field
 * zone drills into pills first, falls through to edit mode only when
 * there are no pills" contract.
 *
 * Source of truth for card `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (bug 3). The
 * field-zone scope-level `field.edit` `CommandDef` (keys: vim Enter /
 * cua Enter) was extended to:
 *
 *   1. Read the focused field-zone `SpatialKey` from the spatial
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

type ListenCallback = (event: { payload: unknown }) => void;

/**
 * Per-test storage for `spatial_drill_in` responses keyed by
 * `SpatialKey`. Tests set entries here before pressing Enter so the
 * mock kernel returns the right child moniker for the focused field.
 */
const drillInResponses = new Map<string, string | null>();

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
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import {
  asLayerName,
  type FocusChangedPayload,
  type SpatialKey,
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

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
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
    const key = (args as { key?: string })?.key ?? "";
    return drillInResponses.has(key) ? drillInResponses.get(key)! : null;
  }
  if (cmd === "spatial_drill_out") return null;
  if (cmd === "spatial_navigate") return null;
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

/** Collect every `spatial_register_zone` invocation argument bag. */
function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Filter `dispatch_command` calls down to those for `ui.setFocus`. */
function setFocusDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "ui.setFocus");
}

/** Filter `dispatch_command` calls down to those for `ui.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "ui.inspect");
}

/** Filter `spatial_drill_in` calls. */
function drillInCalls(): Array<{ key: SpatialKey }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_drill_in")
    .map((c) => c[1] as { key: SpatialKey });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one for the current window.
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
 * Render the inspector for a given task entity inside the production-
 * shaped spatial-nav stack PLUS `<AppShell>` so the global keymap
 * handler is mounted (Enter resolves through the field-zone scope to
 * `field.edit`'s execute closure).
 */
function renderInspector(entity: Entity, tagEntities: Entity[] = []) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asLayerName("window")}>
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
    drillInResponses.clear();
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
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

    const tagsZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:T1.tags",
    );
    expect(tagsZone, "tags field zone must register").toBeTruthy();

    // Stub the kernel so drill-in on the tags zone returns the first
    // pill's moniker.
    drillInResponses.set(tagsZone!.key as string, "tag:tag-bug");

    // Seed focus on the tags field zone.
    await fireFocusChanged({
      next_key: tagsZone!.key as SpatialKey,
      next_moniker: "field:task:T1.tags",
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

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
    expect(drillCalls[0].key).toBe(tagsZone!.key);

    // The closure's success branch fanned out via `setFocus` → the
    // entity-focus bridge dispatched `ui.setFocus` whose
    // `args.scope_chain` opens with `tag:tag-bug`.
    const setFocusCalls = setFocusDispatches();
    expect(setFocusCalls.length).toBeGreaterThanOrEqual(1);
    const targetCall = setFocusCalls.find((c) => {
      const args = c.args as { scope_chain?: string[] } | undefined;
      return args?.scope_chain?.[0] === "tag:tag-bug";
    });
    expect(
      targetCall,
      "ui.setFocus dispatch must carry the first pill's moniker at the head of args.scope_chain",
    ).toBeTruthy();

    // The field stayed in display mode. `BadgeListDisplay` renders
    // pills via `MentionView` → `TextViewer` (a *readonly* CM6 mount),
    // so `.cm-editor` is always present for badge-list fields. To
    // distinguish display vs. edit, check the `.cm-content` node's
    // `aria-readonly` and `contenteditable` attributes — both flip
    // when the multi-select editor mounts in their place.
    const cmContent = container.querySelector(
      '[data-moniker="field:task:T1.tags"] .cm-content',
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
  // siblings. The kernel resolves "right" via `spatial_navigate(pillKey,
  // "right")`; we synthesize the kernel's response with a
  // focus-changed event for the second pill's key.
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
    const bugPill = registeredScopes.find((s) => s.moniker === "tag:tag-bug");
    const uiPill = registeredScopes.find((s) => s.moniker === "tag:tag-ui");
    expect(bugPill, "first pill must register").toBeTruthy();
    expect(uiPill, "second pill must register").toBeTruthy();

    // Seed the bug pill as the focused entity (mid-drill state).
    await fireFocusChanged({
      next_key: bugPill!.key as SpatialKey,
      next_moniker: "tag:tag-bug",
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowRight", code: "ArrowRight" });
      await Promise.resolve();
    });
    await flushSetup();

    // The global `nav.right` command's closure dispatched
    // `spatial_navigate(focusedKey, "right")` for the bug pill's key.
    const navCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_navigate")
      .map((c) => c[1] as { key: SpatialKey; direction: string });
    expect(navCalls.length).toBe(1);
    expect(navCalls[0].key).toBe(bugPill!.key);
    expect(navCalls[0].direction).toBe("right");

    // Synthesize the kernel's response: focus advances to the ui pill.
    await fireFocusChanged({
      next_key: uiPill!.key as SpatialKey,
      next_moniker: "tag:tag-ui",
    });
    await flushSetup();

    // The ui pill's `<FocusScope>` wrapper flips data-focused="true".
    // Pills render as MentionView spans with `data-moniker="tag:..."`.
    const uiPillNode = container.querySelector(
      '[data-moniker="tag:tag-ui"]',
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
  // Escape resolves through the global `nav.drillOut` command, which
  // dispatches `spatial_drill_out(pillKey)` and on a non-null moniker
  // calls `setFocus(...)` against the entity-focus store.
  // -------------------------------------------------------------------------

  it("escape_from_pill_drills_back_to_field_zone", async () => {
    const { unmount } = renderInspector(
      makeTask({ name: "Hello", tags: ["bug", "ui"], id: "T1" }),
      makeTags(),
    );
    await flushSetup();

    const registeredScopes = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_scope")
      .map((c) => c[1] as Record<string, unknown>);
    const bugPill = registeredScopes.find((s) => s.moniker === "tag:tag-bug");
    expect(bugPill).toBeTruthy();

    // Seed the bug pill as the focused entity (mid-drill state).
    await fireFocusChanged({
      next_key: bugPill!.key as SpatialKey,
      next_moniker: "tag:tag-bug",
    });
    await flushSetup();

    // Stub the kernel: drill-out on the bug pill returns the field
    // zone's moniker.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "spatial_drill_out") return "field:task:T1.tags";
      return defaultInvokeImpl(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    // The drill-out closure dispatched `spatial_drill_out` for the
    // pill's key.
    const drillOutCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_drill_out")
      .map((c) => c[1] as { key: SpatialKey });
    expect(drillOutCalls.length).toBe(1);
    expect(drillOutCalls[0].key).toBe(bugPill!.key);

    // The success branch dispatched `ui.setFocus` with the field zone
    // moniker at the head of `args.scope_chain`.
    const setFocusCalls = setFocusDispatches();
    const target = setFocusCalls.find((c) => {
      const args = c.args as { scope_chain?: string[] } | undefined;
      return args?.scope_chain?.[0] === "field:task:T1.tags";
    });
    expect(
      target,
      "Escape from a pill must dispatch ui.setFocus(field zone moniker)",
    ).toBeTruthy();

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

    const nameZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:T1.name",
    );
    expect(nameZone).toBeTruthy();

    // Seed focus on the name field zone (default drill-in returns
    // null — no pills).
    await fireFocusChanged({
      next_key: nameZone!.key as SpatialKey,
      next_moniker: "field:task:T1.name",
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
        '[data-moniker="field:task:T1.name"] .cm-editor',
      );
      expect(
        editor,
        "after Enter on a focused editable field with no pills, the editor must mount",
      ).not.toBeNull();
    });

    // DOM focus lands inside the field zone (CM6's `.cm-content` is a
    // contenteditable child).
    const fieldZone = container.querySelector(
      '[data-moniker="field:task:T1.name"]',
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
  // returns. No editor mounts; no `ui.inspect` dispatch fires.
  // -------------------------------------------------------------------------

  it("enter_on_non_editable_field_with_no_pills_is_noop", async () => {
    const { container, unmount } = renderInspector(
      makeTask({ name: "Hello", tags: [], id: "T1" }),
    );
    await flushSetup();

    const idZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:T1.id",
    );
    expect(idZone).toBeTruthy();

    // Seed focus on the id field zone.
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

    // No editor mounts on the id field.
    expect(
      container.querySelector('[data-moniker="field:task:T1.id"] .cm-editor'),
      "no editor must mount on a non-editable field after Enter",
    ).toBeNull();
    // No inspect dispatch fires.
    expect(
      inspectDispatches().length,
      "Enter on a non-editable field must NOT dispatch ui.inspect",
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

    const tagsZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:T1.tags",
    );
    expect(tagsZone, "tags field zone must register").toBeTruthy();

    // Default drill-in returns null (no pills registered for an empty
    // tags value). Seed focus on the tags field zone.
    await fireFocusChanged({
      next_key: tagsZone!.key as SpatialKey,
      next_moniker: "field:task:T1.tags",
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
        '[data-moniker="field:task:T1.tags"] .cm-content',
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

    // `ui.inspect` must NOT fire — Enter is for drill-in/edit, not
    // for inspecting.
    expect(
      inspectDispatches().length,
      "Enter on an empty pill field must NOT dispatch ui.inspect",
    ).toBe(0);

    unmount();
  });
});
