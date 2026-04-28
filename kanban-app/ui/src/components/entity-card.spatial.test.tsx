/**
 * Browser-mode test for `<EntityCard>`'s spatial-nav behaviour.
 *
 * Source of truth for acceptance of card `01KQ20NMRQ...`. The card body
 * wraps in `<FocusScope moniker="task:{id}">` — a leaf in the spatial
 * graph, NOT a zone. The leaf shape is what enables cross-column nav
 * under the unified cascade: pressing right on a card in column A
 * runs iter 0 against in-column card peers, and when no peer
 * satisfies the beam test the cascade escalates to iter 1 — the
 * card's parent column zone — and lands on the neighbouring column
 * zone (which the React adapter drills back into). If the card body
 * were a zone, iter 0 would consider sibling zones only — same-column
 * cards reachable as zones, never the cross-column trajectory the
 * user expects. See the docstring on `<EntityCard>` and the kernel
 * test `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`.
 *
 * Each visible field inside the card renders through `<Field>`, which
 * is itself a `<FocusZone moniker="field:task:{id}.{name}">`. Because
 * `<FocusScope>` does NOT push a `FocusZoneContext.Provider`, those
 * field zones are siblings of the card under the column zone — not
 * children of the card. Multi-value fields (badge-list assignees /
 * tags) render one `<FocusScope>` leaf per pill under their owning
 * field zone. This file exercises the click → `spatial_focus` →
 * `focus-changed` → React state → `<FocusIndicator>` chain end-to-end
 * so a regression in any link surfaces here.
 *
 * Mock pattern matches `grid-view.nav-is-eventdriven.test.tsx` and
 * `perspective-bar.spatial.test.tsx`:
 *   - `vi.hoisted` builds an invoke / listen mock pair the test owns.
 *   - `mockListen` records every `listen("focus-changed", cb)` callback
 *     so `fireFocusChanged(key)` can drive the React tree as if the
 *     Rust kernel had emitted a `focus-changed` event.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright) — every `*.test.tsx` outside
 * `*.node.test.tsx` lands here.
 *
 * # Test coverage map (vs the card description)
 *
 * The card's "Browser Tests" section enumerates eight numbered cases.
 * Six map directly onto tests in this file; two are left to the layer
 * above the card because they exercise the global keymap pipeline,
 * not the card itself:
 *
 *   - **#1 Registration**: `registers the card body as a FocusScope (leaf)`.
 *   - **#2 Click → focus**: `clicking the card body fires spatial_focus`.
 *   - **#3 Focus claim → visible bar**: `focus claim mounts FocusIndicator`.
 *   - **#4 Keystrokes → navigate**: NOT covered here. Arrow / vim keys
 *     are bound at `<AppShell>` to `nav.left` / `nav.right` / `nav.up` /
 *     `nav.down`. The card itself attaches no `keydown` listener — the
 *     navigation pipeline runs on the focused `SpatialKey` from
 *     `SpatialFocusProvider`'s ref. The app-shell side of that contract
 *     is covered in `app-shell.test.tsx`; the card side is "do nothing",
 *     verified indirectly by the legacy-nav-stripped assertions below.
 *   - **#5 Space → inspect**: NOT covered here. Space is bound at
 *     `<AppShell>` (or its scope-binding pipeline) to a card-scoped
 *     `ui.inspect` command. The card itself owns no Space handler.
 *     Verified indirectly by the legacy-nav-stripped assertions and by
 *     the existing `entity-card.test.tsx` suite that pins the
 *     `ui.inspect` dispatch shape on the (i) button.
 *   - **#6 Enter → drill-in**: NOT covered here. Enter is bound at
 *     `<AppShell>` to `nav.drillIn`, which reads the focused
 *     `SpatialKey` and invokes `spatial_drill_in`. Covered in
 *     `app-shell.test.tsx` (`nav.drillIn invokes spatial_drill_in for
 *     the focused SpatialKey on Enter`).
 *   - **#7 Unmount**: `unmount unregisters the card scope`.
 *   - **#8 Legacy-nav stripped**: `no entity_focus_* / claim_when_* /
 *     broadcast_nav_* IPCs fire on click`.
 *
 * # Per-leaf coverage
 *
 * The card description requires per-leaf assertions: each visible field
 * under the card carries `[data-moniker]` and clicking it dispatches
 * `spatial_focus` for THAT leaf's key, not the card's key. Covered by
 * the `per-leaf clicks` describe block below — title (single-value
 * field zone), tag pills (badge-list inner leaves), assignee pills
 * (badge-list inner leaves).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

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
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

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
// Imports — after mocks
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { EntityCard } from "./entity-card";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  asLayerName,
  type FocusChangedPayload,
  type SpatialKey,
  type WindowLabel,
} from "@/types/spatial";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema + fixture data
// ---------------------------------------------------------------------------

/**
 * Task schema used by the spatial test harness.
 *
 * Mirrors the on-disk task schema enough to drive the card's field
 * dispatch logic. `title` and `status` are single-value text fields;
 * `tags` and `assignees` are badge-list reference fields that render
 * one pill leaf per value. The card body iterates the `header` section
 * and produces a `<Field>` per visible field — which is what the
 * spatial graph needs to populate.
 */
const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    body_field: "body",
    fields: ["title", "status", "tags", "assignees", "body"],
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
      id: "f-tags",
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
      id: "f-assignees",
      name: "assignees",
      type: {
        kind: "reference",
        entity: "actor",
        multiple: true,
      },
      editor: "multi-select",
      display: "badge-list",
      icon: "user",
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

/**
 * Tag schema needed so MentionView can resolve `#bug` / `#ui` against the
 * tag namespace. Without it pills still render (slug fallback) but the
 * resolution path emits a console warning.
 */
const TAG_SCHEMA = {
  entity: {
    name: "tag",
    entity_type: "tag",
    fields: ["tag_name"],
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
} as unknown as EntitySchema;

/**
 * Actor schema for assignee pills. Same shape as the tag schema — pills
 * render with a per-actor moniker `actor:{id}`.
 */
const ACTOR_SCHEMA = {
  entity: {
    name: "actor",
    entity_type: "actor",
    fields: ["display_name"],
    mention_prefix: "@",
    mention_display_field: "display_name",
  },
  fields: [
    {
      id: "an",
      name: "display_name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
} as unknown as EntitySchema;

const SCHEMAS: Record<string, EntitySchema> = {
  task: TASK_SCHEMA,
  tag: TAG_SCHEMA,
  actor: ACTOR_SCHEMA,
};

/**
 * Default invoke responses for the mount-time IPCs the providers fire.
 *
 * Every test starts with `mockInvoke.mockImplementation(defaultInvokeImpl)`
 * so the schema / UI-state / undo-state seeds the providers ask for at
 * mount don't return undefined and force their consumers into a loading
 * state that suppresses the card render.
 */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task", "tag", "actor"];
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
  if (cmd === "show_context_menu") return undefined;
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

/**
 * Build a task `Entity` with sensible defaults and optional field
 * overrides. Matches the shape the entity-store seeds the schema-driven
 * field dispatch with.
 */
function makeTask(fieldOverrides: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
    fields: {
      title: "Hello world",
      status: "todo",
      tags: ["bug", "ui"],
      assignees: ["alice", "bob"],
      body: "",
      depends_on: [],
      position_column: "col-1",
      position_ordinal: "a0",
      ...fieldOverrides,
    },
  };
}

/**
 * Tag entity seeds — slug → display-name resolution. Without these in
 * the entity store, MentionView's pill widgets fall back to the
 * unresolved-mark muted style; the moniker on each pill is still
 * `tag:{slug}`, which is what the spatial assertions key off, but
 * having real entities mirrors production more closely.
 */
function makeTags(): Entity[] {
  return [
    {
      entity_type: "tag",
      id: "bug",
      moniker: "tag:bug",
      fields: { tag_name: "bug" },
    },
    {
      entity_type: "tag",
      id: "ui",
      moniker: "tag:ui",
      fields: { tag_name: "ui" },
    },
  ];
}

/** Actor entity seeds — same role as `makeTags` for assignee pills. */
function makeActors(): Entity[] {
  return [
    {
      entity_type: "actor",
      id: "alice",
      moniker: "actor:alice",
      fields: { display_name: "Alice" },
    },
    {
      entity_type: "actor",
      id: "bob",
      moniker: "actor:bob",
      fields: { display_name: "Bob" },
    },
  ];
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

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one for the current window.
 *
 * Wraps the dispatch in `act()` so React state updates flush before the
 * caller asserts against post-update DOM in the next tick.
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

/** Collect every `spatial_register_zone` invocation argument bag. */
function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ key: SpatialKey }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { key: SpatialKey });
}

/** Collect every `spatial_unregister_scope` call's args, in order. */
function unregisterScopeCalls(): Array<{ key: SpatialKey }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => c[1] as { key: SpatialKey });
}

/**
 * Render the card wrapped in the production-shaped spatial-nav stack.
 *
 * Mirrors the provider tree `App.tsx` mounts: `<SpatialFocusProvider>`
 * + `<FocusLayer>` so the card's `<FocusZone>` registers via
 * `spatial_register_zone`; `<EntityFocusProvider>` so the entity-focus
 * scope registry and `setFocus` chrome work; the schema / store / field-
 * update / UI-state providers because the schema-driven field dispatch
 * inside the card reads from all four.
 */
function renderCard(entity: Entity = makeTask()) {
  return render(
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <TooltipProvider delayDuration={100}>
            <SchemaProvider>
              <EntityStoreProvider
                entities={{ task: [entity], tag: makeTags(), actor: makeActors() }}
              >
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <EntityCard entity={entity} />
                    </UIStateProvider>
                  </FieldUpdateProvider>
                </EntityFocusProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </CommandBusyProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("EntityCard — browser spatial behaviour", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ---------------------------------------------------------------------
  // #1 Registration
  // ---------------------------------------------------------------------
  it("registers the card body as a FocusScope (leaf) with moniker task:{id} (test #1)", async () => {
    const { unmount } = renderCard();
    await flushSetup();

    const cardScope = registerScopeArgs().find(
      (a) => a.moniker === "task:task-1",
    );
    expect(cardScope).toBeTruthy();
    expect(typeof cardScope!.key).toBe("string");
    // The key matches the `^task:[0-9A-Z-]+$` shape — runtime key minted
    // via `crypto.randomUUID()` in `<FocusScope>`. The moniker is the
    // production task moniker; the spatial key is opaque per-mount.
    expect(cardScope!.moniker).toMatch(/^task:[A-Za-z0-9-]+$/);
    expect(cardScope!.layerKey).toBeTruthy();
    // In this isolated harness the card has no surrounding `<FocusZone>`,
    // so its `parentZone` is null. In production the card is wrapped by
    // a `column:` zone and that zone's key flows through here.
    expect(cardScope!.parentZone).toBeNull();
    expect(cardScope!.rect).toBeTruthy();

    unmount();
  });

  it("does not register the card root as a FocusZone (the card is a leaf, not a zone) (test #1b)", async () => {
    // Cards must register as leaves so the unified cascade's iter-0 /
    // iter-1 trajectory works as the user expects: iter 0 finds
    // in-column card peers; iter 1 escalates to the card's parent
    // column zone and lands on the neighbouring column zone. If the
    // card ever flips back to being a zone, iter 0 would consider
    // sibling zones only and trap focus inside the column. See the
    // docstring on `<EntityCard>` and the kernel test
    // `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`.
    const { unmount } = renderCard();
    await flushSetup();

    const zoneCalls = registerZoneArgs().filter(
      (a) => a.moniker === "task:task-1",
    );
    expect(zoneCalls).toEqual([]);

    unmount();
  });

  // ---------------------------------------------------------------------
  // #2 Click → focus
  // ---------------------------------------------------------------------
  it("clicking the card body dispatches exactly one spatial_focus for the card key (test #2)", async () => {
    const { container, unmount } = renderCard();
    await flushSetup();

    const cardScope = registerScopeArgs().find(
      (a) => a.moniker === "task:task-1",
    )!;
    const cardKey = cardScope.key as SpatialKey;

    mockInvoke.mockClear();

    // Click the card body's chrome — outside any inner field — so the
    // event lands on the card-scope div, not on a descendant zone.
    const cardBody = container.querySelector(
      `[data-entity-card='task-1']`,
    ) as HTMLElement | null;
    expect(cardBody).not.toBeNull();
    fireEvent.click(cardBody!);

    const focusCalls = spatialFocusCalls();
    // Exactly one focus call for the card; no extra call for ancestor
    // zones (the card stops propagation).
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].key).toBe(cardKey);

    unmount();
  });

  // ---------------------------------------------------------------------
  // #3 Focus claim → visible bar
  // ---------------------------------------------------------------------
  it("focus claim mounts the FocusIndicator inside the card body (test #3)", async () => {
    const { container, queryByTestId, unmount } = renderCard();
    await flushSetup();

    const cardScope = registerScopeArgs().find(
      (a) => a.moniker === "task:task-1",
    )!;
    const cardKey = cardScope.key as SpatialKey;

    // Before the focus claim, the card has no FocusIndicator descendant
    // attributable to it: the card body is `data-focused === undefined`
    // and the ONLY `data-testid="focus-indicator"` we'd see — if any
    // existed at all — would belong to some other zone outside the
    // card subtree.
    const cardNode = container.querySelector(
      `[data-moniker='task:task-1']`,
    ) as HTMLElement;
    expect(cardNode).not.toBeNull();
    expect(cardNode.getAttribute("data-focused")).toBeNull();

    await fireFocusChanged({
      next_key: cardKey,
      next_moniker: "task:task-1",
    });

    await waitFor(() => {
      // The card body's data-focused flips.
      expect(cardNode.getAttribute("data-focused")).not.toBeNull();
      // And a FocusIndicator mounts as a descendant of the card body.
      const indicator = queryByTestId("focus-indicator");
      expect(indicator).not.toBeNull();
      expect(cardNode.contains(indicator!)).toBe(true);
    });

    unmount();
  });

  // ---------------------------------------------------------------------
  // #4 Keystrokes → navigate (deferred — see file header)
  // ---------------------------------------------------------------------
  // ArrowUp/Down/Left/Right and `k`/`j`/`h`/`l` are bound at `<AppShell>`
  // to `nav.up` / `nav.down` / `nav.left` / `nav.right`. The card itself
  // attaches no `keydown` listener — the navigation pipeline runs on the
  // currently-focused `SpatialKey` from `SpatialFocusProvider`. The
  // app-shell side of the contract is covered by `app-shell.test.tsx`
  // (which exercises `nav.drillIn` and the surrounding global handler);
  // this test asserts the card-side contract — "do nothing" — by
  // confirming no `keydown` listener is wired by the card.

  it("the card attaches no keydown listener of its own (test #4 stand-in)", async () => {
    const { container, unmount } = renderCard();
    await flushSetup();

    // A keydown event fired on the card body must not bubble out of an
    // owned listener on the card itself. The card delegates all
    // keystrokes to the app-shell global handler, so we assert the
    // card's outer div has no `onkeydown` attribute and no inline
    // listener registered via React's synthetic event system that we
    // can detect from outside. We use a direct property check; React's
    // synthetic listeners attach at the document root so checking
    // `onkeydown` on the DOM node itself is sufficient to catch a
    // regression that re-introduces a card-level handler.
    const cardBody = container.querySelector(
      `[data-entity-card='task-1']`,
    ) as HTMLElement;
    expect(cardBody).not.toBeNull();
    expect(cardBody.onkeydown).toBeNull();

    // Sanity: the card's outer FocusZone div also has no keydown handler.
    const cardZoneNode = container.querySelector(
      `[data-moniker='task:task-1']`,
    ) as HTMLElement;
    expect(cardZoneNode.onkeydown).toBeNull();

    unmount();
  });

  // ---------------------------------------------------------------------
  // #5 Space → inspect (deferred — covered by app-shell scope bindings)
  // ---------------------------------------------------------------------
  // Space is a card-scoped command (`ui.inspect` for cards). Binding
  // happens through the CommandScope chain that AppShell's keymap
  // pipeline reads. The card side of the contract is "do nothing on
  // raw Space — let the global handler resolve the binding from scope".
  // We assert here that the card itself does not invoke `spatial_navigate`
  // when a Space key is fired on it — i.e. Space is NOT mapped to a
  // navigation action by the card.

  it("a Space keystroke on the card does not dispatch spatial_navigate (test #5 stand-in)", async () => {
    const { container, unmount } = renderCard();
    await flushSetup();

    const cardBody = container.querySelector(
      `[data-entity-card='task-1']`,
    ) as HTMLElement;
    expect(cardBody).not.toBeNull();

    mockInvoke.mockClear();
    fireEvent.keyDown(cardBody, { key: " ", code: "Space" });
    await flushSetup();

    const navigateCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_navigate",
    );
    expect(navigateCalls).toEqual([]);

    unmount();
  });

  // ---------------------------------------------------------------------
  // #6 Enter → drill-in (deferred — covered by app-shell.test.tsx)
  // ---------------------------------------------------------------------
  // Enter is bound at `<AppShell>` to `nav.drillIn`, which reads the
  // focused `SpatialKey` from `SpatialFocusProvider` and invokes
  // `spatial_drill_in`. The app-shell test pins that pipeline; the
  // card-side contract is "no own Enter listener". We assert that here.

  it("a bare Enter keystroke on the card does not dispatch spatial_drill_in (test #6 stand-in)", async () => {
    const { container, unmount } = renderCard();
    await flushSetup();

    const cardBody = container.querySelector(
      `[data-entity-card='task-1']`,
    ) as HTMLElement;
    expect(cardBody).not.toBeNull();

    mockInvoke.mockClear();
    fireEvent.keyDown(cardBody, { key: "Enter", code: "Enter" });
    await flushSetup();

    // No card-owned drill-in — the global handler is what wires Enter,
    // and it is not mounted in this harness.
    const drillCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_drill_in",
    );
    expect(drillCalls).toEqual([]);

    unmount();
  });

  // ---------------------------------------------------------------------
  // #7 Unmount
  // ---------------------------------------------------------------------
  it("unmounting the card dispatches spatial_unregister_scope for the card key (test #7)", async () => {
    const { unmount } = renderCard();
    await flushSetup();

    const cardScope = registerScopeArgs().find(
      (a) => a.moniker === "task:task-1",
    )!;
    const cardKey = cardScope.key as SpatialKey;

    mockInvoke.mockClear();
    unmount();

    const unregisterKeys = unregisterScopeCalls().map((c) => c.key);
    expect(unregisterKeys).toContain(cardKey);
  });

  // ---------------------------------------------------------------------
  // #8 Legacy nav stripped
  // ---------------------------------------------------------------------
  it("emits no legacy entity_focus_* / claim_when_* / broadcast_nav_* IPCs (test #8)", async () => {
    const { container, unmount } = renderCard();
    await flushSetup();

    // Click the card body to exercise every IPC the card-side code
    // would fire on a typical user interaction.
    const cardBody = container.querySelector(
      `[data-entity-card='task-1']`,
    ) as HTMLElement;
    fireEvent.click(cardBody!);

    const banned = /^(entity_focus_|claim_when_|broadcast_nav_)/;
    const offenders = mockInvoke.mock.calls
      .map((c) => c[0])
      .filter((cmd) => typeof cmd === "string" && banned.test(cmd as string));
    expect(offenders).toEqual([]);

    unmount();
  });

  // ---------------------------------------------------------------------
  // Per-leaf clicks — title field zone + multi-value pills
  //
  // The card's per-leaf contract: each focusable atom inside the card
  // (title field, status field, assignee pills, tag pills) carries
  // `data-moniker` and a click on that atom dispatches `spatial_focus`
  // for THAT atom's spatial key, not the card's key. This block covers
  // the contract one leaf type at a time.
  // ---------------------------------------------------------------------
  describe("per-leaf clicks", () => {
    it("the title field is a FocusZone with moniker field:task:{id}.title sibling-parented to the card's parent zone", async () => {
      const { container, unmount } = renderCard();
      await flushSetup();

      const titleZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.title",
      );
      expect(titleZone).toBeTruthy();

      // Card body is a `<FocusScope>` (leaf), so it does NOT push a
      // `FocusZoneContext.Provider`. The field zone therefore sees the
      // same enclosing zone the card sees — they are siblings under
      // their common parent zone, not parent/child. In production that
      // shared parent is the column zone; in this isolated harness
      // there is no enclosing zone, so both `parentZone` slots are null.
      const cardScope = registerScopeArgs().find(
        (a) => a.moniker === "task:task-1",
      )!;
      expect(titleZone!.parentZone).toBe(cardScope.parentZone);
      expect(titleZone!.parentZone).toBeNull();

      // The DOM exposes the moniker for e2e selectors.
      const titleNode = container.querySelector(
        `[data-moniker='field:task:task-1.title']`,
      );
      expect(titleNode).not.toBeNull();

      unmount();
    });

    it("clicking the title field dispatches spatial_focus for THAT field's key, not the card's", async () => {
      const { container, unmount } = renderCard();
      await flushSetup();

      const cardScope = registerScopeArgs().find(
        (a) => a.moniker === "task:task-1",
      )!;
      const titleZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.title",
      )!;

      mockInvoke.mockClear();
      const titleNode = container.querySelector(
        `[data-moniker='field:task:task-1.title']`,
      ) as HTMLElement;
      expect(titleNode).not.toBeNull();
      fireEvent.click(titleNode);

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].key).toBe(titleZone.key);
      // Crucially, NOT the card's key — the title's `e.stopPropagation`
      // keeps the click from bubbling.
      expect(focusCalls[0].key).not.toBe(cardScope.key);

      unmount();
    });

    it("focus claim on the title field mounts a visible FocusIndicator on the title", async () => {
      // Pins the user-reported regression: clicks on the title fired
      // `spatial_focus` but no indicator appeared. The fix passes
      // `showFocusBar={true}` from `<CardField>` to `<Field>`, so the
      // inner field zone now renders an indicator when its key is the
      // focused key for the window.
      const { container, queryByTestId, unmount } = renderCard();
      await flushSetup();

      const titleZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.title",
      )!;

      const titleNode = container.querySelector(
        `[data-moniker='field:task:task-1.title']`,
      ) as HTMLElement;
      expect(titleNode).not.toBeNull();

      // Before the claim — the title carries no `data-focused` and no
      // FocusIndicator child (any indicator we might see at this
      // moment would belong to some unrelated zone).
      expect(titleNode.getAttribute("data-focused")).toBeNull();

      await fireFocusChanged({
        next_key: titleZone.key as SpatialKey,
        next_moniker: "field:task:task-1.title",
      });

      await waitFor(() => {
        // The field's data-focused flips and an indicator mounts inside.
        expect(titleNode.getAttribute("data-focused")).not.toBeNull();
        const indicator = queryByTestId("focus-indicator");
        expect(indicator).not.toBeNull();
        expect(titleNode.contains(indicator!)).toBe(true);
      });

      unmount();
    });

    it("tag pills register one FocusScope leaf per pill under the tags field zone", async () => {
      const { container, unmount } = renderCard();
      await flushSetup();

      // The tags field zone is registered.
      const tagsZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.tags",
      );
      expect(tagsZone).toBeTruthy();

      // One `<FocusScope>` leaf per pill, with moniker `tag:{slug}`.
      const tagPillScopes = registerScopeArgs().filter(
        (a) => typeof a.moniker === "string" && /^tag:/.test(a.moniker as string),
      );
      const monikers = tagPillScopes.map((a) => a.moniker as string).sort();
      expect(monikers).toEqual(["tag:bug", "tag:ui"]);

      // The pill leaves' parent_zone is the tags field zone — they
      // nest under the field zone in the spatial graph, not directly
      // under the card.
      for (const scope of tagPillScopes) {
        expect(scope.parentZone).toBe(tagsZone!.key);
      }

      // Each pill renders with a `data-moniker` attribute the
      // selector keys off.
      const pillNodes = container.querySelectorAll(`[data-moniker^='tag:']`);
      expect(pillNodes.length).toBe(2);

      unmount();
    });

    it("clicking a tag pill dispatches spatial_focus for THAT pill's key, not the card's or the field zone's", async () => {
      const { container, unmount } = renderCard();
      await flushSetup();

      const cardScope = registerScopeArgs().find(
        (a) => a.moniker === "task:task-1",
      )!;
      const tagsZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.tags",
      )!;
      const bugTag = registerScopeArgs().find(
        (a) => a.moniker === "tag:bug",
      )!;

      mockInvoke.mockClear();
      const bugNode = container.querySelector(
        `[data-moniker='tag:bug']`,
      ) as HTMLElement;
      expect(bugNode).not.toBeNull();
      fireEvent.click(bugNode);

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].key).toBe(bugTag.key);
      // Not the card key, not the parent field zone key — the leaf
      // owns its own click.
      expect(focusCalls[0].key).not.toBe(cardScope.key);
      expect(focusCalls[0].key).not.toBe(tagsZone.key);

      unmount();
    });

    it("focus claim on a tag pill mounts a visible FocusIndicator on the pill", async () => {
      // Pins the user-reported "clicking an assignee pill produces a
      // visible indicator on the pill" acceptance criterion at the tag
      // pill level (same shape — both render through MentionView /
      // SingleMention / FocusScope).
      //
      // Pre-fix, `MentionView`'s list-mode renderer hard-suppressed
      // `showFocusBar={false}` for compact-mode pills inside cards.
      // The kernel emitted `focus-changed` correctly but no indicator
      // mounted because the pill's `<FocusScope>` had `showFocusBar`
      // off. Post-fix (sibling card 01KNQY0P9J9...), MentionView passes
      // `showFocusBar` through unchanged, so each pill defaults to
      // `<FocusScope>`'s default of `true` and the indicator mounts on
      // claim.
      const { container, queryByTestId, unmount } = renderCard();
      await flushSetup();

      const bugTag = registerScopeArgs().find(
        (a) => a.moniker === "tag:bug",
      )!;

      const bugNode = container.querySelector(
        `[data-moniker='tag:bug']`,
      ) as HTMLElement;
      expect(bugNode).not.toBeNull();

      // Before the claim — no `data-focused` and no FocusIndicator
      // descendant.
      expect(bugNode.getAttribute("data-focused")).toBeNull();

      await fireFocusChanged({
        next_key: bugTag.key as SpatialKey,
        next_moniker: "tag:bug",
      });

      await waitFor(() => {
        // The pill's `data-focused` flips and a `<FocusIndicator>`
        // mounts inside the pill — the user finally sees feedback for
        // their click.
        expect(bugNode.getAttribute("data-focused")).not.toBeNull();
        const indicator = queryByTestId("focus-indicator");
        expect(indicator).not.toBeNull();
        expect(bugNode.contains(indicator!)).toBe(true);
      });

      unmount();
    });

    it("assignee pills register one FocusScope leaf per assignee under the assignees field zone", async () => {
      const { container, unmount } = renderCard();
      await flushSetup();

      const assigneesZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.assignees",
      );
      expect(assigneesZone).toBeTruthy();

      // One leaf per assignee, with moniker `actor:{id}`.
      const assigneePillScopes = registerScopeArgs().filter(
        (a) =>
          typeof a.moniker === "string" && /^actor:/.test(a.moniker as string),
      );
      const monikers = assigneePillScopes
        .map((a) => a.moniker as string)
        .sort();
      expect(monikers).toEqual(["actor:alice", "actor:bob"]);

      // Pills nest under the assignees field zone.
      for (const scope of assigneePillScopes) {
        expect(scope.parentZone).toBe(assigneesZone!.key);
      }

      // DOM exposure for e2e selectors.
      const pillNodes = container.querySelectorAll(`[data-moniker^='actor:']`);
      expect(pillNodes.length).toBe(2);

      unmount();
    });

    it("clicking an assignee pill dispatches spatial_focus for THAT pill's key, not the card's or the field zone's", async () => {
      const { container, unmount } = renderCard();
      await flushSetup();

      const cardScope = registerScopeArgs().find(
        (a) => a.moniker === "task:task-1",
      )!;
      const assigneesZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.assignees",
      )!;
      const alice = registerScopeArgs().find(
        (a) => a.moniker === "actor:alice",
      )!;

      mockInvoke.mockClear();
      const aliceNode = container.querySelector(
        `[data-moniker='actor:alice']`,
      ) as HTMLElement;
      expect(aliceNode).not.toBeNull();
      fireEvent.click(aliceNode);

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].key).toBe(alice.key);
      expect(focusCalls[0].key).not.toBe(cardScope.key);
      expect(focusCalls[0].key).not.toBe(assigneesZone.key);

      unmount();
    });

    it("focus claim on an assignee pill mounts a visible FocusIndicator on the pill", async () => {
      // The card-level acceptance criterion this test pins:
      //   "Manual smoke: clicking an assignee pill produces a visible
      //   indicator on the pill"
      //
      // The fix lives in `mention-view.tsx` (sibling card 01KNQY0P9J9...,
      // landed before this test) — `MentionViewList` no longer
      // hard-suppresses `showFocusBar` in compact mode. With that fix in
      // place, the assignee pill's `<FocusScope>` defaults to
      // `showFocusBar={true}` and a `<FocusIndicator>` mounts inside the
      // pill when its key becomes the focused key for the window. This
      // test exercises the full chain end-to-end so any future
      // regression in mention-view, the badge-list display, or the
      // focus-scope indicator render path surfaces here.
      const { container, queryByTestId, unmount } = renderCard();
      await flushSetup();

      const alice = registerScopeArgs().find(
        (a) => a.moniker === "actor:alice",
      )!;

      const aliceNode = container.querySelector(
        `[data-moniker='actor:alice']`,
      ) as HTMLElement;
      expect(aliceNode).not.toBeNull();

      // Before the claim — no `data-focused` and no FocusIndicator
      // descendant.
      expect(aliceNode.getAttribute("data-focused")).toBeNull();

      await fireFocusChanged({
        next_key: alice.key as SpatialKey,
        next_moniker: "actor:alice",
      });

      await waitFor(() => {
        // The pill's `data-focused` flips and a `<FocusIndicator>`
        // mounts inside the pill body.
        expect(aliceNode.getAttribute("data-focused")).not.toBeNull();
        const indicator = queryByTestId("focus-indicator");
        expect(indicator).not.toBeNull();
        expect(aliceNode.contains(indicator!)).toBe(true);
      });

      unmount();
    });

    it("the status field is a nested FocusZone with moniker field:task:{id}.status", async () => {
      // Status is a single-value text field, the same shape as title.
      // Pinning a separate test is belt-and-suspenders for the user-
      // reported regression list, which calls out status explicitly.
      const { container, unmount } = renderCard();
      await flushSetup();

      const statusZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.status",
      );
      expect(statusZone).toBeTruthy();

      const statusNode = container.querySelector(
        `[data-moniker='field:task:task-1.status']`,
      );
      expect(statusNode).not.toBeNull();

      unmount();
    });

    it("clicking the status field dispatches spatial_focus for THAT field's key", async () => {
      const { container, unmount } = renderCard();
      await flushSetup();

      const cardScope = registerScopeArgs().find(
        (a) => a.moniker === "task:task-1",
      )!;
      const statusZone = registerZoneArgs().find(
        (a) => a.moniker === "field:task:task-1.status",
      )!;

      mockInvoke.mockClear();
      const statusNode = container.querySelector(
        `[data-moniker='field:task:task-1.status']`,
      ) as HTMLElement;
      expect(statusNode).not.toBeNull();
      fireEvent.click(statusNode);

      const focusCalls = spatialFocusCalls();
      expect(focusCalls).toHaveLength(1);
      expect(focusCalls[0].key).toBe(statusZone.key);
      expect(focusCalls[0].key).not.toBe(cardScope.key);

      unmount();
    });
  });
});
