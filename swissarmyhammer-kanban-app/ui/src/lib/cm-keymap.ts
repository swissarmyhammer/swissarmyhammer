/**
 * Shared CodeMirror 6 theme and keymap utilities.
 *
 * Every CM6 editor in the app should use these to ensure consistent
 * appearance and keymap behavior (vim/emacs/CUA).
 *
 * The theme is a proper EditorView.theme() that matches the shadcn/ui
 * design system. Per CM6 docs, EditorView.theme() generates scoped
 * selectors with higher specificity than baseTheme, so our styles
 * override CM6's built-in monospace on .cm-scroller.
 *
 * Pass as the `theme` prop on @uiw/react-codemirror (not as an extension).
 */

import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { vim } from "@replit/codemirror-vim";
import { emacs } from "@replit/codemirror-emacs";

/** The app's font stack — must match body in index.css. */
const FONT = 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

/**
 * CM6 theme derived from shadcn/ui CSS variables.
 *
 * CM6's base theme sets `fontFamily: "monospace"` on `.cm-scroller`.
 * EditorView.theme() selectors have higher specificity than baseTheme
 * selectors, so targeting `.cm-scroller` here cleanly overrides it.
 *
 * Pass this as the `theme` prop on the CodeMirror component.
 */
export const shadcnTheme = EditorView.theme({
  // Editor root
  "&": {
    backgroundColor: "transparent",
    color: "var(--foreground)",
  },
  "&.cm-focused": {
    outline: "none",
  },
  // Override view baseTheme: .cm-scroller sets fontFamily: "monospace"
  ".cm-scroller": {
    fontFamily: FONT,
    lineHeight: "inherit",
    overflow: "auto",
  },
  ".cm-content": {
    padding: "0",
    caretColor: "var(--foreground)",
  },
  ".cm-line": {
    padding: "0",
  },
  ".cm-gutters": {
    display: "none",
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
  // Override view baseTheme tooltip styles
  ".cm-tooltip": {
    backgroundColor: "var(--popover)",
    color: "var(--popover-foreground)",
    border: "1px solid var(--border)",
    borderRadius: "calc(var(--radius) - 2px)",
    fontFamily: FONT,
  },
  // Override autocomplete baseTheme: .cm-tooltip.cm-tooltip-autocomplete > ul
  // sets fontFamily: "monospace" — must match the compound selector structure
  ".cm-tooltip.cm-tooltip-autocomplete": {
    "& > ul": {
      fontFamily: FONT,
    },
  },
  ".cm-tooltip-autocomplete ul li[aria-selected]": {
    backgroundColor: "var(--accent)",
    color: "var(--accent-foreground)",
  },
  // Override autocomplete baseTheme: .cm-tooltip.cm-completionInfo
  ".cm-tooltip.cm-completionInfo": {
    fontFamily: FONT,
  },
});

/** @deprecated Use shadcnTheme instead */
export const minimalTheme = shadcnTheme;

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
