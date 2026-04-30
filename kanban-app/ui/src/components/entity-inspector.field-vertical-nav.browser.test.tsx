/**
 * Browser-mode tests pinning the "Up/Down arrow nav between sibling
 * inspector field zones works end-to-end" contract.
 *
 * Source of truth for card `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (bug 2). The
 * inspector panel is a `<FocusZone>` containing a stack of field zones.
 * Beam-search "down" from one field zone must pick the next field zone
 * by rect; "up" must pick the previous one. The kernel-side beam search
 * is covered by Rust unit tests; this file pins the React-side wiring:
 *
 *   1. With a field zone focused, ArrowDown dispatches
 *      `spatial_navigate(fieldKey, "down")`.
 *   2. Symmetric for ArrowUp.
 *   3. After scrolling the inspector body, the same dispatch fires
 *      with the same field key — the rects-on-scroll fix from
 *      `01KQ9XBAG5P9W3JREQYNGAYM8Y` keeps the kernel's view of the
 *      world current.
 *   4. When the kernel resolves the navigation to a new moniker, the
 *      entity-focus bridge mirrors it into the moniker store and the
 *      `<FocusZone>` for the next field flips its `data-focused`
 *      attribute to "true".
 *
 * Mock pattern follows `entity-inspector.spatial-nav.test.tsx` and
 * `board-view.spatial.test.tsx`.
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
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel
} from "@/types/spatial";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds — three fields in document order so down/up have
// unambiguous neighbours.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "tags", "body"],
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
      id: "f3",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      icon: "file-text",
      section: "body",
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

const SCHEMAS: Record<string, unknown> = {
  task: TASK_SCHEMA,
  tag: TAG_SCHEMA,
};

/**
 * Default invoke responses for the mount-time IPCs the providers fire.
 * Tests override `spatial_navigate` and `spatial_update_rect` per case.
 */
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
  if (cmd === "spatial_drill_in") return null;
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

/** Filter `spatial_navigate` calls. */
function spatialNavigateCalls(): Array<{ focusedFq: FullyQualifiedMoniker; direction: string }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_navigate")
    .map((c) => c[1] as { focusedFq: FullyQualifiedMoniker; direction: string });
}

/** Filter `spatial_update_rect` calls. */
function spatialUpdateRectCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_update_rect")
    .map((c) => c[1] as Record<string, unknown>);
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
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render the inspector wrapped in the production-shaped spatial-nav
 * stack PLUS `<AppShell>` so the global keymap handler is mounted —
 * arrow-key dispatches resolve through `BINDING_TABLES.cua` to
 * `nav.up/down/left/right` and call `spatial_navigate`. The
 * `data-testid="scroller"` wrapper mirrors the inspector body's
 * `overflow-y-auto` from `<SlidePanel>` so tests can scroll it and
 * watch the rect-on-scroll listener fire.
 */
function renderInspectorWithShell(entity: Entity) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <TooltipProvider delayDuration={100}>
                  <SchemaProvider>
                    <EntityStoreProvider entities={{ task: [entity], tag: [] }}>
                      <FieldUpdateProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <AppShell>
                            <div
                              data-testid="scroller"
                              style={{ height: "200px", overflowY: "auto" }}
                            >
                              <div style={{ minHeight: "1000px" }}>
                                <EntityInspector entity={entity} />
                              </div>
                            </div>
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

describe("EntityInspector — Up/Down arrow nav between sibling field zones", () => {
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
  // #1: Down from first field lands on second field.
  // -------------------------------------------------------------------------

  it("down_from_first_field_lands_on_second_field", async () => {
    const { container, unmount } = renderInspectorWithShell(
      makeTask({ title: "Hello", tags: [], body: "" }),
    );
    await flushSetup();

    const titleZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    const tagsZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(
      titleZone,
      "title field zone must register",
    ).toBeTruthy();
    expect(tagsZone, "tags field zone must register").toBeTruthy();

    // Seed focus on the first (title) field zone so `nav.down`'s
    // execute closure sees `focusedKey() === titleKey`.
    await fireFocusChanged({
      next_fq: titleZone!.key as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await Promise.resolve();
    });
    await flushSetup();

    // The global `nav.down` command's closure dispatches
    // `spatial_navigate(focusedKey, "down")`. The kernel resolves the
    // beam search and (in production) emits `focus-changed` for the
    // next zone — that side is covered by Rust tests. Here we pin
    // the React side: the IPC fires with the focused key and direction
    // "down".
    const navCalls = spatialNavigateCalls();
    expect(
      navCalls.length,
      "ArrowDown on a focused field zone must dispatch spatial_navigate exactly once",
    ).toBe(1);
    expect(navCalls[0].focusedFq).toBe(titleZone!.focusedFq);
    expect(navCalls[0].direction).toBe("down");

    // Now simulate the kernel's response — the next field zone is the
    // tags row. Emit focus-changed for that key/moniker; the entity-
    // focus bridge mirrors the moniker into the store and the tags
    // zone's wrapper flips `data-focused="true"`.
    await fireFocusChanged({
      next_fq: tagsZone!.key as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    const tagsNode = container.querySelector(
      `[data-segment="field:task:T1.tags"]`,
    ) as HTMLElement | null;
    expect(tagsNode).not.toBeNull();
    expect(
      tagsNode!.getAttribute("data-focused"),
      "after the kernel reports tags as focused, its zone must show data-focused=true",
    ).toBe("true");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: Up from last field lands on previous field.
  // -------------------------------------------------------------------------

  it("up_from_last_field_lands_on_previous_field", async () => {
    const { container, unmount } = renderInspectorWithShell(
      makeTask({ title: "Hello", tags: [], body: "Some body" }),
    );
    await flushSetup();

    const tagsZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    const bodyZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:T1.body",
    );
    expect(tagsZone).toBeTruthy();
    expect(bodyZone).toBeTruthy();

    // Seed focus on the body field zone (the last in document order).
    await fireFocusChanged({
      next_fq: bodyZone!.key as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.body"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
      await Promise.resolve();
    });
    await flushSetup();

    const navCalls = spatialNavigateCalls();
    expect(navCalls.length).toBe(1);
    expect(navCalls[0].focusedFq).toBe(bodyZone!.focusedFq);
    expect(navCalls[0].direction).toBe("up");

    // Simulate the kernel's response — beam-up resolves to the tags
    // zone (the one above body in document order).
    await fireFocusChanged({
      next_fq: tagsZone!.key as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    const tagsNode = container.querySelector(
      `[data-segment="field:task:T1.tags"]`,
    ) as HTMLElement;
    expect(tagsNode.getAttribute("data-focused")).toBe("true");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: After scrolling the inspector body, ArrowDown still picks the
  // correct sibling — the rect-on-scroll fix keeps the kernel's view of
  // the world current.
  //
  // Strategy: dispatch a scroll event on the document (the inspector
  // panel is rendered via `<AppShell>` but for the rect-on-scroll
  // contract any scrollable ancestor will do), assert that
  // `spatial_update_rect` was invoked at least once for the title and
  // tags field zones (the key of each registered zone), then fire
  // ArrowDown and assert the dispatch still uses the same field key.
  // -------------------------------------------------------------------------

  it("down_after_scroll_picks_next_field_in_content_order", async () => {
    const { container, unmount } = renderInspectorWithShell(
      makeTask({ title: "Hello", tags: [], body: "Body text" }),
    );
    await flushSetup();

    const titleZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:T1.title",
    );
    expect(titleZone).toBeTruthy();

    // Seed focus on the title field zone.
    await fireFocusChanged({
      next_fq: titleZone!.key as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.title"),
    });
    await flushSetup();

    // Scroll the inspector-body wrapper. The rect-on-scroll listener
    // installed by every `<FocusZone>` (`useTrackRectOnAncestorScroll`)
    // walks up to find scrollable ancestors and subscribes; scrolling
    // any of them calls `spatial_update_rect(key, rect)` for the
    // descendant zone(s). Use the `data-testid="scroller"` wrapper to
    // mirror the production `<SlidePanel>`'s `overflow-y-auto` body.
    const scroller = container.querySelector(
      '[data-testid="scroller"]',
    ) as HTMLElement | null;
    expect(scroller, "test harness must mount the scroller wrapper").not.toBeNull();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    await act(async () => {
      scroller!.scrollTop = 100;
      scroller!.dispatchEvent(new Event("scroll"));
      await new Promise((r) => setTimeout(r, 30));
    });

    const updateCalls = spatialUpdateRectCalls();
    expect(
      updateCalls.length,
      "the rect-on-scroll listener must refresh at least one field zone's rect after scroll",
    ).toBeGreaterThan(0);
    // At least one of the update calls must target the title zone's
    // key — the listener fires for every registered descendant.
    const titleUpdateCall = updateCalls.find(
      (c) => c.key === titleZone!.key,
    );
    expect(
      titleUpdateCall,
      "rect-on-scroll must include the focused (title) field zone",
    ).toBeTruthy();

    // Now press ArrowDown — the dispatch carries the focused field
    // key, so the kernel can run beam search over the up-to-date
    // rects.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await Promise.resolve();
    });
    await flushSetup();

    const navCalls = spatialNavigateCalls();
    expect(navCalls.length).toBe(1);
    expect(navCalls[0].focusedFq).toBe(titleZone!.focusedFq);
    expect(navCalls[0].direction).toBe("down");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: Down at the last visible field scrolls to bring the next
  // field into view, then settles focus on that field.
  //
  // The kernel's beam search drives the geometry decision; the
  // `<FocusZone>`'s `scrollIntoView` effect fires when the
  // entity-focus store reports it as directly focused. So once the
  // kernel resolves "down" to the body field and emits
  // `focus-changed` with that moniker, the body zone calls
  // `scrollIntoView({ block: "nearest" })`. We pin the contract by
  // observing `data-focused` flipping on the body zone after the
  // synthesized focus-changed.
  // -------------------------------------------------------------------------

  it("down_at_last_visible_field_scrolls_to_bring_next_field_into_view", async () => {
    const { container, unmount } = renderInspectorWithShell(
      makeTask({ title: "Hello", tags: [], body: "Body text" }),
    );
    await flushSetup();

    const tagsZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    const bodyZone = registerZoneArgs().find(
      (a) => a.segment === "field:task:T1.body",
    );
    expect(tagsZone).toBeTruthy();
    expect(bodyZone).toBeTruthy();

    // Pretend tags is the last field "in view" and the focused one;
    // body is below the fold.
    await fireFocusChanged({
      next_fq: tagsZone!.key as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await Promise.resolve();
    });
    await flushSetup();

    const navCalls = spatialNavigateCalls();
    expect(navCalls.length).toBe(1);
    expect(navCalls[0].focusedFq).toBe(tagsZone!.focusedFq);
    expect(navCalls[0].direction).toBe("down");

    // Spy on body zone's `scrollIntoView` (jsdom / chromium-test
    // implementations differ — install a stub so we can observe it).
    const bodyNode = container.querySelector(
      `[data-segment="field:task:T1.body"]`,
    ) as HTMLElement;
    expect(bodyNode).not.toBeNull();
    const scrollSpy = vi.fn();
    bodyNode.scrollIntoView = scrollSpy;

    // Synthesize the kernel's response: focus advances to body.
    await fireFocusChanged({
      next_fq: bodyZone!.key as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.body"),
    });
    await flushSetup();
    // Allow the scroll-into-view useEffect to run after the focus
    // claim flips.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30));
    });

    expect(
      bodyNode.getAttribute("data-focused"),
      "after the kernel reports body as focused, its zone must mark data-focused=true",
    ).toBe("true");
    expect(
      scrollSpy,
      "the focused zone must call scrollIntoView so the just-focused field becomes visible",
    ).toHaveBeenCalled();
    expect(
      scrollSpy.mock.calls[0][0],
      "scroll must use { block: 'nearest' } so the panel scrolls just enough to expose the new field",
    ).toEqual({ block: "nearest" });

    unmount();
  });
});
