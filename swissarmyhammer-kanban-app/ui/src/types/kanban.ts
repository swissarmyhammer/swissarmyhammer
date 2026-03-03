export interface Column {
  id: string;
  name: string;
  order: number;
  task_count?: number;
  ready_count?: number;
}

export interface Swimlane {
  id: string;
  name: string;
  order: number;
  task_count?: number;
}

export interface Tag {
  id: string;
  name: string;
  description?: string;
  color: string;
  task_count?: number;
}

export interface BoardSummary {
  total_tasks: number;
  total_actors: number;
  ready_tasks: number;
  blocked_tasks: number;
}

export interface Board {
  name: string;
  description?: string;
  columns: Column[];
  swimlanes: Swimlane[];
  tags: Tag[];
  summary?: BoardSummary;
}

export interface Position {
  column: string;
  swimlane?: string;
  ordinal: string;
}

export interface Task {
  id: string;
  title: string;
  description?: string;
  position: Position;
  tags: string[];
  assignees: string[];
  depends_on: string[];
  progress?: number;
  created_at: string;
  updated_at: string;
}

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
// Field & Entity schema types (mirrors Rust swissarmyhammer-fields types)
// ---------------------------------------------------------------------------

export interface SelectOption {
  value: string;
  label?: string;
  color?: string;
  icon?: string;
  order: number;
}

/** Discriminated union matching Rust FieldType (tagged on `kind`, kebab-case). */
export type FieldType =
  | { kind: "text"; single_line: boolean }
  | { kind: "markdown"; single_line: boolean }
  | { kind: "date" }
  | { kind: "number"; min?: number; max?: number }
  | { kind: "color" }
  | { kind: "select"; options: SelectOption[] }
  | { kind: "multi-select"; options: SelectOption[] }
  | { kind: "reference"; entity: string; multiple: boolean }
  | { kind: "computed"; derive: string };

export type Editor =
  | "markdown"
  | "select"
  | "multi-select"
  | "date"
  | "color-palette"
  | "number"
  | "none";

export type Display =
  | "markdown"
  | "badge"
  | "badge-list"
  | "avatar"
  | "date"
  | "color-swatch"
  | "number"
  | "text";

export type SortKind = "alphanumeric" | "option-order" | "datetime" | "numeric";

export interface FieldDef {
  id: string;
  name: string;
  description?: string;
  type: FieldType;
  default?: string;
  editor?: Editor;
  display?: Display;
  sort?: SortKind;
  filter?: string;
  group?: string;
  validate?: string;
}

export interface EntityDef {
  name: string;
  body_field?: string;
  fields: string[];
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
