import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView, placeholder as cmPlaceholder } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { getCM, Vim } from "@replit/codemirror-vim";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useKeymap } from "@/lib/keymap-context";
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
      <FieldPlaceholderEditor
        value={text}
        onCommit={(v) => onCommit(v)}
        onCancel={onCancel}
      />
    );
  }

  return (
    <div
      className="text-sm cursor-text min-h-[1.25rem]"
      onClick={onEdit}
    >
      {text ? (
        <div className="prose prose-sm dark:prose-invert max-w-none">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
        </div>
      ) : (
        <span className="text-muted-foreground/50 italic">{field.name.replace(/_/g, " ")}</span>
      )}
    </div>
  );
}

interface EditorProps {
  value: string;
  onCommit: (value: string) => void;
  onCancel: () => void;
  /** Semantic submit — fires on Enter (CUA/emacs) or normal-mode Enter (vim). */
  onSubmit?: (text: string) => void;
  /** Placeholder text shown when the editor is empty. */
  placeholder?: string;
  /** Called on every content change with the current text. */
  onChange?: (text: string) => void;
}

export function FieldPlaceholderEditor({ value, onCommit, onCancel, onSubmit, placeholder, onChange }: EditorProps) {
  const [draft, setDraft] = useState(value);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const keymapCompartment = useRef(new Compartment());
  const { mode } = useKeymap();

  // Guard against re-entrant commits (blur fires after Escape)
  const committedRef = useRef(false);
  useEffect(() => {
    committedRef.current = false;
  }, []);

  const commitAndExit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    const text = editorRef.current?.view
      ? editorRef.current.view.state.doc.toString()
      : draft;
    onCommit(text);
  }, [draft, onCommit]);

  const cancelAndExit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  const commitAndExitRef = useRef(commitAndExit);
  commitAndExitRef.current = commitAndExit;
  const cancelAndExitRef = useRef(cancelAndExit);
  cancelAndExitRef.current = cancelAndExit;

  const saveInPlace = useCallback(() => {
    if (!editorRef.current?.view) return;
    const text = editorRef.current.view.state.doc.toString();
    if (text !== value) onCommit(text);
  }, [value, onCommit]);
  const saveInPlaceRef = useRef(saveInPlace);
  saveInPlaceRef.current = saveInPlace;

  // Semantic submit ref: if onSubmit provided, use it; otherwise commit-and-exit
  const semanticSubmitRef = useRef<(() => void) | null>(null);
  semanticSubmitRef.current = onSubmit
    ? () => {
        if (committedRef.current) return;
        const text = editorRef.current?.view
          ? editorRef.current.view.state.doc.toString()
          : draft;
        if (text.length > 0) onSubmit(text);
      }
    : () => commitAndExitRef.current();

  // Semantic cancel ref: if onSubmit provided, use onCancel directly;
  // otherwise preserve existing defaults (vim: commit, CUA: cancel)
  const semanticCancelRef = useRef<(() => void) | null>(null);
  semanticCancelRef.current = onSubmit
    ? () => cancelAndExitRef.current()
    : mode === "vim"
      ? () => commitAndExitRef.current()
      : () => cancelAndExitRef.current();

  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      if (mode !== "vim") return;
      const cm = getCM(view);
      if (!cm) return;

      if (onSubmit) {
        // Popup/quick-capture mode: auto-enter insert mode so user can type immediately.
        // Same rAF retry pattern as command-palette.tsx.
        let cancelled = false;
        let attempts = 0;
        const tryEnterInsert = () => {
          if (cancelled || attempts > 20) return;
          attempts++;
          const c = getCM(view);
          if (!c) { requestAnimationFrame(tryEnterInsert); return; }
          if (!c.state?.vim?.insertMode) {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            Vim.handleKey(c as any, "i", "mapping");
          }
        };
        requestAnimationFrame(tryEnterInsert);
        // Store cleanup on the view for the effect below
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (view as any).__cancelInsert = () => { cancelled = true; };
      } else {
        // Grid cell editing: ensure we start in normal mode
        if (cm.state?.vim?.insertMode) {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          Vim.exitInsertMode(cm as any);
        }
      }
    },
    [mode, onSubmit],
  );

  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      markdown({ base: markdownLanguage, codeLanguages: languages }),
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: semanticSubmitRef,
        onCancelRef: semanticCancelRef,
        saveInPlaceRef,
      }),
      ...(placeholder ? [cmPlaceholder(placeholder)] : []),
    ],
    [mode, placeholder],
  );

  return (
    <CodeMirror
      ref={editorRef}
      autoFocus
      value={draft}
      onChange={(val) => { setDraft(val); onChange?.(val); }}
      onBlur={() => commitAndExitRef.current()}
      onCreateEditor={handleCreateEditor}
      extensions={extensions}
      theme={shadcnTheme}
      basicSetup={{
        lineNumbers: false,
        foldGutter: false,
        highlightActiveLine: false,
        highlightActiveLineGutter: false,
        indentOnInput: true,
        bracketMatching: false,
        autocompletion: false,
      }}
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
