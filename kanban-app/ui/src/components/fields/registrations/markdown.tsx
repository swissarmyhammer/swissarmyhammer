/**
 * Register markdown editor and displays with the Field registry.
 *
 * Editor: {@link TextEditor} wrapped with mention autocomplete and caller-side
 * Enter/Escape/blur policy. The {@link TextEditor} primitive is purely a
 * string editor; this adapter owns commit-on-Enter (compact mode), cancel-on-
 * Escape, and save-on-blur.
 *
 * Displays: "text" (plain text), "markdown" (rendered GFM with mention pills).
 */

import { useCallback, useMemo, useRef } from "react";
import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import {
  TextEditor,
  type TextEditorHandle,
} from "@/components/fields/text-editor";
import { TextDisplay } from "@/components/fields/displays/text-display";
import { MarkdownDisplay } from "@/components/fields/displays/markdown-display";
import { useMentionExtensions } from "@/hooks/use-mention-extensions";
import { useUIState } from "@/lib/ui-state-context";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";

/**
 * Builds stable refs and extensions for the markdown editor adapter.
 *
 * Track the latest doc text so commit/cancel/blur always read the current
 * value without React state (which races with typing).
 *
 * Vim semantics: Escape from normal mode routes to COMMIT, not cancel — the
 * vim idiom treats Escape as "done editing, save what I have." CUA/emacs treat
 * Escape as the explicit cancel/discard shortcut.
 */
function useMarkdownEditorPolicy(
  text: string,
  mode: FieldEditorProps["mode"],
  onCommit: FieldEditorProps["onCommit"],
  onCancel: FieldEditorProps["onCancel"],
  onChange: FieldEditorProps["onChange"],
) {
  const mentionExtensions = useMentionExtensions();
  const { keymap_mode: keymapMode } = useUIState();

  const latestTextRef = useRef(text);
  const handleChange = useCallback(
    (t: string) => {
      latestTextRef.current = t;
      onChange?.(t);
    },
    [onChange],
  );

  const submitRef = useRef<(() => void) | null>(() => {});
  submitRef.current = () => onCommit(latestTextRef.current);
  const cancelRef = useRef<(() => void) | null>(() => {});
  cancelRef.current =
    keymapMode === "vim"
      ? () => onCommit(latestTextRef.current)
      : () => onCancel();
  const blurSaveRef = useRef<(() => void) | null>(() => {});
  blurSaveRef.current = () => onChange?.(latestTextRef.current);

  // Only compact mode (board cards) treats Enter as submit. Full (inspector)
  // allows newlines in the buffer.
  const extensions = useMemo(
    () => [
      ...mentionExtensions,
      ...buildSubmitCancelExtensions({
        mode: keymapMode,
        onSubmitRef: submitRef,
        onCancelRef: cancelRef,
        saveInPlaceRef: blurSaveRef,
        singleLine: mode === "compact",
        alwaysSubmitOnEnter: mode === "compact",
      }),
    ],
    [keymapMode, mode, mentionExtensions],
  );

  return { handleChange, extensions, blurSaveRef };
}

/**
 * Markdown editor adapter — wires a pure {@link TextEditor} with the field
 * editor policy: mention autocomplete, commit-on-Enter (compact mode only),
 * cancel-on-Escape, and save-draft-on-blur.
 *
 * Exported for integration testing; production callers access it via the field
 * registry.
 */
export function MarkdownEditorAdapter({
  value,
  mode,
  onCommit,
  onCancel,
  onChange,
}: FieldEditorProps) {
  const text =
    typeof value === "string" ? value : value != null ? String(value) : "";
  const { handleChange, extensions, blurSaveRef } = useMarkdownEditorPolicy(
    text,
    mode,
    onCommit,
    onCancel,
    onChange,
  );
  const editorRef = useRef<TextEditorHandle>(null);

  return (
    <div
      className="min-h-[1.25rem]"
      onBlur={(e) => {
        // Save draft when focus leaves the editor entirely.
        if (!e.currentTarget.contains(e.relatedTarget as Node)) {
          blurSaveRef.current?.();
        }
      }}
    >
      <TextEditor
        ref={editorRef}
        value={text}
        onChange={handleChange}
        extensions={extensions}
        singleLine={mode === "compact"}
      />
    </div>
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
