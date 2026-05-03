import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Plus } from "lucide-react";
import { useVirtualizer, type Virtualizer } from "@tanstack/react-virtual";
import { invoke } from "@tauri-apps/api/core";
import { DropZone } from "@/components/drop-zone";
import { computeDropZones, type DropZoneDescriptor } from "@/lib/drop-zones";
import { Field } from "@/components/fields/field";
import { DraggableTaskCard } from "@/components/sortable-task-card";
import { FocusZone, useParentZoneFq } from "@/components/focus-zone";
import { Inspectable } from "@/components/inspectable";
import { Pressable } from "@/components/pressable";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import { useOptionalFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useSchema } from "@/lib/schema-context";
import { useFocusActions } from "@/lib/entity-focus-context";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";
import {
  asPixels,
  asSegment,
  composeFq,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

/**
 * Props for {@link ColumnView} — one column in the kanban board.
 *
 * Carries the column entity and its pre-sorted tasks plus the drag/drop
 * callbacks the parent board uses to route drops through the command layer.
 *
 * Cross-column keyboard navigation now lives in the spatial-nav layer: each
 * column body wraps in a `<FocusZone>` (parent zone = `ui:board`), the
 * column-name header renders a `<Field>` whose own `<FocusZone>` (moniker
 * `field:column:<id>.name`) is the sole spatial-nav registration for the
 * name surface, and each task card body is its own `<FocusScope>` leaf
 * parented at the column. Cards must be leaves so the unified cascade
 * produces the cross-column trajectory:
 * iter 0 scores in-column card peers (leaf candidates), and when no
 * peer satisfies the beam test the cascade escalates to iter 1 — the
 * card's parent column zone — and lands on the neighbouring column
 * zone. Making cards zones would collapse iter 0 into sibling-zones
 * only (same-column cards reachable as zones), trapping focus inside
 * the column. The spatial graph computes nav.up / nav.down / nav.left /
 * nav.right from the registered rectangles, so the legacy
 * neighbor-moniker plumbing and the pull-based claim-predicate
 * threading that used to live here are gone — the column receives only
 * structural / drag-drop wiring now.
 */
interface ColumnViewProps {
  column: Entity;
  /** Tasks for this column, pre-sorted by the backend. */
  tasks: Entity[];
  onAddTask?: (columnId: string) => void;
  /** Called when a task drag starts in this column. */
  onTaskDragStart?: (entity: Entity) => void;
  /** Called when a task drag ends (from this column's card). */
  onTaskDragEnd?: (entity: Entity, dropEffect: string) => void;
  /** Called when a task is dropped on a zone in this column. */
  onDrop?: (descriptor: DropZoneDescriptor, taskData: string) => void;
  /** ID of the task currently being dragged (for no-op zone suppression). */
  dragTaskId?: string | null;
  /** Ref callback for the column container — used for cross-window hit-testing. */
  containerRef?: (el: HTMLDivElement | null) => void;
}

/** Distance from container edge (px) that triggers auto-scroll during drag. */
const SCROLL_ZONE = 40;
/** Pixels per animation frame to scroll when in the edge zone. */
const SCROLL_SPEED = 6;
/** Estimated height (px) of a DropZone + Card pair for the virtualizer. */
const ESTIMATED_ITEM_HEIGHT = 80;
/** Estimated height (px) of the trailing drop zone. */
const TRAILING_ZONE_HEIGHT = 6;
/** Minimum task count to activate virtualization. Below this, all items render directly. */
const VIRTUALIZE_THRESHOLD = 25;

// ---------------------------------------------------------------------------
// useColumnDragScroll — auto-scroll + drag-over handler for a column
// ---------------------------------------------------------------------------

/** Return value from useColumnDragScroll. */
interface ColumnDragScroll {
  setContainerRef: (el: HTMLDivElement | null) => void;
  handleDragOver: (e: React.DragEvent) => void;
}

/**
 * Manages edge-detection auto-scroll during drag and a merged container ref.
 *
 * When the pointer enters the top or bottom SCROLL_ZONE of the column, a rAF
 * loop scrolls the container. The returned `handleDragOver` is the single
 * handler for the column's scrollable div.
 */
function useScrollLoop(elRef: React.MutableRefObject<HTMLDivElement | null>): {
  start: (dir: -1 | 1) => void;
  stop: () => void;
} {
  const rafRef = useRef<number | null>(null);
  const dirRef = useRef(0);

  const stop = useCallback(() => {
    dirRef.current = 0;
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  }, []);

  const start = useCallback(
    (dir: -1 | 1) => {
      dirRef.current = dir;
      if (rafRef.current !== null) return;
      const tick = () => {
        if (dirRef.current === 0 || !elRef.current) {
          rafRef.current = null;
          return;
        }
        elRef.current.scrollBy({ top: dirRef.current * SCROLL_SPEED });
        rafRef.current = requestAnimationFrame(tick);
      };
      rafRef.current = requestAnimationFrame(tick);
    },
    [elRef],
  );

  useEffect(() => () => stop(), [stop]);
  return { start, stop };
}

function useColumnDragScroll(
  parentRef: ((el: HTMLDivElement | null) => void) | undefined,
): ColumnDragScroll {
  const elRef = useRef<HTMLDivElement>(null);
  const { start, stop } = useScrollLoop(
    elRef as React.MutableRefObject<HTMLDivElement | null>,
  );

  const setContainerRef = useCallback(
    (el: HTMLDivElement | null) => {
      (elRef as React.MutableRefObject<HTMLDivElement | null>).current = el;
      parentRef?.(el);
    },
    [parentRef],
  );

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      if (e.dataTransfer.types.includes("Files")) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (!elRef.current) return;
      const rect = elRef.current.getBoundingClientRect();
      if (e.clientY < rect.top + SCROLL_ZONE) start(-1);
      else if (e.clientY > rect.bottom - SCROLL_ZONE) start(1);
      else stop();
    },
    [start, stop],
  );

  return { setContainerRef, handleDragOver };
}

// ---------------------------------------------------------------------------
// useColumnLayout — drop zones for one column
// ---------------------------------------------------------------------------

/** Derived layout data used by ColumnView's render. */
interface ColumnLayout {
  zones: DropZoneDescriptor[];
}

/**
 * Compute the drop-zone descriptor list for one column.
 *
 * Returns one descriptor per insertion point: a "before" zone for each task
 * plus a trailing "after" zone (or a single "empty" zone when the column has
 * no tasks). The keyboard-nav claim predicates that used to live here are
 * gone — the spatial-nav layer derives those from the registered zone /
 * focusable rectangles instead.
 */
function useColumnLayout(props: ColumnViewProps): ColumnLayout {
  const { column, tasks } = props;
  const zones = useMemo(
    () =>
      computeDropZones(
        tasks.map((t) => t.id),
        column.id,
      ),
    [tasks, column.id],
  );
  return { zones };
}

// ---------------------------------------------------------------------------
// Placeholder FQMs — composed from each task's segment under the column FQ
// ---------------------------------------------------------------------------

/**
 * Compute the placeholder FQM for each off-screen task.
 *
 * Path-monikers identity makes per-task `SpatialKey` minting unnecessary:
 * the FQM IS the kernel's identifier, derived deterministically from the
 * task's segment and the enclosing column zone's FQM. Re-registration
 * across scroll-window changes hits the same key, so the kernel's
 * `apply_batch` overwrites the rect without disturbing `last_focused`.
 *
 * Returns a map of `taskId → FullyQualifiedMoniker`. Callers should not
 * mutate it.
 */
function useTaskPlaceholderFqs(
  tasks: Entity[],
  columnFq: FullyQualifiedMoniker | null,
): Map<string, FullyQualifiedMoniker> {
  return useMemo(() => {
    const map = new Map<string, FullyQualifiedMoniker>();
    if (columnFq === null) return map;
    for (const task of tasks) {
      map.set(task.id, composeFq(columnFq, asSegment(task.moniker)));
    }
    return map;
  }, [tasks, columnFq]);
}

// ---------------------------------------------------------------------------
// useVisibleIndexSet — Set of indices currently in the virtualizer's window
// ---------------------------------------------------------------------------

/**
 * Read the set of task indices currently in the virtualizer's visible
 * window.
 *
 * Excludes the trailing-zone pseudo-index (`vi.index === taskCount`) so the
 * placeholder hook only sees real-task indices. The returned set is
 * recomputed when the virtualizer's range changes — `getVirtualItems()` is
 * stable across re-renders that don't shift the window, so the dependent
 * `useEffect` in `usePlaceholderRegistration` only fires on real
 * scroll-window changes.
 */
function useVisibleIndexSet<T extends Element>(
  virtualizer: Virtualizer<T, Element>,
  taskCount: number,
): Set<number> {
  const items = virtualizer.getVirtualItems();
  return useMemo(() => {
    const set = new Set<number>();
    for (const vi of items) {
      if (vi.index < taskCount) set.add(vi.index);
    }
    return set;
    // `items` identity is stable across renders that don't change the
    // window, which is exactly the cache-key we want.
  }, [items, taskCount]);
}

// ---------------------------------------------------------------------------
// usePlaceholderRegistration — register Vec<RegisterEntry> for off-screen rows
// ---------------------------------------------------------------------------

/** Inputs for `usePlaceholderRegistration`. */
interface PlaceholderRegistrationInputs {
  tasks: Entity[];
  /** FQM map keyed by task ID — composed once per task list. */
  taskFqs: Map<string, FullyQualifiedMoniker>;
  /** Set of indices currently in the virtualizer's visible window. */
  visibleIndices: Set<number>;
  /** Layer this column lives in, or `null` outside the spatial-nav stack. */
  layerFq: FullyQualifiedMoniker | null;
  /** Enclosing zone (the column's own zone), or `null` when the column is
   *  at the layer root. */
  parentZone: FullyQualifiedMoniker | null;
  /** The scrollable container — used to derive a sensible placeholder rect. */
  scrollEl: HTMLDivElement | null;
  /**
   * Current scroll offset reported by the virtualizer. Subtracted from
   * placeholder y-coordinates so placeholder rects share the viewport
   * coordinate frame that real-mounted card rects use (their rects come
   * from `getBoundingClientRect()`, which is viewport-relative). Without
   * this, an above-viewport placeholder would land at the visible top
   * edge while a real card on the same content row also sits there —
   * beam search would see overlapping rects in completely different
   * "rows" and pick wrong candidates.
   *
   * `null` when the virtualizer hasn't observed an offset yet (first
   * render before the scroll observer fires). Treated as `0`.
   */
  scrollOffset: number | null;
}

/**
 * Wire-shape companion to the Rust `RegisterEntry::Scope` enum variant.
 *
 * Mirrors the kernel-side `#[serde(tag = "kind", rename_all = "snake_case")]`
 * discriminator. Task placeholders register as `Scope` (matching the
 * kind that `EntityCard` uses for its own `<FocusScope>` leaf) so
 * kind-stability holds when the real mount eventually overwrites the
 * placeholder. Cards must be leaves so the unified cascade's iter-0 /
 * iter-1 trajectory works as the user expects (iter 0 finds in-column
 * card peers; iter 1 escalates to the card's parent column zone and
 * lands on the neighbouring column zone). See the docstring on
 * `<EntityCard>` and the kernel test
 * `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`.
 */
interface ScopeRegisterEntry {
  kind: "scope";
  fq: FullyQualifiedMoniker;
  segment: string;
  rect: { x: number; y: number; width: number; height: number };
  layer_fq: FullyQualifiedMoniker;
  parent_zone: FullyQualifiedMoniker | null;
  overrides: Record<string, never>;
}

/**
 * Register placeholder zones for off-screen tasks via
 * `spatial_register_batch`, and unregister placeholders for tasks that
 * have just become visible.
 *
 * Why placeholders exist: the virtualizer only mounts cards in the visible
 * window, so without placeholders the spatial graph has no entries below
 * (or above) the visible range and `nav.down` past the last visible row
 * dead-ends. Placeholders give the navigator candidate rectangles for
 * every task — when nav lands on a placeholder the column scrolls to
 * bring the real card into view (caller responsibility — that wiring sits
 * outside this hook).
 *
 * Idempotency: the kernel's `apply_batch` is idempotent on `SpatialKey` —
 * re-registering an existing key overwrites its rect and preserves any
 * `last_focused` slot. Re-running this hook on every scroll is therefore
 * cheap; the only real work is the IPC round-trip.
 *
 * Parallel-safety: when the spatial-nav stack is absent (`layerKey ===
 * null`, e.g. a unit test that does not mount `<SpatialFocusProvider>`),
 * the hook is a no-op so column-view renders without crashing in those
 * tests.
 */
function usePlaceholderRegistration(inputs: PlaceholderRegistrationInputs) {
  const {
    tasks,
    taskFqs,
    visibleIndices,
    layerFq,
    parentZone,
    scrollEl,
    scrollOffset,
  } = inputs;
  const spatial = useOptionalSpatialFocusActions();

  // Track each registered placeholder as `(taskId → FullyQualifiedMoniker)`
  // so the unregister path is self-contained even if the surrounding
  // `taskFqs` map has dropped a deleted task's entry by the time this
  // effect runs.
  const registeredRef = useRef<Map<string, FullyQualifiedMoniker>>(new Map());

  useEffect(() => {
    // Outside the spatial-nav stack — nothing to register against. Bail
    // out early; the tests that mount column-view without a provider hit
    // this path and stay quiet.
    if (!spatial || !layerFq) return;

    // Build the placeholder set for the current off-screen tasks, plus
    // the wire-format batch entries to ship across the IPC boundary.
    const wantPlaceholder = new Map<string, FullyQualifiedMoniker>();
    const offScreen: ScopeRegisterEntry[] = [];

    // Skip entirely when we don't have a real scroll element to anchor
    // off — the next render will refire this effect once the ref
    // attaches. Using a fake fallback rect would mislead beam search
    // (real-mounted cards use viewport-relative rects from
    // `getBoundingClientRect()`, so a fabricated one risks colliding).
    const rect = scrollEl?.getBoundingClientRect();
    if (rect) {
      const baseX = rect.x;
      const baseY = rect.y;
      const width = rect.width;
      // `scrollOffset` is the virtualizer's content-y of the visible
      // top edge. Real cards' rects come from `getBoundingClientRect()`
      // which is viewport-relative — they live at viewport-y `baseY +
      // (item.start - scrollOffset)`. Mirror that for placeholders so
      // both systems share one coordinate frame: `baseY + i * H -
      // scrollOffset`. Without this subtraction, an above-viewport
      // placeholder would land at `baseY` (the visible top edge) while
      // a real card on row 50 also sits near `baseY` — beam search
      // would see overlapping rects in completely different "rows".
      const offset = scrollOffset ?? 0;

      for (let i = 0; i < tasks.length; i++) {
        if (visibleIndices.has(i)) continue;
        const task = tasks[i];
        const fq = taskFqs.get(task.id);
        if (!fq) continue;
        wantPlaceholder.set(task.id, fq);
        offScreen.push({
          kind: "scope",
          fq,
          segment: task.moniker,
          rect: {
            x: asPixels(baseX),
            y: asPixels(baseY + i * ESTIMATED_ITEM_HEIGHT - offset),
            width: asPixels(width),
            height: asPixels(ESTIMATED_ITEM_HEIGHT),
          },
          layer_fq: layerFq,
          parent_zone: parentZone,
          overrides: {},
        });
      }
    }

    // Unregister placeholders that should no longer exist — IDs that
    // either became visible (real card now owns the FQM) or left the
    // task list. Done before the batch register so the kernel sees the
    // unregisters first if the same FQM is being recycled.
    const previouslyRegistered = registeredRef.current;
    for (const [id, fq] of previouslyRegistered) {
      if (wantPlaceholder.has(id)) continue;
      spatial.unregisterScope(fq).catch((err) => {
        console.error("[column-view] placeholder unregister failed", err);
      });
    }

    // Register / refresh placeholders for the current off-screen set.
    // Sent as one IPC round-trip so twenty placeholders collapse into a
    // single registry lock.
    if (offScreen.length > 0) {
      invoke("spatial_register_batch", { entries: offScreen }).catch((err) => {
        console.error("[column-view] placeholder batch register failed", err);
      });
    }

    registeredRef.current = wantPlaceholder;
  }, [
    tasks,
    taskFqs,
    visibleIndices,
    layerFq,
    parentZone,
    scrollEl,
    scrollOffset,
    spatial,
  ]);

  // Unregister every live placeholder when the column unmounts so a torn
  // column does not leak stale entries into the registry.
  useEffect(() => {
    const registered = registeredRef;
    return () => {
      if (!spatial) return;
      for (const [, fq] of registered.current) {
        spatial.unregisterScope(fq).catch((err) => {
          console.error("[column-view] placeholder cleanup failed", err);
        });
      }
      registered.current.clear();
    };
  }, [spatial]);
}

// ---------------------------------------------------------------------------
// ColumnView — main exported component
// ---------------------------------------------------------------------------

/**
 * Renders a single column in the board view with drag-drop, focus highlight,
 * and keyboard navigation support.
 */
interface ColumnBodyProps {
  props: ColumnViewProps;
  columnMoniker: string;
  layout: ColumnLayout;
  dragScroll: ColumnDragScroll;
  nameFieldDef: import("@/types/kanban").FieldDef | undefined;
  editingName: boolean;
  setEditingName: (v: boolean) => void;
  setFocus: (fq: FullyQualifiedMoniker | null) => void;
}

/**
 * Renders the column header and the virtualized card list as siblings.
 *
 * The flex chain (`flex flex-col` parent + `flex-1 overflow-y-auto` on the
 * scroll container) is established by the outer `<FocusScope>`
 * directly — its `className` lands on the spatial primitive's root and
 * children render as direct layout children. ColumnBody therefore returns
 * a `<>` fragment so its children participate in that flex chain without
 * an intermediate div.
 */
function ColumnBody({
  props,
  columnMoniker,
  layout,
  dragScroll,
  nameFieldDef,
  editingName,
  setEditingName,
  setFocus,
}: ColumnBodyProps): React.ReactElement {
  const {
    column,
    tasks,
    onAddTask,
    onTaskDragStart,
    onTaskDragEnd,
    dragTaskId,
  } = props;
  const handleZoneDrop = useCallback(
    (descriptor: DropZoneDescriptor, taskData: string) =>
      props.onDrop?.(descriptor, taskData),
    [props],
  );
  return (
    <>
      <ColumnHeader
        column={column}
        columnMoniker={columnMoniker}
        nameFieldDef={nameFieldDef}
        editingName={editingName}
        setEditingName={setEditingName}
        taskCount={tasks.length}
        onAddTask={onAddTask}
        setFocus={setFocus}
      />
      <VirtualizedCardList
        tasks={tasks}
        zones={layout.zones}
        dragTaskId={dragTaskId}
        onZoneDrop={handleZoneDrop}
        onTaskDragStart={onTaskDragStart}
        onTaskDragEnd={onTaskDragEnd}
        containerRef={dragScroll.setContainerRef}
        onDragOver={dragScroll.handleDragOver}
      />
    </>
  );
}

export const ColumnView = memo(function ColumnView(props: ColumnViewProps) {
  const { column } = props;
  const columnMoniker = column.moniker;
  const { getFieldDef } = useSchema();
  const nameFieldDef = getFieldDef("column", "name");
  const [editingName, setEditingName] = useState(false);
  const { setFocus } = useFocusActions();
  const layout = useColumnLayout(props);
  const dragScroll = useColumnDragScroll(props.containerRef);

  // FocusZone's `className` lands on its outer primitive `<div>` and the
  // primitive renders children as direct layout children — the `flex flex-col`
  // chain established here propagates straight into ColumnHeader (header at
  // top) and VirtualizedCardList (`flex-1 overflow-y-auto` fills the rest).
  // `flex-1 min-h-0` participates in the SortableColumn parent's flex chain
  // so the column takes its share of the board's width and gets a bounded
  // scroll height for `useVirtualizer`'s windowing.
  //
  // The column body is a zone (parent of cards), not a leaf — descendants
  // (`task:{id}` card scopes, the column-name `<Field>` zone in the header)
  // register `parentZone = column-zone-key` via `FocusZoneContext`, so beam
  // search treats the column's children as in-zone candidates.
  //
  // `showFocusBar` defaults to `true`. The column is a sized, distinct entity
  // — when the user clicks its body or drills out from a card with Escape,
  // they need a visible indicator on the column itself. Container zones that
  // are viewport-sized chrome (board, perspective, view, navbar) suppress the
  // bar because a focus rectangle around the entire viewport would be visual
  // noise; columns are bounded boxes inside the board, so they advertise
  // their focus the same way cards and field rows do. The
  // `<FocusIndicator>` renders along the left edge of the column box at full
  // height — see `kanban-app/ui/src/components/focus-indicator.tsx`.
  // The column body wraps a real entity (`column:<id>` moniker), so
  // double-clicking the column whitespace should open the inspector
  // for that column. The `<Inspectable>` wrapper owns the
  // `useDispatchCommand("ui.inspect")` hook and its `onDoubleClick`
  // handler; the spatial primitive `<FocusZone>` stays pure-spatial.
  // The architectural guard
  // (`focus-architecture.guards.node.test.ts`, Guards B + C) enforces
  // this for every entity-monikered zone.
  return (
    <Inspectable moniker={asSegment(columnMoniker)}>
      <FocusZone
        moniker={asSegment(columnMoniker)}
        className="flex flex-col flex-1 min-h-0 min-w-[24em] max-w-[48em] shrink-0"
      >
        <ColumnBody
          props={props}
          columnMoniker={columnMoniker}
          layout={layout}
          dragScroll={dragScroll}
          nameFieldDef={nameFieldDef}
          editingName={editingName}
          setEditingName={setEditingName}
          setFocus={setFocus}
        />
      </FocusZone>
    </Inspectable>
  );
});

// ---------------------------------------------------------------------------
// ColumnHeader — name field, badge count, and add-task button
// ---------------------------------------------------------------------------

interface ColumnHeaderProps {
  column: Entity;
  columnMoniker: string;
  nameFieldDef: import("@/types/kanban").FieldDef | undefined;
  editingName: boolean;
  setEditingName: (v: boolean) => void;
  taskCount: number;
  onAddTask?: (columnId: string) => void;
  setFocus: (fq: FullyQualifiedMoniker | null) => void;
}

/** Renders the column header row with name, task count badge, and add button. */
interface ColumnNameFieldProps {
  column: Entity;
  nameFieldDef: import("@/types/kanban").FieldDef | undefined;
  editingName: boolean;
  setEditingName: (v: boolean) => void;
}

function ColumnNameField({
  column,
  nameFieldDef,
  editingName,
  setEditingName,
}: ColumnNameFieldProps): React.ReactElement {
  if (!nameFieldDef) {
    return (
      <span className="text-sm font-semibold text-foreground">
        {getStr(column, "name")}
      </span>
    );
  }
  // `showFocusBar` opts the field zone into rendering its own
  // `<FocusIndicator>`. The column header has no enclosing focus
  // chrome around just the name surface (the column body's own bar
  // sits on the column zone, not on its descendants), so the field
  // zone is the user's only focus cue when the name is the spatial
  // focus. This replaces the focus indicator the synthetic outer
  // `<FocusScope>` rendered before card 01KQAWVDS931PADB0559F2TVCS
  // collapsed it.
  return (
    <Field
      fieldDef={nameFieldDef}
      entityType="column"
      entityId={column.id}
      mode="compact"
      editing={editingName}
      onEdit={() => setEditingName(true)}
      onDone={() => setEditingName(false)}
      onCancel={() => setEditingName(false)}
      showFocusBar
    />
  );
}

function ColumnHeader({
  column,
  nameFieldDef,
  editingName,
  setEditingName,
  taskCount,
  onAddTask,
  setFocus,
}: ColumnHeaderProps) {
  // Inside the column's `<FocusZone>` body — `useOptionalFullyQualifiedMoniker`
  // returns the column's FQM. Compose the column-zone FQM (for the
  // AddTaskButton's setFocus call) under it.
  const columnFq = useOptionalFullyQualifiedMoniker();
  // The column-name surface is registered exactly once — by the inner
  // `<Field>` component as a `<FocusZone moniker="field:column:<id>.name">`.
  // The Field already owns its own `<Inspectable>` wrap, click →
  // `spatial_focus(fq)` handler, and edit-mode plumbing, so the column
  // header renders `<ColumnNameField>` directly without a synthetic
  // outer wrapper. Double-click on the displayed column name enters
  // edit mode via `FieldDisplayContent`'s `onClick={onEdit}` surface;
  // once the editor mounts, `<Inspectable>`'s editable-surface skip
  // suppresses the inspector dispatch on the second click.
  return (
    <div className="px-3 py-2 flex items-center gap-2 rounded">
      <ColumnNameField
        column={column}
        nameFieldDef={nameFieldDef}
        editingName={editingName}
        setEditingName={setEditingName}
      />
      <Badge variant="secondary">{taskCount}</Badge>
      <div className="flex-1" />
      {onAddTask && (
        <AddTaskButton
          columnId={column.id}
          columnName={getStr(column, "name") ?? ""}
          columnFq={columnFq}
          onAddTask={onAddTask}
          setFocus={setFocus}
        />
      )}
    </div>
  );
}

/**
 * The "+" button in the column header that adds a new task.
 *
 * Wraps a `<Pressable asChild>` inside the existing `<TooltipTrigger asChild>`
 * so the button gains both keyboard reachability AND Enter / Space
 * activation. Pre-migration this was a bare `<button>` with no
 * `<FocusScope>` — keyboard users could not focus it at all.
 *
 * Moniker `ui:column.add-task:{columnId}` — entity-disambiguated like
 * `card.inspect:{id}`, namespaced under `ui:column.*` so future
 * column-level icon buttons (filter, sort, etc.) can compose under the
 * same prefix.
 */
function AddTaskButton({
  columnId,
  columnName,
  columnFq,
  onAddTask,
  setFocus,
}: {
  columnId: string;
  columnName: string;
  columnFq: FullyQualifiedMoniker | null;
  onAddTask: (columnId: string) => void;
  setFocus: (fq: FullyQualifiedMoniker | null) => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Pressable
          asChild
          moniker={asSegment(`ui:column.add-task:${columnId}`)}
          ariaLabel={`Add task to ${columnName}`}
          onPress={() => {
            if (columnFq) setFocus(columnFq);
            onAddTask(columnId);
          }}
        >
          <button
            type="button"
            className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
          >
            <Plus className="h-4 w-4" />
          </button>
        </Pressable>
      </TooltipTrigger>
      <TooltipContent>{`Add task to ${columnName}`}</TooltipContent>
    </Tooltip>
  );
}

// ---------------------------------------------------------------------------
// VirtualizedCardList — renders only visible card+zone pairs
// ---------------------------------------------------------------------------

interface VirtualizedCardListProps {
  tasks: Entity[];
  zones: DropZoneDescriptor[];
  dragTaskId?: string | null;
  onZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
  onTaskDragStart?: (entity: Entity) => void;
  onTaskDragEnd?: (entity: Entity, dropEffect: string) => void;
  containerRef: (el: HTMLDivElement | null) => void;
  onDragOver: (e: React.DragEvent) => void;
}

const CONTAINER_CLASS =
  "flex-1 overflow-y-auto [scrollbar-gutter:stable] px-2 pt-1 pb-2 m-1 rounded-lg border-2 border-transparent";

/**
 * Renders the card + drop-zone list inside a column.
 *
 * Routes to an empty placeholder, a direct-rendered small list, or a
 * virtualized list depending on task count.
 */
const VirtualizedCardList = memo(function VirtualizedCardList(
  props: VirtualizedCardListProps,
) {
  const { tasks, containerRef: containerRefProp } = props;
  const scrollRef = useRef<HTMLDivElement>(null);

  const setRef = useCallback(
    (el: HTMLDivElement | null) => {
      (scrollRef as React.MutableRefObject<HTMLDivElement | null>).current = el;
      containerRefProp(el);
    },
    [containerRefProp],
  );

  if (tasks.length === 0) {
    return <EmptyColumn {...props} setRef={setRef} />;
  }
  if (tasks.length < VIRTUALIZE_THRESHOLD) {
    return <SmallCardList {...props} setRef={setRef} />;
  }
  return (
    <VirtualColumn
      {...props}
      scrollRef={scrollRef}
      setRef={setRef}
      containerClass={CONTAINER_CLASS}
    />
  );
});

/** Single drop zone shown when the column has no tasks. */
function EmptyColumn({
  zones,
  dragTaskId,
  onZoneDrop,
  onDragOver,
  setRef,
}: VirtualizedCardListProps & { setRef: (el: HTMLDivElement | null) => void }) {
  return (
    <div ref={setRef} className={CONTAINER_CLASS} onDragOver={onDragOver}>
      <DropZone
        descriptor={zones[0]}
        dragTaskId={dragTaskId}
        onDrop={onZoneDrop}
        variant="empty-column"
      />
    </div>
  );
}

/** Renders all card+zone pairs directly (no virtualization overhead). */
function SmallCardList({
  tasks,
  zones,
  dragTaskId,
  onZoneDrop,
  onTaskDragStart,
  onTaskDragEnd,
  onDragOver,
  setRef,
}: VirtualizedCardListProps & { setRef: (el: HTMLDivElement | null) => void }) {
  return (
    <div ref={setRef} className={CONTAINER_CLASS} onDragOver={onDragOver}>
      {tasks.map((entity, i) => (
        <div key={entity.id}>
          <DropZone
            descriptor={zones[i]}
            dragTaskId={dragTaskId}
            onDrop={onZoneDrop}
          />
          <div className="rounded">
            <DraggableTaskCard
              entity={entity}
              onDragStart={onTaskDragStart}
              onDragEnd={onTaskDragEnd}
            />
          </div>
        </div>
      ))}
      <DropZone
        descriptor={zones[zones.length - 1]}
        dragTaskId={dragTaskId}
        onDrop={onZoneDrop}
      />
    </div>
  );
}

interface VirtualColumnProps {
  tasks: Entity[];
  zones: DropZoneDescriptor[];
  dragTaskId?: string | null;
  onZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
  onTaskDragStart?: (entity: Entity) => void;
  onTaskDragEnd?: (entity: Entity, dropEffect: string) => void;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  setRef: (el: HTMLDivElement | null) => void;
  containerClass: string;
  onDragOver: (e: React.DragEvent) => void;
}

/** Absolute positioning style for a virtual row at `startPx`. */
function virtualRowStyle(startPx: number): React.CSSProperties {
  return {
    position: "absolute",
    top: 0,
    left: 0,
    width: "100%",
    transform: `translateY(${startPx}px)`,
  };
}

interface VirtualRowProps {
  vr: { index: number; start: number };
  tasks: Entity[];
  zones: DropZoneDescriptor[];
  dragTaskId?: string | null;
  onZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
  onTaskDragStart?: (entity: Entity) => void;
  onTaskDragEnd?: (entity: Entity, dropEffect: string) => void;
  measureElement: (node: HTMLElement | null) => void;
}

function VirtualRow(props: VirtualRowProps): React.ReactElement {
  const { vr, tasks, zones, dragTaskId, onZoneDrop, measureElement } = props;
  if (vr.index === tasks.length) {
    return (
      <div
        key="trailing-zone"
        data-index={vr.index}
        ref={measureElement}
        style={virtualRowStyle(vr.start)}
      >
        <DropZone
          descriptor={zones[zones.length - 1]}
          dragTaskId={dragTaskId}
          onDrop={onZoneDrop}
        />
      </div>
    );
  }
  const entity = tasks[vr.index];
  return (
    <div
      key={entity.id}
      data-index={vr.index}
      ref={measureElement}
      style={virtualRowStyle(vr.start)}
    >
      <DropZone
        descriptor={zones[vr.index]}
        dragTaskId={dragTaskId}
        onDrop={onZoneDrop}
      />
      <div className="rounded">
        <DraggableTaskCard
          entity={entity}
          onDragStart={props.onTaskDragStart}
          onDragEnd={props.onTaskDragEnd}
        />
      </div>
    </div>
  );
}

/** Inner component that calls useVirtualizer (hook must be unconditional). */
function VirtualColumn(props: VirtualColumnProps) {
  const { tasks, scrollRef, setRef, containerClass, onDragOver } = props;
  const virtualizer = useVirtualizer({
    count: tasks.length + 1,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) =>
      i < tasks.length ? ESTIMATED_ITEM_HEIGHT : TRAILING_ZONE_HEIGHT,
    overscan: 5,
  });

  // Spatial-nav placeholder wiring. Off-screen rows have no mounted
  // primitives, so without placeholders the spatial graph dead-ends at the
  // visible window. The placeholder FQMs are composed deterministically
  // from each task's segment under the column's FQM, so re-registers
  // across scroll-window changes hit the same key and the kernel's
  // idempotent `apply_batch` overwrites the rect without losing drill-
  // out memory.
  //
  // Hooks live deep inside the column body (here, not on `<ColumnView>`)
  // so they sit alongside the virtualizer they depend on — keeping the
  // outer `<FocusScope>` wrap untouched (the FocusScope wrap was rewritten
  // when `FocusScopeBody` was removed).
  const layerFq = useOptionalEnclosingLayerFq();
  const parentZone = useParentZoneFq();
  const columnFq = useOptionalFullyQualifiedMoniker();
  const taskFqs = useTaskPlaceholderFqs(tasks, columnFq);
  const visibleIndices = useVisibleIndexSet(virtualizer, tasks.length);
  usePlaceholderRegistration({
    tasks,
    taskFqs,
    visibleIndices,
    layerFq,
    parentZone,
    scrollEl: scrollRef.current,
    scrollOffset: virtualizer.scrollOffset,
  });

  return (
    <div ref={setRef} className={containerClass} onDragOver={onDragOver}>
      <div
        style={{
          height: virtualizer.getTotalSize(),
          width: "100%",
          position: "relative",
        }}
      >
        {virtualizer.getVirtualItems().map((vr) => (
          <VirtualRow
            key={
              vr.index === tasks.length ? "trailing-zone" : tasks[vr.index].id
            }
            vr={vr}
            tasks={tasks}
            zones={props.zones}
            dragTaskId={props.dragTaskId}
            onZoneDrop={props.onZoneDrop}
            onTaskDragStart={props.onTaskDragStart}
            onTaskDragEnd={props.onTaskDragEnd}
            measureElement={virtualizer.measureElement}
          />
        ))}
      </div>
    </div>
  );
}
