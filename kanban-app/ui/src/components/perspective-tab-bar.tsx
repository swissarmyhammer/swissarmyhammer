import { useCallback, useContext, useMemo, useRef, useState } from "react";
import { Filter, Group, Plus } from "lucide-react";
import { cn } from "@/lib/utils";
import { usePerspectives } from "@/lib/perspective-context";
import { useViews } from "@/lib/views-context";
import {
  backendDispatch,
  CommandScopeProvider,
  CommandScopeContext,
  scopeChainFromScope,
  useActiveBoardPath,
  type CommandScope,
} from "@/lib/command-scope";
import { useContextMenu } from "@/lib/context-menu";
import { moniker } from "@/lib/moniker";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import { FilterEditor } from "@/components/filter-editor";
import { GroupSelector } from "@/components/group-selector";
import { useSchema } from "@/lib/schema-context";

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
  const { perspectives, activePerspective, setActivePerspectiveId, refresh } =
    usePerspectives();
  const { activeView } = useViews();
  const scope = useContext(CommandScopeContext);
  const scopeChain = useMemo(() => scopeChainFromScope(scope), [scope]);
  const boardPath = useActiveBoardPath();

  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [filterOpen, setFilterOpen] = useState(false);
  const [groupOpen, setGroupOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Get entity schema fields for the group-by selector.
  const { getSchema } = useSchema();
  const entityType = activeView?.entity_type ?? "";
  const schemaFields = useMemo(
    () => getSchema(entityType)?.fields ?? [],
    [getSchema, entityType],
  );

  // Filter perspectives to only those matching the active view kind.
  const viewKind = activeView?.kind ?? "board";
  const filteredPerspectives = useMemo(
    () => perspectives.filter((p) => p.view === viewKind),
    [perspectives, viewKind],
  );

  /** Create a new perspective for the current view kind. */
  const handleAdd = useCallback(() => {
    // Generate a unique name by counting existing "Untitled" perspectives.
    const untitledCount = filteredPerspectives.filter((p) =>
      p.name.startsWith("Untitled"),
    ).length;
    const name =
      untitledCount === 0 ? "Untitled" : `Untitled ${untitledCount + 1}`;

    backendDispatch({
      cmd: "perspective.save",
      args: { name, view: viewKind },
      scopeChain,
      ...(boardPath ? { boardPath } : {}),
    }).catch(console.error);
  }, [filteredPerspectives, viewKind, scopeChain, boardPath]);

  /** Start inline rename for a perspective tab (triggered by double-click). */
  const startRename = useCallback((id: string, currentName: string) => {
    setRenamingId(id);
    setRenameValue(currentName);
    // Focus the input after it renders.
    requestAnimationFrame(() => inputRef.current?.select());
  }, []);

  /** Commit the rename by saving the perspective with the new name. */
  const commitRename = useCallback(
    async (id: string, oldName: string) => {
      const newName = renameValue.trim();
      setRenamingId(null);
      if (!newName || newName === oldName) return;

      // Delete old name then save with new name, keeping the same view kind.
      const perspective = perspectives.find((p) => p.id === id);
      if (!perspective) return;

      try {
        await backendDispatch({
          cmd: "perspective.delete",
          args: { name: oldName },
          scopeChain,
          ...(boardPath ? { boardPath } : {}),
        });
        await backendDispatch({
          cmd: "perspective.save",
          args: { name: newName, view: perspective.view },
          scopeChain,
          ...(boardPath ? { boardPath } : {}),
        });
        await refresh();
      } catch (e) {
        console.error("Failed to rename perspective:", e);
      }
    },
    [renameValue, perspectives, refresh],
  );

  // Don't render when there's no active view.
  if (!activeView) return null;

  return (
    <div className="flex items-center border-b bg-muted/20 px-1 h-8 shrink-0 gap-0.5 overflow-x-auto">
      {filteredPerspectives.map((p) => {
        const isActive = activePerspective?.id === p.id;
        const isRenaming = renamingId === p.id;

        return (
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
              isActive={isActive}
              isRenaming={isRenaming}
              renameValue={renameValue}
              inputRef={inputRef}
              filterOpen={isActive && filterOpen}
              onFilterOpenChange={(open) => {
                if (isActive) setFilterOpen(open);
              }}
              groupOpen={isActive && groupOpen}
              onGroupOpenChange={(open) => {
                if (isActive) setGroupOpen(open);
              }}
              schemaFields={schemaFields}
              onSelect={() => setActivePerspectiveId(p.id)}
              onDoubleClick={() => startRename(p.id, p.name)}
              onRenameChange={setRenameValue}
              onRenameCommit={() => commitRename(p.id, p.name)}
              onRenameCancel={() => setRenamingId(null)}
            />
          </CommandScopeProvider>
        );
      })}

      {/* Add perspective button */}
      <button
        onClick={handleAdd}
        title="New perspective"
        aria-label="Add perspective"
        className="inline-flex items-center justify-center h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-muted/50 rounded-md transition-colors"
      >
        <Plus className="h-3.5 w-3.5" />
      </button>
    </div>
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
  renameValue: string;
  inputRef: React.RefObject<HTMLInputElement | null>;
  filterOpen: boolean;
  onFilterOpenChange: (open: boolean) => void;
  groupOpen: boolean;
  onGroupOpenChange: (open: boolean) => void;
  schemaFields: import("@/types/kanban").FieldDef[];
  onSelect: () => void;
  onDoubleClick: () => void;
  onRenameChange: (value: string) => void;
  onRenameCommit: () => void;
  onRenameCancel: () => void;
}

/**
 * Individual perspective tab that uses the backend command system for
 * context menus. Must be rendered inside a CommandScopeProvider with a
 * perspective moniker so the scope chain is correct.
 */
function PerspectiveTab({
  id,
  name,
  filter,
  group,
  isActive,
  isRenaming,
  renameValue,
  inputRef,
  filterOpen,
  onFilterOpenChange,
  groupOpen,
  onGroupOpenChange,
  schemaFields,
  onSelect,
  onDoubleClick,
  onRenameChange,
  onRenameCommit,
  onRenameCancel,
}: PerspectiveTabProps) {
  const scopeChain = useScopeChain();
  const handleContextMenu = useContextMenu(scopeChain);
  const hasFilter = Boolean(filter);
  const hasGroup = Boolean(group);

  return (
    <div className="inline-flex items-center">
      <button
        onClick={onSelect}
        onDoubleClick={onDoubleClick}
        onContextMenu={handleContextMenu}
        className={cn(
          "inline-flex items-center px-2.5 h-7 text-xs font-medium rounded-t-md border-b-2 transition-colors whitespace-nowrap",
          isActive
            ? "border-primary text-foreground bg-background"
            : "border-transparent text-muted-foreground hover:text-foreground hover:bg-muted/50",
        )}
      >
        {isRenaming ? (
          <input
            ref={inputRef}
            value={renameValue}
            onChange={(e) => onRenameChange(e.target.value)}
            onBlur={onRenameCommit}
            onKeyDown={(e) => {
              if (e.key === "Enter") onRenameCommit();
              if (e.key === "Escape") onRenameCancel();
            }}
            className="bg-transparent border-none outline-none text-xs w-20 p-0"
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          name
        )}
      </button>
      {isActive && (
        <Popover open={filterOpen} onOpenChange={onFilterOpenChange}>
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
              perspectiveId={id}
              onClose={() => onFilterOpenChange(false)}
            />
          </PopoverContent>
        </Popover>
      )}
      {isActive && (
        <Popover open={groupOpen} onOpenChange={onGroupOpenChange}>
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
              perspectiveId={id}
              fields={schemaFields}
              onClose={() => onGroupOpenChange(false)}
            />
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
}

/**
 * Build the scope chain from the current CommandScopeContext,
 * walking from innermost to root and collecting monikers.
 */
function useScopeChain(): string[] {
  const scope = useContext(CommandScopeContext);
  return useMemo(() => {
    const chain: string[] = [];
    let current: CommandScope | null = scope;
    while (current) {
      if (current.moniker) chain.push(current.moniker);
      current = current.parent;
    }
    return chain;
  }, [scope]);
}
