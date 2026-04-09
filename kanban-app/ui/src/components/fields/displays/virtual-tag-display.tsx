import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";

/**
 * Metadata for each virtual tag — color and human-readable description.
 *
 * These mirror the Rust DEFAULT_REGISTRY in virtual_tags.rs.
 * Virtual tags are computed server-side; the frontend only needs
 * slug, color, and tooltip text.
 *
 * TODO: Serve virtual tag metadata from the backend (via schema or a
 * companion endpoint) instead of duplicating it here. If the Rust side
 * adds a new virtual tag or changes a color, this map must be updated
 * manually. See: swissarmyhammer-kanban/src/virtual_tags.rs
 */
const VIRTUAL_TAG_META: Record<string, { color: string; description: string }> =
  {
    READY: {
      color: "0e8a16",
      description: "Task has no unmet dependencies",
    },
    BLOCKED: {
      color: "e36209",
      description: "Task has at least one unmet dependency",
    },
    BLOCKING: {
      color: "d73a4a",
      description: "Other tasks depend on this one",
    },
  };

/** Props for VirtualTagDisplay. */
export interface VirtualTagDisplayProps {
  /** Array of virtual tag slugs (e.g. ["READY", "BLOCKING"]). */
  value: unknown;
}

/**
 * Display component for the virtual_tags field.
 *
 * Renders computed virtual tags (READY, BLOCKED, BLOCKING) as colored pill
 * badges with tooltips. Uses a static color/description map since virtual
 * tags are not real entities in the store.
 *
 * Renders nothing when the value is empty or undefined.
 */
export function VirtualTagDisplay({ value }: VirtualTagDisplayProps) {
  const tags = Array.isArray(value) ? (value as string[]) : [];

  if (tags.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-1">
      {tags.map((slug) => {
        const meta = VIRTUAL_TAG_META[slug];
        if (!meta) return null;

        return (
          <Tooltip key={slug}>
            <TooltipTrigger asChild>
              <span
                className="inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium cursor-default"
                style={{
                  backgroundColor: `color-mix(in srgb, #${meta.color} 20%, transparent)`,
                  color: `#${meta.color}`,
                  border: `1px solid color-mix(in srgb, #${meta.color} 30%, transparent)`,
                }}
              >
                {slug}
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom">{meta.description}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
