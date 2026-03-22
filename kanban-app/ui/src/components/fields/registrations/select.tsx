/**
 * Register select editor and badge display with the Field registry.
 */

import { registerEditor, registerDisplay, type FieldEditorProps, type FieldDisplayProps } from "@/components/fields/field";
import { SelectEditor } from "@/components/fields/editors/select-editor";
import { BadgeDisplay } from "@/components/fields/displays/badge-display";

function SelectEditorAdapter({ field, value, onCommit, onCancel }: FieldEditorProps) {
  return <SelectEditor field={field} value={value} onCommit={onCommit} onCancel={onCancel} mode="compact" />;
}

function BadgeDisplayAdapter({ field, value, entity, mode }: FieldDisplayProps) {
  return <BadgeDisplay field={field} value={value} entity={entity!} mode={mode} />;
}

registerEditor("select", SelectEditorAdapter);
registerDisplay("badge", BadgeDisplayAdapter);
