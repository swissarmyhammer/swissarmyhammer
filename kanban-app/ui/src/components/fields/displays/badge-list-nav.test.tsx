/**
 * Tests for badge-list pill navigation after the spatial-nav zone
 * migration.
 *
 * Two layers of coverage:
 *
 * 1. **Structural** (no spatial provider stack) — pins the per-pill
 *    `<FocusScope>` shape that the spatial graph relies on. Each pill
 *    renders with `data-moniker = "${entityType}:${id|slug}"` nested
 *    under the parent field-row scope. These tests run against the
 *    `<FocusScope>` fallback branch (no `<FocusLayer>` ancestor) — they
 *    do not exercise `spatial_register_*` IPCs, only the React-side
 *    DOM shape.
 *
 * 2. **Click → indicator chain** (production-shaped spatial provider
 *    stack) — pins the user-visible affordance the parent card
 *    `01KNQY0P9J03T24FSM8AVPFPZ9` reopened on: clicking a pill must
 *    dispatch `spatial_focus` for that pill's `SpatialKey`, and when
 *    the kernel echoes a matching `focus-changed` event the per-pill
 *    `useFocusClaim` subscription mounts a visible `<FocusIndicator>`.
 *    The earlier `MentionView` revision hard-suppressed `showFocusBar`
 *    in `mode="compact"`; this block exercises both modes so any
 *    re-introduction of that suppression surfaces here.
 *
 * Cross-zone navigation (beam-search rule 2 — moving from a pill in
 * one row to a pill in another) lives in the Rust spatial-nav unit
 * tests, not here.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import type { ReactNode } from "react";
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
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  })),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

const mockTags = [
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
  {
    id: "tag-3",
    entity_type: "tag",
    moniker: "tag:tag-3",
    fields: { tag_name: "docs", color: "0000ff" },
  },
];

// Task store is empty — reference-field pills in RefNavHarness miss the
// store and fall back to the raw entity id, which is exactly what these
// tests need to verify (moniker = buildMoniker(entityType, rawId)).
const mockEntitiesByType: Record<string, unknown[]> = {
  tag: mockTags,
  task: [],
};

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: (type: string) => mockEntitiesByType[type] ?? [],
    getEntity: vi.fn(),
  }),
  // Passthrough provider — the test provides its own EntityFocusProvider,
  // but MentionView's upstream imports may transitively reference this.
  EntityStoreProvider: ({ children }: { children: ReactNode }) => children,
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    mentionableTypes: [
      { entityType: "tag", prefix: "#", displayField: "tag_name" },
      { entityType: "task", prefix: "^", displayField: "title" },
    ],
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
  }),
}));

// window-container's useBoardData is referenced by cm mention extension
// hooks; MentionView itself doesn't use it, but a transitive import
// drags it in. Stub it so the module loads without a real Tauri engine.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => null,
}));

// ---------------------------------------------------------------------------

import { BadgeListDisplay } from "./badge-list-display";
import { FocusScope } from "@/components/focus-scope";
import { FocusZone } from "@/components/focus-zone";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  asLayerName,
  asMoniker,
  type FocusChangedPayload,
  type SpatialKey,
  type WindowLabel,
} from "@/types/spatial";

import type { Entity, FieldDef } from "@/types/kanban";

const tagField: FieldDef = {
  name: "tags",
  display: "badge-list",
  type: { entity: "tag", commit_display_names: true },
} as unknown as FieldDef;

const refField: FieldDef = {
  name: "depends_on",
  display: "badge-list",
  type: { entity: "task" },
} as unknown as FieldDef;

const taskEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: { tags: ["bugfix", "feature", "docs"] },
};

const refEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: { depends_on: ["task-dep-A", "task-dep-B"] },
};

/** Flush microtasks and pending effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

/** Wait two ticks so mount-time spatial-register effects flush. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
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

// ---------------------------------------------------------------------------
// Structural harnesses (no spatial provider stack)
// ---------------------------------------------------------------------------

/**
 * Full provider tree with a parent FocusScope (simulating a field row)
 * containing BadgeListDisplay. The parent is `` to mirror the
 * real `FieldRow`, which registers as a navigable zone in the spatial-nav
 * graph.
 */
function NavHarness({
  values,
  parentMoniker,
}: {
  values: string[];
  parentMoniker: string;
}) {
  return (
    <EntityFocusProvider>
      <TooltipProvider>
        <FocusScope
          moniker={asMoniker(parentMoniker)}

          commands={[]}
        >
          <BadgeListDisplay
            field={tagField}
            value={values}
            entity={taskEntity}
            mode="full"
          />
        </FocusScope>
      </TooltipProvider>
    </EntityFocusProvider>
  );
}

/**
 * Harness for reference-field navigation (entity ID values, no slug resolution).
 * The parent is `` to mirror the real `FieldRow`.
 */
function RefNavHarness({
  values,
  parentMoniker,
}: {
  values: string[];
  parentMoniker: string;
}) {
  return (
    <EntityFocusProvider>
      <TooltipProvider>
        <FocusScope
          moniker={asMoniker(parentMoniker)}

          commands={[]}
        >
          <BadgeListDisplay
            field={refField}
            value={values}
            entity={refEntity}
            mode="full"
          />
        </FocusScope>
      </TooltipProvider>
    </EntityFocusProvider>
  );
}

// ---------------------------------------------------------------------------
// Spatial-stack harness (production-shaped)
// ---------------------------------------------------------------------------

/**
 * Render `BadgeListDisplay` inside the production-shaped spatial-nav
 * stack so each pill's `<FocusScope>` registers via
 * `spatial_register_scope` and subscribes to per-key claims. The parent
 * `<FocusZone>` mirrors the real `<Field>` zone that wraps the display
 * in production. `mode` is parameterised so tests can pin both compact
 * (card body) and full (inspector row) variants.
 */
function SpatialHarness({
  values,
  parentMoniker,
  mode,
}: {
  values: string[];
  parentMoniker: string;
  mode: "compact" | "full";
}) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asLayerName("window")}>
        <EntityFocusProvider>
          <TooltipProvider>
            <FocusZone moniker={asMoniker(parentMoniker)}>
              <BadgeListDisplay
                field={tagField}
                value={values}
                entity={taskEntity}
                mode={mode}
              />
            </FocusZone>
          </TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

// ---------------------------------------------------------------------------
// Tests — structural
// ---------------------------------------------------------------------------

describe("BadgeListDisplay pill scope structure", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("each tag pill renders as a FocusScope nested inside the parent field row", async () => {
    const { container } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    // Parent field row scope
    const fieldRow = container.querySelector('[data-moniker="field:tags"]');
    expect(fieldRow).toBeTruthy();

    // Each pill is its own FocusScope nested inside the field row.
    const pills = fieldRow!.querySelectorAll(
      '[data-moniker^="tag:"]',
    ) as NodeListOf<HTMLElement>;
    expect(pills.length).toBe(3);
    const monikers = Array.from(pills).map((p) =>
      p.getAttribute("data-moniker"),
    );
    expect(monikers).toEqual(["tag:tag-1", "tag:tag-2", "tag:tag-3"]);
  });

  it("renders the expected scope tree: parent field row plus one scope per pill", async () => {
    // Smoke check: rendering the harness produces the parent field-row
    // scope and exactly N pill scopes nested under it. This is the
    // structural shape the spatial navigator relies on (zone + leaves).
    const { container } = render(
      <NavHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
      />,
    );
    await flush();

    expect(container.querySelectorAll("[data-moniker]").length).toBe(4); // field row + 3 pills
  });
});

describe("BadgeListDisplay reference-field pill structure", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("monikers use buildMoniker(entityType, entityId) directly — no slug resolution", async () => {
    const { container } = render(
      <RefNavHarness
        values={["task-dep-A", "task-dep-B"]}
        parentMoniker="field:depends_on"
      />,
    );
    await flush();

    // The parent field row scope is present.
    const fieldRow = container.querySelector(
      '[data-moniker="field:depends_on"]',
    );
    expect(fieldRow).toBeTruthy();

    // Two pills, each carrying the raw task id as its moniker tail.
    const pills = fieldRow!.querySelectorAll(
      '[data-moniker^="task:"]',
    ) as NodeListOf<HTMLElement>;
    expect(pills.length).toBe(2);
    const monikers = Array.from(pills).map((p) =>
      p.getAttribute("data-moniker"),
    );
    expect(monikers).toEqual(["task:task-dep-A", "task:task-dep-B"]);
  });
});

// ---------------------------------------------------------------------------
// Tests — click → spatial_focus → focus-changed → <FocusIndicator>
// ---------------------------------------------------------------------------

describe("BadgeListDisplay pill click → visible focus indicator", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(
      async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
    );
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("clicking a pill in mode=full dispatches spatial_focus for THAT pill's leaf key", async () => {
    const { container } = render(
      <SpatialHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
        mode="full"
      />,
    );
    await flushSetup();

    const fieldZone = registerZoneArgs().find(
      (a) => a.moniker === "field:tags",
    );
    expect(fieldZone).toBeTruthy();

    const bugPill = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-1",
    );
    expect(bugPill).toBeTruthy();

    mockInvoke.mockClear();
    const pillNode = container.querySelector(
      "[data-moniker='tag:tag-1']",
    ) as HTMLElement | null;
    expect(pillNode).not.toBeNull();
    fireEvent.click(pillNode!);

    const focusCalls = spatialFocusCalls();
    // The leaf's click handler stops propagation, so only one focus
    // call fires — for the leaf's key, not the parent zone's.
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].key).toBe(bugPill!.key);
    expect(focusCalls[0].key).not.toBe(fieldZone!.key);
  });

  it("focus claim mounts <FocusIndicator> on a pill in mode=full", async () => {
    const { container } = render(
      <SpatialHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
        mode="full"
      />,
    );
    await flushSetup();

    const bugPill = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-1",
    )!;
    const pillNode = container.querySelector(
      "[data-moniker='tag:tag-1']",
    ) as HTMLElement;
    expect(pillNode).not.toBeNull();
    // Before the focus claim, no indicator descendant.
    expect(
      pillNode.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();

    await fireFocusChanged({
      next_key: bugPill.key as SpatialKey,
      next_moniker: "tag:tag-1",
    });

    expect(pillNode.getAttribute("data-focused")).toBe("true");
    expect(
      pillNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();
  });

  it("focus claim mounts <FocusIndicator> on a pill in mode=compact (regression: MentionView used to hard-suppress this)", async () => {
    // Pre-fix, `MentionView` set `showFocusBar={false}` for every pill
    // when `mode === "compact"`. That broke the entity-card flow:
    // clicking an assignee or tag pill on a card body produced no
    // visible focus feedback. The fix: stop overriding showFocusBar
    // based on mode — pills default to showing the bar in both modes,
    // and explicit `showFocusBar={false}` from a caller still wins.
    const { container } = render(
      <SpatialHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
        mode="compact"
      />,
    );
    await flushSetup();

    const bugPill = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-1",
    )!;
    const pillNode = container.querySelector(
      "[data-moniker='tag:tag-1']",
    ) as HTMLElement;
    expect(pillNode).not.toBeNull();
    expect(
      pillNode.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();

    await fireFocusChanged({
      next_key: bugPill.key as SpatialKey,
      next_moniker: "tag:tag-1",
    });

    expect(pillNode.getAttribute("data-focused")).toBe("true");
    expect(
      pillNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();
  });

  it("indicator follows focus from one pill to a sibling — only one bar visible at a time", async () => {
    // Drive the focus-changed event from pill A to pill B and verify
    // pill A's indicator unmounts as pill B's mounts. This pins the
    // per-key claim subscription's cross-talk: the previous claim
    // listener fires `false`, the new listener fires `true`, and the
    // visible bar tracks both atomically.
    const { container } = render(
      <SpatialHarness
        values={["bugfix", "feature", "docs"]}
        parentMoniker="field:tags"
        mode="compact"
      />,
    );
    await flushSetup();

    const pillA = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-1",
    )!;
    const pillB = registerScopeArgs().find(
      (a) => a.moniker === "tag:tag-2",
    )!;

    const aNode = container.querySelector(
      "[data-moniker='tag:tag-1']",
    ) as HTMLElement;
    const bNode = container.querySelector(
      "[data-moniker='tag:tag-2']",
    ) as HTMLElement;

    // Step 1: focus on pill A.
    await fireFocusChanged({
      next_key: pillA.key as SpatialKey,
      next_moniker: "tag:tag-1",
    });
    expect(
      aNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();
    expect(
      bNode.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();

    // Step 2: focus moves to pill B. A's indicator must unmount; B's
    // must mount.
    await fireFocusChanged({
      prev_key: pillA.key as SpatialKey,
      next_key: pillB.key as SpatialKey,
      next_moniker: "tag:tag-2",
    });
    expect(
      aNode.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();
    expect(
      bNode.querySelector("[data-testid='focus-indicator']"),
    ).not.toBeNull();
  });
});
