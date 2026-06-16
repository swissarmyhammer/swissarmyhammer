/**
 * Register comment-log display and editor components with the Field
 * registry. The names match the `display:`/`editor:` strings in the
 * builtin `comments` field YAML — the inspector picks them up purely
 * through field metadata, with no comment-specific branching.
 */

import {
  registerDisplay,
  registerEditor,
  type FieldDisplayProps,
  type FieldEditorProps,
} from "@/components/fields/field";
import { CommentLogDisplay } from "@/components/fields/displays/comment-log-display";
import { CommentLogEditor } from "@/components/fields/editors/comment-log-editor";

function CommentLogDisplayAdapter({ field, value, mode }: FieldDisplayProps) {
  return <CommentLogDisplay field={field} value={value} mode={mode} />;
}

function CommentLogEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
}: FieldEditorProps) {
  return (
    <CommentLogEditor
      field={field}
      value={value}
      entity={entity}
      onCommit={onCommit}
      onCancel={onCancel}
      onChange={onChange}
      mode="full"
    />
  );
}

registerDisplay("comment-log", CommentLogDisplayAdapter);
registerEditor("comment-log", CommentLogEditorAdapter);
