import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import { Filter, Group, Plus } from "lucide-react";
import { cn } from "@/lib/utils";
import { usePerspectives } from "@/lib/perspective-context";
import { useViews } from "@/lib/views-context";
import { useDispatchCommand, CommandScopeProvider } from "@/lib/command-scope";
import { useContextMenu } from "@/lib/context-menu";
import { moniker } from "@/lib/moniker";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  FilterEditor,
  type FilterEditorHandle,
} from "@/components/filter-editor";
import { GroupSelector } from "@/components/group-selector";
import { TextEditor } from "@/components/fields/text-editor";
import { useSchema } from "@/lib/schema-context";
import type { FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Start-rename callback registry — bridges AppShell command dispatch to the
// PerspectiveTabBar component that owns the rename state.
// ---------------------------------------------------------------------------

type StartRenameCallback = () => void;
const startRenameCallbacks = new Set<StartRenameCallback>();

/**
 * Subscribe to "start rename" signals.
 *
 * Called by `usePerspectiveTabBar` to enter rename mode when the command
 * palette (or any other source) dispatches `ui.perspective.startRename`.
 *
 * @returns An unsubscribe function.
 */
export function onStartRename(cb: StartRenameCallback): () => void {
  startRenameCallbacks.add(cb);
  return () => {
    startRenameCallbacks.delete(cb);
  };
}

/**
 * Trigger all registered "start rename" callbacks.
 *
 * Intended to be called from AppShell's global command handler (or tests).
 */
export function triggerStartRename(): void {
  for (const cb of startRenameCallbacks) cb();
}

// ---------------------------------------------------------------------------
// Rename hook — encapsulates inline rename state and dispatch
// ---------------------------------------------------------------------------

/** Manages inline rename state and commit logic for perspective tabs. */
function usePerspectiveRename() {
  const dispatchPerspectiveRename = useDispatchCommand("perspective.rename");
  const { refresh } = usePerspectives();
  const [renamingId, setRenamingId] = useState<string | null>(null);

  const startRename = useCallback((id: string) => {
    setRenamingId(id);
  }, []);

  const commitRename = useCallback(
    async (id: string, oldName: string, newName: string) => {
      console.warn("[rename] commitRename called", { id, oldName, newName });
      setRenamingId(null);
      const trimmed = newName.trim();
      if (!trimmed || trimmed === oldName) {
        console.warn("[rename] skipped — name unchanged or empty", {
          trimmed,
          oldName,
        });
        return;
      }
      try {
        console.warn("[rename] dispatching perspective.rename", {
          id,
          new_name: trimmed,
        });
        await dispatchPerspectiveRename({ args: { id, new_name: trimmed } });
        await refresh();
        console.warn("[rename] dispatch succeeded");
      } catch (e) {
        console.warn("[rename] dispatch FAILED", e);
      }
    },
    [dispatchPerspectiveRename, refresh],
  );

  const cancelRename = useCallback(() => setRenamingId(null), []);
  return { renamingId, startRename, commitRename, cancelRename };
}

// ---------------------------------------------------------------------------
// Tab bar state hook — derived state and refs for PerspectiveTabBar
// ---------------------------------------------------------------------------

/**
 * Collects all derived state, refs, and callbacks needed by PerspectiveTabBar.
 *
 * Extracted so the component JSX stays within readable length.
 */
function usePerspectiveTabBar() {
  const { perspectives, activePerspective, setActivePerspectiveId } =
    usePerspectives();
  const { activeView } = useViews();
  const { renamingId, startRename, commitRename, cancelRename } =
    usePerspectiveRename();
  const filterEditorRef = useRef<FilterEditorHandle>(null);
  const handleFilterFocus = useCallback(() => {
    filterEditorRef.current?.focus();
  }, []);
  const viewKind = activeView?.kind ?? "board";
  const filteredPerspectives = useMemo(
    () => perspectives.filter((p) => p.view === viewKind),
    [perspectives, viewKind],
  );

  // Subscribe to the module-level start-rename signal so the command palette
  // (via AppShell's global command) can trigger inline rename mode.
  useEffect(() => {
    return onStartRename(() => {
      if (activePerspective) {
        startRename(activePerspective.id);
      }
    });
  }, [activePerspective, startRename]);

  return {
    activeView,
    activePerspective,
    setActivePerspectiveId,
    filteredPerspectives,
    viewKind,
    filterEditorRef,
    handleFilterFocus,
    renamingId,
    startRename,
    commitRename,
    cancelRename,
  };
}

// ---------------------------------------------------------------------------
// Main tab bar
// ---------------------------------------------------------------------------

/**
 * A compact tab bar that shows perspectives for the current view kind.
 *
 * Layout: tabs on the left (scrollable) + filter formula bar on the right.
 * The formula bar is an always-visible CM6 editor for the active perspective's
 * filter expression — analogous to Excel's formula bar. Clicking the filter
 * icon button on the active tab moves focus into the formula bar.
 *
 * Sits between the NavBar and the view content area.
 */
export function PerspectiveTabBar() {
  const {
    activeView,
    activePerspective,
    setActivePerspectiveId,
    filteredPerspectives,
    viewKind,
    filterEditorRef,
    handleFilterFocus,
    renamingId,
    startRename,
    commitRename,
    cancelRename,
  } = usePerspectiveTabBar();

  if (!activeView) return null;

  return (
    <div className="flex items-center border-b bg-muted/20 px-1 h-8 shrink-0">
      {/* Left: scrollable perspective tabs + add button */}
      <div className="flex items-center gap-0.5 overflow-x-auto shrink-0 max-w-[60%]">
        {filteredPerspectives.map((p) => (
          <ScopedPerspectiveTab
            key={p.id}
            perspective={p}
            activePerspectiveId={activePerspective?.id}
            renamingId={renamingId}
            onSelect={() => setActivePerspectiveId(p.id)}
            onDoubleClick={() => startRename(p.id)}
            onRenameCommit={(text) => commitRename(p.id, p.name, text)}
            onRenameCancel={cancelRename}
            onFilterFocus={handleFilterFocus}
          />
        ))}
        <AddPerspectiveButton
          filteredPerspectives={filteredPerspectives}
          viewKind={viewKind}
        />
      </div>
      {/* Right: filter formula bar — always visible when a perspective is active */}
      {activePerspective && (
        <FilterFormulaBar
          key={activePerspective.id}
          ref={filterEditorRef}
          filter={activePerspective.filter}
          perspectiveId={activePerspective.id}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Scoped perspective tab — CommandScopeProvider + PerspectiveTab together
// ---------------------------------------------------------------------------

/** Minimal perspective shape used within the tab bar render tree. */
interface Perspective {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
}

/** Props for a perspective tab rendered inside its own CommandScopeProvider. */
interface ScopedPerspectiveTabProps {
  perspective: Perspective;
  /** ID of the currently active perspective, used to compute `isActive`. */
  activePerspectiveId: string | undefined;
  /** ID of the perspective currently being renamed, or null if none. */
  renamingId: string | null;
  onSelect: () => void;
  onDoubleClick: () => void;
  onRenameCommit: (newName: string) => void;
  onRenameCancel: () => void;
  /** Called when the filter icon button is clicked — focuses the formula bar. */
  onFilterFocus: () => void;
}

/**
 * Wraps a single perspective tab in its CommandScopeProvider.
 *
 * Extracted from the PerspectiveTabBar map to keep the parent component concise.
 */
function ScopedPerspectiveTab({
  perspective,
  activePerspectiveId,
  renamingId,
  onSelect,
  onDoubleClick,
  onRenameCommit,
  onRenameCancel,
  onFilterFocus,
}: ScopedPerspectiveTabProps) {
  return (
    <CommandScopeProvider
      commands={[]}
      moniker={moniker("perspective", perspective.id)}
    >
      <PerspectiveTab
        id={perspective.id}
        name={perspective.name}
        filter={perspective.filter}
        group={perspective.group}
        isActive={activePerspectiveId === perspective.id}
        isRenaming={renamingId === perspective.id}
        onSelect={onSelect}
        onDoubleClick={onDoubleClick}
        onRenameCommit={onRenameCommit}
        onRenameCancel={onRenameCancel}
        onFilterFocus={onFilterFocus}
      />
    </CommandScopeProvider>
  );
}

// ---------------------------------------------------------------------------
// Add-perspective button
// ---------------------------------------------------------------------------

/** "+" button that creates a new perspective for the current view kind. */
function AddPerspectiveButton({
  filteredPerspectives,
  viewKind,
}: {
  filteredPerspectives: Array<{ name: string }>;
  viewKind: string;
}) {
  const dispatchPerspectiveSave = useDispatchCommand("perspective.save");

  const handleAdd = useCallback(() => {
    const untitledCount = filteredPerspectives.filter((p) =>
      p.name.startsWith("Untitled"),
    ).length;
    const name =
      untitledCount === 0 ? "Untitled" : `Untitled ${untitledCount + 1}`;
    dispatchPerspectiveSave({ args: { name, view: viewKind } }).catch(
      console.error,
    );
  }, [filteredPerspectives, viewKind, dispatchPerspectiveSave]);

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={handleAdd}
          aria-label="Add perspective"
          className="inline-flex items-center justify-center h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-muted/50 rounded-md transition-colors"
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
      </TooltipTrigger>
      <TooltipContent>New perspective</TooltipContent>
    </Tooltip>
  );
}

// ---------------------------------------------------------------------------
// Inner tab component — rendered inside CommandScopeProvider so
// useContextMenu sees the perspective scope and builds the correct chain.
// ---------------------------------------------------------------------------

/** Props for an individual perspective tab button and its inline action buttons. */
interface PerspectiveTabProps {
  id: string;
  name: string;
  filter?: string;
  group?: string;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: () => void;
  onDoubleClick: () => void;
  /** Called with the new name text when the rename editor commits. */
  onRenameCommit: (newName: string) => void;
  onRenameCancel: () => void;
  /** Called when the filter icon button is clicked — focuses the formula bar. */
  onFilterFocus: () => void;
}

/**
 * Individual perspective tab that uses the backend command system for
 * context menus. Renders an inline filter focus button for the active tab
 * that moves keyboard focus into the formula bar (no popover).
 *
 * Must be rendered inside a CommandScopeProvider with a perspective
 * moniker so the scope chain is correct.
 */
function PerspectiveTab({
  id,
  name,
  filter,
  group,
  isActive,
  isRenaming,
  onSelect,
  onDoubleClick,
  onRenameCommit,
  onRenameCancel,
  onFilterFocus,
}: PerspectiveTabProps) {
  const handleContextMenu = useContextMenu();
  const [groupOpen, setGroupOpen] = useState(false);

  const { getSchema } = useSchema();
  const { activeView } = useViews();
  const entityType = activeView?.entity_type ?? "";
  const schemaFields = useMemo(
    () => getSchema(entityType)?.fields ?? [],
    [getSchema, entityType],
  );

  return (
    <div className="inline-flex items-center">
      <TabButton
        name={name}
        isActive={isActive}
        isRenaming={isRenaming}
        onSelect={onSelect}
        onDoubleClick={onDoubleClick}
        onContextMenu={handleContextMenu}
        onRenameCommit={onRenameCommit}
        onRenameCancel={onRenameCancel}
      />
      {isActive && (
        <FilterFocusButton filter={filter} onFocus={onFilterFocus} />
      )}
      {isActive && (
        <GroupPopoverButton
          group={group}
          perspectiveId={id}
          fields={schemaFields}
          open={groupOpen}
          onOpenChange={setGroupOpen}
        />
      )}
    </div>
  );
}

/**
 * The clickable tab button — shows the perspective name or a CM6 rename editor.
 *
 * When `isRenaming` is true, renders `InlineRenameEditor` in place of the
 * name text so the user can type a new name directly in the tab.
 */
function TabButton({
  name,
  isActive,
  isRenaming,
  onSelect,
  onDoubleClick,
  onContextMenu,
  onRenameCommit,
  onRenameCancel,
}: {
  name: string;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: () => void;
  onDoubleClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  onRenameCommit: (newName: string) => void;
  onRenameCancel: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      className={cn(
        "inline-flex items-center px-2.5 h-7 text-xs font-medium rounded-t-md border-b-2 transition-colors whitespace-nowrap",
        isActive
          ? "border-primary text-foreground bg-background"
          : "border-transparent text-muted-foreground hover:text-foreground hover:bg-muted/50",
      )}
    >
      {isRenaming ? (
        <InlineRenameEditor
          name={name}
          onCommit={onRenameCommit}
          onCancel={onRenameCancel}
        />
      ) : (
        name
      )}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Rename guard hook and inline rename editor
// ---------------------------------------------------------------------------

/**
 * Creates guarded commit and cancel callbacks for inline rename.
 *
 * The committedRef prevents double-fire from concurrent blur + Enter events.
 */
function useRenameGuards(
  onCommit: (text: string) => void,
  onCancel: () => void,
) {
  const committedRef = useRef(false);

  const guardedCommit = useCallback(
    (text: string) => {
      console.warn("[rename] guardedCommit called", {
        text,
        alreadyCommitted: committedRef.current,
      });
      if (committedRef.current) return;
      committedRef.current = true;
      onCommit(text);
    },
    [onCommit],
  );

  const guardedCancel = useCallback(() => {
    console.warn("[rename] guardedCancel called", {
      alreadyCommitted: committedRef.current,
    });
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return { guardedCommit, guardedCancel };
}

/**
 * Inline CM6 rename editor — uses TextEditor with singleLine mode.
 *
 * Blur-commit is handled here via a wrapper div's onBlur, not inside
 * TextEditor. This keeps TextEditor's blur behavior identical regardless
 * of singleLine — the wrapper is responsible for committing when focus
 * leaves the rename editor entirely.
 */
function InlineRenameEditor({
  name,
  onCommit,
  onCancel,
}: {
  name: string;
  onCommit: (newName: string) => void;
  onCancel: () => void;
}) {
  const latestTextRef = useRef(name);
  const { guardedCommit, guardedCancel } = useRenameGuards(onCommit, onCancel);
  const trackText = useCallback((text: string) => {
    latestTextRef.current = text;
  }, []);

  return (
    <div
      className="min-w-[5rem]"
      onClick={(e) => e.stopPropagation()}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) {
          guardedCommit(latestTextRef.current);
        }
      }}
    >
      <TextEditor
        value={name}
        onCommit={guardedCommit}
        onCancel={guardedCancel}
        onChange={trackText}
        singleLine
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Filter focus button — clicking moves focus into the formula bar (no popover)
// ---------------------------------------------------------------------------

/**
 * Filter icon button on the active tab.
 *
 * Does NOT open a popover. Instead, clicking calls `onFocus` which focuses
 * the formula bar CM6 editor to the right of the tabs.
 */
function FilterFocusButton({
  filter,
  onFocus,
}: {
  filter?: string;
  onFocus: () => void;
}) {
  const hasFilter = Boolean(filter);
  return (
    <button
      aria-label="Filter"
      className={cn(
        "inline-flex items-center justify-center h-5 w-5 rounded transition-colors -ml-1",
        hasFilter
          ? "text-primary"
          : "text-muted-foreground/50 hover:text-muted-foreground",
      )}
      onClick={(e) => {
        e.stopPropagation();
        onFocus();
      }}
    >
      <Filter className="h-3 w-3" fill={hasFilter ? "currentColor" : "none"} />
    </button>
  );
}

// ---------------------------------------------------------------------------
// Filter formula bar — always-visible CM6 filter editor in the right region
// ---------------------------------------------------------------------------

/** Props for the always-visible filter formula bar in the tab bar's right region. */
interface FilterFormulaBarProps {
  /** Current filter expression for the active perspective. */
  filter?: string;
  /** Perspective ID to dispatch filter commands against. */
  perspectiveId: string;
}

/**
 * Excel-style filter formula bar embedded in the right side of the tab bar.
 *
 * Always visible when a perspective is active. Contains a Filter icon and
 * a borderless CM6 filter editor with placeholder text. Exposes `focus()`
 * via forwardRef so the parent can focus the editor when the filter button
 * is clicked.
 *
 * Uses `key={perspectiveId}` on the parent to remount when switching
 * perspectives, ensuring the CM6 initial value reflects the new perspective.
 */
const FilterFormulaBar = forwardRef<FilterEditorHandle, FilterFormulaBarProps>(
  function FilterFormulaBar({ filter, perspectiveId }, ref) {
    const editorRef = useRef<FilterEditorHandle>(null);

    // Forward focus to the inner editor handle so parents can call focus().
    useImperativeHandle(
      ref,
      () => ({
        focus() {
          editorRef.current?.focus();
        },
      }),
      [],
    );

    return (
      <div
        data-testid="filter-formula-bar"
        className="flex items-center gap-1.5 flex-1 min-w-0 border-l border-border/50 pl-2 ml-1 cursor-text"
        onClick={() => editorRef.current?.focus()}
      >
        <Filter
          className="h-3.5 w-3.5 shrink-0 text-muted-foreground/60"
          aria-hidden="true"
        />
        <FilterEditor
          ref={editorRef}
          filter={filter ?? ""}
          perspectiveId={perspectiveId}
        />
      </div>
    );
  },
);

// ---------------------------------------------------------------------------
// Group popover button — unchanged from original
// ---------------------------------------------------------------------------

/**
 * Group-by icon button with popover for the active perspective.
 *
 * Opens a `GroupSelector` in a Radix Popover anchored below the button.
 * Highlighted in primary color when a group is active on the perspective.
 */
function GroupPopoverButton({
  group,
  perspectiveId,
  fields,
  open,
  onOpenChange,
}: {
  group?: string;
  perspectiveId: string;
  fields: FieldDef[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const hasGroup = Boolean(group);
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <button
          aria-label="Group"
          className={cn(
            "inline-flex items-center justify-center h-5 w-5 rounded transition-colors -ml-0.5",
            hasGroup
              ? "text-primary"
              : "text-muted-foreground/50 hover:text-muted-foreground",
          )}
          onClick={(e) => e.stopPropagation()}
        >
          <Group
            className="h-3 w-3"
            fill={hasGroup ? "currentColor" : "none"}
          />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={4}
        className="p-3 w-auto"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <GroupSelector
          group={group}
          perspectiveId={perspectiveId}
          fields={fields}
          onClose={() => onOpenChange(false)}
        />
      </PopoverContent>
    </Popover>
  );
}
