/**
 * Generic CM6 mention hover tooltip — parameterized by prefix character.
 */

import { Facet } from "@codemirror/state";
import { EditorView, hoverTooltip, type Tooltip } from "@codemirror/view";
import { findMentionsInText } from "@/lib/mention-finder";
import type { MentionMeta } from "@/lib/mention-meta";

export type { MentionMeta };

/** Find the mention at a given document position within a single line. */
function mentionAtPos(
  text: string,
  pos: number,
  lineFrom: number,
  prefix: string,
  slugs: string[],
): { slug: string; from: number; to: number } | null {
  const hits = findMentionsInText(text, prefix, slugs);
  for (const hit of hits) {
    const from = lineFrom + hit.index;
    const to = from + hit.length;
    if (pos >= from && pos <= to) {
      return { slug: hit.slug, from, to };
    }
  }
  return null;
}

/** Build the DOM for a tooltip showing entity info. */
function buildTooltipDom(
  cssClass: string,
  prefix: string,
  slug: string,
  info: MentionMeta,
): HTMLElement {
  const dom = document.createElement("div");
  dom.className = cssClass;

  const header = document.createElement("div");
  header.style.display = "flex";
  header.style.alignItems = "center";
  header.style.gap = "6px";
  header.style.marginBottom = info.description ? "4px" : "0";

  const dot = document.createElement("span");
  dot.style.width = "10px";
  dot.style.height = "10px";
  dot.style.borderRadius = "50%";
  dot.style.backgroundColor = `#${info.color}`;
  dot.style.flexShrink = "0";
  header.appendChild(dot);

  const name = document.createElement("strong");
  name.textContent = `${prefix}${slug}`;
  header.appendChild(name);

  dom.appendChild(header);

  if (info.description) {
    const desc = document.createElement("div");
    desc.style.color = "var(--color-muted-foreground, #999)";
    desc.style.fontSize = "0.85em";
    desc.textContent = info.description;
    dom.appendChild(desc);
  }

  return dom;
}

/**
 * Create a mention tooltip extension for a given prefix.
 *
 * @param prefix - The mention prefix character (e.g. `#`, `@`)
 * @param cssClass - CSS class for the tooltip container
 */
export function createMentionTooltips(prefix: string, cssClass: string) {
  const metaFacet = Facet.define<
    Map<string, MentionMeta>,
    Map<string, MentionMeta>
  >({
    combine(values) {
      return values.length > 0 ? values[values.length - 1] : new Map();
    },
  });

  const hoverSource = hoverTooltip((view, pos) => {
    const line = view.state.doc.lineAt(pos);
    const meta = view.state.facet(metaFacet);
    const slugs = Array.from(meta.keys());
    const hit = mentionAtPos(line.text, pos, line.from, prefix, slugs);
    if (!hit) return null;

    const info = meta.get(hit.slug);
    if (!info) return null;

    return {
      pos: hit.from,
      end: hit.to,
      above: true,
      create: () => ({
        dom: buildTooltipDom(cssClass, prefix, hit.slug, info),
      }),
    } satisfies Tooltip;
  });

  const theme = EditorView.baseTheme({
    [`.${cssClass}`]: {
      padding: "6px 10px",
      borderRadius: "6px",
      fontSize: "13px",
      maxWidth: "300px",
    },
  });

  return {
    metaFacet,
    /** Extension bundle: pass metadata as Map<slug, MentionMeta> */
    extension(meta: Map<string, MentionMeta>) {
      return [metaFacet.of(meta), hoverSource, theme];
    },
  };
}
