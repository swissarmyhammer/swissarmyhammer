/**
 * Generic CM6 mention decorations — parameterized by prefix character
 * and CSS class. Highlights known `prefix+slug` patterns as colored pills.
 *
 * When metadata is available for a slug (via metaFacet), mentions render as
 * replacement widgets showing the entity's display name. When the cursor is
 * inside or adjacent to a mention, the widget degrades to a mark decoration
 * so the user can edit the raw slug text.
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
import { type MentionMeta, sanitizeHexColor } from "@/lib/mention-meta";
import { MentionWidget } from "@/lib/cm-mention-widget";

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
 * Check whether a document position is inside or adjacent to a range.
 *
 * Used to decide if the cursor is "touching" a mention, in which case
 * we show the raw slug instead of the widget so the user can edit it.
 */
function isCursorNear(from: number, to: number, head: number): boolean {
  return head >= from - 1 && head <= to + 1;
}

/**
 * Collect mention decorations for a single line of text.
 *
 * For each mention hit:
 * - If the slug has metadata with a color AND the cursor is not near the
 *   mention range, emit a `Decoration.replace` with a `MentionWidget`.
 * - If the cursor IS near the range (and the state is editable), emit a
 *   `Decoration.mark` so the raw slug text is visible and editable.
 * - If the state is read-only, the cursor-nearness check is skipped so
 *   widgets always render (there's no editing concern to protect).
 * - If the slug has no color (stale/unknown), emit the `defaultMark`.
 */
function decorateLine(
  text: string,
  lineFrom: number,
  prefix: string,
  slugs: string[],
  meta: Map<string, MentionMeta>,
  cssClass: string,
  colorVar: string,
  defaultMark: Decoration,
  selectionHead: number,
  readOnly: boolean,
  out: PositionedDecoration[],
): void {
  const codeRanges = findInlineCodeRanges(text);
  const hits = findMentionsInText(text, prefix, slugs);
  for (const hit of hits) {
    const start = hit.index;
    const end = start + hit.length;
    if (codeRanges.some(([a, b]) => start >= a && end <= b)) continue;

    const from = lineFrom + start;
    const to = lineFrom + end;
    const info = meta.get(hit.slug);
    const color = info ? sanitizeHexColor(info.color) : "";

    if (!info || !color) {
      // No metadata or invalid/missing color — muted mark on raw slug
      out.push({ from, to, decoration: defaultMark });
    } else if (!readOnly && isCursorNear(from, to, selectionHead)) {
      // Editable state with cursor touching this mention —
      // show raw slug with colored mark so the user can edit.
      out.push({
        from,
        to,
        decoration: Decoration.mark({
          class: cssClass,
          attributes: { style: `${colorVar}: #${color}` },
        }),
      });
    } else {
      // Read-only state, or cursor is elsewhere —
      // replace slug text with the display-name widget.
      out.push({
        from,
        to,
        decoration: Decoration.replace({
          widget: new MentionWidget(prefix, hit.slug, info.displayName, color),
          inclusive: false,
        }),
      });
    }
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
  meta: Map<string, MentionMeta>,
  prefix: string,
  cssClass: string,
  colorVar: string,
  defaultMark: Decoration,
): DecorationSet {
  const out: PositionedDecoration[] = [];
  const doc = view.state.doc;
  const slugs = Array.from(meta.keys());
  let inFence = false;

  const selectionHead = view.state.selection.main.head;
  const readOnly = view.state.readOnly;
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
      meta,
      cssClass,
      colorVar,
      defaultMark,
      selectionHead,
      readOnly,
      out,
    );
  }

  out.sort((a, b) => a.from - b.from || a.to - b.to);
  return Decoration.set(out.map((d) => d.decoration.range(d.from, d.to)));
}

/**
 * Common pill appearance rules — shared between the parameterized mark
 * class (e.g. `.cm-tag-pill`) and the widget class `.cm-mention-pill`.
 *
 * Accepts any CSS custom-property name so the same shape can drive either
 * a prefix-specific color (`--tag-color`) or the widget's `--mention-color`.
 */
function pillStyles(colorVar: string) {
  return {
    backgroundColor: `color-mix(in srgb, var(${colorVar}, #888) 20%, transparent)`,
    color: `var(${colorVar}, #888)`,
    border: `1px solid color-mix(in srgb, var(${colorVar}, #888) 30%, transparent)`,
    borderRadius: "9999px",
    padding: "0 6px 1px",
    fontSize: "0.75rem",
    fontWeight: "500",
  };
}

/**
 * Build the CM6 baseTheme for mention pill styling.
 *
 * Styles both the mark-decoration class (used for cursor-adjacent and
 * stale mentions) and the widget class `.cm-mention-pill` (used for
 * replace-widget mentions). Both share the same pill appearance via
 * shared `pillStyles` rules.
 */
function buildMentionTheme(cssClass: string, colorVar: string) {
  return EditorView.baseTheme({
    [`.${cssClass}`]: pillStyles(colorVar),
    ".cm-mention-pill": {
      ...pillStyles("--mention-color"),
      display: "inline-flex",
    },
  });
}

/** Create a ViewPlugin that rebuilds decorations on doc/viewport changes. */
function buildMentionPlugin(
  metaFacet: Facet<Map<string, MentionMeta>, Map<string, MentionMeta>>,
  prefix: string,
  cssClass: string,
  colorVar: string,
  defaultMark: Decoration,
) {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        const meta = view.state.facet(metaFacet);
        this.decorations = buildDecorations(
          view,
          meta,
          prefix,
          cssClass,
          colorVar,
          defaultMark,
        );
      }

      update(update: ViewUpdate) {
        if (
          update.docChanged ||
          update.viewportChanged ||
          update.selectionSet
        ) {
          const meta = update.state.facet(metaFacet);
          this.decorations = buildDecorations(
            update.view,
            meta,
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
}

/**
 * Create a mention decoration extension bundle for a given prefix.
 *
 * @param prefix - The mention prefix character (e.g. `#`, `@`)
 * @param cssClass - CSS class applied to decorated mentions
 * @param colorVar - CSS custom property name for the color
 */
export function createMentionDecorations(
  prefix: string,
  cssClass: string,
  colorVar: string,
) {
  const metaFacet = Facet.define<
    Map<string, MentionMeta>,
    Map<string, MentionMeta>
  >({
    combine(values) {
      return values.length > 0 ? values[values.length - 1] : new Map();
    },
  });

  const defaultMark = Decoration.mark({ class: cssClass });
  const plugin = buildMentionPlugin(
    metaFacet,
    prefix,
    cssClass,
    colorVar,
    defaultMark,
  );
  const theme = buildMentionTheme(cssClass, colorVar);

  /**
   * `atomicRanges` makes arrow-key navigation skip over replace-widget
   * decorations in a single step. Reads the decoration set that the
   * plugin already computes, so we don't pay the rebuild cost twice.
   *
   * When the cursor approaches a widget, the decoration system degrades
   * it to a mark (via `isCursorNear`), so atomic ranges do not interfere
   * with character-by-character editing of mention text.
   */
  const atomic = EditorView.atomicRanges.of((view) => {
    return view.plugin(plugin)?.decorations ?? Decoration.none;
  });

  return {
    metaFacet,
    /** Extension bundle: pass metadata as Map<slug, MentionMeta> */
    extension(meta: Map<string, MentionMeta>) {
      return [metaFacet.of(meta), plugin, theme, atomic];
    },
  };
}
