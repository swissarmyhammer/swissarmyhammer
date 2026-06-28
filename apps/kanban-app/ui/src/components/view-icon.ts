import type { LucideIcon } from "lucide-react";
import type { ViewDef } from "@/types/kanban";
import { iconByName } from "@/lib/icon-name";

/**
 * Resolve the lucide icon component for a view's metadata-declared `icon`.
 *
 * The icon is supplied by the view definition (YAML `icon` property) — the
 * single source of truth. Delegates to `iconByName`: returns `null` when the
 * view declares no icon or the name does not map to a known lucide component,
 * so callers apply their own documented fallback (the left-nav uses
 * `LayoutGrid`).
 *
 * @param view - The view definition whose `icon` property to resolve.
 * @returns The matching LucideIcon component, or `null` if unresolved.
 */
export function viewIcon(view: ViewDef): LucideIcon | null {
  return iconByName(view.icon);
}
