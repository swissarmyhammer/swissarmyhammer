import type { FieldDef } from "@/types/kanban";

/**
 * Shared props for all field editors.
 *
 * Editors are pure UI: they manage draft state and call onCommit(value) when
 * done. Field handles persistence via updateField — editors never call it.
 */
export interface EditorProps {
  value: unknown;

  // --- Lifecycle callbacks ---
  /** Called with the final value when the editor commits. */
  onCommit: (value: unknown) => void;
  /** Signal to container: editing is complete, close me. No value — editor already saved. */
  onDone?: () => void;
  /** Signal to container: discard changes and close. */
  onCancel: () => void;
  /** Semantic submit — fires on Enter (CUA/emacs) or normal-mode Enter (vim). */
  onSubmit?: (value: unknown) => void;
  /** Report intermediate value changes for debounced autosave. */
  onChange?: (value: unknown) => void;

  /** Visual density — `compact` for board cards, `full` for inspector panes. */
  mode: "compact" | "full";
}

export { SelectEditor } from "./select-editor";
export { NumberEditor } from "./number-editor";
export { DateEditor } from "./date-editor";
export { ColorPaletteEditor } from "./color-palette-editor";
export { MultiSelectEditor } from "./multi-select-editor";
export { AttachmentEditor } from "./attachment-editor";
export { ReferenceSelectEditor } from "./reference-select-editor";

/** Resolve which editor component to use for a field — reads directly from the YAML-configured `editor` property. */
export function resolveEditor(field: FieldDef): string {
  return field.editor ?? "none";
}
