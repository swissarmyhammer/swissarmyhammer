/**
 * Field — the single data-bound control for rendering and editing any entity field.
 *
 * Reads its value from the entity store. Writes via updateField. Stays in sync
 * when the entity store updates. Dispatches to the correct display and editor
 * components based on the field's YAML metadata.
 *
 * Inspector and grid both render <Field>. Nothing else touches editors directly.
 *
 * THIS IS A SKELETON. It does nothing yet. Every test should fail.
 */

import type { FieldDef } from "@/types/kanban";

export interface FieldProps {
  /** YAML field metadata — type, editor, display, section, etc. */
  fieldDef: FieldDef;
  /** Entity type (e.g. "task", "tag"). */
  entityType: string;
  /** Entity ID. */
  entityId: string;
  /** Presentation mode — compact (grid cells) or full (inspector rows). */
  mode: "compact" | "full";
  /** Whether the field is in edit mode. Container controls this. */
  editing: boolean;
  /** User wants to enter edit mode (click, Enter). */
  onEdit?: () => void;
  /** Editing finished — field already saved. Close the editor. */
  onDone?: () => void;
  /** Editing cancelled — discard and close. */
  onCancel?: () => void;
}

export function Field(_props: FieldProps) {
  // SKELETON — intentionally does nothing.
  // The test matrix should fail on every assertion.
  return null;
}
