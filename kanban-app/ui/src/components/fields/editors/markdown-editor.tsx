import { EditableMarkdown } from "@/components/editable-markdown";
import { FieldPlaceholderEditor } from "@/components/fields/field-placeholder";
import type { Entity } from "@/types/kanban";

/**
 * Shared props for all field editors.
 *
 * Editors own their own persistence: when they have entity identity props
 * (entityType, entityId, fieldName), they call useFieldUpdate().updateField()
 * directly. Containers provide only lifecycle signals (onDone, onCancel).
 *
 * Migration: onCommit is the legacy callback where containers provided save
 * logic. Once all editors call updateField themselves, onCommit will be removed
 * and onDone (no value parameter) will replace it.
 */
export interface EditorProps {
  value: unknown;

  // --- Entity identity (editors save themselves when these are provided) ---
  /** Entity type to save to (e.g. "task", "tag"). */
  entityType?: string;
  /** Entity ID to save to. */
  entityId?: string;
  /** Field name to save to (e.g. "title", "body"). */
  fieldName?: string;

  // --- Lifecycle callbacks ---
  /**
   * Legacy: container-provided save callback. Being replaced by editor-owned
   * saves via useFieldUpdate(). Will be removed once all editors are migrated.
   */
  onCommit: (value: unknown) => void;
  /** Signal to container: editing is complete, close me. No value — editor already saved. */
  onDone?: () => void;
  /** Signal to container: discard changes and close. */
  onCancel: () => void;
  /** Semantic submit — fires on Enter (CUA/emacs) or normal-mode Enter (vim). */
  onSubmit?: (value: unknown) => void;

  mode: "compact" | "full";
}

interface MarkdownEditorProps extends EditorProps {
  multiline?: boolean;
  tags?: Entity[];
  placeholder?: string;
  initialEditing?: boolean;
}

/**
 * Markdown editor — compact: FieldPlaceholderEditor (inline CM6),
 * full: EditableMarkdown (display/edit toggle with tag decorations).
 */
export function MarkdownEditor({
  value,
  entityType,
  entityId,
  fieldName,
  onCommit,
  onCancel,
  onSubmit,
  mode,
  multiline,
  tags,
  placeholder,
  initialEditing,
}: MarkdownEditorProps) {
  const text = typeof value === "string" ? value : value != null ? String(value) : "";

  if (mode === "compact") {
    return (
      <FieldPlaceholderEditor
        value={text}
        entityType={entityType}
        entityId={entityId}
        fieldName={fieldName}
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
      initialEditing={initialEditing}
    />
  );
}
