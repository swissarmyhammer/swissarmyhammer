/**
 * Shared CM6 extension factory for vim-mode-aware submit/cancel semantics.
 *
 * Consumers wire up `onSubmitRef` and `onCancelRef` and get correct behavior
 * across vim, emacs, and CUA keymaps without knowing about mode state.
 *
 * Vim mode:
 *   - Insert Escape → normal mode (internal to CM6, no callback)
 *   - Insert Escape → then calls saveInPlaceRef (optional)
 *   - Normal Escape → onCancelRef
 *   - Normal Enter  → onSubmitRef (if doc is non-empty)
 *
 * CUA/emacs mode:
 *   - Escape → onCancelRef
 *   - Enter  → onSubmitRef
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
}

/**
 * Build CM6 extensions that route Escape and Enter to semantic callbacks,
 * respecting the current keymap mode and vim insert/normal state.
 */
export function buildSubmitCancelExtensions(opts: SubmitCancelOptions): Extension[] {
  const { mode, onSubmitRef, onCancelRef, saveInPlaceRef } = opts;

  if (mode === "vim") {
    return [
      EditorView.domEventHandlers({
        keydown(event, view) {
          if (event.key === "Escape") {
            const cm = getCM(view);
            if (cm?.state?.vim?.insertMode) {
              // In insert mode: let CM6/vim handle Escape (→ normal mode),
              // then save in place if the ref is provided.
              if (saveInPlaceRef?.current) {
                setTimeout(() => saveInPlaceRef.current?.(), 0);
              }
              return false;
            }
            // Normal mode: semantic cancel
            onCancelRef.current?.();
            return true;
          }
          if (event.key === "Enter") {
            const cm = getCM(view);
            if (!cm?.state?.vim?.insertMode) {
              // Normal mode + doc has content: semantic submit
              const text = view.state.doc.toString();
              if (text.length > 0) {
                onSubmitRef.current?.();
                return true;
              }
            }
            // Insert mode: let CM6 handle Enter normally
            return false;
          }
          return false;
        },
      }),
    ];
  }

  // CUA / emacs: simple keymap bindings
  return [
    keymap.of([
      {
        key: "Escape",
        run: () => {
          onCancelRef.current?.();
          return true;
        },
      },
      {
        key: "Enter",
        run: () => {
          onSubmitRef.current?.();
          return true;
        },
      },
    ]),
  ];
}
