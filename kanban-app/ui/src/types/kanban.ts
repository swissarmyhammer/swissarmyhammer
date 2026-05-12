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

/**
 * A saved perspective defining view, filter, sort, group, and field layout.
 *
 * `view` (kind) vs `view_id` (instance) compatibility rule:
 *
 * When `view_id` is set, the perspective belongs to exactly that view
 * instance. When `view_id` is absent (legacy-shared), the perspective
 * appears in every view whose kind matches `view`. Newly created
 * perspectives always set `view_id`; legacy `view_id`-less perspectives
 * keep working unchanged until re-saved.
 */
export interface PerspectiveDef {
  readonly id: string;
  readonly name: string;
  /** Legacy: view kind ("board", "grid"). Retained for backwards compat. */
  readonly view: string;
  /**
   * Id of the specific view instance this perspective is scoped to.
   * Optional — when absent, the perspective is shared across all views
   * whose kind matches `view`. See the type-level compatibility rule above.
   */
  readonly view_id?: string;
  readonly fields?: readonly PerspectiveFieldEntry[];
  /** Filter DSL expression (e.g. `#bug && @will`). Evaluated server-side. */
  readonly filter?: string;
  readonly group?: string;
  readonly sort?: readonly PerspectiveSortEntry[];
}

// ---------------------------------------------------------------------------
// Command schema (YAML-shape mirror)
//
// Mirrors `swissarmyhammer_commands::CommandDef` and its supporting types
// from `swissarmyhammer-commands/src/types.rs`. This is the *YAML-shape*
// CommandDef — the JSON wire format the backend emits from
// `commands_for_scope` — NOT the *runtime* CommandDef from
// `lib/command-scope.tsx`, which carries an `execute` closure and is
// resolved through the frontend scope tree.
//
// All new fields (`tab_button`, `params[].shape`, `params[].options_from`,
// `params[].options`) are optional so existing payloads parse unchanged.
// ---------------------------------------------------------------------------

/**
 * Tab-button affordance metadata mirrored from Rust's `TabButtonDef`.
 *
 * Present means the command renders as a tab-button on surfaces that
 * consume `tab_button`-tagged commands (today: the perspective tab bar);
 * absent means no tab-button affordance — the command still surfaces in
 * palettes / menus per its other metadata.
 */
export interface TabButtonDef {
  /**
   * Lucide-react icon component name (e.g. `"filter"`, `"group"`,
   * `"arrow-up-down"`). Resolved by the frontend's icon registry at
   * render time; an unknown name renders a fallback glyph.
   */
  readonly icon: string;
}

/**
 * Shape of a parameter for runtime collection — mirrors Rust's
 * `ParamShape` enum (kebab-cased on the wire).
 *
 * `shape` answers "how should the UI ask the user for this value?". When
 * absent on a `ParamDef`, the param's `from` field already supplies the
 * value and the frontend does not render a picker for it.
 */
export type ParamShape =
  | "enum"
  | "text"
  | "expression"
  | "number"
  | "date"
  | "boolean";

/**
 * Source of a parameter value — mirrors Rust's `ParamSource` enum
 * (snake_cased on the wire). Distinct from `shape` (the picker UX): a
 * param can come `from: "args"` and still carry a `shape` so the UI
 * knows how to collect it before dispatch.
 */
export type ParamSource = "scope_chain" | "target" | "args" | "default";

/**
 * A single option value for an enum-shaped param — mirrors Rust's
 * `ParamOption`. Used as an inline alternative to a backend resolver
 * when the option list is static and known at YAML write time.
 */
export interface ParamOption {
  /** Machine-readable value that flows into the command's args bag. */
  readonly value: string;
  /** Human-readable label shown in the picker UI. */
  readonly label: string;
}

/** A parameter definition for a command — mirrors Rust's `ParamDef`. */
export interface ParamDef {
  readonly name: string;
  readonly from?: ParamSource;
  readonly entity_type?: string;
  /** Default value when nothing else supplies it (`serde_json::Value`). */
  readonly default?: unknown;
  /**
   * Shape of this param for runtime collection. When absent, the param's
   * `from` field already supplies the value — the runtime never asks the
   * user for it.
   */
  readonly shape?: ParamShape;
  /**
   * For enum-shaped params, names the backend resolver that supplies
   * the concrete option list at `commands_for_scope` emission time.
   * Resolver names are stringly-typed and looked up in a backend
   * resolver registry.
   */
  readonly options_from?: string;
  /**
   * Inline option list for enum-shaped params whose values are static
   * and known at YAML write time. When both `options_from` and
   * `options` are present, the resolver wins; treat inline `options`
   * as a fallback. Also populated by the backend on emit when
   * `options_from` resolved successfully.
   */
  readonly options?: readonly ParamOption[];
}

/**
 * YAML-loaded command metadata — mirrors Rust's `CommandDef` from
 * `swissarmyhammer-commands`.
 *
 * Carries the command's identity, scope requirements, optional UI-surface
 * metadata (menu placement, tab-button affordance), and parameter
 * declarations. Emitted to the frontend by `commands_for_scope`; consumed
 * by `<CommandButton>`, `<CommandPopover>`, palettes, and native menus.
 *
 * This is distinct from the runtime `CommandDef` in
 * `lib/command-scope.tsx`, which is the scope-tree node that carries an
 * `execute` closure. The frontend converts wire-format `CommandDef` ->
 * runtime `CommandDef` at the dispatcher boundary.
 */
export interface CommandDef {
  readonly id: string;
  readonly name: string;
  readonly menu_name?: string;
  readonly scope?: string;
  readonly visible?: boolean;
  readonly keys?: ViewCommandKeys;
  readonly params?: readonly ParamDef[];
  readonly undoable?: boolean;
  readonly context_menu?: boolean;
  readonly context_menu_group?: number;
  readonly context_menu_order?: number;
  readonly view_kinds?: readonly string[];
  /**
   * When set, this command renders as a tab-button affordance on
   * surfaces that consume `tab_button`-tagged commands (today: the
   * perspective tab bar). Absent means no tab-button affordance.
   */
  readonly tab_button?: TabButtonDef;
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
  /**
   * Muted hint text rendered by displays when the field value is empty.
   * When set, empty-state display renderers (currently `badge` and
   * `badge-list`) render this string in place of the hardcoded `-` /
   * `None` fallback. Since the click-to-edit surface only mounts the
   * display (not the editor) at rest, this is the only cue the user
   * sees for what to add into an empty field.
   */
  placeholder?: string;
  sort?: string;
  filter?: string;
  group?: string;
  /** Whether this field can be used as a group-by target in perspectives. */
  groupable?: boolean;
  validate?: string;
}

/**
 * Declarative inspector/card section from the YAML schema.
 *
 * Sections partition an entity's fields into ordered groups separated by
 * dividers. Each section may optionally carry a `label` (rendered above the
 * section in the inspector) and an `on_card` flag (opts the section into the
 * card view beneath the header section).
 */
export interface SectionDef {
  id: string;
  label?: string;
  on_card?: boolean;
}

/** Schema definition for an entity type (from YAML). */
export interface EntityDef {
  name: string;
  icon?: string;
  body_field?: string;
  fields: string[];
  /**
   * Ordered inspector sections. When present, the inspector and card render
   * fields grouped by the declared sections (with dividers and optional
   * labels). When omitted or empty, renderers fall back to the implicit
   * `header`/`body`/`footer` three-section layout.
   */
  sections?: readonly SectionDef[];
  mention_prefix?: string;
  mention_display_field?: string;
  /**
   * Raw entity field that supplies the mention slug verbatim (no slugify).
   * When set, the frontend uses this field — typically `id` — as the
   * mention slug everywhere: CM6 decorations, tooltips, autocomplete,
   * pills, and reference field badges. See `MentionableType.slugField`.
   */
  mention_slug_field?: string;
  search_display_field?: string;
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
// Virtual tag metadata — served by the backend VirtualTagRegistry
// ---------------------------------------------------------------------------

/** Metadata for a virtual tag (READY, BLOCKED, BLOCKING) from the backend. */
export interface VirtualTagMeta {
  slug: string;
  color: string;
  description: string;
}

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

/** Response shape from get_board_data command. */
export interface BoardDataResponse {
  board: EntityBag;
  columns: EntityBag[];
  tags: EntityBag[];
  virtual_tag_meta?: VirtualTagMeta[];
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
  virtualTagMeta: VirtualTagMeta[];
  summary: BoardSummary;
}

/** Convert a BoardDataResponse into the entity-based BoardData. */
export function parseBoardData(data: BoardDataResponse): BoardData {
  return {
    board: entityFromBag(data.board),
    columns: data.columns.map(entityFromBag),
    tags: data.tags.map(entityFromBag),
    virtualTagMeta: data.virtual_tag_meta ?? [],
    summary: data.summary,
  };
}
