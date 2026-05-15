/**
 * Register the virtual-badge-list display adapter with the Field registry.
 *
 * This display type renders computed virtual tags (READY, BLOCKED, BLOCKING)
 * as colored pill badges using a static color map instead of the entity store.
 */

import {
  registerDisplay,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { VirtualTagDisplay } from "@/components/fields/displays/virtual-tag-display";

/**
 * Adapter that narrows FieldDisplayProps (field, entity, value, mode) down to
 * VirtualTagDisplayProps (value, mode). Virtual tags use a static color map
 * rather than entity lookups, so the extra props are unused — but `mode` is
 * forwarded so the display can apply the compact-mode height contract.
 */
function VirtualBadgeListAdapter({ value, mode }: FieldDisplayProps) {
  return <VirtualTagDisplay value={value} mode={mode} />;
}

registerDisplay("virtual-badge-list", VirtualBadgeListAdapter);
