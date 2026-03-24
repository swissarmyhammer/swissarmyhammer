/**
 * Field — metadata-driven, data-bound control for any entity field.
 *
 * Subscribes to field-level changes in the entity store. Re-renders only when
 * its specific field value changes — not when other fields on the same entity
 * change. Works regardless of change source (local edit, other window, file
 * watcher, etc.).
 *
 * Resolves editor and display components from registries — no switch statements,
 * no hardcoded field types. Adding a new field type means registering a component,
 * not touching this file.
 */

import { useCallback, type ComponentType } from "react";
import { useEntityStore, useFieldValue } from "@/lib/entity-store-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import { resolveEditor } from "@/components/fields/editors";
import type { FieldDef, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Contracts — every editor and display implements one of these
// ---------------------------------------------------------------------------

/** Props that every editor component receives from Field. */
export interface FieldEditorProps {
  field: FieldDef;
  value: unknown;
  entity?: Entity;
  mode: "compact" | "full";
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

/** Props that every display component receives from Field. */
export interface FieldDisplayProps {
  field: FieldDef;
  value: unknown;
  entity?: Entity;
  mode: "compact" | "full";
}

// ---------------------------------------------------------------------------
// Registries — editors and displays register themselves here
// ---------------------------------------------------------------------------

const editorRegistry = new Map<string, ComponentType<FieldEditorProps>>();
const displayRegistry = new Map<string, ComponentType<FieldDisplayProps>>();

/** Register an editor component for a given editor type name. */
export function registerEditor(
  name: string,
  component: ComponentType<FieldEditorProps>,
) {
  editorRegistry.set(name, component);
}

/** Register a display component for a given display type name. */
export function registerDisplay(
  name: string,
  component: ComponentType<FieldDisplayProps>,
) {
  displayRegistry.set(name, component);
}

// ---------------------------------------------------------------------------
// Field component
// ---------------------------------------------------------------------------

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

/**
 * Data-bound field control.
 *
 * Subscribes to its specific field value via useFieldValue — re-renders
 * only when this field changes. Resolves editor/display from registries.
 */
export function Field({
  fieldDef,
  entityType,
  entityId,
  mode,
  editing,
  onEdit,
  onDone,
  onCancel,
}: FieldProps) {
  const { getEntity } = useEntityStore();
  const { updateField } = useFieldUpdate();

  // Reactive field value — re-renders when this specific field changes.
  const value = useFieldValue(entityType, entityId, fieldDef.name);

  // Entity reference for editors that need context (e.g. multi-select).
  const entity = getEntity(entityType, entityId);

  /** Editor commits a value → Field persists and signals done. */
  const handleCommit = useCallback(
    (newValue: unknown) => {
      updateField(entityType, entityId, fieldDef.name, newValue).catch(
        () => {},
      );
      onDone?.();
    },
    [updateField, entityType, entityId, fieldDef.name, onDone],
  );

  const handleCancel = useCallback(() => {
    onCancel?.();
  }, [onCancel]);

  if (editing) {
    const Editor = editorRegistry.get(fieldDef.editor ?? "");
    if (!Editor) return null;
    return (
      <Editor
        field={fieldDef}
        value={value}
        entity={entity}
        mode={mode}
        onCommit={handleCommit}
        onCancel={handleCancel}
      />
    );
  }

  const Display = displayRegistry.get(fieldDef.display ?? "text");
  if (!Display) return null;

  const editable = resolveEditor(fieldDef) !== "none";

  if (!editable) {
    return (
      <Display field={fieldDef} value={value} entity={entity} mode={mode} />
    );
  }

  return (
    <div className="text-sm cursor-text min-h-[1.25rem]" onClick={onEdit}>
      <Display field={fieldDef} value={value} entity={entity} mode={mode} />
    </div>
  );
}
