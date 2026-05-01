/**
 * End-to-end test fixture — a 3×3 board with 2 perspectives.
 *
 * Used by `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` to
 * boot the full `<App/>` against realistic data. The fixture is shaped to
 * match the wire format of the Tauri commands that `RustEngineContainer`
 * fires on mount (`get_board_data`, `list_entities`, `list_open_boards`,
 * `list_views`, `perspective.list`, `get_ui_state`,
 * `list_entity_types`, `get_entity_schema`) so the production
 * provider stack hydrates without stubbing any context internals.
 *
 * # Shape
 *
 *   - 1 board (`board:E2E`, "End-to-End Test Board", percent_complete=50).
 *   - 3 columns (`column:TODO`, `column:DOING`, `column:DONE`).
 *   - 9 task cards (T1–T3 in TODO, D1–D3 in DOING, N1–N3 in DONE) — each
 *     with `position_column` and `position_ordinal` so the BoardView's
 *     column virtualizer can place them correctly.
 *   - 2 perspectives (`perspective:default` (active) and
 *     `perspective:secondary`), both `view: "board"`.
 *   - 1 view (`board-1`, kind "board") so `useViews()` resolves to a
 *     usable active view without auto-creating one.
 *   - Schemas for `task`, `column`, `board` with a minimal field set
 *     (`task.title`, `task.status`, `column.name`, `board.percent_complete`).
 *
 * # Why this shape
 *
 * Family 1 (click → focus indicator) needs at least 5 distinct focusable
 * leaf types: a card, a column body whitespace, a perspective tab, a
 * nav-bar button, and a field row in the inspector. Three columns with
 * three cards each give the navigation-family tests a deterministic
 * cross-zone landscape: T1 is in the leftmost column, N3 is in the
 * rightmost, and right/left/up/down all have somewhere to go without
 * relying on an empty column edge case.
 *
 * Two perspectives let Family 6 dblclick the active tab and confirm the
 * inactive tab does NOT mount a rename editor. One perspective with
 * `id: "default"` is marked active via `get_ui_state`'s
 * `windows.main.active_perspective_id`.
 *
 * # Pinned moniker shapes
 *
 * The fixture freezes the moniker shapes the test asserts against:
 *
 *   - Cards: `task:T1`, `task:T2`, `task:T3`, `task:D1`, …
 *   - Columns: `column:TODO`, `column:DOING`, `column:DONE`
 *   - Board: `board:E2E`
 *   - Perspective tabs: `perspective_tab:default`, `perspective_tab:secondary`
 *   - Active perspective scope: `perspective:default`
 *
 * If the production code ever changes how it derives any of these, the
 * fixture's column/card maps must be updated in lockstep.
 */

import type {
  BoardDataResponse,
  EntityBag,
  EntityListResponse,
  EntitySchema,
  PerspectiveDef,
  ViewDef,
} from "@/types/kanban";

// ---------------------------------------------------------------------------
// Identifiers and lookup tables
// ---------------------------------------------------------------------------

/** Pinned board id used across the fixture and test assertions. */
export const E2E_BOARD_ID = "E2E";

/** Pinned board moniker (used by Family 8's registry-shape audit). */
export const E2E_BOARD_MONIKER = `board:${E2E_BOARD_ID}`;

/** Pinned board path — the value the test seeds into `<App/>` via URL. */
export const E2E_BOARD_PATH = "/test/end-to-end-board";

/** Pinned board name — Family 1 asserts the inspector renders the title. */
export const E2E_BOARD_NAME = "End-to-End Test Board";

/** Column id → display name. Order matters: defines the strip's left → right layout. */
export const E2E_COLUMNS = [
  { id: "TODO", name: "Todo", order: 0 },
  { id: "DOING", name: "Doing", order: 1 },
  { id: "DONE", name: "Done", order: 2 },
] as const;

/**
 * Task fixtures — three per column, indexed 1..3 inside each column so
 * each task's column membership can be derived from the leading letter
 * (`T*` → TODO, `D*` → DOING, `N*` → DONE).
 */
export const E2E_TASKS = [
  { id: "T1", column: "TODO", title: "First Todo Task" },
  { id: "T2", column: "TODO", title: "Second Todo Task" },
  { id: "T3", column: "TODO", title: "Third Todo Task" },
  { id: "D1", column: "DOING", title: "First Doing Task" },
  { id: "D2", column: "DOING", title: "Second Doing Task" },
  { id: "D3", column: "DOING", title: "Third Doing Task" },
  { id: "N1", column: "DONE", title: "First Done Task" },
  { id: "N2", column: "DONE", title: "Second Done Task" },
  { id: "N3", column: "DONE", title: "Third Done Task" },
] as const;

/**
 * Map: task id → column id. Lets the test cross-check the column a
 * focused card lives in after a navigation gesture without parsing the
 * moniker each time.
 */
export const E2E_TASK_COLUMN_BY_ID = new Map<string, string>(
  E2E_TASKS.map((t) => [t.id, t.column]),
);

/**
 * Return the column id a moniker resolves to.
 *
 * Recognizes two moniker shapes:
 *
 *   - `task:<id>` — the task's column via [`E2E_TASK_COLUMN_BY_ID`].
 *   - `column:<id>` — the column id directly (e.g. `column:TODO` →
 *     `TODO`).
 *
 * Under the unified-cascade kernel from
 * `01KQ7S6WHK9RCCG2R4FN474EFD`, cross-column horizontal nav now lands
 * focus on the column-zone moniker rather than a card-leaf inside the
 * destination column. Tests that assert "the focused element's column
 * identity is colX" must accept both shapes — `task:T1` and
 * `column:TODO` are both valid focused monikers tied to column TODO.
 *
 * Returns `null` when the moniker matches neither shape.
 */
export function columnOfTaskMoniker(moniker: string): string | null {
  // Accept either the bare segment shape (`task:T1`, `column:TODO`) or
  // the path-monikers FQM shape (`/window/.../task:T1`,
  // `/window/.../column:TODO`). The FQM's trailing segment after the
  // last `/` is the legacy moniker.
  const segment = moniker.includes("/")
    ? moniker.slice(moniker.lastIndexOf("/") + 1)
    : moniker;
  const taskMatch = /^task:([0-9A-Za-z]+)$/.exec(segment);
  if (taskMatch) return E2E_TASK_COLUMN_BY_ID.get(taskMatch[1]) ?? null;
  const columnMatch = /^column:([0-9A-Za-z]+)$/.exec(segment);
  if (columnMatch) return columnMatch[1];
  return null;
}

// ---------------------------------------------------------------------------
// Entity bag builders
// ---------------------------------------------------------------------------

/**
 * Build a flat `EntityBag` (the shape `Entity::to_json()` emits) from a
 * task fixture row. The test expects every card to register with
 * `data-moniker="task:<id>"`, so the moniker field carries that prefix.
 */
function makeTaskBag(t: { id: string; column: string; title: string }, ord: number): EntityBag {
  return {
    entity_type: "task",
    id: t.id,
    moniker: `task:${t.id}`,
    title: t.title,
    status: "todo",
    position_column: t.column,
    // Lexicographic ordinals so the column body sorts the cards 1, 2, 3.
    position_ordinal: `a${ord}`,
  };
}

/** Build a column entity bag. */
function makeColumnBag(c: { id: string; name: string; order: number }): EntityBag {
  return {
    entity_type: "column",
    id: c.id,
    moniker: `column:${c.id}`,
    name: c.name,
    order: c.order,
  };
}

/** Build the board entity bag. */
function makeBoardBag(): EntityBag {
  return {
    entity_type: "board",
    id: E2E_BOARD_ID,
    moniker: E2E_BOARD_MONIKER,
    name: E2E_BOARD_NAME,
    percent_complete: 50,
  };
}

// ---------------------------------------------------------------------------
// Bootstrap-command response builders — what `<App/>` sees on mount
// ---------------------------------------------------------------------------

/** Response shape for `get_board_data` — matches `BoardDataResponse`. */
export function getBoardDataResponse(): BoardDataResponse {
  return {
    board: makeBoardBag(),
    columns: E2E_COLUMNS.map(makeColumnBag),
    tags: [],
    virtual_tag_meta: [],
    summary: {
      total_tasks: E2E_TASKS.length,
      total_actors: 0,
      ready_tasks: E2E_TASKS.length,
      blocked_tasks: 0,
      done_tasks: 0,
      percent_complete: 50,
    },
  };
}

/** Response for `list_entities` keyed by entity type. */
export function listEntitiesResponse(entityType: string): EntityListResponse {
  if (entityType === "task") {
    const entities = E2E_TASKS.map((t, i) => makeTaskBag(t, i));
    return { entities, count: entities.length };
  }
  // Actors, projects, and any other type are empty in this fixture.
  return { entities: [], count: 0 };
}

/** Response for `list_open_boards`. */
export function listOpenBoardsResponse() {
  return [{ path: E2E_BOARD_PATH, is_active: true, name: E2E_BOARD_NAME }];
}

/** Response for `list_views` — one board view, the active one. */
export const E2E_VIEWS: ViewDef[] = [
  {
    id: "board-1",
    name: "Board",
    kind: "board",
    icon: "kanban",
    card_fields: ["title", "status"],
    commands: [],
  },
];

export function listViewsResponse(): ViewDef[] {
  return E2E_VIEWS;
}

/** Two perspectives — the first is active. */
export const E2E_PERSPECTIVES: PerspectiveDef[] = [
  {
    id: "default",
    name: "Default",
    view: "board",
    fields: [],
    sort: [],
  },
  {
    id: "secondary",
    name: "Secondary",
    view: "board",
    fields: [],
    sort: [],
  },
];

/**
 * Response for `dispatch_command(perspective.list)` — wraps the list in
 * the `{ result, undoable }` envelope the dispatcher returns.
 */
export function perspectiveListDispatchResponse() {
  return {
    result: {
      perspectives: E2E_PERSPECTIVES,
      count: E2E_PERSPECTIVES.length,
    },
    undoable: false,
  };
}

/**
 * Response for `get_ui_state`. The `windows.main` entry pins the board
 * path, active view, and active perspective so the App skips its
 * "fall through to refresh()" auto-select branch and renders the
 * fixture immediately.
 */
export function getUIStateResponse() {
  return {
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [E2E_BOARD_PATH],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {
      main: {
        board_path: E2E_BOARD_PATH,
        inspector_stack: [],
        active_view_id: "board-1",
        active_perspective_id: "default",
        palette_open: false,
        palette_mode: "command",
        app_mode: "normal",
      },
    },
    recent_boards: [],
  };
}

/** Response for `get_undo_state`. */
export function getUndoStateResponse() {
  return { can_undo: false, can_redo: false };
}

// ---------------------------------------------------------------------------
// Schema responses — task / column / board with a minimal field set
// ---------------------------------------------------------------------------

/** `list_entity_types` response. */
export function listEntityTypesResponse(): string[] {
  return ["task", "column", "board"];
}

/**
 * Map: entity type → `EntitySchema`. The schemas declare exactly the
 * fields the test cares about so renderers (`Field`, `BoardSummary`,
 * `Inspector`) have something to read without exploding on missing
 * defs.
 */
export const E2E_SCHEMAS: Record<string, EntitySchema> = {
  task: {
    entity: { name: "task", fields: ["title", "status"] },
    fields: [
      {
        id: "title",
        name: "title",
        type: { kind: "string" },
        editor: "text",
        display: "text",
        section: "header",
      },
      {
        id: "status",
        name: "status",
        type: { kind: "string" },
        editor: "select",
        display: "badge",
        section: "body",
      },
    ],
  },
  column: {
    entity: { name: "column", fields: ["name"] },
    fields: [
      {
        id: "name",
        name: "name",
        type: { kind: "string" },
        editor: "text",
        display: "text",
        section: "header",
      },
    ],
  },
  board: {
    entity: { name: "board", fields: ["name", "percent_complete"] },
    fields: [
      {
        id: "name",
        name: "name",
        type: { kind: "string" },
        editor: "text",
        display: "text",
        section: "header",
      },
      {
        id: "percent_complete",
        name: "percent_complete",
        type: { kind: "number" },
        editor: "number",
        display: "percent",
        section: "header",
      },
    ],
  },
};

/** Schema response for a given entity type. */
export function getEntitySchemaResponse(entityType: string): EntitySchema | null {
  return E2E_SCHEMAS[entityType] ?? null;
}
