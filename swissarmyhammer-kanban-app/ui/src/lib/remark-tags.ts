/**
 * Remark plugin for #tag pills — thin wrapper around generic remark-mentions.
 */

import { remarkMentions } from "@/lib/remark-mentions";
export type { MentionPillNode as TagPillNode } from "@/lib/remark-mentions";

/**
 * Create a remark plugin that highlights known tags.
 * Usage: `remarkPlugins={[remarkGfm, remarkTags(slugs)]}`
 */
export function remarkTags(slugs: string[]) {
  return remarkMentions("#", slugs, "tagPill", "tag-pill");
}
