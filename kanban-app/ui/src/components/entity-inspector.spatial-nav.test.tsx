/**
 * Spatial-nav integration tests for `<EntityInspector>` — per-leaf focus
 * indicator behaviour.
 *
 * After cards `01KQ5QB6F4MTD35GBTARJH4JEW` (Field-as-zone) and
 * `01KNQXYC4...` (architecture fix) landed, the inspector's structural
 * shape is:
 *
 *   ↳ `<FocusZone moniker="field:{type}:{id}.{name}">`   (per row, owned by `<Field>`)
 *       ↳ display content — single-value: bare; badge-list: one
 *         `<FocusScope moniker="{type}:{id}">` per pill leaf
 *
 * The user-reported regression for this card is that *clicking* a leaf
 * (label icon, inline display, badge-list pill) does not show a visible
 * focus indicator even though structural wrapping is correct. These
 * tests pin the click → `spatial_focus` → `focus-changed` →
 * `useFocusClaim` → `<FocusIndicator>` chain end-to-end so a regression
 * anywhere in the pipeline surfaces here.
 *
 * The mocking pattern follows `entity-card.spatial.test.tsx` and
 * `inspectors-container.spatial-nav.test.tsx`:
 *   - `vi.hoisted` builds an invoke / listen mock pair the test owns.
 *   - The Tauri mocks capture `spatial_register_zone` /
 *     `spatial_register_scope` payloads so we know which `SpatialKey`
 *     each zone and pill owns.
 *   - The `listen("focus-changed", cb)` mock records the React-side
 *     handler so `fireFocusChanged(key)` can simulate the kernel
 *     emitting a focus event back to the renderer.
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
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider, CommandScopeProvider } from "@/lib/command-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  asLayerName,
  type FocusChangedPayload,
  type SpatialKey,
  type WindowLabel,
} from "@/types/spatial";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds
// ---------------------------------------------------------------------------

/**
 * Inspector schema covering the three leaf shapes the card description
 * enumerates: a label-iconed single-value editable row (`title`), a
 * computed display-only icon row (`progress`), and a badge-list row with
 * pill leaves (`tags`).
 */
const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "tags", "progress", "body"],
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
      name: "progress",
      type: { kind: "computed", derive: "parse-body-progress" },
      editor: "none",
      display: "progress",
      icon: "bar-chart",
      section: "header",
    },
    {
      id: "f4",
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
  return undefined;
}

function makeTask(fields: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
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

/** Wait two ticks so mount-time effects flush before assertions. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one for the current window. The renderer responds
 * by flipping the matching `useFocusClaim` subscription to `true`,
 * which mounts the visible `<FocusIndicator>` inside the focused
 * primitive's `<div>`.
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

/**
 * Render the inspector wrapped in the production-shaped spatial-nav stack.
 *
 * Mirrors the provider tree `App.tsx` mounts: `<SpatialFocusProvider>`
 * + `<FocusLayer>` so each `<Field>`'s `<FocusZone>` and each pill's
 * `<FocusScope>` register via `spatial_register_zone` /
 * `spatial_register_scope`; `<EntityFocusProvider>` so the entity-focus
 * scope registry and `setFocus` chrome work; the schema / store /
 * field-update / UI-state / command-scope providers because the
 * schema-driven field dispatch reads from all five.
 */
function renderInspector(entity: Entity = makeTask({ title: "Hello", tags: ["bug", "ui"] }), tagEntities: Entity[] = makeTags()) {
  return render(
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <TooltipProvider delayDuration={100}>
            <SchemaProvider>
              <EntityStoreProvider
                entities={{ task: [entity], tag: tagEntities }}
              >
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <CommandScopeProvider commands={[]}>
                        <EntityInspector entity={entity} />
                      </CommandScopeProvider>
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

describe("EntityInspector — spatial-nav per-leaf focus indicator", () => {
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
  // Click → spatial_focus dispatch
  //
  // The first half of the chain: clicking the leaf must invoke
  // `spatial_focus` with that leaf's `SpatialKey`. The Rust kernel echoes
  // back a `focus-changed` event in production; here we drive that
  // synthetically below in the second half of each test.
  // -------------------------------------------------------------------------

  it("clicking a single-value field row dispatches spatial_focus for THAT field's zone key", async () => {
    const { container, unmount } = renderInspector();
    await flushSetup();

    const titleZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:task-1.title",
    );
    expect(titleZone).toBeTruthy();

    mockInvoke.mockClear();
    const titleNode = container.querySelector(
      `[data-moniker='field:task:task-1.title']`,
    ) as HTMLElement | null;
    expect(titleNode).not.toBeNull();
    fireEvent.click(titleNode!);

    const focusCalls = spatialFocusCalls();
    expect(focusCalls.length).toBeGreaterThanOrEqual(1);
    expect(focusCalls[0].key).toBe(titleZone!.key);

    unmount();
  });

  it("clicking a badge-list pill dispatches spatial_focus for THAT pill's leaf key, not the row zone's", async () => {
    const { container, unmount } = renderInspector();
    await flushSetup();

    const tagsZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:task-1.tags",
    );
    expect(tagsZone).toBeTruthy();

    const bugPill = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-bug",
    );
    expect(bugPill).toBeTruthy();

    mockInvoke.mockClear();
    const pillNode = container.querySelector(
      `[data-moniker='tag:tag-bug']`,
    ) as HTMLElement | null;
    expect(pillNode).not.toBeNull();
    fireEvent.click(pillNode!);

    const focusCalls = spatialFocusCalls();
    // The leaf's click handler stops propagation so the parent zone
    // does not also fire a focus call.
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].key).toBe(bugPill!.key);
    expect(focusCalls[0].key).not.toBe(tagsZone!.key);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Focus claim → visible <FocusIndicator>
  //
  // The second half of the chain: when the kernel echoes a
  // `focus-changed` event with the leaf's `SpatialKey` as the new
  // focused key, the `useFocusClaim` subscription on the matching
  // primitive flips `data-focused` and mounts a `<FocusIndicator>`
  // child. This is the user-visible affordance that was missing per the
  // card's reopen status.
  // -------------------------------------------------------------------------

  it("focus claim mounts <FocusIndicator> inside a single-value field row's zone", async () => {
    const { container, unmount } = renderInspector();
    await flushSetup();

    const titleZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:task-1.title",
    )!;
    const titleNode = container.querySelector(
      `[data-moniker='field:task:task-1.title']`,
    ) as HTMLElement;

    // Before the focus claim: no indicator descendant attributable to
    // this row, and `data-focused` is unset. (The mount-time
    // first-field-focus effect targets the *entity* focus store, not
    // the spatial claim registry — the visible bar tracks
    // spatial-focus-claim only.)
    expect(
      titleNode.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();

    await fireFocusChanged({
      next_key: titleZone.key as SpatialKey,
      next_moniker: "field:task:task-1.title",
    });

    // The row's data-focused flips to "true" and a FocusIndicator
    // mounts as a descendant.
    expect(titleNode.getAttribute("data-focused")).toBe("true");
    expect(
      titleNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();

    unmount();
  });

  it("focus claim mounts <FocusIndicator> inside a computed display-only field row's zone", async () => {
    // The progress field uses `editor: "none"` and `display: "progress"` —
    // the inline display is not click-to-edit, but the row IS a focusable
    // zone and clicking it must show the focus bar. Pin the chain for
    // this leaf shape too.
    const { container, unmount } = renderInspector(
      makeTask({
        title: "Hello",
        tags: ["bug"],
        progress: { total: 4, completed: 2, percent: 50 },
      }),
    );
    await flushSetup();

    const progressZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:task-1.progress",
    )!;
    const progressNode = container.querySelector(
      `[data-moniker='field:task:task-1.progress']`,
    ) as HTMLElement;
    expect(progressNode).not.toBeNull();
    expect(
      progressNode.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();

    await fireFocusChanged({
      next_key: progressZone.key as SpatialKey,
      next_moniker: "field:task:task-1.progress",
    });

    expect(progressNode.getAttribute("data-focused")).toBe("true");
    expect(
      progressNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();

    unmount();
  });

  it("focus claim mounts <FocusIndicator> inside a badge-list pill leaf", async () => {
    // The leaf-shape regression the card explicitly calls out: a tag
    // pill in the inspector must show the focus bar when its key
    // becomes the focused key. Before this card landed, MentionView
    // hard-suppressed `showFocusBar` for compact-mode pills; the
    // inspector renders pills in `mode="full"` so it was unaffected,
    // but the entity-card's `mode="compact"` path is what was broken
    // for users. We pin the inspector path here and rely on the
    // entity-card spatial test for the compact-mode counterpart.
    const { container, unmount } = renderInspector();
    await flushSetup();

    const bugPill = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-bug",
    )!;
    const pillNode = container.querySelector(
      `[data-moniker='tag:tag-bug']`,
    ) as HTMLElement;
    expect(pillNode).not.toBeNull();
    expect(
      pillNode.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();

    await fireFocusChanged({
      next_key: bugPill.key as SpatialKey,
      next_moniker: "tag:tag-bug",
    });

    expect(pillNode.getAttribute("data-focused")).toBe("true");
    expect(
      pillNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Drill-out
  //
  // Pressing Escape from a pill in production lands focus on the
  // enclosing field-row zone. From the React side that just means the
  // kernel emits `focus-changed(prev=pill_key, next=field_zone_key)` —
  // the indicator follows. We exercise the React side; the kernel's
  // decision to route Escape to the field zone lives in the Rust crate.
  // -------------------------------------------------------------------------

  it("drilling out from a pill to its field row moves the visible indicator with the focus", async () => {
    const { container, unmount } = renderInspector();
    await flushSetup();

    const tagsZone = registerZoneArgs().find(
      (a) => a.moniker === "field:task:task-1.tags",
    )!;
    const bugPill = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-bug",
    )!;

    const pillNode = container.querySelector(
      `[data-moniker='tag:tag-bug']`,
    ) as HTMLElement;
    const tagsNode = container.querySelector(
      `[data-moniker='field:task:task-1.tags']`,
    ) as HTMLElement;

    // Step 1: focus lands on the pill — indicator is on the pill, not
    // the field row.
    await fireFocusChanged({
      next_key: bugPill.key as SpatialKey,
      next_moniker: "tag:tag-bug",
    });
    expect(
      pillNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();
    expect(tagsNode.getAttribute("data-focused")).toBeNull();

    // Step 2: drill-out to the field-row zone — the indicator follows.
    await fireFocusChanged({
      prev_key: bugPill.key as SpatialKey,
      next_key: tagsZone.key as SpatialKey,
      next_moniker: "field:task:task-1.tags",
    });
    expect(tagsNode.getAttribute("data-focused")).toBe("true");
    expect(
      tagsNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();

    unmount();
  });
});
