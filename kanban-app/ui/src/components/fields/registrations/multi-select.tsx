/**
 * Register multi-select editor and related displays with the Field registry.
 *
 * Displays: badge-list (tags, depends_on, attachments), avatar (assignees)
 */

import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { MultiSelectEditor } from "@/components/fields/editors/multi-select-editor";
import { BadgeListDisplay } from "@/components/fields/displays/badge-list-display";
import { AvatarDisplay } from "@/components/fields/displays/avatar-display";

function MultiSelectEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
  mode,
}: FieldEditorProps) {
  return (
    <MultiSelectEditor
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

function BadgeListDisplayAdapter({
  field,
  value,
  entity,
  mode,
}: FieldDisplayProps) {
  return (
    <BadgeListDisplay
      field={field}
      value={value}
      entity={entity!}
      mode={mode}
    />
  );
}

function AvatarDisplayAdapter({
  field,
  value,
  entity,
  mode,
}: FieldDisplayProps) {
  return (
    <AvatarDisplay field={field} value={value} entity={entity!} mode={mode} />
  );
}

registerEditor("multi-select", MultiSelectEditorAdapter);
registerDisplay("badge-list", BadgeListDisplayAdapter);
registerDisplay("avatar", AvatarDisplayAdapter);
