/**
 * Register select editor and badge display with the Field registry.
 *
 * When the field is a reference (field.type.entity is set), uses
 * ReferenceSelectEditor (searchable combobox). Otherwise falls back
 * to the static SelectEditor for enum/options fields.
 */

import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { SelectEditor } from "@/components/fields/editors/select-editor";
import { ReferenceSelectEditor } from "@/components/fields/editors/reference-select-editor";
import { BadgeDisplay } from "@/components/fields/displays/badge-display";

function SelectEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
}: FieldEditorProps) {
  // Reference fields use the searchable combobox editor
  if (field.type.entity) {
    return (
      <ReferenceSelectEditor
        field={field}
        value={value}
        entity={entity}
        onCommit={onCommit}
        onCancel={onCancel}
        onChange={onChange}
        mode="compact"
      />
    );
  }

  // Static enum/options fields use the original select editor
  return (
    <SelectEditor
      field={field}
      value={value}
      entity={entity}
      onCommit={onCommit}
      onCancel={onCancel}
      onChange={onChange}
      mode="compact"
    />
  );
}

function BadgeDisplayAdapter({
  field,
  value,
  entity,
  mode,
}: FieldDisplayProps) {
  return (
    <BadgeDisplay field={field} value={value} entity={entity!} mode={mode} />
  );
}

registerEditor("select", SelectEditorAdapter);
registerDisplay("badge", BadgeDisplayAdapter);
