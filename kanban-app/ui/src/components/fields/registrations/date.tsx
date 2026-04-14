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

function DateEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
}: FieldEditorProps) {
  return (
    <DateEditor
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

function DateDisplayAdapter({ field, value, entity, mode }: FieldDisplayProps) {
  return (
    <DateDisplay field={field} value={value} entity={entity!} mode={mode} />
  );
}

registerEditor("date", DateEditorAdapter);
registerDisplay("date", DateDisplayAdapter);
