/**
 * Shared CM6 extension factory for vim-mode-aware submit/cancel semantics.
 *
 * Vim mode:
 *   - Insert Escape → normal mode (vim handles, we optionally save in place)
 *   - Normal Escape → onCancelRef (via two-phase DOM listener, fires after vim)
 *   - Normal Enter  → onSubmitRef (via capture-phase DOM listener on .cm-editor)
 *   - Insert Enter  → newline (vim handles, we don't intercept)
 *
 * CUA/emacs mode:
 *   - Escape → onCancelRef
 *   - Enter  → onSubmitRef (only when singleLine is true)
 */

import { keymap, ViewPlugin } from "@codemirror/view";
import { Prec, type Extension } from "@codemirror/state";
import { completionStatus } from "@codemirror/autocomplete";
import { getCM } from "@replit/codemirror-vim";

/** Generic ref type — avoids importing React in this utility. */
interface Ref<T> {
  current: T;
}

export interface SubmitCancelOptions {
  /** Active keymap mode: "vim", "cua", or "emacs". */
  mode: string;
  /** Called on semantic submit (normal-mode Enter / CUA Enter). */
  onSubmitRef: Ref<(() => void) | null>;
  /** Called on semantic cancel (normal-mode Escape / CUA Escape). */
  onCancelRef: Ref<(() => void) | null>;
  /** Called when vim exits insert mode (save-in-place). Optional. */
  saveInPlaceRef?: Ref<(() => void) | null>;
  /**
   * When true, Enter triggers submit (single-line input behavior).
   * When false, Enter is not intercepted (multiline editing).
   * Default: true.
   */
  singleLine?: boolean;
  /**
   * When true, Enter always fires onSubmitRef even in vim insert mode.
   * Use for inputs where newlines are never valid (e.g. command palette).
   * Default: false.
   */
  alwaysSubmitOnEnter?: boolean;
}

/**
 * Build CM6 extensions that route Escape and Enter to semantic callbacks.
 *
 * Vim Enter uses one of two strategies:
 *   - alwaysSubmitOnEnter: Prec.highest keymap binding that intercepts Enter
 *     inside CM6's key dispatch, before vim can process it. No newlines ever.
 *   - Default: capture-phase DOM listener that checks vim state — normal mode
 *     submits, insert mode passes through to vim for newline insertion.
 *
 * Vim Escape uses a two-phase DOM listener strategy:
 *   - Capture phase reads vim state and handles normal mode immediately
 *     (cancel + stopPropagation), preventing vim from consuming the event.
 *   - Bubble phase handles the insert-mode case: vim has already exited
 *     insert mode, so we just stopPropagation to prevent ancestor handlers
 *     (backdrop, global app.dismiss) from also reacting.
 *
 * CUA/emacs uses Prec.highest keymap.of bindings.
 */
export function buildSubmitCancelExtensions(
  opts: SubmitCancelOptions,
): Extension[] {
  const {
    mode,
    onSubmitRef,
    onCancelRef,
    saveInPlaceRef,
    singleLine = true,
    alwaysSubmitOnEnter = false,
  } = opts;

  if (mode === "vim") {
    return [
      ...buildVimEnterExtension(singleLine, alwaysSubmitOnEnter, onSubmitRef),
      buildVimEscapeExtension(onCancelRef, saveInPlaceRef),
    ];
  }

  return buildCuaExtensions(singleLine, onSubmitRef, onCancelRef);
}

/**
 * Vim Enter extension — two strategies:
 *
 * alwaysSubmitOnEnter: Prec.highest keymap intercepts Enter before vim.
 * Default: capture-phase DOM listener checks vim state — normal mode submits,
 * insert mode passes through for newline insertion.
 */
function buildVimEnterExtension(
  singleLine: boolean,
  alwaysSubmitOnEnter: boolean,
  onSubmitRef: Ref<(() => void) | null>,
): Extension[] {
  if (!singleLine) return [];

  if (alwaysSubmitOnEnter) {
    return [
      Prec.highest(
        keymap.of([
          {
            key: "Enter",
            run: (view) => {
              // Yield to autocomplete so Enter can accept a selected
              // completion. "pending" counts too: the async source is
              // in-flight and the user is waiting on a suggestion, so
              // Enter must not commit stale partial text.
              if (completionStatus(view.state) !== null) return false;
              const text = view.state.doc.toString();
              if (text.length > 0) onSubmitRef.current?.();
              return true;
            },
          },
        ]),
      ),
    ];
  }

  return [
    ViewPlugin.define((view) => {
      const handler = (event: KeyboardEvent) => {
        if (event.key !== "Enter") return;
        const cm = getCM(view);
        if (cm?.state?.vim?.insertMode) return;
        // Yield to autocomplete so Enter can accept a selected completion.
        // Includes "pending" — see alwaysSubmitOnEnter branch above.
        if (completionStatus(view.state) !== null) return;
        const text = view.state.doc.toString();
        if (text.length > 0) {
          event.preventDefault();
          event.stopPropagation();
          onSubmitRef.current?.();
        }
      };
      view.dom.addEventListener("keydown", handler, true);
      return {
        destroy() {
          view.dom.removeEventListener("keydown", handler, true);
        },
      };
    }),
  ];
}

/**
 * Vim Escape extension — two-phase DOM listener.
 *
 * Capture phase: normal mode → cancel immediately. Insert mode → let vim handle.
 * Bubble phase: stop propagation + optional save-in-place after vim exits insert.
 */
function buildVimEscapeExtension(
  onCancelRef: Ref<(() => void) | null>,
  saveInPlaceRef?: Ref<(() => void) | null>,
): Extension {
  return ViewPlugin.define((view) => {
    let wasInsert = false;
    const capture = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const cm = getCM(view);
      wasInsert = !!cm?.state?.vim?.insertMode;
      if (!wasInsert) {
        event.preventDefault();
        event.stopPropagation();
        onCancelRef.current?.();
      }
    };
    const bubble = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      if (wasInsert && saveInPlaceRef?.current) {
        setTimeout(() => saveInPlaceRef.current?.(), 0);
      }
    };
    view.dom.addEventListener("keydown", capture, true);
    view.dom.addEventListener("keydown", bubble, false);
    return {
      destroy() {
        view.dom.removeEventListener("keydown", capture, true);
        view.dom.removeEventListener("keydown", bubble, false);
      },
    };
  });
}

/** CUA/emacs extensions — Prec.highest keybindings for Escape and (optionally) Enter. */
function buildCuaExtensions(
  singleLine: boolean,
  onSubmitRef: Ref<(() => void) | null>,
  onCancelRef: Ref<(() => void) | null>,
): Extension[] {
  return [
    Prec.highest(
      keymap.of([
        {
          key: "Escape",
          run: () => {
            onCancelRef.current?.();
            return true;
          },
        },
        ...(singleLine
          ? [
              {
                key: "Enter",
                run: (view: import("@codemirror/view").EditorView) => {
                  // Yield to autocomplete so Enter can accept a selected
                  // completion. "pending" counts too: the async source is
                  // in-flight and the user is waiting on a suggestion.
                  if (completionStatus(view.state) !== null) return false;
                  onSubmitRef.current?.();
                  return true;
                },
              },
            ]
          : []),
      ]),
    ),
  ];
}
