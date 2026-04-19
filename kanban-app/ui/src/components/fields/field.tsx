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
import { useDebouncedSave } from "@/lib/use-debounced-save";
import { resolveEditor } from "@/components/fields/editors";
import type { EditorProps } from "@/components/fields/editors";
import type { LucideIcon } from "lucide-react";
import type { FieldDef, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Contracts — every editor and display implements one of these
// ---------------------------------------------------------------------------

/**
 * Props that every editor component receives from Field.
 *
 * Callback signatures (onCommit, onCancel, onChange) are picked from
 * EditorProps so the two interfaces stay in sync automatically.
 */
export interface FieldEditorProps extends Pick<
  EditorProps,
  "value" | "mode" | "onCommit" | "onCancel" | "onChange"
> {
  field: FieldDef;
  entity?: Entity;
}

/** Props that every display component receives from Field. */
export interface FieldDisplayProps {
  field: FieldDef;
  value: unknown;
  entity?: Entity;
  mode: "compact" | "full";
  /** Persist a value without exiting display mode (e.g. checkbox toggle). */
  onCommit?: (value: unknown) => void;
}

// ---------------------------------------------------------------------------
// Registries — editors and displays register themselves here
// ---------------------------------------------------------------------------

/**
 * Optional metadata a display registration may expose.
 *
 * - `isEmpty`: predicate the inspector uses to suppress the surrounding row
 *   (icon, tooltip, flex gap) when the underlying display would render
 *   nothing. Consulted only for non-editable fields so editable fields with
 *   empty values stay clickable. Displays own their own notion of emptiness
 *   so the inspector stays free of hardcoded field names.
 *
 * - `iconOverride`: value-dependent icon function the parent layout (inspector
 *   row, card field) calls to replace the static YAML field icon. When the
 *   function returns a LucideIcon it replaces the static icon; when it
 *   returns null the static icon is used as fallback. This is general-purpose
 *   — any display can register one.
 *
 * - `tooltipOverride`: value-dependent tooltip text function the parent layout
 *   calls to replace the static YAML field description in the icon tooltip.
 *   When the function returns a non-null string it replaces the static text;
 *   when it returns null the static text is used as fallback. This is
 *   general-purpose — any display can register one.
 */
export interface DisplayRegistration {
  component: ComponentType<FieldDisplayProps>;
  isEmpty?: (value: unknown) => boolean;
  iconOverride?: (value: unknown) => LucideIcon | null;
  tooltipOverride?: (value: unknown) => string | null;
}

/** Options accepted by {@link registerDisplay}. */
export interface RegisterDisplayOptions {
  /** See {@link DisplayRegistration.isEmpty}. */
  isEmpty?: (value: unknown) => boolean;
  /** See {@link DisplayRegistration.iconOverride}. */
  iconOverride?: (value: unknown) => LucideIcon | null;
  /** See {@link DisplayRegistration.tooltipOverride}. */
  tooltipOverride?: (value: unknown) => string | null;
}

const editorRegistry = new Map<string, ComponentType<FieldEditorProps>>();
const displayRegistry = new Map<string, DisplayRegistration>();

/** Register an editor component for a given editor type name. */
export function registerEditor(
  name: string,
  component: ComponentType<FieldEditorProps>,
) {
  editorRegistry.set(name, component);
}

/**
 * Register a display component for a given display type name.
 *
 * @param name - Display type identifier (matches `display:` in field YAML).
 * @param component - React component rendered for values of this display.
 * @param options - Optional metadata (see {@link RegisterDisplayOptions}).
 */
export function registerDisplay(
  name: string,
  component: ComponentType<FieldDisplayProps>,
  options?: RegisterDisplayOptions,
) {
  displayRegistry.set(name, {
    component,
    isEmpty: options?.isEmpty,
    iconOverride: options?.iconOverride,
    tooltipOverride: options?.tooltipOverride,
  });
}

/**
 * Look up the `isEmpty` predicate registered for a display, if any.
 *
 * Returns `undefined` when the display is unregistered or when the
 * registration did not supply an `isEmpty` option.
 */
export function getDisplayIsEmpty(
  name: string,
): ((value: unknown) => boolean) | undefined {
  return displayRegistry.get(name)?.isEmpty;
}

/**
 * Look up the `iconOverride` function registered for a display, if any.
 *
 * Returns `undefined` when the display is unregistered or when the
 * registration did not supply an `iconOverride` option.
 */
export function getDisplayIconOverride(
  name: string,
): ((value: unknown) => LucideIcon | null) | undefined {
  return displayRegistry.get(name)?.iconOverride;
}

/**
 * Look up the `tooltipOverride` function registered for a display, if any.
 *
 * Returns `undefined` when the display is unregistered or when the
 * registration did not supply a `tooltipOverride` option.
 */
export function getDisplayTooltipOverride(
  name: string,
): ((value: unknown) => string | null) | undefined {
  return displayRegistry.get(name)?.tooltipOverride;
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
 * Collects the commit / cancel / debounced-change callbacks a Field needs.
 *
 * Split out of {@link Field} so the component stays readable. All three
 * callbacks close over the same entity/field identity and share the debounced
 * save cancel, which is why they live together.
 */
function useFieldHandlers(
  entityType: string,
  entityId: string,
  fieldName: string,
  onDone?: () => void,
  onCancel?: () => void,
) {
  const { updateField } = useFieldUpdate();
  const { onChange: debouncedOnChange, cancel: cancelSave } = useDebouncedSave({
    updateField,
    entityType,
    entityId,
    fieldName,
  });

  const handleCommit = useCallback(
    (newValue: unknown) => {
      cancelSave();
      updateField(entityType, entityId, fieldName, newValue).catch(() => {});
      onDone?.();
    },
    [cancelSave, updateField, entityType, entityId, fieldName, onDone],
  );

  const handleDisplayCommit = useCallback(
    (newValue: unknown) => {
      updateField(entityType, entityId, fieldName, newValue).catch(() => {});
    },
    [updateField, entityType, entityId, fieldName],
  );

  const handleCancel = useCallback(() => {
    cancelSave();
    onCancel?.();
  }, [cancelSave, onCancel]);

  return { handleCommit, handleDisplayCommit, handleCancel, debouncedOnChange };
}

/** Resolves the editor from the registry and renders it, or null if unregistered. */
function FieldEditor(props: {
  fieldDef: FieldDef;
  value: unknown;
  entity: Entity | undefined;
  mode: "compact" | "full";
  onCommit: (value: unknown) => void;
  onCancel: () => void;
  onChange: (value: unknown) => void;
}) {
  const Editor = editorRegistry.get(props.fieldDef.editor ?? "");
  if (!Editor) return null;
  return (
    <Editor
      field={props.fieldDef}
      value={props.value}
      entity={props.entity}
      mode={props.mode}
      onCommit={props.onCommit}
      onCancel={props.onCancel}
      onChange={props.onChange}
    />
  );
}

/**
 * Resolves the display from the registry and renders it. Wraps in a
 * click-to-edit surface when the field is editable; bare otherwise.
 */
function FieldDisplayContent(props: {
  fieldDef: FieldDef;
  value: unknown;
  entity: Entity | undefined;
  mode: "compact" | "full";
  onEdit?: () => void;
  onCommit: (value: unknown) => void;
}) {
  const Display = displayRegistry.get(
    props.fieldDef.display ?? "text",
  )?.component;
  if (!Display) return null;
  const inner = (
    <Display
      field={props.fieldDef}
      value={props.value}
      entity={props.entity}
      mode={props.mode}
      onCommit={props.onCommit}
    />
  );
  if (resolveEditor(props.fieldDef) === "none") return inner;
  return (
    <div className="text-sm cursor-text min-h-[1.25rem]" onClick={props.onEdit}>
      {inner}
    </div>
  );
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
  const value = useFieldValue(entityType, entityId, fieldDef.name);
  const entity = useEntityStore().getEntity(entityType, entityId);
  const { handleCommit, handleDisplayCommit, handleCancel, debouncedOnChange } =
    useFieldHandlers(entityType, entityId, fieldDef.name, onDone, onCancel);

  if (editing) {
    return (
      <FieldEditor
        fieldDef={fieldDef}
        value={value}
        entity={entity}
        mode={mode}
        onCommit={handleCommit}
        onCancel={handleCancel}
        onChange={debouncedOnChange}
      />
    );
  }

  return (
    <FieldDisplayContent
      fieldDef={fieldDef}
      value={value}
      entity={entity}
      mode={mode}
      onEdit={onEdit}
      onCommit={handleDisplayCommit}
    />
  );
}
