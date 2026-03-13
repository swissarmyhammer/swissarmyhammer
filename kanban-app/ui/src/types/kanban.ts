// ---------------------------------------------------------------------------
// View definitions
// ---------------------------------------------------------------------------

export interface ViewCommandKeys {
  readonly vim?: string;
  readonly cua?: string;
  readonly emacs?: string;
}

export interface ViewCommand {
  readonly id: string;
  readonly name: string;
  readonly description?: string;
  readonly keys?: ViewCommandKeys;
}

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
// Board types
// ---------------------------------------------------------------------------

export interface OpenBoard {
  path: string;
  is_active: boolean;
}

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

export interface FieldDef {
  id: string;
  name: string;
  description?: string;
  type: FieldType;
  default?: string;
  editor?: string;
  display?: string;
  /** Where to render in the inspector layout: "header" | "body" | "footer" | "hidden". Default: "body". */
  section?: string;
  sort?: string;
  filter?: string;
  group?: string;
  validate?: string;
}

export interface EntityDef {
  name: string;
  body_field?: string;
  fields: string[];
  mention_prefix?: string;
  mention_display_field?: string;
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
export function getBool(entity: Entity, field: string, fallback = false): boolean {
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

/** Raw entity bag from Entity::to_json() — flat JSON with entity_type + id + fields. */
export type EntityBag = Record<string, unknown> & { entity_type: string; id: string };

/** Convert a flat entity bag from the backend into an Entity. */
export function entityFromBag(bag: EntityBag): Entity {
  const { entity_type, id, ...fields } = bag;
  return { entity_type, id, fields };
}

// ---------------------------------------------------------------------------
// Board summary — aggregate counts returned by get_board_data
// ---------------------------------------------------------------------------

export interface BoardSummary {
  total_tasks: number;
  total_actors: number;
  ready_tasks: number;
  blocked_tasks: number;
}

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

/** Response shape from get_board_data command. */
export interface BoardDataResponse {
  board: EntityBag;
  columns: EntityBag[];
  swimlanes: EntityBag[];
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
  swimlanes: Entity[];
  tags: Entity[];
  summary: BoardSummary;
}

/** Convert a BoardDataResponse into the entity-based BoardData. */
export function parseBoardData(data: BoardDataResponse): BoardData {
  return {
    board: entityFromBag(data.board),
    columns: data.columns.map(entityFromBag),
    swimlanes: data.swimlanes.map(entityFromBag),
    tags: data.tags.map(entityFromBag),
    summary: data.summary,
  };
}
