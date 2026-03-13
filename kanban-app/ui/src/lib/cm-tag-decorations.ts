/**
 * CM6 tag decoration extensions — thin wrapper around generic mention decorations.
 */

import { createMentionDecorations } from "@/lib/cm-mention-decorations";

const tagMentions = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

/** Facet providing a map of tag slug → hex color (without #) */
export const tagColorsFacet = tagMentions.colorsFacet;

/**
 * Extension bundle for tag decorations.
 * Pass tag colors as a Map<slug, hexColor> (without #).
 */
export function tagDecorations(colors: Map<string, string>) {
  return tagMentions.extension(colors);
}
