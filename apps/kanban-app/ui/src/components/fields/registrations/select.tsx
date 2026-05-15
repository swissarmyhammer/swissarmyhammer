/**
 * Register select editor and badge display with the Field registry.
 *
 * For reference fields (`field.type.entity` set), delegates to the CM6-based
 * `SingleSelectEditor`, so every mention-style editor in the app shares the
 * same CM6 foundation (decorations, pill widgets, autocomplete, submit/cancel
 * semantics). For static-options fields (enums with `field.type.options`),
 * falls back to the original shadcn-based `SelectEditor`.
 */

import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { SelectEditor } from "@/components/fields/editors/select-editor";
import { SingleSelectEditor } from "@/components/fields/editors/single-select-editor";
import { BadgeDisplay } from "@/components/fields/displays/badge-display";

function SelectEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
  mode,
}: FieldEditorProps) {
  // Reference fields route to the unified CM6 single-select editor.
  if (field.type.entity) {
    return (
      <SingleSelectEditor
        field={field}
        value={value}
        entity={entity}
        onCommit={onCommit}
        onCancel={onCancel}
        onChange={onChange}
        mode={mode}
      />
    );
  }

  // Static enum/options fields stay on the shadcn Select — they're not
  // mention-based, so there's no pill pipeline to share.
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
