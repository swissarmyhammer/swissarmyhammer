/**
 * Board selector — Field-driven name + path stem + dropdown chevron.
 *
 * The name is editable in place via the Field component (schema-driven).
 * The path stem + chevron open a Radix Select dropdown to switch boards.
 *
 * # Spatial-nav model
 *
 * The selector is a **navigable container** — it contains multiple
 * focusable surfaces (the editable name `<Field>`, the dropdown
 * trigger, the optional tear-off button). The caller (e.g. `<NavBar>`)
 * wraps this component in a `<FocusZone>`; inside this component we
 * wrap each interactive non-Field surface in its own `<FocusScope>`
 * leaf. The `<Field>` is itself a `<FocusZone>` so it participates as a
 * peer zone with its own leaves.
 */

import { useState } from "react";
import { ExternalLink } from "lucide-react";
import { useDispatchCommand } from "@/lib/command-scope";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
} from "@/components/ui/select";
import { Field } from "@/components/fields/field";
import { FocusScope } from "@/components/focus-scope";
import { Pressable } from "@/components/pressable";
import { useSchema } from "@/lib/schema-context";
import { useFieldValue } from "@/lib/entity-store-context";
import { asSegment } from "@/types/spatial";
import type { Entity, OpenBoard } from "@/types/kanban";

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
  const { getSchema, getFieldDef } = useSchema();
  // Derive the display field from the board entity schema (search_display_field)
  // rather than hardcoding "name". Falls back to "name" if schema hasn't loaded.
  const displayFieldName =
    getSchema("board")?.entity.search_display_field ?? "name";
  const nameFieldDef = getFieldDef("board", displayFieldName);
  const [editingName, setEditingName] = useState(false);
  const dispatchNewWindow = useDispatchCommand("window.new");
  // Live board name from entity store — stays current across windows
  const boardName = useFieldValue(
    "board",
    boardEntity?.id ?? "",
    displayFieldName,
  );

  if (boards.length === 0) return null;

  const selected = boards.find((b) => b.path === selectedPath);
  const displayName =
    (typeof boardName === "string" && boardName) || selected?.name || "";
  const stem = selectedPath ? pathStem(selectedPath) : "";

  return (
    <div className={`flex items-center gap-1.5 min-w-0 ${className ?? ""}`}>
      <div className="font-semibold truncate min-w-0 flex-1">
        {boardEntity && nameFieldDef ? (
          <Field
            fieldDef={nameFieldDef}
            entityType="board"
            entityId={boardEntity.id}
            mode="compact"
            editing={editingName}
            onEdit={() => setEditingName(true)}
            onDone={() => setEditingName(false)}
            onCancel={() => setEditingName(false)}
            showFocus
          />
        ) : (
          <span className="text-sm cursor-text truncate">{displayName}</span>
        )}
      </div>

      <FocusScope moniker={asSegment("board-selector.dropdown")}>
        <Select
          value={selectedPath ?? undefined}
          onValueChange={(path) => {
            onSelect(path);
          }}
        >
          <SelectTrigger
            className="border-none shadow-none h-auto py-0 px-0 gap-1 w-auto min-w-0 focus-visible:ring-0 focus-visible:border-transparent"
            size="sm"
          >
            {stem && (
              <span className="text-xs text-muted-foreground/50 shrink-0">
                {stem}
              </span>
            )}
          </SelectTrigger>
          <SelectContent position="popper">
            {boards.map((b) => {
              const name =
                b.path === selectedPath
                  ? displayName
                  : b.name || pathStem(b.path);
              return (
                <SelectItem key={b.path} value={b.path}>
                  <span>{name}</span>
                  <span className="ml-2 text-muted-foreground/50">
                    {pathStem(b.path)}
                  </span>
                </SelectItem>
              );
            })}
          </SelectContent>
        </Select>
      </FocusScope>

      {showTearOff && selectedPath && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Pressable
              asChild
              moniker={asSegment("board-selector.tear-off")}
              ariaLabel="Open in new window"
              onPress={() => {
                dispatchNewWindow({ args: { board_path: selectedPath } }).catch(
                  console.error,
                );
              }}
            >
              <button
                type="button"
                className="inline-flex items-center justify-center h-6 w-6 rounded-md text-muted-foreground/40 hover:bg-accent hover:text-accent-foreground transition-colors"
              >
                <ExternalLink className="h-3.5 w-3.5" />
              </button>
            </Pressable>
          </TooltipTrigger>
          <TooltipContent side="bottom">Open in new window</TooltipContent>
        </Tooltip>
      )}
    </div>
  );
}
