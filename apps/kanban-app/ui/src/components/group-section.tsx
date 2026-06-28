/**
 * GroupSection renders a single collapsible group within a grouped board view.
 *
 * Each section shows a header with the group label, task count badge, and
 * a collapse/expand chevron. When expanded, it renders a full BoardView
 * containing only the tasks belonging to that group.
 *
 * Collapse state is owned by the parent (`<GroupedBoardView>`), not by
 * this component. The outer virtualizer recycles `<GroupSection>`
 * instances as they scroll out of view; a `useState` here would die
 * with the unmount and the user's collapse choice would silently
 * reset. Hoisting the state to the parent (keyed by `bucket.value`)
 * keeps the choice durable across recycling.
 *
 * # Spatial scope + keyboard collapse
 *
 * Each section wraps its content in a `<FocusScope moniker="group:<value>">`
 * so the inner board (`board:<id>` and its columns / cards) composes a focus
 * path that is UNIQUE per group — the same board renders inside every
 * section, so without this segment their FQMs would collide and a keyboard
 * command could not tell which group holds focus.
 *
 * On top of that scope the section registers the `group.toggleCollapse`
 * (vim `z o`) webview-bus handler via `useFocusedWebviewCommandHandlers`,
 * gated to focus being within this section. The command is DEFINED by the
 * `board-commands` builtin plugin (`keys: { vim: "z o" }`, `scope:
 * ["ui:board"]`); the handler here is its presentation-only behavior —
 * flipping the parent's collapse map for this bucket. Only the focused
 * group's handler is live, so a dispatch flips exactly the group the user is
 * in (the `<Field>` / `<Pressable>` many-instance precedent).
 */

import { useMemo } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { BoardView } from "@/components/board-view";
import { FocusScope } from "@/components/focus-scope";
import { useFocusedWebviewCommandHandlers } from "@/lib/use-focused-webview-command-handlers";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { moniker } from "@/lib/moniker";
import { asSegment } from "@/types/spatial";
import type { GroupBucket } from "@/lib/group-utils";
import type { BoardData } from "@/types/kanban";

interface GroupSectionProps {
  /** The group bucket containing the label and tasks for this section. */
  bucket: GroupBucket;
  /** Board data (columns, tags, etc.) shared across all sections. */
  board: BoardData;
  /** The field name being grouped by. */
  groupField: string;
  /** Whether this section is currently collapsed. Owned by the parent. */
  collapsed: boolean;
  /** Callback invoked when the user clicks the section's collapse toggle. */
  onToggleCollapsed: () => void;
}

/**
 * Tailwind class that gives an expanded group section a bounded, viewport-
 * relative height.
 *
 * Load-bearing for virtualization:
 *
 * `<ColumnView>` uses TanStack `useVirtualizer` with
 * `getScrollElement: () => scrollRef.current` where `scrollRef` is the
 * column's inner `flex-1 overflow-y-auto` div. The virtualizer measures
 * that element's `clientHeight` to compute how many cards intersect the
 * viewport. In the ungrouped path that flex chain bottoms out at the
 * app viewport (`h-screen`), so the column's scroll element ends up
 * bounded and the virtualizer windows correctly.
 *
 * In the grouped path the outer `<GroupedBoardView>` is itself
 * `overflow-y-auto` (vertical scroll between sections), and each
 * `<GroupSection>` is a `shrink-0` child of that vertical strip. Without
 * an explicit height on the section body, the inner `flex-1` chain has
 * no finite ancestor to flex against — `clientHeight` of the column's
 * scroll element collapses to the natural height of its content, which
 * for 2300 cards is several thousand pixels. The virtualizer concludes
 * "the viewport already shows everything" and mounts every card.
 *
 * Pinning the expanded body to a definite viewport-relative height
 * restores the bounded ancestor the virtualizer needs. 70vh leaves room
 * for the next section's header to peek into the viewport so the user
 * keeps the "multiple groups visible at once" affordance the grouped
 * view exists for.
 */
const EXPANDED_BODY_CLASS = "h-[70vh] min-h-0 flex flex-col";

/**
 * Collapsible group section that wraps a BoardView with only the group's tasks.
 *
 * Collapse state is **controlled** by the parent (`<GroupedBoardView>`).
 * The parent maintains a per-bucket collapsed map keyed by
 * `bucket.value` and passes the current value plus a toggle callback
 * down here. This makes the section recycle-safe under outer
 * virtualization: when the section unmounts as it scrolls out of view
 * the collapse state survives in the parent and is restored when the
 * section re-mounts on scroll-back.
 *
 * The section root carries `data-group-section` and
 * `data-group-value="<bucket.value>"` for outer-virtualizer test
 * selectors and DOM-mounted-count assertions.
 *
 * @param bucket - Group bucket with label and tasks.
 * @param board - Board data shared across all group sections.
 * @param groupField - The field name used for grouping (not read inside the
 *   section — the per-group spatial scope keys off `bucket.value`).
 * @param collapsed - Whether the section is currently collapsed.
 * @param onToggleCollapsed - Invoked on header click to flip the parent's
 *   collapse map for this bucket.
 */
export function GroupSection({
  bucket,
  board,
  groupField: _groupField,
  collapsed,
  onToggleCollapsed,
}: GroupSectionProps) {
  // Each section is its own spatial sub-tree keyed by the bucket value:
  // `group:<value>`. The inner `<BoardView>`'s `board:<id>` scope and every
  // column / card below it compose their FQMs UNDER this segment, so the
  // focus path is unique per group even though the same board (and the same
  // `board:<id>` moniker) renders inside every section. Without this the
  // grouped board's column / card FQMs would be identical across groups and a
  // keyboard command could not tell which group holds focus.
  const groupSegment = useMemo(
    () => asSegment(moniker("group", bucket.value)),
    [bucket.value],
  );

  // Register the `group.toggleCollapse` (vim `z o`) bus handler WHILE spatial
  // focus is within this section's subtree. The webview command bus holds one
  // handler per id; focus-gating means only the focused group's handler is
  // live, so a dispatch flips exactly the group the user is in. The command is
  // DEFINED by the `board-commands` plugin (keys + `ui:board` scope); this is
  // its presentation-only behavior — `onToggleCollapsed` flips the parent's
  // collapse map for this bucket, no durable mutation. Mirrors the
  // `<Field>` / `<Pressable>` many-instance precedent. The hook self-degrades
  // to never registering outside the spatial provider stack (the same
  // condition under which the `<FocusScope>` below is omitted).
  const handlers = useMemo(
    () => ({ "group.toggleCollapse": () => onToggleCollapsed() }),
    [onToggleCollapsed],
  );
  useFocusedWebviewCommandHandlers(groupSegment, handlers);

  const body = (
    <>
      <div className="h-px bg-border mx-3" />
      <button
        type="button"
        aria-label={bucket.label}
        className="flex items-center gap-2 px-3 py-1 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors w-full text-left"
        onClick={onToggleCollapsed}
      >
        {collapsed ? (
          <ChevronRight className="h-3.5 w-3.5" />
        ) : (
          <ChevronDown className="h-3.5 w-3.5" />
        )}
        <span>{bucket.label}</span>
        <Badge variant="secondary" className="text-xs px-1.5 py-0">
          {bucket.tasks.length}
        </Badge>
      </button>
      {!collapsed && (
        <div className={EXPANDED_BODY_CLASS} data-testid="group-section-body">
          <BoardView
            board={board}
            tasks={bucket.tasks}
            groupValue={bucket.value}
          />
        </div>
      )}
    </>
  );

  // The `<FocusScope>` requires the spatial-nav provider stack (it throws
  // outside a `<FocusLayer>`). Production always mounts it, but lightweight
  // unit tests render a `<GroupSection>` bare. Mirror `PerspectiveSpatialZone`
  // (perspective-container.tsx): render the spatial frame only when the stack
  // is present, otherwise a plain `<div>` carrying the same
  // `data-group-section` / `data-group-value` selectors and `shrink-0` layout
  // class. Either branch renders identical content; only the FQM frame differs.
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return (
      <div
        className="shrink-0"
        data-group-section=""
        data-group-value={bucket.value}
      >
        {body}
      </div>
    );
  }

  return (
    <FocusScope
      moniker={groupSegment}
      // Viewport-spanning group strip: a focus rectangle around the whole
      // section would be noise. The board columns / cards inside own their
      // own focus indicators. The section is a passive spatial frame for FQM
      // composition + the collapse-toggle handler, not a click target, so
      // event handling is suppressed too (the header button owns its click).
      showFocus={false}
      handleEvents={false}
      className="shrink-0"
      data-group-section=""
      data-group-value={bucket.value}
    >
      {body}
    </FocusScope>
  );
}
