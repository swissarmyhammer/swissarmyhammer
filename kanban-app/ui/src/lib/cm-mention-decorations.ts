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

  function buildDecorations(
    view: EditorView,
    colors: Map<string, string>,
  ): DecorationSet {
    const decorations: {
      from: number;
      to: number;
      decoration: typeof defaultMark;
    }[] = [];
    const doc = view.state.doc;
    const slugs = Array.from(colors.keys());
    let inFence = false;

    for (let i = 1; i <= doc.lines; i++) {
      const line = doc.line(i);
      const text = line.text;

      if (FENCE_RE.test(text)) {
        inFence = !inFence;
        continue;
      }
      if (inFence) continue;
      if (HEADING_RE.test(text)) continue;

      const codeRanges: [number, number][] = [];
      INLINE_CODE_RE.lastIndex = 0;
      let codeMatch: RegExpExecArray | null;
      while ((codeMatch = INLINE_CODE_RE.exec(text)) !== null) {
        codeRanges.push([
          codeMatch.index,
          codeMatch.index + codeMatch[0].length,
        ]);
      }

      const hits = findMentionsInText(text, prefix, slugs);
      for (const hit of hits) {
        const start = hit.index;
        const end = start + hit.length;

        const inCode = codeRanges.some(([a, b]) => start >= a && end <= b);
        if (inCode) continue;

        const color = colors.get(hit.slug);
        if (color) {
          decorations.push({
            from: line.from + start,
            to: line.from + end,
            decoration: Decoration.mark({
              class: cssClass,
              attributes: { style: `${colorVar}: #${color}` },
            }),
          });
        } else {
          decorations.push({
            from: line.from + start,
            to: line.from + end,
            decoration: defaultMark,
          });
        }
      }
    }

    decorations.sort((a, b) => a.from - b.from || a.to - b.to);
    return Decoration.set(
      decorations.map((d) => d.decoration.range(d.from, d.to)),
    );
  }

  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        const colors = view.state.facet(colorsFacet);
        this.decorations = buildDecorations(view, colors);
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged) {
          const colors = update.state.facet(colorsFacet);
          this.decorations = buildDecorations(update.view, colors);
        }
      }
    },
    {
      decorations: (v) => v.decorations,
    },
  );

  const theme = EditorView.baseTheme({
    [`.${cssClass}`]: {
      backgroundColor: `color-mix(in srgb, var(${colorVar}, #888) 20%, transparent)`,
      color: `var(${colorVar}, #888)`,
      borderRadius: "3px",
      padding: "0 3px",
      fontWeight: "500",
    },
  });

  return {
    colorsFacet,
    /** Extension bundle: pass colors as Map<slug, hexColor> (without #) */
    extension(colors: Map<string, string>) {
      return [colorsFacet.of(colors), plugin, theme];
    },
  };
}
