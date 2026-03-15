/**
 * Shared CM6 extension factory for vim-mode-aware submit/cancel semantics.
 *
 * Vim mode:
 *   - Insert Escape → normal mode (vim handles, we optionally save in place)
 *   - Normal Escape → onCancelRef (via domEventHandlers — vim doesn't consume Esc)
 *   - Normal Enter  → onSubmitRef (via capture-phase DOM listener on .cm-editor)
 *   - Insert Enter  → newline (vim handles, we don't intercept)
 *
 * CUA/emacs mode:
 *   - Escape → onCancelRef
 *   - Enter  → onSubmitRef (only when singleLine is true)
 */

import { keymap, EditorView, ViewPlugin } from "@codemirror/view";
import { Prec, type Extension } from "@codemirror/state";
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
}

/**
 * Build CM6 extensions that route Escape and Enter to semantic callbacks.
 *
 * Vim Enter uses a capture-phase DOM keydown listener attached directly
 * to the .cm-editor element. This fires BEFORE CM6/vim's own event
 * processing. We check vim state and preventDefault+stopPropagation
 * if we handle it, so vim never sees the event.
 *
 * Vim Escape uses CM6 domEventHandlers (vim doesn't consume Esc in
 * normal mode, so our handler runs fine).
 *
 * CUA/emacs uses Prec.highest keymap.of bindings.
 */
export function buildSubmitCancelExtensions(opts: SubmitCancelOptions): Extension[] {
  const { mode, onSubmitRef, onCancelRef, saveInPlaceRef, singleLine = true } = opts;

  if (mode === "vim") {
    return [
      // Enter: capture-phase DOM listener beats vim's event processing.
      ...(singleLine
        ? [
            ViewPlugin.define((view) => {
              const handler = (event: KeyboardEvent) => {
                if (event.key !== "Enter") return;
                const cm = getCM(view);
                if (cm?.state?.vim?.insertMode) return; // let vim insert newline
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
          ]
        : []),
      // Escape: domEventHandlers (vim doesn't consume Esc in normal mode)
      EditorView.domEventHandlers({
        keydown(event, view) {
          if (event.key === "Escape") {
            const cm = getCM(view);
            if (cm?.state?.vim?.insertMode) {
              if (saveInPlaceRef?.current) {
                setTimeout(() => saveInPlaceRef.current?.(), 0);
              }
              return false;
            }
            onCancelRef.current?.();
            return true;
          }
          return false;
        },
      }),
    ];
  }

  // CUA / emacs — Prec.highest to beat markdown extension's Enter
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
                run: () => {
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
