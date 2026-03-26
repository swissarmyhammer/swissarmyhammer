/**
 * Register number editor and display with the Field registry.
 */

import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { NumberEditor } from "@/components/fields/editors/number-editor";
import { NumberDisplay } from "@/components/fields/displays/number-display";

/** Number editor adapter — wraps NumberEditor to match FieldEditorProps. */
function NumberEditorAdapter({ value, onCommit, onCancel }: FieldEditorProps) {
  return (
    <NumberEditor
      value={value}
      onCommit={onCommit}
      onCancel={onCancel}
      mode="compact"
    />
  );
}

/** Number display adapter — wraps NumberDisplay to match FieldDisplayProps. */
function NumberDisplayAdapter({
  field,
  value,
  entity,
  mode,
}: FieldDisplayProps) {
  return (
    <NumberDisplay field={field} value={value} entity={entity!} mode={mode} />
  );
}

registerEditor("number", NumberEditorAdapter);
registerDisplay("number", NumberDisplayAdapter);
