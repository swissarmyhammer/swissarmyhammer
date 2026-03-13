/**
 * CM6 tag hover tooltips — thin wrapper around generic mention tooltips.
 */

import { createMentionTooltips, type MentionMeta } from "@/lib/cm-mention-tooltip";

const tagMentionTooltips = createMentionTooltips("#", "cm-tag-tooltip");

/** Tag metadata for tooltips */
export type TagMeta = MentionMeta;

/** Facet providing tag metadata for hover tooltips */
export const tagMetaFacet = tagMentionTooltips.metaFacet;

/**
 * Extension bundle for tag hover tooltips.
 * Pass tag metadata as Map<slug, TagMeta>.
 */
export function tagTooltips(meta: Map<string, TagMeta>) {
  return tagMentionTooltips.extension(meta);
}
