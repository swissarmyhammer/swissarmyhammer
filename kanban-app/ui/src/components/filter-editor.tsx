/**
 * Inline CM6 editor for perspective filter expressions using the filter DSL.
 *
 * Rendered inside a Radix Popover anchored to a filter icon on the
 * perspective tab bar. Uses the same CM6 infrastructure as text-editor.tsx:
 * shadcnTheme, keymapExtension, and buildSubmitCancelExtensions.
 *
 * The filter expression uses the kanban filter DSL (`#tag && @user || !#done`).
 * Invalid expressions show an inline error message with a red border.
 */

import { memo, useCallback, useMemo, useRef, useState } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { filterLanguage } from "@/lang-filter";
import { parser as filterParser } from "@/lang-filter/parser";
import { EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { getCM, Vim } from "@replit/codemirror-vim";
import { X } from "lucide-react";
import { useUIState } from "@/lib/ui-state-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useDispatchCommand } from "@/lib/command-scope";
import { useMentionExtensions } from "@/hooks/use-mention-extensions";
import { cn } from "@/lib/utils";

/** Max retry attempts when waiting for vim mode to initialize on the CM instance. */
const MAX_VIM_INSERT_ATTEMPTS = 20;

/** Static basicSetup config — mirrors text-editor.tsx. */
const BASIC_SETUP = {
  lineNumbers: false,
  foldGutter: false,
  highlightActiveLine: false,
  highlightActiveLineGutter: false,
  indentOnInput: false,
  bracketMatching: true,
  autocompletion: false,
} as const;

interface FilterEditorProps {
  /** Current filter expression from the perspective. */
  filter: string;
  /** Perspective ID to update. */
  perspectiveId: string;
  /** Called after a successful submit or clear to close the popover. */
  onClose: () => void;
}

/**
 * Memoized CodeMirror wrapper — prevents re-renders from parent context changes.
 * Follows the StableCodeMirror pattern from text-editor.tsx.
 */
const StableCodeMirror = memo(function StableCodeMirror({
  editorRef,
  initialValue,
  onCreateEditor,
  extensions,
  placeholder,
  className,
}: {
  editorRef: React.RefObject<ReactCodeMirrorRef | null>;
  initialValue: string;
  onCreateEditor: (view: EditorView) => void;
  extensions: import("@codemirror/state").Extension[];
  placeholder?: string;
  className?: string;
}) {
  return (
    <CodeMirror
      ref={editorRef}
      autoFocus
      value={initialValue}
      onCreateEditor={onCreateEditor}
      extensions={extensions}
      theme={shadcnTheme}
      basicSetup={BASIC_SETUP}
      className={className}
      placeholder={placeholder}
    />
  );
});

/**
 * Validate a filter expression by parsing it with the Lezer grammar.
 *
 * Walks the parse tree looking for error nodes. Returns null if the expression
 * is valid, or an error message string describing the problem.
 */
function validateFilter(expression: string): string | null {
  if (!expression.trim()) return null;
  const tree = filterParser.parse(expression);
  let error: string | null = null;
  tree.iterate({
    enter(node) {
      if (node.type.isError && !error) {
        error = "Invalid filter expression";
      }
    },
  });
  return error;
}

/**
 * Hook for filter editor commit/dispatch callbacks.
 *
 * Manages the committed guard, dispatches filter/clearFilter commands,
 * and validates input before saving.
 */
function useFilterCommit(
  perspectiveId: string,
  onClose: () => void,
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
) {
  const dispatchFilter = useDispatchCommand("perspective.filter");
  const dispatchClearFilter = useDispatchCommand("perspective.clearFilter");
  const [error, setError] = useState<string | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const committedRef = useRef(false);

  const handleSubmit = useCallback(() => {
    if (committedRef.current) return;
    const text = editorRef.current?.view?.state.doc.toString() ?? "";
    const trimmed = text.trim();

    if (!trimmed) {
      committedRef.current = true;
      dispatchClearFilter({ args: { perspective_id: perspectiveId } }).catch(console.error);
      onCloseRef.current();
      return;
    }

    const validationError = validateFilter(trimmed);
    if (validationError) { setError(validationError); return; }

    committedRef.current = true;
    dispatchFilter({ args: { filter: trimmed, perspective_id: perspectiveId } }).catch(console.error);
    onCloseRef.current();
  }, [perspectiveId]);

  const handleCancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCloseRef.current();
  }, []);

  const handleClear = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    dispatchClearFilter({ args: { perspective_id: perspectiveId } }).catch(console.error);
    onCloseRef.current();
  }, [perspectiveId]);

  const clearError = useCallback(() => setError(null), []);

  return { error, clearError, handleSubmit, handleCancel, handleClear };
}

/**
 * Hook that builds the CM6 extension array and vim auto-insert handler.
 *
 * Composes the keymap, filter language, submit/cancel bindings, and
 * error-clearing listener into a single extension array.
 */
function useFilterExtensions(
  handleSubmit: () => void,
  handleCancel: () => void,
  clearError: () => void,
) {
  const keymapCompartment = useRef(new Compartment());
  const { keymap_mode: mode } = useUIState();

  const submitRef = useRef<(() => void) | null>(null);
  submitRef.current = handleSubmit;
  const cancelRef = useRef<(() => void) | null>(null);
  cancelRef.current = mode === "vim" ? () => handleSubmit() : () => handleCancel();

  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      if (mode !== "vim") return;
      let attempts = 0;
      const tryEnterInsert = () => {
        if (attempts > MAX_VIM_INSERT_ATTEMPTS) return;
        attempts++;
        const c = getCM(view);
        if (!c) { requestAnimationFrame(tryEnterInsert); return; }
        if (!c.state?.vim?.insertMode) {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          Vim.handleKey(c as any, "i", "mapping");
        }
      };
      requestAnimationFrame(tryEnterInsert);
    },
    [mode],
  );

  const mentionExts = useMentionExtensions({
    includeVirtualTags: true,
    includeFilterSigils: true,
  });

  const changeExtension = useMemo(
    () => [EditorView.updateListener.of((update) => { if (update.docChanged) clearError(); })],
    [clearError],
  );

  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      filterLanguage(),
      ...mentionExts,
      ...buildSubmitCancelExtensions({ mode, onSubmitRef: submitRef, onCancelRef: cancelRef, singleLine: true }),
      ...changeExtension,
    ],
    [mode, mentionExts, changeExtension],
  );

  return { handleCreateEditor, extensions };
}

/**
 * CM6 editor for editing perspective filter DSL expressions.
 *
 * - Enter saves the filter via `perspective.filter` command
 * - Escape cancels without saving
 * - Clear button removes the filter via `perspective.clearFilter` command
 * - Invalid expressions show a red border and error message
 */
export function FilterEditor({ filter, perspectiveId, onClose }: FilterEditorProps) {
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const initialValueRef = useRef(filter);
  const { error, clearError, handleSubmit, handleCancel, handleClear } =
    useFilterCommit(perspectiveId, onClose, editorRef);
  const { handleCreateEditor, extensions } =
    useFilterExtensions(handleSubmit, handleCancel, clearError);

  return (
    <div className="w-80" data-testid="filter-editor">
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-xs font-medium text-muted-foreground">
          Filter Expression
        </span>
        {filter && (
          <button
            onClick={handleClear}
            className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
            aria-label="Clear filter"
          >
            <X className="h-3 w-3" />
            Clear
          </button>
        )}
      </div>
      <div
        className={cn(
          "rounded-md border bg-background",
          error ? "border-destructive" : "border-input",
        )}
      >
        <StableCodeMirror
          editorRef={editorRef}
          initialValue={initialValueRef.current}
          onCreateEditor={handleCreateEditor}
          extensions={extensions}
          placeholder="#bug && @will"
          className="text-xs"
        />
      </div>
      {error && (
        <p className="text-xs text-destructive mt-1" data-testid="filter-error">
          {error}
        </p>
      )}
      <p className="text-xs text-muted-foreground/70 mt-1">
        #tag @user ^ref, &&/and, ||/or, !/not, () &mdash; Enter to save, Esc to
        cancel
      </p>
    </div>
  );
}
