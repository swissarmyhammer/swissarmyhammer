import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap, EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { vim, getCM, Vim } from "@replit/codemirror-vim";
import { emacs } from "@replit/codemirror-emacs";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useKeymap } from "@/lib/keymap-context";

interface EditableMarkdownProps {
  value: string;
  onCommit: (value: string) => void;
  className?: string;
  inputClassName?: string;
  multiline?: boolean;
  placeholder?: string;
}

/** Regex matching a GFM task list checkbox in markdown source */
const CHECKBOX_RE = /- \[([ xX])\]/g;

/**
 * Toggle the Nth checkbox in a markdown string.
 * Returns the updated string or null if the index is out of range.
 */
function toggleCheckbox(source: string, index: number): string | null {
  let count = 0;
  return source.replace(CHECKBOX_RE, (match, check) => {
    if (count++ === index) {
      return check === " " ? "- [x]" : "- [ ]";
    }
    return match;
  });
}

/** Minimal CM6 theme: no gutters, transparent bg, matches surrounding text */
const minimalTheme = EditorView.theme({
  "&": { backgroundColor: "transparent" },
  ".cm-gutters": { display: "none" },
  ".cm-content": { padding: "0" },
  "&.cm-focused": { outline: "none" },
  ".cm-line": { padding: "0" },
  ".cm-scroller": { overflow: "auto" },
});

/** Build keymap extension based on mode */
function keymapExtension(mode: string) {
  switch (mode) {
    case "vim":
      return vim();
    case "emacs":
      return emacs();
    default:
      return [];
  }
}

export function EditableMarkdown({
  value,
  onCommit,
  className,
  inputClassName,
  multiline,
  placeholder,
}: EditableMarkdownProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const clickCoordsRef = useRef<{ x: number; y: number } | null>(null);
  const keymapCompartment = useRef(new Compartment());
  const { mode } = useKeymap();

  // Keep draft in sync when value changes externally
  useEffect(() => {
    if (!editing) setDraft(value);
  }, [value, editing]);

  // Save + exit the editor
  const commitAndExit = useCallback(() => {
    setEditing(false);
    const trimmed = draft.trim();
    if (trimmed !== value) {
      onCommit(trimmed);
    }
  }, [draft, value, onCommit]);

  // Ref so DOM event handlers always see the latest closure
  const commitAndExitRef = useRef(commitAndExit);
  commitAndExitRef.current = commitAndExit;

  // Save current value without leaving the editor (vim insert→normal)
  const saveInPlace = useCallback(() => {
    if (!editorRef.current?.view) return;
    const text = editorRef.current.view.state.doc.toString().trim();
    if (text !== value) {
      onCommit(text);
    }
  }, [value, onCommit]);
  const saveInPlaceRef = useRef(saveInPlace);
  saveInPlaceRef.current = saveInPlace;

  // Hot-swap keymap only when mode actually changes while editor is open
  const prevModeRef = useRef<string | null>(null);
  useEffect(() => {
    if (!editing || !editorRef.current?.view) {
      prevModeRef.current = null;
      return;
    }
    if (prevModeRef.current !== null && prevModeRef.current !== mode) {
      editorRef.current.view.dispatch({
        effects: keymapCompartment.current.reconfigure(keymapExtension(mode)),
      });
    }
    prevModeRef.current = mode;
  }, [mode, editing]);

  // After editor mounts and gets focus, ensure vim is in normal mode
  // and position cursor at click location
  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      // Ensure vim starts in normal mode (solid block cursor)
      if (mode === "vim") {
        const cm = getCM(view);
        if (cm?.state?.vim?.insertMode) {
          Vim.exitInsertMode(cm);
        }
      }

      // Position cursor at click coordinates via CM6 API.
      // posAtCoords requires real DOM layout (getClientRects) so guard
      // against environments where layout isn't available (e.g. jsdom).
      const coords = clickCoordsRef.current;
      clickCoordsRef.current = null;
      if (coords) {
        try {
          const pos = view.posAtCoords(coords);
          if (pos !== null) {
            view.dispatch({ selection: { anchor: pos } });
          }
        } catch {
          // No layout available — cursor stays at default position
        }
      }
    },
    [mode]
  );

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      // Store click coordinates — CM6 posAtCoords will resolve them after mount
      clickCoordsRef.current = { x: e.clientX, y: e.clientY };
      setDraft(value);
      setEditing(true);
    },
    [value]
  );

  // Memoize extensions so keystroke-driven re-renders don't recreate
  // the vim/emacs extension and blow away modal state
  const extensions = useMemo(
    () => [
      minimalTheme,
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      // Vim mode: intercept Escape at the DOM level to check vim state
      ...(mode === "vim"
        ? [
            EditorView.domEventHandlers({
              keydown(event, view) {
                if (event.key === "Escape") {
                  const cm = getCM(view);
                  if (cm?.state?.vim?.insertMode) {
                    // Insert mode: let vim handle Escape (→ normal mode),
                    // then save the value on next tick
                    setTimeout(() => saveInPlaceRef.current(), 0);
                    return false;
                  }
                  // Normal mode: save and exit the editor
                  commitAndExitRef.current();
                  return true;
                }
                return false;
              },
            }),
          ]
        : [
            // CUA / Emacs: Escape saves and exits
            keymap.of([
              {
                key: "Escape",
                run: () => {
                  commitAndExitRef.current();
                  return true;
                },
              },
            ]),
          ]),
      // Single-line: Enter saves and exits
      ...(!multiline
        ? [
            keymap.of([
              {
                key: "Enter",
                run: () => {
                  commitAndExitRef.current();
                  return true;
                },
              },
            ]),
          ]
        : []),
      ...(multiline
        ? [markdown({ base: markdownLanguage, codeLanguages: languages })]
        : []),
    ],
    [mode, multiline]
  );

  if (editing) {
    return (
      <CodeMirror
        ref={editorRef}
        autoFocus
        value={draft}
        onChange={(val) => setDraft(val)}
        onBlur={commitAndExit}
        onCreateEditor={handleCreateEditor}
        extensions={extensions}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          indentOnInput: !!multiline,
          bracketMatching: false,
          autocompletion: false,
        }}
        className={inputClassName ?? className}
      />
    );
  }

  // Display mode
  const isEmpty = !value && placeholder;
  const display = value || placeholder;

  if (!display) {
    return (
      <span
        className={`${className ?? ""} text-muted-foreground italic cursor-text`}
        onClick={handleClick}
      >
        {placeholder}
      </span>
    );
  }

  if (isEmpty) {
    return (
      <span
        className={`${className ?? ""} text-muted-foreground italic cursor-text`}
        onClick={handleClick}
      >
        {placeholder}
      </span>
    );
  }

  // Track checkbox index for interactive toggling
  let checkboxIndex = 0;

  return (
    <div className={`${className ?? ""} cursor-text`} onClick={handleClick}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          // Make task list checkboxes interactive
          input: (props) => {
            if (props.type === "checkbox") {
              const idx = checkboxIndex++;
              return (
                <input
                  type="checkbox"
                  checked={props.checked ?? false}
                  onChange={() => {
                    const updated = toggleCheckbox(value, idx);
                    if (updated !== null) onCommit(updated);
                  }}
                  onClick={(e) => e.stopPropagation()}
                />
              );
            }
            return <input {...props} />;
          },
        }}
      >
        {value}
      </ReactMarkdown>
    </div>
  );
}
