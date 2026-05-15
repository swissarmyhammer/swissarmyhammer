/**
 * Register attachment and attachment-list display and editor components
 * with the Field registry.
 */

import {
  registerDisplay,
  registerEditor,
  type FieldDisplayProps,
  type FieldEditorProps,
} from "@/components/fields/field";
import {
  AttachmentDisplay,
  AttachmentListDisplay,
} from "@/components/fields/displays/attachment-display";
import { AttachmentEditor } from "@/components/fields/editors/attachment-editor";

function AttachmentDisplayAdapter({
  field,
  value,
  mode,
  onCommit,
}: FieldDisplayProps) {
  return (
    <AttachmentDisplay
      field={field}
      value={value}
      mode={mode}
      onCommit={onCommit}
    />
  );
}

function AttachmentListDisplayAdapter({
  field,
  value,
  mode,
  onCommit,
}: FieldDisplayProps) {
  return (
    <AttachmentListDisplay
      field={field}
      value={value}
      mode={mode}
      onCommit={onCommit}
    />
  );
}

function AttachmentEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
}: FieldEditorProps) {
  return (
    <AttachmentEditor
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

registerDisplay("attachment", AttachmentDisplayAdapter);
registerDisplay("attachment-list", AttachmentListDisplayAdapter);
registerEditor("attachment", AttachmentEditorAdapter);
