import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Plus } from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { listen } from "@tauri-apps/api/event";
import { DropZone } from "@/components/drop-zone";
import { computeDropZones, type DropZoneDescriptor } from "@/lib/drop-zones";
import { Field } from "@/components/fields/field";
import { DraggableTaskCard } from "@/components/sortable-task-card";
import { FocusScope } from "@/components/focus-scope";
import { Inspectable } from "@/components/inspectable";
import { Pressable } from "@/components/pressable";
import { useOptionalFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";

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
  /**
   * Stops the auto-scroll rAF loop. Wired to the column scroller's
   * `onDragLeave` so the loop ends when the pointer leaves the column
   * without a drop. The hook also subscribes to the global `drag-ended`
   * event (emitted by `useTaskDragHandlers` after every task drag
   * completes) so a drop near a column edge — where `dragover` stops
   * firing while `dirRef.current` is still non-zero — does not leave
   * the loop running and fight the user's scroll.
   */
  handleDragLeave: () => void;
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

  // The auto-scroll rAF loop is only stopped from inside `handleDragOver`
  // when the pointer is in the middle of the column. If the user drops a
  // card with the pointer still in the top or bottom SCROLL_ZONE,
  // `dragover` stops firing while `dirRef.current` is still non-zero —
  // the loop keeps calling `scrollBy` every frame and fights the user's
  // post-drop scroll. `useTaskDragHandlers` already emits a global
  // `drag-ended` event the moment a task drag completes (success or
  // cancel); subscribing to it here is the canonical "drag is over"
  // signal and stops the loop regardless of where the pointer landed.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void listen("drag-ended", () => stop()).then((un) => {
      if (cancelled) un();
      else unlisten = un;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [stop]);

  // Stop the loop when the pointer leaves the column scroller without a
  // drop — the user dragged out to a different column or off the board.
  // Without this, the loop would keep running until `dragend` fires on
  // the source card, which is observably laggy on long drags.
  const handleDragLeave = useCallback(() => {
    stop();
  }, [stop]);

  return { setContainerRef, handleDragOver, handleDragLeave };
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
        onDragLeave={dragScroll.handleDragLeave}
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
  // register `parentZone = column-zone-key` via `FocusScopeContext`, so beam
  // search treats the column's children as in-zone candidates.
  //
  // `showFocus={false}` — the column is a NON-FOCUSABLE structural zone. This
  // is what makes cards the navigation tier: a card's nearest *focusable*
  // ancestor is `None` (its column and the board well are both zones), so all
  // cards across all columns share one tier and arrow keys glide smoothly
  // card→card, including across columns, without ever landing on a column or
  // diving into a card's inner fields (those sit a tier deeper and are reached
  // only by drill-in). Marking the column focusable would split each column
  // into its own tier and block cross-column arrow nav. This matches the other
  // structural containers (board, perspective, view, navbar), which are all
  // non-focusable zones.
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
      <FocusScope
        moniker={asSegment(columnMoniker)}
        showFocus={false}
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
        />
      </FocusScope>
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
  // `showFocus` opts the field zone into rendering its own
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
      showFocus
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
}: ColumnHeaderProps) {
  // Inside the column's `<FocusZone>` body — `useOptionalFullyQualifiedMoniker`
  // returns the column's FQM. The AddTaskButton dispatches
  // `nav.focus({ args: { fq: columnFq } })` (card
  // `01KR7CDEFWWVF4WH0BCHE8Y21J`) so the new task lands inside the
  // user-targeted column.
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
}: {
  columnId: string;
  columnName: string;
  columnFq: FullyQualifiedMoniker | null;
  onAddTask: (columnId: string) => void;
}) {
  // Card `01KR7CDEFWWVF4WH0BCHE8Y21J`: focus claims flow through
  // `nav.focus`, the single auditable command that wraps the
  // kernel-facing `setFocus` primitive. The "+" button moves focus to
  // the column zone before delegating to `onAddTask` so the new task
  // lands inside the user-targeted column.
  const dispatchNavFocus = useDispatchCommand("nav.focus");
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Pressable
          asChild
          moniker={asSegment(`ui:column.add-task:${columnId}`)}
          ariaLabel={`Add task to ${columnName}`}
          onPress={() => {
            if (columnFq) {
              void dispatchNavFocus({ args: { fq: columnFq } }).catch((err) =>
                console.error("[AddTaskButton] nav.focus dispatch failed", err),
              );
            }
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
  onDragLeave: () => void;
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
  onDragLeave,
  setRef,
}: VirtualizedCardListProps & { setRef: (el: HTMLDivElement | null) => void }) {
  return (
    <div
      ref={setRef}
      className={CONTAINER_CLASS}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
    >
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
  onDragLeave,
  setRef,
}: VirtualizedCardListProps & { setRef: (el: HTMLDivElement | null) => void }) {
  return (
    <div
      ref={setRef}
      className={CONTAINER_CLASS}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
    >
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
  onDragLeave: () => void;
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
  const { tasks, scrollRef, setRef, containerClass, onDragOver, onDragLeave } =
    props;
  const virtualizer = useVirtualizer({
    count: tasks.length + 1,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) =>
      i < tasks.length ? ESTIMATED_ITEM_HEIGHT : TRAILING_ZONE_HEIGHT,
    overscan: 5,
  });

  return (
    <div
      ref={setRef}
      className={containerClass}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
    >
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
