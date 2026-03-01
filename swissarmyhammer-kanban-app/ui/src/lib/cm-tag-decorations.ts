import { Facet } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";
import { findTagsInText } from "@/lib/tag-finder";

/** Lines starting with ``` (fenced code block delimiters) */
const FENCE_RE = /^```/;

/** Inline code span boundaries */
const INLINE_CODE_RE = /`[^`]+`/g;

/** Markdown heading prefix */
const HEADING_RE = /^#{1,6}\s/;

/** Facet providing a map of tag slug → hex color (without #) */
export const tagColorsFacet = Facet.define<
  Map<string, string>,
  Map<string, string>
>({
  combine(values) {
    return values.length > 0 ? values[values.length - 1] : new Map();
  },
});

/** CSS class for decorated tags */
const tagMark = Decoration.mark({ class: "cm-tag-pill" });

/** Build tag pill decorations using known slugs from the facet */
function buildDecorations(
  view: EditorView,
  colors: Map<string, string>
): DecorationSet {
  const decorations: { from: number; to: number; decoration: typeof tagMark }[] = [];
  const doc = view.state.doc;
  const slugs = Array.from(colors.keys());
  let inFence = false;

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    const text = line.text;

    // Track fenced code blocks
    if (FENCE_RE.test(text)) {
      inFence = !inFence;
      continue;
    }
    if (inFence) continue;

    // Skip heading lines
    if (HEADING_RE.test(text)) continue;

    // Collect inline code ranges to skip
    const codeRanges: [number, number][] = [];
    INLINE_CODE_RE.lastIndex = 0;
    let codeMatch: RegExpExecArray | null;
    while ((codeMatch = INLINE_CODE_RE.exec(text)) !== null) {
      codeRanges.push([codeMatch.index, codeMatch.index + codeMatch[0].length]);
    }

    // Find known tags by literal matching
    const hits = findTagsInText(text, slugs);
    for (const hit of hits) {
      const start = hit.index;
      const end = start + hit.length;

      // Skip if inside inline code
      const inCode = codeRanges.some(([a, b]) => start >= a && end <= b);
      if (inCode) continue;

      const color = colors.get(hit.slug);
      if (color) {
        decorations.push({
          from: line.from + start,
          to: line.from + end,
          decoration: Decoration.mark({
            class: "cm-tag-pill",
            attributes: { style: `--tag-color: #${color}` },
          }),
        });
      } else {
        decorations.push({
          from: line.from + start,
          to: line.from + end,
          decoration: tagMark,
        });
      }
    }
  }

  // Sort by position (required by RangeSet)
  decorations.sort((a, b) => a.from - b.from || a.to - b.to);
  return Decoration.set(decorations.map((d) => d.decoration.range(d.from, d.to)));
}

/** ViewPlugin that highlights known #tag patterns with colored pill styling */
const tagDecorationPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      const colors = view.state.facet(tagColorsFacet);
      this.decorations = buildDecorations(view, colors);
    }

    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        const colors = update.state.facet(tagColorsFacet);
        this.decorations = buildDecorations(update.view, colors);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  }
);

/** CM6 theme for tag pill styling */
const tagPillTheme = EditorView.baseTheme({
  ".cm-tag-pill": {
    backgroundColor: "color-mix(in srgb, var(--tag-color, #888) 20%, transparent)",
    color: "var(--tag-color, #888)",
    borderRadius: "3px",
    padding: "0 3px",
    fontWeight: "500",
  },
});

/**
 * Extension bundle for tag decorations.
 * Pass tag colors as a Map<slug, hexColor> (without #).
 */
export function tagDecorations(colors: Map<string, string>) {
  return [tagColorsFacet.of(colors), tagDecorationPlugin, tagPillTheme];
}
