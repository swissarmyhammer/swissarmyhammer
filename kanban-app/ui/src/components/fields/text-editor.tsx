import { memo, useCallback, useMemo, useRef } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
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
  /**
   * Single-line mode for inline rename and similar short inputs.
   *
   * Enter always commits (no newlines), even in vim insert mode.
   * All other behavior (Escape, blur, vim keybindings) is identical
   * to regular multiline field editing.
   */
  singleLine?: boolean;
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

/** Keep prop callbacks in stable refs so CM6 extensions see latest values without reconfig. */
function useStableRefs(
  props: Pick<EditorProps, "value" | "onCommit" | "onCancel" | "onSubmit" | "onChange">,
) {
  const valueRef = useRef(props.value);
  valueRef.current = props.value;
  const onCommitRef = useRef(props.onCommit);
  onCommitRef.current = props.onCommit;
  const onCancelRef = useRef(props.onCancel);
  onCancelRef.current = props.onCancel;
  const onSubmitRef = useRef(props.onSubmit);
  onSubmitRef.current = props.onSubmit;
  const onChangeRef = useRef(props.onChange);
  onChangeRef.current = props.onChange;
  return { valueRef, onCommitRef, onCancelRef, onSubmitRef, onChangeRef };
}

/** Guarded commit/cancel/save-in-place actions backed by a single committedRef. */
function useExitActions(
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
  refs: ReturnType<typeof useStableRefs>,
) {
  const committedRef = useRef(false);

  const commitAndExit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    const text = editorRef.current?.view
      ? editorRef.current.view.state.doc.toString()
      : refs.valueRef.current;
    refs.onCommitRef.current(text);
  }, [editorRef, refs]);

  const cancelAndExit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    refs.onCancelRef.current();
  }, [refs]);

  const commitAndExitRef = useRef(commitAndExit);
  commitAndExitRef.current = commitAndExit;
  const cancelAndExitRef = useRef(cancelAndExit);
  cancelAndExitRef.current = cancelAndExit;

  const saveInPlace = useCallback(() => {
    if (!editorRef.current?.view) return;
    const text = editorRef.current.view.state.doc.toString();
    if (text !== refs.valueRef.current) refs.onChangeRef.current?.(text);
  }, [editorRef, refs]);
  const saveInPlaceRef = useRef(saveInPlace);
  saveInPlaceRef.current = saveInPlace;

  /** Stable blur handler — saves draft via onChange without exiting. */
  const handleBlur = useCallback(() => {
    if (!committedRef.current && editorRef.current?.view) {
      const text = editorRef.current.view.state.doc.toString();
      refs.onChangeRef.current?.(text);
    }
  }, [editorRef, refs]);

  return { commitAndExitRef, cancelAndExitRef, saveInPlaceRef, handleBlur };
}

/**
 * Semantic submit/cancel refs that route to the correct exit action.
 *
 * Submit: onSubmit provided → call it with text; otherwise commit-and-exit.
 * Cancel: vim → commit (edits must never be lost); CUA/emacs → cancel.
 */
function useSemanticActions(
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
  refs: ReturnType<typeof useStableRefs>,
  exits: ReturnType<typeof useExitActions>,
  mode: string,
) {
  const semanticSubmitRef = useRef<(() => void) | null>(null);
  semanticSubmitRef.current = refs.onSubmitRef.current
    ? () => {
        const text = editorRef.current?.view
          ? editorRef.current.view.state.doc.toString()
          : refs.valueRef.current;
        if (text.length > 0) refs.onSubmitRef.current!(text);
      }
    : () => exits.commitAndExitRef.current();

  const semanticCancelRef = useRef<(() => void) | null>(null);
  semanticCancelRef.current =
    mode === "vim"
      ? () => exits.commitAndExitRef.current()
      : () => exits.cancelAndExitRef.current();

  return { semanticSubmitRef, semanticCancelRef };
}

/**
 * Build the CM6 extension array for TextEditor.
 *
 * Composes keymap, markdown language, submit/cancel bindings, onChange listener,
 * and any caller-provided extra extensions.
 */
function useEditorExtensions(
  mode: string,
  singleLine: boolean | undefined,
  onChange: ((text: string) => void) | undefined,
  onChangeRef: React.RefObject<((text: string) => void) | undefined>,
  semanticSubmitRef: React.RefObject<(() => void) | null>,
  semanticCancelRef: React.RefObject<(() => void) | null>,
  saveInPlaceRef: React.RefObject<(() => void) | null>,
  extraExtensions: import("@codemirror/state").Extension[] | undefined,
) {
  const keymapCompartment = useRef(new Compartment());

  const changeExtension = useMemo(() => {
    if (!onChange) return [];
    return [
      EditorView.updateListener.of((update) => {
        if (update.docChanged) onChangeRef.current?.(update.state.doc.toString());
      }),
    ];
  }, [onChange, onChangeRef]);

  return useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      markdown({ base: markdownLanguage }),
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: semanticSubmitRef,
        onCancelRef: semanticCancelRef,
        saveInPlaceRef,
        alwaysSubmitOnEnter: singleLine,
      }),
      ...changeExtension,
      ...(extraExtensions ?? []),
    ],
    [mode, singleLine, semanticSubmitRef, semanticCancelRef, saveInPlaceRef, changeExtension, extraExtensions],
  );
}

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
  singleLine,
  extraExtensions,
}: EditorProps) {
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const { keymap_mode: mode } = useUIState();
  const initialValueRef = useRef(value);

  const refs = useStableRefs({ value, onCommit, onCancel, onSubmit, onChange });
  const exits = useExitActions(editorRef, refs);
  const { semanticSubmitRef, semanticCancelRef } = useSemanticActions(editorRef, refs, exits, mode);

  const extensions = useEditorExtensions(
    mode, singleLine, onChange, refs.onChangeRef,
    semanticSubmitRef, semanticCancelRef, exits.saveInPlaceRef,
    extraExtensions,
  );

  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      if (mode !== "vim") return;
      const cm = getCM(view);
      if (!cm) return;
      if (cm.state?.vim?.insertMode) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        Vim.exitInsertMode(cm as any);
      }
    },
    [mode],
  );

  return (
    <StableCodeMirror
      editorRef={editorRef}
      initialValue={initialValueRef.current}
      onBlur={exits.handleBlur}
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
