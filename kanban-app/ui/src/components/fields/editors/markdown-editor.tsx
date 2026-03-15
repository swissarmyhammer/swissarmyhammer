import { EditableMarkdown } from "@/components/editable-markdown";
import { FieldPlaceholderEditor } from "@/components/fields/field-placeholder";
import type { Entity } from "@/types/kanban";

export interface EditorProps {
  value: unknown;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
  /** Semantic submit — fires on Enter (CUA/emacs) or normal-mode Enter (vim). */
  onSubmit?: (value: unknown) => void;
  mode: "compact" | "full";
}

interface MarkdownEditorProps extends EditorProps {
  multiline?: boolean;
  tags?: Entity[];
  placeholder?: string;
}

/**
 * Markdown editor — compact: FieldPlaceholderEditor (inline CM6),
 * full: EditableMarkdown (display/edit toggle with tag decorations).
 */
export function MarkdownEditor({
  value,
  onCommit,
  onCancel,
  onSubmit,
  mode,
  multiline,
  tags,
  placeholder,
}: MarkdownEditorProps) {
  const text = typeof value === "string" ? value : value != null ? String(value) : "";

  if (mode === "compact") {
    return (
      <FieldPlaceholderEditor
        value={text}
        onCommit={(v) => onCommit(v)}
        onCancel={onCancel}
        onSubmit={onSubmit ? (v) => onSubmit(v) : undefined}
      />
    );
  }

  return (
    <EditableMarkdown
      value={text}
      onCommit={(v) => onCommit(v)}
      multiline={multiline}
      tags={tags}
      className="text-sm leading-relaxed cursor-text"
      inputClassName="text-sm leading-relaxed bg-transparent w-full"
      placeholder={placeholder}
    />
  );
}
