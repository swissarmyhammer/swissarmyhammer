/**
 * Syntax highlighting mapping for the filter DSL grammar.
 *
 * Maps Ref nodes, keyword nodes (`not`, `and`, `or`), operator nodes
 * (`!`, `&&`, `||`), and parentheses to CodeMirror highlight tags so they
 * render in distinct colors within the editor theme.
 *
 * Tag, Mention, and Project nodes are intentionally NOT mapped here — they
 * get their colors from the mention decoration system (colored pills) in
 * `cm-mention-decorations.ts`. Adding them to `styleTags` would cause
 * `defaultHighlightStyle` to override the entity pill colors.
 */

import { styleTags, tags as t } from "@lezer/highlight";

export const highlighting = styleTags({
  // Tag, Mention, and Project are intentionally omitted — they get their visual
  // styling from the mention decoration system (colored pills), not syntax
  // highlighting. Adding them here causes defaultHighlightStyle to override
  // entity colors.
  Ref: t.link,
  "not and or": t.keyword,
  "Bang AmpAmp PipePipe": t.operator,
  "( )": t.paren,
});
