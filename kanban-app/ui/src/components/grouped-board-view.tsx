/**
 * GroupedBoardView conditionally wraps BoardView with group sections.
 *
 * When a group field is active (from the perspective), it computes group
 * buckets and renders a vertical stack of collapsible GroupSection
 * components, each containing a full column layout for that group's
 * tasks.
 *
 * When no grouping is active, it renders BoardView directly with no
 * visual change.
 *
 * Outer virtualization
 * --------------------
 *
 * For high-cardinality grouping fields (the trigger case is `tags`, but
 * any field with many distinct values reproduces — `project`, `assignees`,
 * etc.) the unbounded `groups.map(...)` previously mounted every
 * `<GroupSection>` at once. Each section instantiates a full
 * `<BoardView>` (one inner virtualizer per column), so at 100+ groups
 * mount cost stacked quadratically even though the inner card
 * virtualizer correctly windowed cards inside each section.
 *
 * The fix wraps the group list in a TanStack `useVirtualizer` so only
 * viewport-visible sections (plus a small overscan) mount. The outer
 * scroll container is the same `flex flex-col flex-1 min-h-0
 * overflow-y-auto` root that existed before — the virtualizer simply
 * reads `clientHeight` off it and decides which group indices fall in
 * the visible window. Each rendered section is absolutely positioned
 * inside a total-height spacer using the standard TanStack pattern
 * (`translateY(${start}px)` over `position: relative` + `height:
 * totalSize`).
 *
 * Collapse state
 * --------------
 *
 * Outer virtualization recycles `<GroupSection>` instances as they
 * scroll out of view. A `useState` *inside* the section would die with
 * the unmount and the user's collapsed/expanded choice would silently
 * reset on scroll-back. Collapse state is therefore lifted here, keyed
 * by `bucket.value`, and passed down as `collapsed` +
 * `onToggleCollapsed` props.
 *
 * Sections start **collapsed** by default — the lazy `useState`
 * initializer seeds the set with every bucket value. At high cardinality
 * (hundreds of groups) starting expanded produces visible jumpiness:
 * the virtualizer's expanded-height estimate is approximate, each
 * section's real height differs once Tailwind layout settles, and the
 * resulting `measureElement` corrections re-shuffle the total-size
 * spacer for several frames. Collapsed sections render only the header
 * (~40px) which `COLLAPSED_HEIGHT_PX` matches exactly, so the spacer is
 * stable from the first paint. Users opt in to the cost of mounting a
 * group's `<BoardView>` by clicking its header. Toggling adds/removes
 * the value from the collapsed set.
 *
 * The collapse map's value space is intrinsic to the active `groupField`
 * — values from `project` are project ids; values from `tag` are tag
 * names; ungrouped buckets all carry the empty-string `value`. When the
 * user switches `groupField` (e.g. `project` → `tag`) the prior keys are
 * stale and may collide with new-field values by coincidence (most
 * obviously the empty-string ungrouped bucket every field emits). To
 * guarantee a clean slate on field changes we render `<GroupedBoardBody>`
 * under `key={groupField}` so React remounts it on every field flip,
 * naturally resetting the collapsed set.
 *
 * Drag-and-drop interaction
 * -------------------------
 *
 * dnd-kit holds drag state keyed by the source/target DOM elements'
 * registered `useDraggable` / `useDroppable` ids. If a section unmounts
 * mid-drag (because the user scrolls the source or target past the
 * outer virtualizer's overscan window) those registrations die and the
 * `active`/`over` refs go stale. Drop completes against the wrong
 * target or the drag silently aborts.
 *
 * The fix is to suspend outer virtualization while a drag session is
 * active — when `useDragSession().session !== null` we short-circuit to
 * a plain `groups.map(...)` so every section is mounted and every
 * dnd-kit registration stays alive. Mount cost during a drag is the
 * same as the pre-virtualization baseline (~2300 cards × 200 groups
 * worst case), but a drag is a transient mode measured in seconds at
 * the outside, and correctness of drop targets matters more than mount
 * cost during that window.
 */

import { useCallback, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useActivePerspective } from "@/components/perspective-container";
import { useSchema } from "@/lib/schema-context";
import { BoardView } from "@/components/board-view";
import { GroupSection } from "@/components/group-section";
import { computeGroups } from "@/lib/group-utils";
import { useDragSession } from "@/lib/drag-session-context";
import type { GroupBucket } from "@/lib/group-utils";
import type { BoardData, Entity } from "@/types/kanban";

interface GroupedBoardViewProps {
  /** Board data (columns, tags, summary). */
  board: BoardData;
  /** All task entities — filtering is handled by BoardView/perspective. */
  tasks: Entity[];
}

/**
 * Estimated pixel height of a collapsed section in the outer virtualizer.
 *
 * A collapsed section renders only the header strip (~28px button +
 * 1px divider). We undershoot slightly so the virtualizer mounts a
 * little more than strictly necessary on first pass; `measureElement`
 * refines the actual height after layout.
 */
const COLLAPSED_HEIGHT_PX = 40;

/**
 * Default header strip height (px) added on top of the body when computing
 * the expanded estimate. Mirrors the px-3 py-1 + chevron + label row.
 */
const SECTION_HEADER_HEIGHT_PX = 40;

/**
 * Fallback window inner height (px) used when the browser does not
 * expose `window.innerHeight` (e.g. in SSR or stub environments).
 *
 * Matches the perf-test viewport so the virtualizer's initial estimate
 * stays sensible in both production and test environments.
 */
const FALLBACK_VIEWPORT_PX = 800;

/**
 * Estimate the pixel height of an expanded section.
 *
 * Section body is `h-[70vh]` per `<GroupSection>`'s
 * `EXPANDED_BODY_CLASS`. Read the current `window.innerHeight` so the
 * estimate tracks the user's viewport — `measureElement` refines once
 * the section mounts and reports its actual height.
 *
 * Called inline from the virtualizer's `estimateSize` callback rather
 * than memoised. The arithmetic is cheap (one multiply + one floor + one
 * add) and being called per-index keeps the estimate responsive to
 * window resizes: a resize triggers a re-render via React's layout
 * effects upstream, the estimate immediately reflects the new viewport,
 * and the virtualizer recomputes its total-size spacer without waiting
 * for every section to remount and report through `measureElement`.
 */
function estimateExpandedHeight(): number {
  const vh =
    typeof window !== "undefined" && window.innerHeight > 0
      ? window.innerHeight
      : FALLBACK_VIEWPORT_PX;
  return Math.floor(vh * 0.7) + SECTION_HEADER_HEIGHT_PX;
}

/**
 * Renders the board with optional grouping.
 *
 * Reads `groupField` from the active perspective. When no `groupField`
 * is set, delegates directly to `<BoardView>` so the ungrouped path is
 * a no-op. When grouping is active, computes group buckets via
 * `computeGroups`, hoists the collapse state for every bucket, and
 * renders the buckets through an outer `useVirtualizer` so DOM mount
 * cost is bounded regardless of group cardinality.
 *
 * @param board - Board data shared across all sections.
 * @param tasks - All task entities for the board.
 */
export function GroupedBoardView({ board, tasks }: GroupedBoardViewProps) {
  const { groupField } = useActivePerspective();
  const { getSchema } = useSchema();

  const fieldDefs = useMemo(() => {
    const schema = getSchema("task");
    return schema?.fields ?? [];
  }, [getSchema]);

  const groups = useMemo(() => {
    if (!groupField) return null;
    return computeGroups(tasks, groupField, fieldDefs);
  }, [tasks, groupField, fieldDefs]);

  // No grouping active — render the plain board view. Guarding on
  // `groupField` (rather than the derived `groups`) lets TypeScript
  // narrow `groupField` from `string | undefined` to `string` for the
  // rest of the function, so it can be forwarded to `<GroupedBoardBody>`
  // without a non-null assertion. `groups` is non-null iff `groupField`
  // is set, so the two guards are equivalent at runtime.
  if (!groupField || !groups) {
    return <BoardView board={board} tasks={tasks} />;
  }

  // `key={groupField}` forces a remount whenever the active group field
  // changes (e.g. `project` → `tag`). This resets the collapsed-set
  // inside `<GroupedBoardBody>` so values from the prior field cannot
  // bleed into the new field's value space (most notably the empty
  // ungrouped-bucket value every field emits — but also any arbitrary
  // string collision across dimensions). See the file header for the
  // full rationale.
  return (
    <GroupedBoardBody
      key={groupField}
      board={board}
      groups={groups}
      groupField={groupField}
    />
  );
}

interface GroupedBoardBodyProps {
  /** Board data shared across all group sections. */
  board: BoardData;
  /** Pre-computed group buckets in display order. */
  groups: GroupBucket[];
  /** The field name being grouped by. Forwarded to each `<GroupSection>`. */
  groupField: string;
}

/**
 * Inner component for the grouped path.
 *
 * Split from `<GroupedBoardView>` so hook usage stays unconditional
 * (the ungrouped path returns before any of the virtualizer-related
 * hooks fire). Owns the collapse-state map and the outer virtualizer
 * instance. Renders the viewport-visible window of sections inside the
 * standard TanStack absolute-positioned total-height container.
 */
function GroupedBoardBody({
  board,
  groups,
  groupField,
}: GroupedBoardBodyProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  // Start every bucket collapsed (see file header for the jumpiness
  // rationale). Lazy initializer runs once per mount; combined with the
  // `key={groupField}` remount above, switching the active group field
  // produces a fresh fully-collapsed set for the new field's value space.
  const [collapsedSet, setCollapsedSet] = useState<Set<string>>(
    () => new Set(groups.map((g) => g.value)),
  );
  // Suspend outer virtualization while a drag is active so dnd-kit's
  // per-element registrations cannot die underneath an in-progress
  // drag. See the file header for the full rationale.
  const { session: dragSession } = useDragSession();
  const dragActive = dragSession !== null;

  const toggleCollapsed = useCallback((value: string) => {
    setCollapsedSet((prev) => {
      const next = new Set(prev);
      if (next.has(value)) {
        next.delete(value);
      } else {
        next.add(value);
      }
      return next;
    });
  }, []);

  const virtualizer = useVirtualizer({
    count: groups.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) =>
      collapsedSet.has(groups[i].value)
        ? COLLAPSED_HEIGHT_PX
        : estimateExpandedHeight(),
    overscan: 2,
  });

  const virtualRows = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();

  // While a drag is active, mount every section so dnd-kit's source and
  // target registrations cannot be unmounted by virtualizer recycling
  // during the drag. The cost is bounded by drag duration (seconds);
  // correctness of drop targets matters more in that window than mount
  // economy.
  if (dragActive) {
    return (
      <div
        ref={scrollRef}
        data-group-list=""
        data-drag-bypass="true"
        className="flex flex-col flex-1 min-h-0 overflow-y-auto"
      >
        {groups.map((bucket) => (
          <GroupSection
            key={bucket.value}
            bucket={bucket}
            board={board}
            groupField={groupField}
            collapsed={collapsedSet.has(bucket.value)}
            onToggleCollapsed={() => toggleCollapsed(bucket.value)}
          />
        ))}
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      data-group-list=""
      className="flex flex-col flex-1 min-h-0 overflow-y-auto"
    >
      <div
        style={{
          height: totalSize,
          width: "100%",
          position: "relative",
        }}
      >
        {virtualRows.map((virtualRow) => {
          const bucket = groups[virtualRow.index];
          return (
            <div
              key={bucket.value}
              data-index={virtualRow.index}
              ref={virtualizer.measureElement}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <GroupSection
                bucket={bucket}
                board={board}
                groupField={groupField}
                collapsed={collapsedSet.has(bucket.value)}
                onToggleCollapsed={() => toggleCollapsed(bucket.value)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
