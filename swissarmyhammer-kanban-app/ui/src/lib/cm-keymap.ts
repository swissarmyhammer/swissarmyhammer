/**
 * Shared CodeMirror 6 theme and keymap utilities.
 *
 * Every CM6 editor in the app should use these to ensure consistent
 * appearance and keymap behavior (vim/emacs/CUA).
 *
 * The theme uses shadcn/ui CSS variables so CM6 editors blend seamlessly
 * with the rest of the app. The `theme="none"` prop on @uiw/react-codemirror
 * disables its built-in theme, letting this be the sole styling source.
 */

import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { vim } from "@replit/codemirror-vim";
import { emacs } from "@replit/codemirror-emacs";

/**
 * CM6 theme matching the shadcn/ui design system.
 *
 * Uses CSS variables from index.css so it adapts to light/dark mode.
 * Must be paired with `theme="none"` on the CodeMirror component
 * to prevent @uiw/react-codemirror from injecting its default
 * monospace theme.
 */
export const minimalTheme = EditorView.theme({
  "&": {
    backgroundColor: "transparent",
    color: "var(--foreground)",
    fontFamily: "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
    fontSize: "inherit",
    fontWeight: "inherit",
    letterSpacing: "inherit",
    lineHeight: "inherit",
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-gutters": {
    display: "none",
  },
  ".cm-content": {
    padding: "0",
    caretColor: "var(--foreground)",
  },
  ".cm-line": {
    padding: "0",
  },
  ".cm-scroller": {
    overflow: "auto",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--foreground)",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
    backgroundColor: "var(--accent)",
  },
  ".cm-activeLine": {
    backgroundColor: "transparent",
  },
  ".cm-selectionMatch": {
    backgroundColor: "var(--accent)",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--popover)",
    color: "var(--popover-foreground)",
    border: "1px solid var(--border)",
    borderRadius: "calc(var(--radius) - 2px)",
  },
  ".cm-tooltip-autocomplete ul li[aria-selected]": {
    backgroundColor: "var(--accent)",
    color: "var(--accent-foreground)",
  },
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
