/**
 * Remark plugin that transforms known `#tag` patterns in text nodes
 * into custom `tagPill` AST nodes for rendering as colored pills.
 *
 * Tag identification is driven by the known slug list from the backend —
 * no regex defining valid tag characters.
 */
import type { Root, Text, PhrasingContent } from "mdast";
import { visit } from "unist-util-visit";
import { findTagsInText } from "@/lib/tag-finder";

/** Custom AST node for a tag pill */
export interface TagPillNode {
  type: "tagPill";
  data: {
    hName: "tag-pill";
    hProperties: { slug: string };
  };
  children: [{ type: "text"; value: string }];
}

/**
 * Create a remark plugin that highlights known tags.
 * Usage: `remarkPlugins={[remarkGfm, remarkTags(slugs)]}`
 */
export function remarkTags(slugs: string[]) {
  return () => (tree: Root) => {
    visit(tree, "text", (node: Text, index, parent) => {
      if (!parent || index === undefined) return;

      // Don't transform inside code or headings
      const ptype = parent.type as string;
      if (ptype === "code" || ptype === "inlineCode" || ptype === "heading") {
        return;
      }

      const text = node.value;
      const hits = findTagsInText(text, slugs);
      if (hits.length === 0) return;

      const parts: PhrasingContent[] = [];
      let lastIndex = 0;

      for (const hit of hits) {
        // Text before the tag
        if (hit.index > lastIndex) {
          parts.push({ type: "text", value: text.slice(lastIndex, hit.index) });
        }

        // The tag pill node
        const full = text.slice(hit.index, hit.index + hit.length);
        parts.push({
          type: "tagPill",
          data: {
            hName: "tag-pill",
            hProperties: { slug: hit.slug },
          },
          children: [{ type: "text", value: full }],
        } as unknown as PhrasingContent);

        lastIndex = hit.index + hit.length;
      }

      // Remaining text after last tag
      if (lastIndex < text.length) {
        parts.push({ type: "text", value: text.slice(lastIndex) });
      }

      // Replace the original text node with our parts
      parent.children.splice(index, 1, ...parts);
    });
  };
}
