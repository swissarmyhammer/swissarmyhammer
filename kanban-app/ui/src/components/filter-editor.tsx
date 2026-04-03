/**
 * Inline CM6 JavaScript editor for perspective filter expressions.
 *
 * Rendered inside a Radix Popover anchored to a filter icon on the
 * perspective tab bar. Uses the same CM6 infrastructure as text-editor.tsx:
 * shadcnTheme, keymapExtension, and buildSubmitCancelExtensions.
 *
 * The filter expression is a JS snippet evaluated via `new Function()` —
 * field names are available as local variables (e.g. `Status !== "Done"`).
 * Invalid expressions show an inline error message with a red border.
 */

import { memo, useCallback, useMemo, useRef, useState } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { getCM, Vim } from "@replit/codemirror-vim";
import { X } from "lucide-react";
import { useUIState } from "@/lib/ui-state-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { backendDispatch } from "@/lib/command-scope";
import { cn } from "@/lib/utils";

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
 * Validate a filter expression by attempting to compile it via `new Function()`.
 *
 * Returns null if the expression is valid, or an error message string.
 */
function validateFilter(expression: string): string | null {
  if (!expression.trim()) return null;
  try {
    // eslint-disable-next-line no-new-func
    new Function("fields", `with(fields) { return (${expression}); }`);
    return null;
  } catch (err) {
    return err instanceof Error ? err.message : String(err);
  }
}

/**
 * CM6 JavaScript editor for editing perspective filter expressions.
 *
 * - Enter saves the filter via `perspective.filter` command
 * - Escape cancels without saving
 * - Clear button removes the filter via `perspective.clearFilter` command
 * - Invalid expressions show a red border and error message
 */
export function FilterEditor({
  filter,
  perspectiveId,
  onClose,
}: FilterEditorProps) {
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const keymapCompartment = useRef(new Compartment());
  const { keymap_mode: mode } = useUIState();

  const [error, setError] = useState<string | null>(null);

  // Capture initial value so the memo wrapper sees a stable string reference.
  const initialValueRef = useRef(filter);

  // Refs for stable callbacks
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const committedRef = useRef(false);

  /** Submit the filter expression to the backend. */
  const handleSubmit = useCallback(() => {
    if (committedRef.current) return;
    const text = editorRef.current?.view
      ? editorRef.current.view.state.doc.toString()
      : "";

    const trimmed = text.trim();

    // Empty submit = clear filter
    if (!trimmed) {
      committedRef.current = true;
      backendDispatch({
        cmd: "perspective.clearFilter",
        args: { perspective_id: perspectiveId },
      }).catch(console.error);
      onCloseRef.current();
      return;
    }

    // Validate before saving
    const validationError = validateFilter(trimmed);
    if (validationError) {
      setError(validationError);
      return;
    }

    committedRef.current = true;
    backendDispatch({
      cmd: "perspective.filter",
      args: { filter: trimmed, perspective_id: perspectiveId },
    }).catch(console.error);
    onCloseRef.current();
  }, [perspectiveId]);

  /** Cancel without saving. */
  const handleCancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCloseRef.current();
  }, []);

  /** Clear the filter entirely. */
  const handleClear = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    backendDispatch({
      cmd: "perspective.clearFilter",
      args: { perspective_id: perspectiveId },
    }).catch(console.error);
    onCloseRef.current();
  }, [perspectiveId]);

  // Stable refs for submit/cancel extensions
  const submitRef = useRef<(() => void) | null>(null);
  submitRef.current = handleSubmit;
  const cancelRef = useRef<(() => void) | null>(null);
  cancelRef.current =
    mode === "vim" ? () => handleSubmit() : () => handleCancel();

  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      if (mode !== "vim") return;
      const cm = getCM(view);
      if (!cm) return;
      // Auto-enter insert mode so user can type immediately.
      let cancelled = false;
      let attempts = 0;
      const tryEnterInsert = () => {
        if (cancelled || attempts > 20) return;
        attempts++;
        const c = getCM(view);
        if (!c) {
          requestAnimationFrame(tryEnterInsert);
          return;
        }
        if (!c.state?.vim?.insertMode) {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          Vim.handleKey(c as any, "i", "mapping");
        }
      };
      requestAnimationFrame(tryEnterInsert);
    },
    [mode],
  );

  // Clear error when user types
  const changeExtension = useMemo(
    () => [
      EditorView.updateListener.of((update) => {
        if (update.docChanged) setError(null);
      }),
    ],
    [],
  );

  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      javascript(),
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: submitRef,
        onCancelRef: cancelRef,
        singleLine: true,
      }),
      ...changeExtension,
    ],
    [mode, changeExtension],
  );

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
          placeholder='Status !== "Done"'
          className="text-xs"
        />
      </div>
      {error && (
        <p
          className="text-xs text-destructive mt-1"
          data-testid="filter-error"
        >
          {error}
        </p>
      )}
      <p className="text-xs text-muted-foreground/70 mt-1">
        Enter to save, Escape to cancel
      </p>
    </div>
  );
}
