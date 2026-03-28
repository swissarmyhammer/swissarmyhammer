/**
 * Register date editor and display with the Field registry.
 */

import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { DateEditor } from "@/components/fields/editors/date-editor";
import { DateDisplay } from "@/components/fields/displays/date-display";

function DateEditorAdapter({ value, onCommit, onCancel }: FieldEditorProps) {
  return (
    <DateEditor
      value={value}
      onCommit={onCommit}
      onCancel={onCancel}
      mode="compact"
    />
  );
}

function DateDisplayAdapter({ field, value, entity, mode }: FieldDisplayProps) {
  return (
    <DateDisplay field={field} value={value} entity={entity!} mode={mode} />
  );
}

registerEditor("date", DateEditorAdapter);
registerDisplay("date", DateDisplayAdapter);
