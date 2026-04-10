/**
 * Syntax highlighting mapping for the filter DSL grammar.
 *
 * Maps grammar node types to CodeMirror highlight tags so that tags, mentions,
 * refs, and operators each render in distinct colors within the editor theme.
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
