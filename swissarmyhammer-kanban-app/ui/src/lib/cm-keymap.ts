/**
 * Shared CodeMirror 6 theme and keymap utilities.
 *
 * Every CM6 editor in the app should use these to ensure consistent
 * appearance and keymap behavior (vim/emacs/CUA).
 */

import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { vim } from "@replit/codemirror-vim";
import { emacs } from "@replit/codemirror-emacs";

/** Minimal CM6 theme — transparent background, no chrome. */
export const minimalTheme = EditorView.theme({
  "&": { backgroundColor: "transparent", fontFamily: "inherit", fontSize: "inherit" },
  ".cm-gutters": { display: "none" },
  ".cm-content": { padding: "0" },
  "&.cm-focused": { outline: "none" },
  ".cm-line": { padding: "0" },
  ".cm-scroller": { overflow: "auto" },
});

/** Build keymap extension for the given editor mode. */
export function keymapExtension(mode: string): Extension | Extension[] {
  switch (mode) {
    case "vim":
      return vim();
    case "emacs":
      return emacs();
    default:
      return [];
  }
}
