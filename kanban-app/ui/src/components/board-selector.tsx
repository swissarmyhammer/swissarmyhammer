/**
 * Board selector — EditableMarkdown name + path stem + dropdown chevron.
 *
 * The name is editable in place and persists via useFieldUpdate.
 * The path stem + chevron open a Radix Select dropdown to switch boards.
 */

import { invoke } from "@tauri-apps/api/core";
import { ExternalLink } from "lucide-react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
} from "@/components/ui/select";
import { EditableMarkdown } from "@/components/editable-markdown";
import { useFieldUpdate } from "@/lib/field-update-context";
import type { Entity, OpenBoard } from "@/types/kanban";
import { getStr } from "@/types/kanban";

/** Extract the last meaningful path segment (parent of .kanban). */
export function pathStem(path: string): string {
  const parts = path.split("/").filter(Boolean);
  const last = parts[parts.length - 1];
  return last === ".kanban" && parts.length > 1
    ? parts[parts.length - 2]
    : last || path;
}

interface BoardSelectorProps {
  boards: OpenBoard[];
  selectedPath: string | null;
  onSelect: (path: string) => void;
  /** The active board entity — used for live name display and in-place editing. */
  boardEntity?: Entity;
  /** Show tear-off button to open board in a new window. */
  showTearOff?: boolean;
  className?: string;
}

export function BoardSelector({
  boards,
  selectedPath,
  onSelect,
  boardEntity,
  showTearOff,
  className,
}: BoardSelectorProps) {
  if (boards.length === 0) return null;

  const { updateField } = useFieldUpdate();
  const selected = boards.find((b) => b.path === selectedPath);
  const displayName = boardEntity ? getStr(boardEntity, "name", "") : (selected?.name ?? "");
  const stem = selectedPath ? pathStem(selectedPath) : "";

  const handleRename = (name: string) => {
    if (boardEntity) {
      updateField(boardEntity.entity_type, boardEntity.id, "name", name).catch(() => {});
    }
  };

  return (
    <div className={`flex items-center gap-1.5 min-w-0 ${className ?? ""}`}>
      <EditableMarkdown
        value={displayName}
        onCommit={handleRename}
        className="text-sm font-semibold cursor-text truncate"
        inputClassName="text-sm font-semibold bg-transparent border-b border-ring"
      />

      <Select value={selectedPath ?? undefined} onValueChange={onSelect}>
        <SelectTrigger
          className="border-none shadow-none h-auto py-0 px-0 gap-1 w-auto min-w-0 focus-visible:ring-0 focus-visible:border-transparent"
          size="sm"
        >
          {stem && (
            <span className="text-xs text-muted-foreground/50 shrink-0">{stem}</span>
          )}
        </SelectTrigger>
        <SelectContent position="popper">
          {boards.map((b) => {
            const name = b.path === selectedPath
              ? displayName
              : (b.name || pathStem(b.path));
            return (
              <SelectItem key={b.path} value={b.path}>
                <span>{name}</span>
                <span className="ml-2 text-muted-foreground/50">{pathStem(b.path)}</span>
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>

      {showTearOff && selectedPath && (
        <button
          type="button"
          className="p-1 rounded text-muted-foreground/40 hover:text-muted-foreground hover:bg-muted transition-colors"
          title="Open in new window"
          onClick={() => {
            invoke("create_window", { boardPath: selectedPath }).catch(console.error);
          }}
        >
          <ExternalLink className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}
