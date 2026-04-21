import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Plus } from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { DropZone } from "@/components/drop-zone";
import { computeDropZones, type DropZoneDescriptor } from "@/lib/drop-zones";
import { Field } from "@/components/fields/field";
import { DraggableTaskCard } from "@/components/sortable-task-card";
import { FocusScope } from "@/components/focus-scope";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useSchema } from "@/lib/schema-context";
import {
  useEntityFocus,
  type ClaimPredicate,
} from "@/lib/entity-focus-context";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

/**
 * Props for {@link ColumnView} — one column in the kanban board.
 *
 * Carries the column entity and its pre-sorted tasks plus the navigation
 * monikers of neighboring columns so the column can compute focus-claim
 * predicates for keyboard nav (up/down within the column, left/right across
 * columns, first/last at board edges). Drag/drop callbacks are optional and
 * wired by the parent board to route drops back through the command layer.
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
  /**
   * Task monikers in the column to the left (in order), or empty array.
   * Used to compute nav.right claimWhen predicates for cross-column navigation.
   */
  leftColumnTaskMonikers?: string[];
  /** Column header moniker for the column to the left, or null. */
  leftColumnHeaderMoniker?: string | null;
  /**
   * Task monikers in the column to the right (in order), or empty array.
   * Used to compute nav.left claimWhen predicates for cross-column navigation.
   */
  rightColumnTaskMonikers?: string[];
  /** Column header moniker for the column to the right, or null. */
  rightColumnHeaderMoniker?: string | null;
  /**
   * All task monikers on the board, across all columns.
   * Used for nav.first/nav.last predicates (any board card focused -> claim).
   */
  allBoardTaskMonikers?: Set<string>;
  /** All column header monikers on the board. Used for nav.first/nav.last. */
  allBoardHeaderMonikers?: Set<string>;
  /** Whether this column contains the overall first card on the board. */
  isFirstColumn?: boolean;
  /** Whether this column contains the overall last card on the board. */
  isLastColumn?: boolean;
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
// Navigation predicate hooks — extracted from ColumnView to keep it focused
// ---------------------------------------------------------------------------

/** Params for the header navigation predicate hook. */
interface HeaderClaimParams {
  taskMonikers: string[];
  rightColumnHeaderMoniker: string | null;
  leftColumnHeaderMoniker: string | null;
  rightColumnTaskMonikers: string[];
  leftColumnTaskMonikers: string[];
  taskCount: number;
  isFirstColumn: boolean;
  isLastColumn: boolean;
  columnNameMoniker: string;
  isBoardElement: (f: string | null) => boolean;
}

/**
 * Compute claimWhen predicates for the column name field FocusScope.
 *
 * These predicates tell broadcastNavCommand when this header should claim
 * focus: nav.up from the first card, nav.left/right from adjacent headers,
 * cross-column nav into empty columns, and nav.first/last at board edges.
 */
function useHeaderClaimPredicates(p: HeaderClaimParams): ClaimPredicate[] {
  return useMemo<ClaimPredicate[]>(() => {
    const predicates: ClaimPredicate[] = [];

    if (p.taskMonikers.length > 0) {
      const firstCard = p.taskMonikers[0];
      predicates.push({ command: "nav.up", when: (f) => f === firstCard });
    }
    if (p.rightColumnHeaderMoniker) {
      predicates.push({
        command: "nav.left",
        when: (f) => f === p.rightColumnHeaderMoniker,
      });
    }
    if (p.leftColumnHeaderMoniker) {
      predicates.push({
        command: "nav.right",
        when: (f) => f === p.leftColumnHeaderMoniker,
      });
    }
    if (p.taskCount === 0) {
      for (const m of p.rightColumnTaskMonikers) {
        predicates.push({ command: "nav.left", when: (f) => f === m });
      }
      for (const m of p.leftColumnTaskMonikers) {
        predicates.push({ command: "nav.right", when: (f) => f === m });
      }
    }
    if (p.isFirstColumn && p.taskCount === 0) {
      predicates.push({
        command: "nav.first",
        when: (f) => p.isBoardElement(f) && f !== p.columnNameMoniker,
      });
    }
    if (p.isLastColumn && p.taskCount === 0) {
      predicates.push({
        command: "nav.last",
        when: (f) => p.isBoardElement(f) && f !== p.columnNameMoniker,
      });
    }
    return predicates;
  }, [p]);
}

/** Params for the per-card navigation predicate hook. */
interface CardClaimParams {
  taskMonikers: string[];
  columnMoniker: string;
  columnNameMoniker: string;
  rightColumnTaskMonikers: string[];
  leftColumnTaskMonikers: string[];
  isFirstColumn: boolean;
  isLastColumn: boolean;
  isBoardElement: (f: string | null) => boolean;
}

/**
 * Compute claimWhen predicates for each card in the column, indexed by position.
 *
 * Each card declares when it should claim focus for nav.up/down (within column),
 * nav.left/right (cross-column with clamped index), and nav.first/last (board edges).
 */
function verticalNavPredicates(
  i: number,
  p: CardClaimParams,
): ClaimPredicate[] {
  const out: ClaimPredicate[] = [];
  if (i === 0) {
    out.push({
      command: "nav.down",
      when: (f) => f === p.columnNameMoniker || f === p.columnMoniker,
    });
  } else {
    const prev = p.taskMonikers[i - 1];
    out.push({ command: "nav.down", when: (f) => f === prev });
  }
  if (i < p.taskMonikers.length - 1) {
    const next = p.taskMonikers[i + 1];
    out.push({ command: "nav.up", when: (f) => f === next });
  }
  return out;
}

function horizontalNavPredicates(
  i: number,
  p: CardClaimParams,
): ClaimPredicate[] {
  const out: ClaimPredicate[] = [];
  for (let ri = 0; ri < p.rightColumnTaskMonikers.length; ri++) {
    const rightMoniker = p.rightColumnTaskMonikers[ri];
    if (Math.min(ri, p.taskMonikers.length - 1) === i) {
      out.push({ command: "nav.left", when: (f) => f === rightMoniker });
    }
  }
  for (let li = 0; li < p.leftColumnTaskMonikers.length; li++) {
    const leftMoniker = p.leftColumnTaskMonikers[li];
    if (Math.min(li, p.taskMonikers.length - 1) === i) {
      out.push({ command: "nav.right", when: (f) => f === leftMoniker });
    }
  }
  return out;
}

function edgeNavPredicates(i: number, p: CardClaimParams): ClaimPredicate[] {
  const out: ClaimPredicate[] = [];
  if (p.isFirstColumn && i === 0) {
    out.push({
      command: "nav.first",
      when: (f) => p.isBoardElement(f) && f !== p.taskMonikers[0],
    });
  }
  if (p.isLastColumn && i === p.taskMonikers.length - 1) {
    out.push({
      command: "nav.last",
      when: (f) =>
        p.isBoardElement(f) && f !== p.taskMonikers[p.taskMonikers.length - 1],
    });
  }
  return out;
}

function buildCardPredicates(i: number, p: CardClaimParams): ClaimPredicate[] {
  return [
    ...verticalNavPredicates(i, p),
    ...horizontalNavPredicates(i, p),
    ...edgeNavPredicates(i, p),
  ];
}

function useCardClaimPredicates(p: CardClaimParams): ClaimPredicate[][] {
  return useMemo<ClaimPredicate[][]>(
    () => p.taskMonikers.map((_, i) => buildCardPredicates(i, p)),
    [p],
  );
}

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
// useColumnLayout — zones, monikers, and navigation predicates
// ---------------------------------------------------------------------------

/** Derived layout data used by ColumnView's render. */
interface ColumnLayout {
  zones: DropZoneDescriptor[];
  nameFieldClaimWhen: ClaimPredicate[];
  cardClaimPredicates: ClaimPredicate[][];
}

function useIsBoardElement(
  allBoardTaskMonikers: Set<string> | undefined,
  allBoardHeaderMonikers: Set<string> | undefined,
): (f: string | null) => boolean {
  return useCallback(
    (f: string | null): boolean => {
      if (!f) return false;
      return !!(allBoardTaskMonikers?.has(f) || allBoardHeaderMonikers?.has(f));
    },
    [allBoardTaskMonikers, allBoardHeaderMonikers],
  );
}

/** Compute drop zones and navigation predicates for one column. */
function useColumnLayout(props: ColumnViewProps): ColumnLayout {
  const { column, tasks } = props;
  const leftColumnTaskMonikers = props.leftColumnTaskMonikers ?? [];
  const rightColumnTaskMonikers = props.rightColumnTaskMonikers ?? [];
  const isFirstColumn = props.isFirstColumn ?? false;
  const isLastColumn = props.isLastColumn ?? false;
  const columnMoniker = column.moniker;
  const columnNameMoniker = `${columnMoniker}.name`;

  const zones = useMemo(
    () =>
      computeDropZones(
        tasks.map((t) => t.id),
        column.id,
      ),
    [tasks, column.id],
  );
  const taskMonikers = useMemo(() => tasks.map((t) => t.moniker), [tasks]);
  const isBoardElement = useIsBoardElement(
    props.allBoardTaskMonikers,
    props.allBoardHeaderMonikers,
  );

  const nameFieldClaimWhen = useHeaderClaimPredicates({
    taskMonikers,
    rightColumnHeaderMoniker: props.rightColumnHeaderMoniker ?? null,
    leftColumnHeaderMoniker: props.leftColumnHeaderMoniker ?? null,
    rightColumnTaskMonikers,
    leftColumnTaskMonikers,
    taskCount: tasks.length,
    isFirstColumn,
    isLastColumn,
    columnNameMoniker,
    isBoardElement,
  });

  const cardClaimPredicates = useCardClaimPredicates({
    taskMonikers,
    columnMoniker,
    columnNameMoniker,
    rightColumnTaskMonikers,
    leftColumnTaskMonikers,
    isFirstColumn,
    isLastColumn,
    isBoardElement,
  });

  return { zones, nameFieldClaimWhen, cardClaimPredicates };
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
  columnNameMoniker: string;
  layout: ColumnLayout;
  dragScroll: ColumnDragScroll;
  nameFieldDef: import("@/types/kanban").FieldDef | undefined;
  editingName: boolean;
  setEditingName: (v: boolean) => void;
  setFocus: (moniker: string) => void;
}

function ColumnBody({
  props,
  columnMoniker,
  columnNameMoniker,
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
    <div className="flex flex-col min-h-0 min-w-0 flex-1">
      <ColumnHeader
        column={column}
        columnMoniker={columnMoniker}
        columnNameMoniker={columnNameMoniker}
        nameFieldClaimWhen={layout.nameFieldClaimWhen}
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
        cardClaimPredicates={layout.cardClaimPredicates}
        containerRef={dragScroll.setContainerRef}
        onDragOver={dragScroll.handleDragOver}
      />
    </div>
  );
}

export const ColumnView = memo(function ColumnView(props: ColumnViewProps) {
  const { column } = props;
  const columnMoniker = column.moniker;
  const columnNameMoniker = `${columnMoniker}.name`;
  const { getFieldDef } = useSchema();
  const nameFieldDef = getFieldDef("column", "name");
  const [editingName, setEditingName] = useState(false);
  const { setFocus } = useEntityFocus();
  const layout = useColumnLayout(props);
  const dragScroll = useColumnDragScroll(props.containerRef);

  return (
    <FocusScope
      moniker={columnMoniker}
      className="flex flex-col min-h-0 min-w-[24em] max-w-[48em] flex-1 shrink-0"
    >
      <ColumnBody
        props={props}
        columnMoniker={columnMoniker}
        columnNameMoniker={columnNameMoniker}
        layout={layout}
        dragScroll={dragScroll}
        nameFieldDef={nameFieldDef}
        editingName={editingName}
        setEditingName={setEditingName}
        setFocus={setFocus}
      />
    </FocusScope>
  );
});

// ---------------------------------------------------------------------------
// ColumnHeader — name field, badge count, and add-task button
// ---------------------------------------------------------------------------

interface ColumnHeaderProps {
  column: Entity;
  columnMoniker: string;
  columnNameMoniker: string;
  nameFieldClaimWhen: ClaimPredicate[];
  nameFieldDef: import("@/types/kanban").FieldDef | undefined;
  editingName: boolean;
  setEditingName: (v: boolean) => void;
  taskCount: number;
  onAddTask?: (columnId: string) => void;
  setFocus: (moniker: string) => void;
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
    />
  );
}

function ColumnHeader({
  column,
  columnMoniker,
  columnNameMoniker,
  nameFieldClaimWhen,
  nameFieldDef,
  editingName,
  setEditingName,
  taskCount,
  onAddTask,
  setFocus,
}: ColumnHeaderProps) {
  return (
    <div
      className="column-header-focus px-3 py-2 flex items-center gap-2 rounded"
      onClickCapture={() => setFocus(columnNameMoniker)}
    >
      <FocusScope
        moniker={columnNameMoniker}
        claimWhen={nameFieldClaimWhen}
        className="inline"
      >
        <ColumnNameField
          column={column}
          nameFieldDef={nameFieldDef}
          editingName={editingName}
          setEditingName={setEditingName}
        />
      </FocusScope>
      <Badge variant="secondary">{taskCount}</Badge>
      <div className="flex-1" />
      {onAddTask && (
        <AddTaskButton
          columnId={column.id}
          columnName={getStr(column, "name") ?? ""}
          columnMoniker={columnMoniker}
          onAddTask={onAddTask}
          setFocus={setFocus}
        />
      )}
    </div>
  );
}

/** The "+" button in the column header that adds a new task. */
function AddTaskButton({
  columnId,
  columnName,
  columnMoniker,
  onAddTask,
  setFocus,
}: {
  columnId: string;
  columnName: string;
  columnMoniker: string;
  onAddTask: (columnId: string) => void;
  setFocus: (moniker: string) => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={`Add task to ${columnName}`}
          className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
          onClick={() => {
            setFocus(columnMoniker);
            onAddTask(columnId);
          }}
        >
          <Plus className="h-4 w-4" />
        </button>
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
  cardClaimPredicates: ClaimPredicate[][];
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
  cardClaimPredicates,
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
              claimWhen={cardClaimPredicates[i]}
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
  cardClaimPredicates: ClaimPredicate[][];
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
  cardClaimPredicates: ClaimPredicate[][];
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
          claimWhen={props.cardClaimPredicates[vr.index]}
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
            cardClaimPredicates={props.cardClaimPredicates}
            measureElement={virtualizer.measureElement}
          />
        ))}
      </div>
    </div>
  );
}
