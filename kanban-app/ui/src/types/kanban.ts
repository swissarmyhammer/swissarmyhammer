// ---------------------------------------------------------------------------
// Entity command definitions
// ---------------------------------------------------------------------------

/** Keymap bindings for an entity command, keyed by input mode. */
export interface EntityCommandKeys {
  readonly vim?: string;
  readonly cua?: string;
  readonly emacs?: string;
}

/** A command defined on an entity type in the YAML schema. */
export interface EntityCommand {
  readonly id: string;
  readonly name: string;
  readonly context_menu?: boolean;
  readonly keys?: EntityCommandKeys;
}

// ---------------------------------------------------------------------------
// View definitions
// ---------------------------------------------------------------------------

/** Keymap bindings for a view command, keyed by input mode. */
export interface ViewCommandKeys {
  readonly vim?: string;
  readonly cua?: string;
  readonly emacs?: string;
}

/** A command defined on a view in the YAML schema. */
export interface ViewCommand {
  readonly id: string;
  readonly name: string;
  readonly description?: string;
  readonly keys?: ViewCommandKeys;
}

/** Definition of a UI view (board, grid, etc.) from the YAML schema. */
export interface ViewDef {
  readonly id: string;
  readonly name: string;
  readonly icon?: string;
  readonly kind: string;
  readonly entity_type?: string;
  readonly card_fields?: readonly string[];
  readonly commands?: readonly ViewCommand[];
}

// ---------------------------------------------------------------------------
// Perspective definitions
// ---------------------------------------------------------------------------

/** A field column entry within a perspective's field list. */
export interface PerspectiveFieldEntry {
  readonly field: string;
  readonly caption?: string;
  readonly width?: number;
  readonly editor?: string;
  readonly display?: string;
  readonly sort_comparator?: string;
}

/** A sort entry within a perspective — field name + direction. */
export interface PerspectiveSortEntry {
  readonly field: string;
  readonly direction: "asc" | "desc";
}

/** A saved perspective defining view, filter, sort, group, and field layout. */
export interface PerspectiveDef {
  readonly id: string;
  readonly name: string;
  readonly view: string;
  readonly fields?: readonly PerspectiveFieldEntry[];
  /** Filter DSL expression (e.g. `#bug && @will`). Evaluated server-side. */
  readonly filter?: string;
  readonly group?: string;
  readonly sort?: readonly PerspectiveSortEntry[];
}

// ---------------------------------------------------------------------------
// Board types
// ---------------------------------------------------------------------------

/** A currently open board, as returned by the backend. */
export interface OpenBoard {
  path: string;
  is_active: boolean;
  name: string;
}

/** A recently opened board from the MRU list. */
export interface RecentBoard {
  path: string;
  name: string;
  last_opened: string;
}

// ---------------------------------------------------------------------------
// Field & Entity schema types
//
// These are intentionally open — kind, editor, display, and sort are plain
// strings so new field types can be added via YAML without touching TS.
// Type-specific properties live in the FieldType bag keyed by convention
// (e.g. "options" for select, "derive" for computed, "entity" for reference).
// ---------------------------------------------------------------------------

/** An option value for select-type fields. */
export interface SelectOption {
  value: string;
  label?: string;
  color?: string;
  icon?: string;
  order: number;
}

/** Field type descriptor — `kind` discriminates, extra keys vary by kind. */
export interface FieldType {
  kind: string;
  [key: string]: unknown;
}

/** Schema definition for a single entity field (from YAML). */
export interface FieldDef {
  id: string;
  name: string;
  description?: string;
  type: FieldType;
  default?: string;
  editor?: string;
  display?: string;
  /** Lucide icon name for display in the inspector (e.g. "file-text", "users", "tag"). */
  icon?: string;
  /** Where to render in the inspector layout: "header" | "body" | "footer" | "hidden". Default: "body". */
  section?: string;
  sort?: string;
  filter?: string;
  group?: string;
  /** Whether this field can be used as a group-by target in perspectives. */
  groupable?: boolean;
  validate?: string;
}

/** Schema definition for an entity type (from YAML). */
export interface EntityDef {
  name: string;
  icon?: string;
  body_field?: string;
  fields: string[];
  mention_prefix?: string;
  mention_display_field?: string;
  search_display_field?: string;
  commands?: readonly EntityCommand[];
}

/** Schema response from get_entity_schema IPC command. */
export interface EntitySchema {
  entity: EntityDef;
  fields: FieldDef[];
}

/** A generic dynamic entity — entity_type + id + arbitrary field values. */
export interface Entity {
  entity_type: string;
  id: string;
  /** Canonical moniker from the backend (e.g. "task:01ABC" or "task:01ABC:archive"). */
  moniker: string;
  fields: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Safe field accessors — mirror Rust's entity.get_str() / get_string_list()
// ---------------------------------------------------------------------------

/** Read a string field, returning fallback if missing/null/wrong type. */
export function getStr(entity: Entity, field: string, fallback = ""): string {
  const v = entity.fields[field];
  return typeof v === "string" ? v : fallback;
}

/** Read a string array field, returning [] if missing/null/wrong type. */
export function getStrList(entity: Entity, field: string): string[] {
  const v = entity.fields[field];
  return Array.isArray(v) ? (v as string[]) : [];
}

/** Read a numeric field, returning fallback if missing/null/wrong type. */
export function getNum(entity: Entity, field: string, fallback = 0): number {
  const v = entity.fields[field];
  return typeof v === "number" ? v : fallback;
}

/** Read a boolean field, returning fallback if missing/null/wrong type. */
export function getBool(
  entity: Entity,
  field: string,
  fallback = false,
): boolean {
  const v = entity.fields[field];
  return typeof v === "boolean" ? v : fallback;
}

// ---------------------------------------------------------------------------
// Entity bag conversion
//
// Entity::to_json() on the Rust side produces a flat JSON object with
// entity_type, id, and all field values at the top level. These helpers
// convert between that flat format and the Entity interface.
// ---------------------------------------------------------------------------

/** Raw entity bag from Entity::to_json() — flat JSON with entity_type + id + moniker + fields. */
export type EntityBag = Record<string, unknown> & {
  entity_type: string;
  id: string;
  moniker: string;
};

/** Convert a flat entity bag from the backend into an Entity. */
export function entityFromBag(bag: EntityBag): Entity {
  const { entity_type, id, moniker, ...fields } = bag;
  return { entity_type, id, moniker, fields };
}

// ---------------------------------------------------------------------------
// Board summary — aggregate counts returned by get_board_data
// ---------------------------------------------------------------------------

/** Aggregate counts for a board — tasks by status, actor totals, etc. */
export interface BoardSummary {
  total_tasks: number;
  total_actors: number;
  ready_tasks: number;
  blocked_tasks: number;
  done_tasks: number;
  percent_complete: number;
}

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

/** Response shape from get_board_data command. */
export interface BoardDataResponse {
  board: EntityBag;
  columns: EntityBag[];
  tags: EntityBag[];
  summary: BoardSummary;
}

/** Response shape from list_entities command. */
export interface EntityListResponse {
  entities: EntityBag[];
  count: number;
}

// ---------------------------------------------------------------------------
// BoardData — entity-based board state used by UI components
// ---------------------------------------------------------------------------

/** Entity-based board data. All sub-collections are Entity arrays. */
export interface BoardData {
  board: Entity;
  columns: Entity[];
  tags: Entity[];
  summary: BoardSummary;
}

/** Convert a BoardDataResponse into the entity-based BoardData. */
export function parseBoardData(data: BoardDataResponse): BoardData {
  return {
    board: entityFromBag(data.board),
    columns: data.columns.map(entityFromBag),
    tags: data.tags.map(entityFromBag),
    summary: data.summary,
  };
}
