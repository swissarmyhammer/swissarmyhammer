/**
 * Shared CM6 extension factory for vim-mode-aware submit/cancel semantics.
 *
 * Vim mode: EditorView.domEventHandlers checks vim state for Enter/Escape.
 * CUA/emacs mode: keymap.of bindings for Enter/Escape.
 *
 * Vim mode:
 *   - Insert Escape → normal mode (vim handles, we optionally save in place)
 *   - Normal Escape → onCancelRef
 *   - Normal Enter  → onSubmitRef (if singleLine and doc is non-empty)
 *
 * CUA/emacs mode:
 *   - Escape → onCancelRef
 *   - Enter  → onSubmitRef (only when singleLine is true)
 */

import { keymap, EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { getCM } from "@replit/codemirror-vim";

/** Generic ref type — avoids importing React in this utility. */
interface Ref<T> {
  current: T;
}

export interface SubmitCancelOptions {
  /** Active keymap mode: "vim", "cua", or "emacs". */
  mode: string;
  /** Called on semantic submit (Enter in normal mode / CUA). */
  onSubmitRef: Ref<(() => void) | null>;
  /** Called on semantic cancel (Escape in normal mode / CUA). */
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
 * This is the same pattern that worked in the original FieldPlaceholderEditor
 * before the refactor — EditorView.domEventHandlers for vim, keymap.of for CUA.
 */
export function buildSubmitCancelExtensions(opts: SubmitCancelOptions): Extension[] {
  const { mode, onSubmitRef, onCancelRef, saveInPlaceRef, singleLine = true } = opts;

  if (mode === "vim") {
    return [
      EditorView.domEventHandlers({
        keydown(event, view) {
          if (event.key === "Escape") {
            const cm = getCM(view);
            if (cm?.state?.vim?.insertMode) {
              // Insert mode: let vim handle Escape (→ normal mode),
              // then save in place if provided.
              if (saveInPlaceRef?.current) {
                setTimeout(() => saveInPlaceRef.current?.(), 0);
              }
              return false;
            }
            // Normal mode: cancel
            onCancelRef.current?.();
            return true;
          }
          if (event.key === "Enter" && singleLine) {
            const cm = getCM(view);
            if (!cm?.state?.vim?.insertMode) {
              // Normal mode: submit if doc has content
              const text = view.state.doc.toString();
              if (text.length > 0) {
                onSubmitRef.current?.();
                return true;
              }
            }
            return false;
          }
          return false;
        },
      }),
    ];
  }

  // CUA / emacs
  return [
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
  ];
}
