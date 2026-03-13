import { useMemo } from "react";
import {
  SortableContext,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { useDroppable } from "@dnd-kit/core";
import { Inbox, Plus } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SortableEntityCard } from "@/components/sortable-task-card";
import { FocusScope } from "@/components/focus-scope";
import { Badge } from "@/components/ui/badge";
import { moniker } from "@/lib/moniker";
import { useInspect } from "@/lib/inspect-context";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

interface ColumnViewProps {
  column: Entity;
  tasks: Entity[];
  blockedIds: Set<string>;
  onAddTask?: (columnId: string) => void;
  onRenameColumn?: (columnId: string, name: string) => void;
  presorted?: boolean;
}

export function ColumnView({ column, tasks, blockedIds, onAddTask, onRenameColumn, presorted }: ColumnViewProps) {
  const inspectEntity = useInspect();
  const columnMoniker = moniker("column", column.id);

  const sorted = useMemo(
    () =>
      presorted
        ? tasks
        : [...tasks].sort((a, b) =>
            getStr(a, "position_ordinal", "a0").localeCompare(
              getStr(b, "position_ordinal", "a0")
            )
          ),
    [tasks, presorted]
  );

  const taskIds = useMemo(() => sorted.map((e) => e.id), [sorted]);
  const { setNodeRef } = useDroppable({ id: `drop:${column.id}` });

  const commands = useMemo(() => [
    {
      id: "entity.inspect",
      name: "Inspect column",
      target: columnMoniker,
      contextMenu: true,
      execute: () => inspectEntity(columnMoniker),
    },
  ], [columnMoniker, inspectEntity]);

  return (
    <FocusScope moniker={columnMoniker} commands={commands} className="flex flex-col min-h-0 min-w-0 flex-1">
      <div className="flex flex-col min-h-0 min-w-0 flex-1">
        <div className="px-3 py-2 flex items-center gap-2">
          <EditableMarkdown
            value={getStr(column, "name")}
            onCommit={(name) => onRenameColumn?.(column.id, name)}
            className="text-sm font-semibold text-foreground cursor-text"
            inputClassName="text-sm font-semibold text-foreground bg-transparent border-b border-ring w-full"
          />
          <Badge variant="secondary">{tasks.length}</Badge>
          <div className="flex-1" />
          {onAddTask && (
            <button
              type="button"
              className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
              onClick={() => onAddTask(column.id)}
              title={`Add task to ${getStr(column, "name")}`}
            >
              <Plus className="h-4 w-4" />
            </button>
          )}
        </div>
        <div ref={setNodeRef} className="flex-1 overflow-y-auto px-2 pt-1 pb-2 space-y-1.5">
          <SortableContext items={taskIds} strategy={verticalListSortingStrategy}>
            {sorted.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full text-muted-foreground opacity-40">
                <Inbox className="h-8 w-8 mb-2" />
                <p className="text-xs">No tasks</p>
              </div>
            ) : (
              sorted.map((entity) => (
                <SortableEntityCard
                  key={entity.id}
                  entity={entity}
                  isBlocked={blockedIds.has(entity.id)}
                />
              ))
            )}
          </SortableContext>
        </div>
      </div>
    </FocusScope>
  );
}
