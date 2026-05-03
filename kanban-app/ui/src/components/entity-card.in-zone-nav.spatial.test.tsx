/**
 * Browser-mode wiring test for the **any-kind iter-0 sibling rule**
 * inside an `<EntityCard>`.
 *
 * # Scope: wiring, not algorithm
 *
 * The kernel algorithm (any-kind iter-0 cascade, in-beam Android
 * scoring, drill-out fallback) is exercised by Rust integration tests
 * in `swissarmyhammer-focus/tests/in_zone_any_kind_first.rs` against a
 * synthetic card-shaped fixture. This file pins the **React-side
 * wiring**: that the card's children register with the shapes the
 * kernel needs to resolve the trajectory `card.inspect:{id} → title
 * field zone` for ArrowLeft, and the React tree updates `data-focused`
 * on the right node when the kernel emits `focus-changed`.
 *
 * The test runs through the shared spatial-nav harness at
 * `kanban-app/ui/src/test/spatial-shadow-registry.ts`, which captures
 * the production `spatial_register_*` calls into a JS shadow registry
 * and routes `spatial_navigate(key, direction)` through the kernel's
 * JS port. The port mirrors `BeamNavStrategy::next` exactly — including
 * the any-kind iter-0 rule — so a regression in either the React-side
 * registration shape or the JS-side cascade port surfaces here.
 *
 * Mock pattern matches `entity-card.spatial.test.tsx`'s ancestors that
 * use the shared harness (the `vi.hoisted` factory + `setupSpatialHarness`).
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — file-scoped, forwarding to spies owned by the
// shared spatial-nav harness module.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen } = await vi.hoisted(async () => {
  const helper = await import("@/test/spatial-shadow-registry");
  return {
    mockInvoke: helper.mockInvoke,
    mockListen: helper.mockListen,
  };
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
import {
  setupSpatialHarness,
  type SpatialHarness,
} from "@/test/spatial-shadow-registry";
import { EntityCard } from "./entity-card";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import {
  asSegment,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Schema + fixture
// ---------------------------------------------------------------------------

/**
 * Task schema with a single visible header field — `title`. Keeping the
 * schema minimal makes the in-card sibling layout deterministic: the
 * card carries `[title field zone, inspect leaf]` with no other field
 * rows competing for vertical or horizontal candidates. (The drag
 * handle button renders in the row but is intentionally NOT a
 * `<FocusScope>` — see `entity-card.tsx::DragHandle` — so it is invisible
 * to spatial nav.)
 */
const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    body_field: "body",
    fields: ["title", "body"],
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
      id: "f-body",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
} as unknown as EntitySchema;

/** Default invoke responses for the AppShell-driven harness. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    return entityType === "task" ? TASK_SCHEMA : TASK_SCHEMA;
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
  if (cmd === "show_context_menu") return undefined;
  return undefined;
}

/** Build a single task entity with a non-empty title so the field zone has content. */
function makeTask(): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
    fields: {
      title: "Hello",
      body: "",
      depends_on: [],
      position_column: "col-1",
      position_ordinal: "a0",
    },
  };
}

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 80));
  });
}

/**
 * Substitute CSS for the card layout — the browser test bundle does
 * NOT load Tailwind, so the card's `flex items-start gap-2` chrome
 * collapses into a vertical block stack and the inner row
 * (`[drag-handle] [title] [inspect]`) loses its horizontal layout.
 *
 * Cardinal navigation is *defined* by registered rects, so without
 * horizontal layout `ArrowLeft` from the inspect leaf has no
 * horizontally-aligned candidate. Inject the small handful of rules
 * that produce the production row shape — narrow enough not to
 * affect any other layout.
 */
const TEST_CARD_CSS = `
  .flex { display: flex; }
  .flex-1 { flex: 1 1 0%; min-width: 0; }
  .items-start { align-items: flex-start; }
  .gap-2 { gap: 0.5rem; }
  .shrink-0 { flex-shrink: 0; }
  .min-w-0 { min-width: 0; }
  .relative { position: relative; }
  /* The card body itself needs a non-zero width so its children have
     room to lay out horizontally. */
  [data-entity-card] { width: 400px; }
`;

/** Inject the card-layout CSS into the document head exactly once. */
function ensureTestCardCss(): void {
  if (document.querySelector("style[data-test-card-layout]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-card-layout", "");
  style.textContent = TEST_CARD_CSS;
  document.head.appendChild(style);
}

/**
 * Pull the most recent register record for a moniker from the
 * captured invoke calls. Prefers zone over scope when both exist
 * (deduping idempotent re-registers). Returns `null` if not found.
 */
function findRegisterRecord(
  moniker: string,
): { kind: "zone" | "scope"; record: Record<string, unknown> } | null {
  for (let i = mockInvoke.mock.calls.length - 1; i >= 0; i--) {
    const c = mockInvoke.mock.calls[i];
    const cmd = c[0];
    if (cmd === "spatial_register_zone" || cmd === "spatial_register_scope") {
      const r = c[1] as Record<string, unknown>;
      if (r && r.segment === moniker) {
        return {
          kind: cmd === "spatial_register_zone" ? "zone" : "scope",
          record: r,
        };
      }
    }
  }
  return null;
}

/**
 * Render the card inside the production-shaped spatial-nav stack
 * wrapped by `<AppShell>` so the global keybinding pipeline is live.
 *
 * The outer `<div>` enforces a wide-enough viewport that the card's
 * inner row (`[drag-handle] [title] [inspect]`) lays out horizontally
 * and the field rows stack vertically beneath. Without an enforced
 * width the card collapses to a single column and the cardinal-nav
 * candidates have no horizontal offsets to pick between.
 */
function renderCardWithShell() {
  ensureTestCardCss();
  return render(
    <div
      style={{
        width: "1000px",
        height: "600px",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <UIStateProvider>
              <AppModeProvider>
                <UndoProvider>
                  <SchemaProvider>
                    <EntityStoreProvider entities={{ task: [makeTask()] }}>
                      <TooltipProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <FieldUpdateProvider>
                            <AppShell>
                              <EntityCard entity={makeTask()} />
                            </AppShell>
                          </FieldUpdateProvider>
                        </ActiveBoardPathProvider>
                      </TooltipProvider>
                    </EntityStoreProvider>
                  </SchemaProvider>
                </UndoProvider>
              </AppModeProvider>
            </UIStateProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </div>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("EntityCard — in-zone any-kind sibling navigation", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    harness = setupSpatialHarness({ defaultInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  /**
   * `ArrowLeft` from the inspect leaf inside the card lands on the
   * title field zone — the in-zone sibling immediately to its left.
   *
   * Pre-fix kernel behaviour: same-kind iter 0 filtered out the field
   * zone for a leaf-origin search and the cascade fell through to the
   * next horizontally-aligned leaf (no in-zone field-zone peer was
   * considered). Post-fix: any-kind iter 0 considers the field zone
   * as a peer and picks it.
   *
   * Note: the card's drag-handle button is intentionally NOT a
   * `<FocusScope>` (drag-and-drop is mouse-only — see
   * `entity-card.tsx::DragHandle` and `board-view.tsx::useSensor(PointerSensor, …)`),
   * so the inspect leaf's only in-zone horizontal peer is the title
   * field zone. The assertion below pins both halves: ArrowLeft lands
   * on the title zone, and the drag handle is absent from the
   * spatial-nav DOM altogether.
   *
   * The Rust regression for the algorithm itself lives at
   * `swissarmyhammer-focus/tests/in_zone_any_kind_first.rs`. This
   * test pins the wiring half: the React tree updates `data-focused`
   * on the title field zone after the kernel emits `focus-changed`.
   */
  it("ArrowLeft from card.inspect:{id} lands on the title field zone", async () => {
    const { container, unmount } = renderCardWithShell();
    await flushSetup();

    // Capture the registered FQMs so the focus-changed event we seed
    // matches the kernel's actual moniker for the inspect leaf.
    const inspectLeaf = findRegisterRecord("card.inspect:task-1");
    expect(
      inspectLeaf,
      "card.inspect:{id} must register as a leaf scope",
    ).toBeTruthy();
    expect(inspectLeaf!.kind).toBe("scope");
    const inspectFq = inspectLeaf!.record.fq as FullyQualifiedMoniker;

    const titleZone = findRegisterRecord("field:task:task-1.title");
    expect(
      titleZone,
      "field:task:task-1.title must register as a zone",
    ).toBeTruthy();
    expect(titleZone!.kind).toBe("zone");
    const titleFq = titleZone!.record.fq as FullyQualifiedMoniker;

    // Seed focus on the inspect leaf so `nav.left`'s execute closure
    // sees its FQM as the focused key.
    await harness.fireFocusChanged({
      next_fq: inspectFq,
      next_segment: asSegment("card.inspect:task-1"),
    });
    await flushSetup();

    // ArrowLeft is the cua binding for `nav.left`. The keymap pipeline
    // routes it to `spatial_navigate(focused, "left")`. The shadow
    // navigator runs the kernel logic and emits `focus-changed` with
    // the resulting FQM.
    await userEvent.keyboard("{ArrowLeft}");
    await flushSetup();

    // The title field zone's DOM node must carry `data-focused` — the
    // kernel landed there and the React tree updated.
    const titleNode = container.querySelector(
      `[data-segment='field:task:task-1.title']`,
    ) as HTMLElement | null;
    expect(titleNode, "title field zone must be in the DOM").not.toBeNull();
    expect(
      titleNode!.getAttribute("data-focused"),
      `ArrowLeft from card.inspect:task-1 must land on the title field zone \
       (in-zone sibling under the new any-kind iter-0 rule). The kernel's \
       returned FQM should be ${String(titleFq)}.`,
    ).not.toBeNull();

    // Belt-and-braces: the drag handle is intentionally NOT a
    // `<FocusScope>` (mouse-only affordance — see
    // `entity-card.tsx::DragHandle`), so it must not appear in the
    // spatial-nav DOM at all. If a future change re-introduced the
    // FocusScope, this assertion would fail and the in-zone candidate
    // set for ArrowLeft would change underneath this test.
    const dragNode = container.querySelector(
      `[data-segment='card.drag-handle:task-1']`,
    ) as HTMLElement | null;
    expect(
      dragNode,
      "the drag handle must not register a spatial-nav segment — " +
        "it is mouse-only and intentionally excluded from the focus graph",
    ).toBeNull();

    unmount();
  });

  /**
   * `ArrowDown` from the inspect leaf inside the card lands on a
   * field zone in the SAME card — NOT outside the card.
   *
   * The card has only one rendered `on_card` section (`header`)
   * holding the title field. Pressing Down from the inspect leaf
   * should land on the title field zone (geometrically below the
   * inspect leaf's row in the card layout), not escape the card.
   *
   * In the simplified single-field card used by this test the title
   * field zone occupies the body of the card; `Down` from the
   * inspect leaf at the top-right finds the title zone as the
   * closest in-zone Down peer. The wider point: focus stays inside
   * the card, never escalating to a peer of the card itself.
   */
  it("ArrowDown from card.inspect:{id} stays inside the same card", async () => {
    const { container, unmount } = renderCardWithShell();
    await flushSetup();

    const inspectLeaf = findRegisterRecord("card.inspect:task-1");
    expect(inspectLeaf).toBeTruthy();
    const inspectFq = inspectLeaf!.record.fq as FullyQualifiedMoniker;

    const cardZone = findRegisterRecord("task:task-1");
    expect(cardZone, "task:{id} must register as the card zone").toBeTruthy();
    const cardFq = cardZone!.record.fq as FullyQualifiedMoniker;
    const cardFqStr = String(cardFq);

    // Seed focus on the inspect leaf.
    await harness.fireFocusChanged({
      next_fq: inspectFq,
      next_segment: asSegment("card.inspect:task-1"),
    });
    await flushSetup();

    await userEvent.keyboard("{ArrowDown}");
    await flushSetup();

    // The post-nav focused element's FQM (read via `data-fq` if
    // present, else inferred from the focused `data-segment` node)
    // must be a path-descendant of the card zone — focus stayed in
    // the same card. We assert this via DOM containment: the focused
    // node lives inside the card body's element subtree.
    const cardNode = container.querySelector(
      `[data-segment='task:task-1']`,
    ) as HTMLElement | null;
    expect(cardNode, "card zone must be in the DOM").not.toBeNull();

    const focused = container.querySelector(
      "[data-focused='true'][data-segment]",
    ) as HTMLElement | null;
    expect(focused, "ArrowDown must select something").not.toBeNull();
    expect(
      cardNode!.contains(focused!) || focused === cardNode,
      `ArrowDown from card.inspect:task-1 must stay inside the card \
       (FQM path-descendant of ${cardFqStr}); landed on \
       ${focused!.getAttribute("data-segment")}`,
    ).toBe(true);

    unmount();
  });
});
