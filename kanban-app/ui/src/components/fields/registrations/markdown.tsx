/**
 * Register markdown editor and displays with the Field registry.
 *
 * Editor: TextEditor (compact CM6 inline editor)
 * Displays: "text" (plain text), "markdown" (rendered GFM with mention pills)
 */

import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { TextEditor } from "@/components/fields/text-editor";
import { TextDisplay } from "@/components/fields/displays/text-display";
import { MarkdownDisplay } from "@/components/fields/displays/markdown-display";

/** Markdown editor adapter — wraps TextEditor to match FieldEditorProps. */
function MarkdownEditorAdapter({
  value,
  mode,
  onCommit,
  onCancel,
}: FieldEditorProps) {
  const text =
    typeof value === "string" ? value : value != null ? String(value) : "";
  return (
    <TextEditor
      value={text}
      onCommit={(v) => onCommit(v)}
      onCancel={onCancel}
      onSubmit={mode === "compact" ? (v) => onCommit(v) : undefined}
    />
  );
}

/** Text display adapter — wraps TextDisplay to match FieldDisplayProps. */
function TextDisplayAdapter({ field, value, entity, mode }: FieldDisplayProps) {
  return (
    <TextDisplay field={field} value={value} entity={entity!} mode={mode} />
  );
}

/** Markdown display adapter — wraps MarkdownDisplay to match FieldDisplayProps. */
function MarkdownDisplayAdapter({
  field,
  value,
  entity,
  mode,
  onCommit,
}: FieldDisplayProps) {
  return (
    <MarkdownDisplay
      field={field}
      value={value}
      entity={entity!}
      mode={mode}
      onCommit={onCommit as ((value: string) => void) | undefined}
    />
  );
}

// Register
registerEditor("markdown", MarkdownEditorAdapter);
registerDisplay("text", TextDisplayAdapter);
registerDisplay("markdown", MarkdownDisplayAdapter);
