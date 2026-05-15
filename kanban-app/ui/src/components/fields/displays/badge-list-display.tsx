import { useMemo } from "react";
import { MentionView, type MentionItem } from "@/components/mention-view";
import type { CommandDef } from "@/lib/command-scope";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { DisplayProps } from "./text-display";

/**
 * Badge list display — renders mention pills for any badge-list field.
 *
 * Thin adapter over `MentionView`: inspects the field type, builds a list
 * of `{entityType, id | slug}` references, and hands rendering off to the
 * CM6-based `MentionView` widget pipeline.
 *
 * Handles both field shapes:
 * - **Reference fields** (e.g. `depends_on`) — `value` is an array of entity
 *   IDs; each becomes `{entityType, id}`. `MentionView` resolves each to
 *   its display name for the CM6 pill.
 * - **Computed tag fields** — `value` is an array of tag slugs; each
 *   becomes `{entityType, slug}`. `MentionView` finds the tag entity by
 *   slug match and renders its display name.
 *
 * For tags on a task, a `task.untag` extra command is added to every
 * pill's context menu so the user can remove a tag in-place.
 *
 * Empty-state handling stays here rather than inside `MentionView` because
 * the empty presentation is field-display semantic (`-` in compact grid
 * cells vs. italic `None` in full inspector rows), not mention-rendering
 * semantic.
 */
/** Extract the `value` array as a stably-referenced string array. */
function useStableValues(value: unknown): string[] {
  const valuesKey = Array.isArray(value) ? (value as string[]).join("\0") : "";
  return useMemo(
    () => (Array.isArray(value) ? (value as string[]) : []),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [valuesKey],
  );
}

/** Build the MentionView items array from raw values + field-type info. */
function useMentionItems(
  values: string[],
  targetEntityType: string | undefined,
  isComputedSlug: boolean,
): MentionItem[] {
  return useMemo<MentionItem[]>(() => {
    if (!targetEntityType) return [];
    if (isComputedSlug) {
      return values.map((slug) => ({ entityType: targetEntityType, slug }));
    }
    return values.map((id) => ({ entityType: targetEntityType, id }));
  }, [values, targetEntityType, isComputedSlug]);
}

/**
 * For tags on a task, add an in-place "Remove Tag" command to every pill's
 * context menu. Gated on `targetEntityType === "tag"` so only tag fields
 * get the untag menu.
 */
function useTagUntagCommands(
  targetEntityType: string | undefined,
  isComputedSlug: boolean,
  entityId: string,
): CommandDef[] | undefined {
  return useMemo<CommandDef[] | undefined>(() => {
    if (targetEntityType !== "tag" || !isComputedSlug || !entityId) {
      return undefined;
    }
    return [
      {
        id: "task.untag",
        name: "Remove Tag",
        contextMenu: true,
        args: { id: entityId },
      },
    ];
  }, [targetEntityType, isComputedSlug, entityId]);
}

/** Props for {@link EmptyBadgeList}. */
interface EmptyBadgeListProps {
  /** Display mode — drives the styling and fallback text. */
  mode: "compact" | "full";
  /** Optional YAML-configured placeholder; falls back to mode-specific defaults. */
  placeholder?: string;
}

/**
 * Empty-state rendering — compact grid cells vs. full inspector rows.
 *
 * When the field declares a YAML `placeholder`, use it for both modes
 * (the configured hint wins over the mode-specific default), relying on
 * the existing muted styling to keep it visually recessed. Without a
 * placeholder, preserve the original `-` / `None` fallback so fields
 * that haven't opted in render identically to before.
 */
function EmptyBadgeList({ mode, placeholder }: EmptyBadgeListProps) {
  if (mode === "compact") {
    return (
      <span className="text-muted-foreground/50">{placeholder ?? "-"}</span>
    );
  }
  return (
    <span className="text-sm text-muted-foreground italic">
      {placeholder ?? "None"}
    </span>
  );
}

/**
 * Badge list display — renders mention pills for any badge-list field.
 *
 * Thin adapter over `MentionView`: inspects the field type, builds a list
 * of `{entityType, id | slug}` references, and hands rendering off to the
 * CM6-based `MentionView` widget pipeline. Handles both reference fields
 * (IDs) and computed tag fields (slugs), plus task-scoped `task.untag`
 * context-menu extras for tag lists.
 */
export function BadgeListDisplay({ field, value, entity, mode }: DisplayProps) {
  const values = useStableValues(value);
  const targetEntityType = field.type.entity as string | undefined;
  const isComputedSlug = !!(field.type as Record<string, unknown>)
    .commit_display_names;

  const items = useMentionItems(values, targetEntityType, isComputedSlug);
  const extraCommands = useTagUntagCommands(
    targetEntityType,
    isComputedSlug,
    entity.id,
  );

  if (values.length === 0) {
    const empty = (
      <EmptyBadgeList mode={mode} placeholder={field.placeholder} />
    );
    return mode === "compact" ? (
      <CompactCellWrapper>{empty}</CompactCellWrapper>
    ) : (
      empty
    );
  }

  const pills = (
    <MentionView items={items} mode={mode} extraCommands={extraCommands} />
  );
  return mode === "compact" ? (
    <CompactCellWrapper>{pills}</CompactCellWrapper>
  ) : (
    pills
  );
}
