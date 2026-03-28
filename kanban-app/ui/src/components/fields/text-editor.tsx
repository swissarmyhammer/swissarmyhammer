import { memo, useCallback, useMemo, useRef } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView, placeholder as cmPlaceholder } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { getCM, Vim } from "@replit/codemirror-vim";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useUIState } from "@/lib/ui-state-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import type { FieldDef } from "@/types/kanban";

interface FieldPlaceholderProps {
  field: FieldDef;
  value: unknown;
  editing: boolean;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

/**
 * Fallback presenter/editor for field types without a dedicated component.
 *
 * - Display: ReactMarkdown with GFM (prose styling)
 * - Edit: CodeMirror 6 with vim/emacs/CUA keymaps
 */
export function FieldPlaceholder({
  field,
  value,
  editing,
  onEdit,
  onCommit,
  onCancel,
}: FieldPlaceholderProps) {
  const text = toText(value);

  if (editing) {
    return (
      <TextEditor
        value={text}
        onCommit={(v) => onCommit(v)}
        onCancel={onCancel}
      />
    );
  }

  return (
    <div className="text-sm cursor-text min-h-[1.25rem]" onClick={onEdit}>
      {text ? (
        <div className="prose prose-sm dark:prose-invert max-w-none">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
        </div>
      ) : (
        <span className="text-muted-foreground/50 italic">
          {field.name.replace(/_/g, " ")}
        </span>
      )}
    </div>
  );
}

/** Static basicSetup config — hoisted to module level to avoid recreating on each render. */
const BASIC_SETUP = {
  lineNumbers: false,
  foldGutter: false,
  highlightActiveLine: false,
  highlightActiveLineGutter: false,
  indentOnInput: true,
  bracketMatching: false,
  autocompletion: false,
} as const;

interface EditorProps {
  value: string;
  /** Called with the final value when the editor commits. */
  onCommit: (value: string) => void;
  onCancel: () => void;
  /** Semantic submit — fires on Enter (CUA/emacs) or normal-mode Enter (vim). */
  onSubmit?: (text: string) => void;
  /** Placeholder text shown when the editor is empty. */
  placeholder?: string;
  /** Called on every content change with the current text. */
  onChange?: (text: string) => void;
  /** Popup mode — when true, auto-enters vim insert mode (e.g. quick-capture). */
  popup?: boolean;
  /** Additional CM6 extensions (e.g. mention decorations, autocomplete). */
  extraExtensions?: import("@codemirror/state").Extension[];
}

/**
 * Memoized CodeMirror wrapper that prevents re-renders from parent context changes.
 *
 * `@uiw/react-codemirror` is not wrapped in React.memo, so every parent re-render
 * runs all its internal hooks — including an O(n) doc.toString() comparison.
 * This wrapper ensures CodeMirror only re-renders when its props actually change.
 */
const StableCodeMirror = memo(function StableCodeMirror({
  editorRef,
  initialValue,
  onBlur,
  onCreateEditor,
  extensions,
  placeholder,
  className,
}: {
  editorRef: React.RefObject<ReactCodeMirrorRef | null>;
  initialValue: string;
  onBlur: () => void;
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
      onBlur={onBlur}
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
 * Single-purpose CM6 text/markdown editor used across the field system.
 *
 * Runs in uncontrolled mode — CodeMirror owns the document, React never
 * re-renders during typing. The value prop is only used for initialization.
 * Commit reads from `view.state.doc.toString()` at exit time.
 */
export function TextEditor({
  value,
  onCommit,
  onCancel,
  onSubmit,
  placeholder,
  onChange,
  popup,
  extraExtensions,
}: EditorProps) {
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const keymapCompartment = useRef(new Compartment());
  const { keymap_mode: mode } = useUIState();

  // Capture initial value so the memo wrapper sees a stable string reference.
  const initialValueRef = useRef(value);

  // Refs for stable callbacks — avoids recreating closures on every render
  const valueRef = useRef(value);
  valueRef.current = value;
  const onCommitRef = useRef(onCommit);
  onCommitRef.current = onCommit;
  const onCancelRef = useRef(onCancel);
  onCancelRef.current = onCancel;
  const onSubmitRef = useRef(onSubmit);
  onSubmitRef.current = onSubmit;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  // Guard against re-entrant commits (blur fires after Escape)
  const committedRef = useRef(false);

  /** Commit the current editor content and signal done. */
  const commitAndExit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    const text = editorRef.current?.view
      ? editorRef.current.view.state.doc.toString()
      : valueRef.current;
    onCommitRef.current(text);
  }, []);

  /** Cancel without saving. */
  const cancelAndExit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancelRef.current();
  }, []);

  const commitAndExitRef = useRef(commitAndExit);
  commitAndExitRef.current = commitAndExit;
  const cancelAndExitRef = useRef(cancelAndExit);
  cancelAndExitRef.current = cancelAndExit;

  /** Save current value without leaving the editor (vim insert→normal). */
  const saveInPlace = useCallback(() => {
    if (!editorRef.current?.view) return;
    const text = editorRef.current.view.state.doc.toString();
    if (text !== valueRef.current) {
      onCommitRef.current(text);
    }
  }, []);
  const saveInPlaceRef = useRef(saveInPlace);
  saveInPlaceRef.current = saveInPlace;

  // Semantic submit ref: if onSubmit provided, use it; otherwise commit-and-exit
  const semanticSubmitRef = useRef<(() => void) | null>(null);
  semanticSubmitRef.current = onSubmitRef.current
    ? () => {
        if (committedRef.current) return;
        const text = editorRef.current?.view
          ? editorRef.current.view.state.doc.toString()
          : valueRef.current;
        if (text.length > 0) {
          onSubmitRef.current!(text);
        }
      }
    : () => commitAndExitRef.current();

  // Semantic cancel ref:
  // - Vim inline editing: Escape commits — edits must never be lost.
  // - CUA/emacs: Escape always cancels (standard discard behavior).
  const semanticCancelRef = useRef<(() => void) | null>(null);
  semanticCancelRef.current =
    mode === "vim"
      ? () => commitAndExitRef.current()
      : () => cancelAndExitRef.current();

  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      if (mode !== "vim") return;
      const cm = getCM(view);
      if (!cm) return;

      if (popup) {
        // Popup/quick-capture mode: auto-enter insert mode so user can type immediately.
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
      } else {
        // Board/grid editing: ensure we start in normal mode
        if (cm.state?.vim?.insertMode) {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          Vim.exitInsertMode(cm as any);
        }
      }
    },
    [mode, popup],
  );

  // Forward onChange to parent if provided — via CM6 updateListener, not React state
  const changeExtension = useMemo(() => {
    if (!onChange) return [];
    return [
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onChangeRef.current?.(update.state.doc.toString());
        }
      }),
    ];
  }, [onChange]);

  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      markdown({ base: markdownLanguage }),
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: semanticSubmitRef,
        onCancelRef: semanticCancelRef,
        saveInPlaceRef,
      }),
      ...(placeholder ? [cmPlaceholder(placeholder)] : []),
      ...changeExtension,
      ...(extraExtensions ?? []),
    ],
    [mode, placeholder, changeExtension, extraExtensions],
  );

  // Stable onBlur — reads from ref
  const handleBlur = useCallback(() => {
    commitAndExitRef.current();
  }, []);

  return (
    <StableCodeMirror
      editorRef={editorRef}
      initialValue={initialValueRef.current}
      onBlur={handleBlur}
      onCreateEditor={handleCreateEditor}
      extensions={extensions}
      placeholder={placeholder}
      className="text-sm"
    />
  );
}

/** Coerce any field value to a string for display/editing. */
function toText(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  if (typeof value === "boolean") return value ? "Yes" : "No";
  if (Array.isArray(value)) return value.join(", ");
  return JSON.stringify(value);
}
