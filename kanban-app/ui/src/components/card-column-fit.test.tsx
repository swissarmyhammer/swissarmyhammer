/**
 * Card-vs-column width regression test.
 *
 * A `ColumnView` renders a `SortableTaskCard` (→ `EntityCard`) inside a
 * column wrapper that clamps the column between `min-w-[24em]` and
 * `max-w-[48em]`. Cards must always size to the column — even when a task
 * carries a long unbreakable string (URL, slug, identifier without
 * whitespace), the card must not produce horizontal overflow inside itself.
 *
 * The scenario this guards: without a clean `min-w-0` chain from the
 * column down through the card, long unbreakable content forces the card
 * wider than its column, making `card.scrollWidth > card.clientWidth` and
 * in the worst case pushing the column strip past the board's scroll
 * container. We verify the card body's own `scrollWidth <= clientWidth`
 * so no intrinsic-width leak is present.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";

import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { UIStateProvider } from "@/lib/ui-state-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment } from "@/types/spatial";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — ColumnView dispatches commands and reads schemas.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "body"],
    commands: [],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f2",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
};

const COLUMN_SCHEMA = {
  entity: {
    name: "column",
    body_field: null,
    fields: ["name"],
    commands: [],
  },
  fields: [
    {
      id: "f1",
      name: "name",
      type: { kind: "text", single_line: true },
      editor: "text",
      display: "text",
      section: "header",
    },
  ],
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types")
    return Promise.resolve(["task", "column"]);
  if (args[0] === "get_entity_schema") {
    const name = (args[1] as Record<string, unknown>)?.entityType;
    if (name === "task") return Promise.resolve(TASK_SCHEMA);
    if (name === "column") return Promise.resolve(COLUMN_SCHEMA);
    return Promise.resolve(null);
  }
  if (args[0] === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  if (args[0] === "list_commands_for_scope") return Promise.resolve([]);
  if (args[0] === "show_context_menu") return Promise.resolve();
  return Promise.resolve("ok");
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
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
// Test-only Tailwind shim — same pattern used by `app-layout.test.tsx`.
// We shim the column-width utilities (min-w-[24em], max-w-[48em], shrink-0)
// so the column clamps to its real minimum width in the browser harness,
// along with the flex/min-w/overflow utilities that build the min-w-0 chain.
// ---------------------------------------------------------------------------

const CARD_FIT_SHIM = `
.flex { display: flex; }
.flex-col { flex-direction: column; }
.flex-1 { flex: 1 1 0%; }
.min-h-0 { min-height: 0; }
.min-w-0 { min-width: 0; }
.overflow-hidden { overflow: hidden; }
.overflow-x-auto { overflow-x: auto; }
.overflow-y-auto { overflow-y: auto; }
.break-words { overflow-wrap: break-word; }
.items-start { align-items: flex-start; }
.gap-1\\.5 { gap: 0.375rem; }
.gap-2 { gap: 0.5rem; }
.px-3 { padding-left: 0.75rem; padding-right: 0.75rem; }
.py-2 { padding-top: 0.5rem; padding-bottom: 0.5rem; }
.rounded-md { border-radius: 0.375rem; }
.text-sm { font-size: 0.875rem; }
.truncate { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.block { display: block; }
.space-y-0\\.5 > * + * { margin-top: 0.125rem; }
.min-w-\\[24em\\] { min-width: 24em; }
.max-w-\\[48em\\] { max-width: 48em; }
.shrink-0 { flex-shrink: 0; }
`;

function installShim() {
  let style = document.getElementById(
    "card-column-fit-shim",
  ) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement("style");
    style.id = "card-column-fit-shim";
    style.textContent = CARD_FIT_SHIM;
    document.head.appendChild(style);
  }
}

// ---------------------------------------------------------------------------
// Fixture helpers — post-mock imports.
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { ColumnView } from "./column-view";

/** A 60-character URL with no spaces or punctuation that would break naturally. */
const LONG_URL =
  "https://example.com/verylongpathsegmentwithoutbreaks/xyzAbc123";

/** Build a column entity used as the test fixture. */
function makeColumn(): Entity {
  return {
    id: "col-1",
    entity_type: "column",
    moniker: "column:col-1",
    fields: { name: "Todo", order: 0 },
  };
}

/** Build a task entity whose title is the long unbreakable URL. */
function makeTaskWithLongTitle(): Entity {
  return {
    id: "task-long",
    entity_type: "task",
    moniker: "task:task-long",
    fields: {
      title: LONG_URL,
      body: "",
      position_column: "col-1",
      position_ordinal: "a0",
    },
  };
}

/**
 * Render a `ColumnView` inside a 24em-wide host (the column's declared min
 * width). The host's width is what forces the content-fit constraint:
 * without a clean `min-w-0` chain, the card would push wider than 24em.
 */
function renderColumnWithCard(column: Entity, task: Entity) {
  installShim();

  const host = document.createElement("div");
  // 24em at default 16px/em = 384px — the column's declared minimum.
  host.style.width = "24em";
  host.style.height = "500px";
  host.style.display = "flex";
  host.style.flexDirection = "column";
  host.style.overflow = "hidden";
  host.setAttribute("data-card-fit-host", "");
  document.body.appendChild(host);

  const result = render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: [task], column: [column] }}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/card-fit">
                  <DragSessionProvider>
                    <UIStateProvider>
                      <FieldUpdateProvider>
                        <ColumnView column={column} tasks={[task]} />
                      </FieldUpdateProvider>
                    </UIStateProvider>
                  </DragSessionProvider>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
    { container: host },
  );

  return { ...result, host };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Card-column fit — cards never exceed their column", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    document
      .querySelectorAll("[data-card-fit-host]")
      .forEach((el) => el.remove());
  });

  it("a card with a 60-char unbreakable URL title has scrollWidth <= clientWidth", async () => {
    const column = makeColumn();
    const task = makeTaskWithLongTitle();
    const { host } = renderColumnWithCard(column, task);

    // Schema + field registrations load asynchronously; wait a tick.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    const card = host.querySelector<HTMLElement>(
      `[data-entity-card="${task.id}"]`,
    );
    expect(card).toBeTruthy();

    // The card's visual bounds must never exceed its column width. Any
    // intrinsic-width leak from the long URL would push scrollWidth past
    // the visible clientWidth.
    expect(card!.scrollWidth).toBeLessThanOrEqual(card!.clientWidth);

    host.remove();
  });

  it("a card with a long URL fits within the column's bounding rect", async () => {
    const column = makeColumn();
    const task = makeTaskWithLongTitle();
    const { host } = renderColumnWithCard(column, task);

    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    const columnEl = host.querySelector<HTMLElement>(
      `[data-segment="column:${column.id}"]`,
    );
    expect(columnEl).toBeTruthy();

    const card = host.querySelector<HTMLElement>(
      `[data-entity-card="${task.id}"]`,
    );
    expect(card).toBeTruthy();

    const columnWidth = columnEl!.getBoundingClientRect().width;
    const cardWidth = card!.getBoundingClientRect().width;
    expect(cardWidth).toBeLessThanOrEqual(columnWidth);

    host.remove();
  });
});
