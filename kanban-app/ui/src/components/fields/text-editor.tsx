/**
 * Pure CodeMirror 6 string-editing primitive.
 *
 * {@link TextEditor} owns exactly four things:
 *
 *   1. The CodeMirror buffer.
 *   2. Emitting `onChange(text)` on every doc change.
 *   3. Exposing `focus()` via forwardRef.
 *   4. Accepting caller-provided `extensions` (language, decorations, keymaps).
 *
 * It does NOT own commit/cancel/submit/blur policy, debounced autosave,
 * repeated-commit guards, or any "close the editor" callback. Those concerns
 * belong in the caller (see {@link InlineRenameEditor}, {@link FilterEditor},
 * {@link QuickCapture}, {@link MarkdownEditorAdapter}).
 *
 * Why a pure primitive: previous iterations bundled commit machinery tuned for
 * popover field editors into `TextEditor` itself. The formula bar — an always-
 * open, always-live input with no close and no draft — kept hitting sharp edges
 * because the primitive kept re-running "commit and close" logic. Callers have
 * very different policies (popover vs. formula bar vs. rename vs. command
 * palette); keeping the primitive dumb lets each caller wire the policy it
 * actually needs via CM6 extensions.
 *
 * Keymap mode (vim/cua/emacs) is still applied here because it is part of the
 * editing primitive — the mode controls *character input*, not commit policy.
 * Callers that want Enter to submit add their own Prec.highest keymap binding
 * via the `extensions` prop.
 */

import {
  forwardRef,
  memo,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
} from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { getCM, Vim } from "@replit/codemirror-vim";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { useUIState } from "@/lib/ui-state-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";

/** Handle exposed by TextEditor via forwardRef so callers can move focus. */
export interface TextEditorHandle {
  /** Move keyboard focus into the CM6 editor. */
  focus(): void;
  /**
   * Imperatively replace the buffer contents.
   *
   * Prop `value` is captured at mount and never re-applied (see the file-level
   * docstring for why). Callers that need to force-reset the buffer after
   * mount (e.g. the × clear button in the formula bar) call this instead.
   */
  setValue(text: string): void;
}

/** Props for the pure {@link TextEditor} primitive. */
export interface TextEditorProps {
  /**
   * Initial buffer value. Subsequent changes to this prop do NOT reset the
   * document — the CM6 buffer is the source of truth after mount. Callers
   * that need to replace the contents should unmount/remount via `key`.
   */
  value: string;
  /** Called on every doc change with the new text. */
  onChange?: (text: string) => void;
  /** Caller-supplied CM6 extensions (keymaps, decorations, autocomplete). */
  extensions?: Extension[];
  /**
   * Language extension to use. Defaults to
   * `markdown({ base: markdownLanguage })` when omitted.
   */
  languageExtension?: Extension;
  /** Placeholder text shown when the editor is empty. */
  placeholder?: string;
  /**
   * Single-line mode. Suppresses newline insertion from Enter (so Enter can be
   * used for caller-defined semantics like commit or flush). Does NOT bind any
   * commit/submit action — callers own that via their own keymap extension.
   *
   * Defaults to false (multiline).
   */
  singleLine?: boolean;
  /**
   * Whether to auto-focus the editor on mount. Defaults to true.
   *
   * Set to false for always-visible editors (e.g. formula bar) that should
   * only focus on explicit user interaction, not on render.
   */
  autoFocus?: boolean;
  /** Optional class applied to the outer CM6 element. */
  className?: string;
}

/** Static basicSetup config — hoisted to module level to avoid recreation. */
const BASIC_SETUP = {
  lineNumbers: false,
  foldGutter: false,
  highlightActiveLine: false,
  highlightActiveLineGutter: false,
  indentOnInput: true,
  bracketMatching: false,
  autocompletion: false,
} as const;

/**
 * Memoized CodeMirror wrapper that prevents re-renders from parent context
 * churn.
 *
 * `@uiw/react-codemirror` is not wrapped in React.memo, so every parent
 * re-render runs all its internal hooks — including an `O(n)` doc.toString()
 * comparison inside its `value` useEffect, which (when run with an out-of-date
 * `value` prop after the user has typed) can force the doc back to the mount-
 * time initial value. Stable identity on this boundary keeps the editor's
 * internal state intact across parent re-renders.
 */
const StableCodeMirror = memo(function StableCodeMirror({
  editorRef,
  initialValue,
  onCreateEditor,
  extensions,
  placeholder,
  className,
  autoFocus,
}: {
  editorRef: React.RefObject<ReactCodeMirrorRef | null>;
  initialValue: string;
  onCreateEditor: (view: EditorView) => void;
  extensions: Extension[];
  placeholder?: string;
  className?: string;
  autoFocus: boolean;
}) {
  return (
    <CodeMirror
      ref={editorRef}
      autoFocus={autoFocus}
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

/** Builds a CM6 updateListener that forwards doc changes via a ref. */
function useChangeExtension(
  onChangeRef: React.RefObject<((text: string) => void) | undefined>,
) {
  return useMemo(
    () =>
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return;
        const text = update.state.doc.toString();
        console.warn("[filter-diag] TextEditor onChange updateListener", {
          text,
          hasCallback: !!onChangeRef.current,
        });
        onChangeRef.current?.(text);
      }),
    [onChangeRef],
  );
}

/** Builds a CM6 transactionFilter that rejects edits introducing newlines. */
function useSingleLineGuard(singleLine: boolean): Extension {
  return useMemo(
    () =>
      singleLine
        ? EditorState.transactionFilter.of((tr) => {
            if (!tr.docChanged) return tr;
            const hasNewline = tr.newDoc.toString().includes("\n");
            return hasNewline ? [] : tr;
          })
        : [],
    [singleLine],
  );
}

/**
 * Build a stable-identity extension array backed by Compartments so the pieces
 * that change over time (keymap mode, caller extensions) can be reconfigured
 * imperatively without triggering `@uiw/react-codemirror`'s blanket reconfigure
 * useEffect (which fires on *any* extensions-prop identity change and replaces
 * the entire config — including our updateListeners — mid-typing).
 *
 * Returns the mount-time extension array (stable identity forever) plus the
 * compartments needed to update the pieces. The caller wires effects that
 * dispatch `compartment.reconfigure(...)` when mode / extra extensions change.
 */
function useCompartmentalExtensions(
  mode: string,
  singleLine: boolean,
  onChangeRef: React.RefObject<((text: string) => void) | undefined>,
  extraExtensions: Extension[] | undefined,
  languageExtension: Extension | undefined,
): {
  extensions: Extension[];
  keymapCompartment: Compartment;
  extrasCompartment: Compartment;
} {
  const defaultLanguage = useMemo(
    () => markdown({ base: markdownLanguage }),
    [],
  );
  const changeExtension = useChangeExtension(onChangeRef);
  const singleLineGuard = useSingleLineGuard(singleLine);
  const keymapCompartment = useRef(new Compartment()).current;
  const extrasCompartment = useRef(new Compartment()).current;

  // Captured at mount — IDENTITY NEVER CHANGES. All dynamic updates flow
  // through the two compartments via imperative view.dispatch effects.
  const extensionsRef = useRef<Extension[] | null>(null);
  if (extensionsRef.current === null) {
    extensionsRef.current = [
      keymapCompartment.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      languageExtension ?? defaultLanguage,
      singleLineGuard,
      changeExtension,
      extrasCompartment.of(extraExtensions ?? []),
    ];
  }

  return {
    extensions: extensionsRef.current,
    keymapCompartment,
    extrasCompartment,
  };
}

/**
 * Imperatively reconfigure a compartment when its content changes. Reads the
 * EditorView via the editorRef rather than storing it in React state, so the
 * reconfigure happens outside the React render cycle.
 */
function useCompartmentReconfigure(
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
  compartment: Compartment,
  content: Extension | Extension[],
) {
  useEffect(() => {
    const view = editorRef.current?.view;
    if (!view) return;
    console.warn("[filter-diag] TextEditor compartment RECONFIGURE");
    view.dispatch({ effects: compartment.reconfigure(content) });
  }, [editorRef, compartment, content]);
}

/**
 * Returns an `onCreateEditor` callback that exits vim insert mode on mount.
 *
 * CM6 + vim defaults to insert mode. This exits to normal mode so vim users
 * start in the expected state. Stable across re-renders — no closures over
 * prop callbacks.
 */
function useVimExitInsertOnCreate(mode: string) {
  return useMemo(
    () => (view: EditorView) => {
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
}

/**
 * Exposes `focus()` on the parent-supplied ref.
 *
 * Kept in a hook so the main TextEditor body stays compact.
 */
function useTextEditorHandle(
  ref: React.ForwardedRef<TextEditorHandle>,
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
) {
  useImperativeHandle(
    ref,
    () => ({
      focus() {
        editorRef.current?.view?.focus();
      },
      setValue(text: string) {
        const view = editorRef.current?.view;
        if (!view) return;
        if (view.state.doc.toString() === text) return;
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: text },
        });
      },
    }),
    [editorRef],
  );
}

/** Bundle the CM6 plumbing (refs, extensions, compartment reconfigures). */
function useTextEditorSetup(
  props: TextEditorProps,
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
) {
  const { keymap_mode: mode } = useUIState();
  const onChangeRef = useRef(props.onChange);
  onChangeRef.current = props.onChange;
  const initialValueRef = useRef(props.value);
  const { extensions, keymapCompartment, extrasCompartment } =
    useCompartmentalExtensions(
      mode,
      props.singleLine ?? false,
      onChangeRef,
      props.extensions,
      props.languageExtension,
    );
  useCompartmentReconfigure(
    editorRef,
    keymapCompartment,
    keymapExtension(mode),
  );
  useCompartmentReconfigure(
    editorRef,
    extrasCompartment,
    props.extensions ?? [],
  );
  const handleCreateEditor = useVimExitInsertOnCreate(mode);
  return {
    initialValue: initialValueRef.current,
    extensions,
    handleCreateEditor,
  };
}

/**
 * Pure CM6 editor primitive. See the file-level docstring for the full
 * contract. Accepts only what's needed to host a string buffer; all policy
 * (commit, cancel, submit, blur-save) lives in callers.
 */
export const TextEditor = forwardRef<TextEditorHandle, TextEditorProps>(
  function TextEditor(props, ref) {
    const editorRef = useRef<ReactCodeMirrorRef>(null);
    const { initialValue, extensions, handleCreateEditor } = useTextEditorSetup(
      props,
      editorRef,
    );
    useTextEditorHandle(ref, editorRef);
    return (
      <StableCodeMirror
        editorRef={editorRef}
        initialValue={initialValue}
        onCreateEditor={handleCreateEditor}
        extensions={extensions}
        placeholder={props.placeholder}
        className={props.className ?? "text-sm"}
        autoFocus={props.autoFocus ?? true}
      />
    );
  },
);
