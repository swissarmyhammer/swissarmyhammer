/**
 * Generic remark plugin that transforms known `prefix+slug` patterns in
 * text nodes into custom AST nodes for rendering as colored pills.
 *
 * Parameterized by prefix character and HTML element name.
 */
import type { Root, Text, PhrasingContent } from "mdast";
import { visit, SKIP } from "unist-util-visit";
import { findMentionsInText } from "@/lib/mention-finder";

/** Custom AST node for a mention pill */
export interface MentionPillNode {
  type: string;
  data: {
    hName: string;
    hProperties: { slug: string };
  };
  children: [{ type: "text"; value: string }];
}

/**
 * Create a remark plugin that highlights known mentions.
 *
 * @param prefix - The mention prefix character (e.g. `#`, `@`)
 * @param slugs - Known slugs to match
 * @param nodeType - AST node type (e.g. `"tagPill"`, `"actorPill"`)
 * @param hName - HTML element name for rendering (e.g. `"tag-pill"`, `"actor-pill"`)
 */
export function remarkMentions(
  prefix: string,
  slugs: string[],
  nodeType: string,
  hName: string,
) {
  return () => (tree: Root) => {
    visit(tree, "text", (node: Text, index, parent) => {
      if (!parent || index === undefined) return;

      const ptype = parent.type as string;
      if (ptype === "code" || ptype === "inlineCode" || ptype === "heading") {
        return;
      }

      const text = node.value;
      const hits = findMentionsInText(text, prefix, slugs);
      if (hits.length === 0) return;

      const parts: PhrasingContent[] = [];
      let lastIndex = 0;

      for (const hit of hits) {
        if (hit.index > lastIndex) {
          parts.push({ type: "text", value: text.slice(lastIndex, hit.index) });
        }

        const full = text.slice(hit.index, hit.index + hit.length);
        parts.push({
          type: nodeType,
          data: {
            hName,
            hProperties: { slug: hit.slug },
          },
          children: [{ type: "text", value: full }],
        } as unknown as PhrasingContent);

        lastIndex = hit.index + hit.length;
      }

      if (lastIndex < text.length) {
        parts.push({ type: "text", value: text.slice(lastIndex) });
      }

      parent.children.splice(index, 1, ...parts);
      // Skip past the nodes we just inserted so visit doesn't re-enter
      // them (the pill's child text still contains the prefix+slug which
      // would match again, causing infinite recursion).
      return [SKIP, index + parts.length] as const;
    });
  };
}
