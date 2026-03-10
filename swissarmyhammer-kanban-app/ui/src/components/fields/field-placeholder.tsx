import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap, EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { getCM, Vim } from "@replit/codemirror-vim";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useKeymap } from "@/lib/keymap-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
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
        <span className="text-muted-foreground italic">Empty</span>
      )}
    </div>
  );
}

interface EditorProps {
  value: string;
  onCommit: (value: string) => void;
  onCancel: () => void;
}

export function FieldPlaceholderEditor({ value, onCommit, onCancel }: EditorProps) {
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

  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      if (mode === "vim") {
        const cm = getCM(view);
        if (cm?.state?.vim?.insertMode) {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          Vim.exitInsertMode(cm as any);
        }
      }
    },
    [mode],
  );

  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      markdown({ base: markdownLanguage, codeLanguages: languages }),
      // Vim mode: Escape in normal mode commits, insert mode → save in place
      ...(mode === "vim"
        ? [
            EditorView.domEventHandlers({
              keydown(event, view) {
                if (event.key === "Escape") {
                  const cm = getCM(view);
                  if (cm?.state?.vim?.insertMode) {
                    setTimeout(() => saveInPlaceRef.current(), 0);
                    return false;
                  }
                  commitAndExitRef.current();
                  return true;
                }
                return false;
              },
            }),
          ]
        : [
            keymap.of([
              {
                key: "Escape",
                run: () => {
                  cancelAndExitRef.current();
                  return true;
                },
              },
            ]),
          ]),
    ],
    [mode],
  );

  return (
    <CodeMirror
      ref={editorRef}
      autoFocus
      value={draft}
      onChange={(val) => setDraft(val)}
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
