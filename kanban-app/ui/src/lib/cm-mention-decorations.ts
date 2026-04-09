/**
 * Generic CM6 mention decorations — parameterized by prefix character
 * and CSS class. Highlights known `prefix+slug` patterns as colored pills.
 */

import { Facet } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";
import { findMentionsInText } from "@/lib/mention-finder";

/** Lines starting with ``` (fenced code block delimiters) */
const FENCE_RE = /^```/;

/** Inline code span boundaries */
const INLINE_CODE_RE = /`[^`]+`/g;

/** Markdown heading prefix */
const HEADING_RE = /^#{1,6}\s/;

/** A positioned decoration ready for sorting and conversion to a RangeSet. */
interface PositionedDecoration {
  from: number;
  to: number;
  decoration: Decoration;
}

/**
 * Find inline code ranges in a line of text.
 *
 * Returns an array of [start, end] tuples marking backtick-delimited spans
 * so mention decorations can skip them.
 */
function findInlineCodeRanges(text: string): [number, number][] {
  const ranges: [number, number][] = [];
  INLINE_CODE_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = INLINE_CODE_RE.exec(text)) !== null) {
    ranges.push([m.index, m.index + m[0].length]);
  }
  return ranges;
}

/**
 * Collect mention decorations for a single line of text.
 *
 * Skips mentions that fall inside inline code spans. Pushes colored or
 * default-colored decoration entries into the `out` array.
 */
function decorateLine(
  text: string,
  lineFrom: number,
  prefix: string,
  slugs: string[],
  colors: Map<string, string>,
  cssClass: string,
  colorVar: string,
  defaultMark: Decoration,
  out: PositionedDecoration[],
): void {
  const codeRanges = findInlineCodeRanges(text);
  const hits = findMentionsInText(text, prefix, slugs);
  for (const hit of hits) {
    const start = hit.index;
    const end = start + hit.length;
    if (codeRanges.some(([a, b]) => start >= a && end <= b)) continue;

    const color = colors.get(hit.slug);
    out.push({
      from: lineFrom + start,
      to: lineFrom + end,
      decoration: color
        ? Decoration.mark({
            class: cssClass,
            attributes: { style: `${colorVar}: #${color}` },
          })
        : defaultMark,
    });
  }
}

/**
 * Scan the entire document for mention patterns and build a DecorationSet.
 *
 * Skips fenced code blocks and markdown headings. Delegates per-line work
 * to `decorateLine`.
 */
function buildDecorations(
  view: EditorView,
  colors: Map<string, string>,
  prefix: string,
  cssClass: string,
  colorVar: string,
  defaultMark: Decoration,
): DecorationSet {
  const out: PositionedDecoration[] = [];
  const doc = view.state.doc;
  const slugs = Array.from(colors.keys());
  let inFence = false;

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    if (FENCE_RE.test(line.text)) {
      inFence = !inFence;
      continue;
    }
    if (inFence || HEADING_RE.test(line.text)) continue;
    decorateLine(
      line.text,
      line.from,
      prefix,
      slugs,
      colors,
      cssClass,
      colorVar,
      defaultMark,
      out,
    );
  }

  out.sort((a, b) => a.from - b.from || a.to - b.to);
  return Decoration.set(out.map((d) => d.decoration.range(d.from, d.to)));
}

/**
 * Build the CM6 baseTheme for mention pill styling.
 *
 * Matches the `MentionPillInner` React component: fully rounded pill with
 * colored background, border, and text.
 */
function buildMentionTheme(cssClass: string, colorVar: string) {
  return EditorView.baseTheme({
    [`.${cssClass}`]: {
      backgroundColor: `color-mix(in srgb, var(${colorVar}, #888) 20%, transparent)`,
      color: `var(${colorVar}, #888)`,
      border: `1px solid color-mix(in srgb, var(${colorVar}, #888) 30%, transparent)`,
      borderRadius: "9999px",
      padding: "0 6px 1px",
      fontSize: "0.75rem",
      fontWeight: "500",
    },
  });
}

/**
 * Create a mention decoration extension bundle for a given prefix.
 *
 * @param prefix - The mention prefix character (e.g. `#`, `@`)
 * @param cssClass - CSS class applied to decorated mentions (e.g. `cm-tag-pill`, `cm-actor-pill`)
 * @param colorVar - CSS custom property name for the color (e.g. `--tag-color`, `--actor-color`)
 */
export function createMentionDecorations(
  prefix: string,
  cssClass: string,
  colorVar: string,
) {
  /** Facet providing a map of slug → hex color (without #) */
  const colorsFacet = Facet.define<Map<string, string>, Map<string, string>>({
    combine(values) {
      return values.length > 0 ? values[values.length - 1] : new Map();
    },
  });

  const defaultMark = Decoration.mark({ class: cssClass });

  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        const colors = view.state.facet(colorsFacet);
        this.decorations = buildDecorations(
          view,
          colors,
          prefix,
          cssClass,
          colorVar,
          defaultMark,
        );
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged) {
          const colors = update.state.facet(colorsFacet);
          this.decorations = buildDecorations(
            update.view,
            colors,
            prefix,
            cssClass,
            colorVar,
            defaultMark,
          );
        }
      }
    },
    { decorations: (v) => v.decorations },
  );

  const theme = buildMentionTheme(cssClass, colorVar);

  return {
    colorsFacet,
    /** Extension bundle: pass colors as Map<slug, hexColor> (without #) */
    extension(colors: Map<string, string>) {
      return [colorsFacet.of(colors), plugin, theme];
    },
  };
}
