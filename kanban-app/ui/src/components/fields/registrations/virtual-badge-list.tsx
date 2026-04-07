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

/** Adapter that bridges FieldDisplayProps to VirtualTagDisplay. */
function VirtualBadgeListAdapter({ value }: FieldDisplayProps) {
  return <VirtualTagDisplay value={value} />;
}

registerDisplay("virtual-badge-list", VirtualBadgeListAdapter);
