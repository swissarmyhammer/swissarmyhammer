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
 * Vim Enter uses a capture-phase DOM listener on .cm-editor — fires before
 * vim, so we can check vim state and intercept when appropriate.
 *
 * Vim Escape uses a two-phase strategy:
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
  } = opts;

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
      // Escape: two-phase handler on .cm-editor (view.dom).
      //
      // Capture phase: reads vim state BEFORE vim processes the event.
      //   - Normal mode → immediately cancel/exit + stopPropagation.
      //     This fires before vim's own handlers, preventing vim from
      //     consuming the event (which broke the old bubble-only approach).
      //   - Insert mode → record the state, let event continue to vim.
      //
      // Bubble phase: only needed for the insert-mode case.
      //   - Stops propagation so ancestor handlers (backdrop onKeyDown,
      //     global app.dismiss) don't also react to the Escape that vim
      //     just used to exit insert mode.
      //   - Optionally saves in place after vim has processed.
      ViewPlugin.define((view) => {
        let wasInsert = false;
        const capture = (event: KeyboardEvent) => {
          if (event.key !== "Escape") return;
          const cm = getCM(view);
          wasInsert = !!cm?.state?.vim?.insertMode;
          if (!wasInsert) {
            // Normal mode — cancel/exit the editor immediately.
            event.preventDefault();
            event.stopPropagation();
            onCancelRef.current?.();
          }
          // Insert mode — let event through so vim exits to normal.
        };
        const bubble = (event: KeyboardEvent) => {
          if (event.key !== "Escape") return;
          // Stop ancestors from seeing this Escape (e.g. backdrop, app.dismiss).
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
