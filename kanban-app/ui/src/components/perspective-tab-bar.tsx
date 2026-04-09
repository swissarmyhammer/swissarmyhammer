import { useCallback, useMemo, useRef, useState } from "react";
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
import { FilterEditor } from "@/components/filter-editor";
import { GroupSelector } from "@/components/group-selector";
import { TextEditor } from "@/components/fields/text-editor";
import { useSchema } from "@/lib/schema-context";
import type { FieldDef } from "@/types/kanban";

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
        console.warn("[rename] skipped — name unchanged or empty", { trimmed, oldName });
        return;
      }

      try {
        console.warn("[rename] dispatching perspective.rename", { id, new_name: trimmed });
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
// Main tab bar
// ---------------------------------------------------------------------------

/**
 * A compact tab bar that shows perspectives for the current view kind.
 *
 * Sits between the NavBar and the view content area. Each tab shows a
 * perspective name; clicking switches the active perspective. A "+" button
 * at the end creates a new perspective for the current view kind.
 *
 * Right-click on a tab opens a native OS context menu via the backend command
 * system. Double-click on a tab starts inline rename.
 */
export function PerspectiveTabBar() {
  const { perspectives, activePerspective, setActivePerspectiveId } =
    usePerspectives();
  const { activeView } = useViews();
  const { renamingId, startRename, commitRename, cancelRename } =
    usePerspectiveRename();

  const viewKind = activeView?.kind ?? "board";
  const filteredPerspectives = useMemo(
    () => perspectives.filter((p) => p.view === viewKind),
    [perspectives, viewKind],
  );

  if (!activeView) return null;

  return (
    <div className="flex items-center border-b bg-muted/20 px-1 h-8 shrink-0 gap-0.5 overflow-x-auto">
      {filteredPerspectives.map((p) => (
        <CommandScopeProvider
          key={p.id}
          commands={[]}
          moniker={moniker("perspective", p.id)}
        >
          <PerspectiveTab
            id={p.id}
            name={p.name}
            filter={p.filter}
            group={p.group}
            isActive={activePerspective?.id === p.id}
            isRenaming={renamingId === p.id}
            onSelect={() => setActivePerspectiveId(p.id)}
            onDoubleClick={() => startRename(p.id)}
            onRenameCommit={(text) => commitRename(p.id, p.name, text)}
            onRenameCancel={cancelRename}
          />
        </CommandScopeProvider>
      ))}
      <AddPerspectiveButton
        filteredPerspectives={filteredPerspectives}
        viewKind={viewKind}
      />
    </div>
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
}

/**
 * Individual perspective tab that uses the backend command system for
 * context menus. Owns its own filter/group popover state.
 *
 * Must be rendered inside a CommandScopeProvider with a perspective
 * moniker so the scope chain is correct.
 */
function PerspectiveTab({
  id, name, filter, group, isActive, isRenaming,
  onSelect, onDoubleClick, onRenameCommit, onRenameCancel,
}: PerspectiveTabProps) {
  const handleContextMenu = useContextMenu();
  const [filterOpen, setFilterOpen] = useState(false);
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
        <FilterPopoverButton
          filter={filter}
          perspectiveId={id}
          open={filterOpen}
          onOpenChange={setFilterOpen}
        />
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

/** The clickable tab button — shows the perspective name or a CM6 rename editor. */
function TabButton({
  name, isActive, isRenaming,
  onSelect, onDoubleClick, onContextMenu,
  onRenameCommit, onRenameCancel,
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
  const committedRef = useRef(false);

  const guardedCommit = useCallback(
    (text: string) => {
      console.warn("[rename] guardedCommit called", { text, alreadyCommitted: committedRef.current });
      if (committedRef.current) return;
      committedRef.current = true;
      onCommit(text);
    },
    [onCommit],
  );

  const guardedCancel = useCallback(() => {
    console.warn("[rename] guardedCancel called", { alreadyCommitted: committedRef.current });
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

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
// Filter and group popover buttons
// ---------------------------------------------------------------------------

/** Filter icon button with popover for the active perspective. */
function FilterPopoverButton({
  filter,
  perspectiveId,
  open,
  onOpenChange,
}: {
  filter?: string;
  perspectiveId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const hasFilter = Boolean(filter);
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <button
          aria-label="Filter"
          className={cn(
            "inline-flex items-center justify-center h-5 w-5 rounded transition-colors -ml-1",
            hasFilter
              ? "text-primary"
              : "text-muted-foreground/50 hover:text-muted-foreground",
          )}
          onClick={(e) => e.stopPropagation()}
        >
          <Filter
            className="h-3 w-3"
            fill={hasFilter ? "currentColor" : "none"}
          />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={4}
        className="p-3 w-auto"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <FilterEditor
          filter={filter ?? ""}
          perspectiveId={perspectiveId}
          onClose={() => onOpenChange(false)}
        />
      </PopoverContent>
    </Popover>
  );
}

/** Group-by icon button with popover for the active perspective. */
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
