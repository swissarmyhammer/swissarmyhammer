import { Facet } from "@codemirror/state";
import { EditorView, hoverTooltip, type Tooltip } from "@codemirror/view";
import { findTagsInText } from "@/lib/tag-finder";

/** Tag metadata for tooltips: slug → { color, description? } */
export interface TagMeta {
  color: string;
  description?: string;
}

/** Facet providing tag metadata for hover tooltips */
export const tagMetaFacet = Facet.define<
  Map<string, TagMeta>,
  Map<string, TagMeta>
>({
  combine(values) {
    return values.length > 0 ? values[values.length - 1] : new Map();
  },
});

/** Find the tag at a given document position using known slugs */
function tagAtPos(
  text: string,
  pos: number,
  lineFrom: number,
  slugs: string[],
): { slug: string; from: number; to: number } | null {
  const hits = findTagsInText(text, slugs);
  for (const hit of hits) {
    const from = lineFrom + hit.index;
    const to = from + hit.length;
    if (pos >= from && pos <= to) {
      return { slug: hit.slug, from, to };
    }
  }
  return null;
}

/** Hover tooltip source for known #tag patterns */
const tagHoverTooltip = hoverTooltip((view, pos) => {
  const line = view.state.doc.lineAt(pos);
  const meta = view.state.facet(tagMetaFacet);
  const slugs = Array.from(meta.keys());
  const hit = tagAtPos(line.text, pos, line.from, slugs);
  if (!hit) return null;

  const tagInfo = meta.get(hit.slug);
  if (!tagInfo) return null;

  return {
    pos: hit.from,
    end: hit.to,
    above: true,
    create() {
      const dom = document.createElement("div");
      dom.className = "cm-tag-tooltip";

      const header = document.createElement("div");
      header.style.display = "flex";
      header.style.alignItems = "center";
      header.style.gap = "6px";
      header.style.marginBottom = tagInfo.description ? "4px" : "0";

      const dot = document.createElement("span");
      dot.style.width = "10px";
      dot.style.height = "10px";
      dot.style.borderRadius = "50%";
      dot.style.backgroundColor = `#${tagInfo.color}`;
      dot.style.flexShrink = "0";
      header.appendChild(dot);

      const name = document.createElement("strong");
      name.textContent = `#${hit.slug}`;
      header.appendChild(name);

      dom.appendChild(header);

      if (tagInfo.description) {
        const desc = document.createElement("div");
        desc.style.color = "var(--color-muted-foreground, #999)";
        desc.style.fontSize = "0.85em";
        desc.textContent = tagInfo.description;
        dom.appendChild(desc);
      }

      return { dom };
    },
  } satisfies Tooltip;
});

/** CM6 theme for the tooltip */
const tagTooltipTheme = EditorView.baseTheme({
  ".cm-tag-tooltip": {
    padding: "6px 10px",
    borderRadius: "6px",
    fontSize: "13px",
    maxWidth: "300px",
  },
});

/**
 * Extension bundle for tag hover tooltips.
 * Pass tag metadata as Map<slug, TagMeta>.
 */
export function tagTooltips(meta: Map<string, TagMeta>) {
  return [tagMetaFacet.of(meta), tagHoverTooltip, tagTooltipTheme];
}
