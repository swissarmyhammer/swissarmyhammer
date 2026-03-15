/**
 * Shared CM6 extension factory for vim-mode-aware submit/cancel semantics.
 *
 * Vim mode: Uses Vim.defineAction + Vim.mapCommand to register normal-mode
 * Enter/Escape handlers inside vim's own key dispatch. A WeakMap routes
 * the global actions to per-editor callbacks.
 *
 * CUA/emacs mode: keymap.of bindings for Enter/Escape.
 *
 * Vim mode:
 *   - Insert Escape → normal mode (vim handles; save-in-place via vim-mode-change signal)
 *   - Normal Escape → onCancelRef
 *   - Normal Enter  → onSubmitRef (if singleLine and doc is non-empty)
 *
 * CUA/emacs mode:
 *   - Escape → onCancelRef
 *   - Enter  → onSubmitRef (only when singleLine is true)
 */

import { keymap, ViewPlugin } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { CodeMirror as CM, Vim, getCM } from "@replit/codemirror-vim";

// CodeMirror.on/off are runtime methods not in the .d.ts — cast once here.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const CodeMirror = CM as any;

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

// ---------------------------------------------------------------------------
// Per-editor callback registry (keyed by CM5 adapter from getCM)
// ---------------------------------------------------------------------------

interface EditorCallbacks {
  onSubmitRef: Ref<(() => void) | null>;
  onCancelRef: Ref<(() => void) | null>;
  saveInPlaceRef?: Ref<(() => void) | null>;
  singleLine: boolean;
}

const editorCallbacks = new WeakMap<object, EditorCallbacks>();

// ---------------------------------------------------------------------------
// One-time global vim action + mapping registration
// ---------------------------------------------------------------------------

let vimActionsRegistered = false;

function ensureVimActions() {
  if (vimActionsRegistered) return;
  vimActionsRegistered = true;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  Vim.defineAction("submit", (cm: any) => {
    const entry = editorCallbacks.get(cm);
    if (!entry?.singleLine) return;
    // Only submit if doc has content — read from CM6 view
    const view = cm?.cm6;
    const text = view?.state?.doc?.toString() ?? "";
    if (text.length > 0) {
      entry.onSubmitRef.current?.();
    }
  });

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  Vim.defineAction("cancel", (cm: any) => {
    const entry = editorCallbacks.get(cm);
    entry?.onCancelRef.current?.();
  });

  Vim.mapCommand("<CR>", "action", "submit", undefined, { context: "normal" });
  Vim.mapCommand("<Esc>", "action", "cancel", undefined, { context: "normal" });
}

/**
 * Build CM6 extensions that route Escape and Enter to semantic callbacks.
 *
 * Vim mode uses Vim.defineAction + Vim.mapCommand so that Enter/Escape in
 * normal mode are handled inside vim's own key dispatch — no DOM-level race.
 * A ViewPlugin registers/unregisters the per-editor callbacks in a WeakMap.
 *
 * CUA/emacs mode uses standard CM6 keymap.of bindings.
 */
export function buildSubmitCancelExtensions(opts: SubmitCancelOptions): Extension[] {
  const { mode, onSubmitRef, onCancelRef, saveInPlaceRef, singleLine = true } = opts;

  if (mode === "vim") {
    ensureVimActions();

    return [
      ViewPlugin.define((view) => {
        const cm = getCM(view);

        // Register this editor's callbacks in the global WeakMap
        if (cm) {
          editorCallbacks.set(cm, { onSubmitRef, onCancelRef, saveInPlaceRef, singleLine });
        }

        // Listen for vim-mode-change to trigger save-in-place on insert → normal
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const onModeChange = (event: any) => {
          if (event?.mode === "normal" && saveInPlaceRef?.current) {
            saveInPlaceRef.current();
          }
        };
        if (cm) {
          CodeMirror.on(cm, "vim-mode-change", onModeChange);
        }

        return {
          destroy() {
            if (cm) {
              editorCallbacks.delete(cm);
              CodeMirror.off(cm, "vim-mode-change", onModeChange);
            }
          },
        };
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
