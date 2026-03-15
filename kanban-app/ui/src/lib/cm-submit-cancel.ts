/**
 * Shared CM6 extension factory for vim-mode-aware submit/cancel semantics.
 *
 * Vim mode: uses Vim.defineAction + Vim.mapCommand to register normal-mode
 * keybindings that vim handles natively. No DOM hacks needed.
 *
 * CUA/emacs mode: uses Prec.highest + EditorView.domEventHandlers.
 *
 * Vim mode:
 *   - Insert Escape → normal mode (vim built-in)
 *   - Normal Escape → onCancelRef
 *   - Normal Enter  → onSubmitRef (if doc is non-empty)
 *   - Insert→Normal transition → saveInPlaceRef (optional)
 *
 * CUA/emacs mode:
 *   - Escape → onCancelRef
 *   - Enter  → onSubmitRef (only when singleLine is true)
 */

import { EditorView } from "@codemirror/view";
import { Prec, type Extension } from "@codemirror/state";
import { Vim, getCM } from "@replit/codemirror-vim";

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
   * When true, Enter always triggers submit (single-line input behavior).
   * When false, Enter only submits in vim normal mode; in CUA/emacs it
   * inserts a newline as normal (multiline editing).
   * Default: true.
   */
  singleLine?: boolean;
}

// Counter to generate unique action names per editor instance
let actionCounter = 0;

/**
 * Build CM6 extensions that route Escape and Enter to semantic callbacks,
 * respecting the current keymap mode and vim insert/normal state.
 */
export function buildSubmitCancelExtensions(opts: SubmitCancelOptions): Extension[] {
  const { mode, onSubmitRef, onCancelRef, saveInPlaceRef, singleLine = true } = opts;

  if (mode === "vim") {
    // Use unique action names so multiple editors don't collide
    const id = ++actionCounter;
    const cancelAction = `_sc_cancel_${id}`;
    const submitAction = `_sc_submit_${id}`;

    // Define vim actions that call our refs
    Vim.defineAction(cancelAction, () => {
      onCancelRef.current?.();
    });

    Vim.defineAction(submitAction, (cm) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const view = (cm as any)?.cm as EditorView | undefined;
      const text = view ? view.state.doc.toString() : "";
      if (text.length > 0) {
        onSubmitRef.current?.();
      }
    });

    // Map Escape and Enter in normal mode to our actions
    Vim.mapCommand("<Esc>", "action", cancelAction, {}, { context: "normal" });
    Vim.mapCommand("<CR>", "action", submitAction, {}, { context: "normal" });

    // Handle insert→normal transition for save-in-place
    if (saveInPlaceRef) {
      return [
        EditorView.domEventHandlers({
          keydown(event, view) {
            if (event.key === "Escape") {
              const cm = getCM(view);
              if (cm?.state?.vim?.insertMode && saveInPlaceRef.current) {
                setTimeout(() => saveInPlaceRef.current?.(), 0);
              }
            }
            // Never consume — let vim handle everything
            return false;
          },
        }),
      ];
    }

    return [];
  }

  // CUA / emacs: DOM-level handlers with highest precedence
  return [
    Prec.highest(
      EditorView.domEventHandlers({
        keydown(event) {
          if (event.key === "Escape") {
            onCancelRef.current?.();
            return true;
          }
          if (event.key === "Enter" && singleLine) {
            onSubmitRef.current?.();
            return true;
          }
          return false;
        },
      }),
    ),
  ];
}
