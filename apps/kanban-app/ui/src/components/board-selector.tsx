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

import { useCallback, useState } from "react";
import { ExternalLink, Share2, type LucideIcon } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
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

/**
 * Per-agent result of exposing the board, returned by the
 * `expose_board_to_agents` Tauri command. `ok` distinguishes a successful
 * registration from a failure; `message` is a human-readable line that already
 * names the agent.
 */
interface AgentExposeResult {
  ok: boolean;
  message: string;
}

/** Extract the last meaningful path segment (parent of .kanban). */
export function pathStem(path: string): string {
  const parts = path.split("/").filter(Boolean);
  const last = parts[parts.length - 1];
  return last === ".kanban" && parts.length > 1
    ? parts[parts.length - 2]
    : last || path;
}

/**
 * Label for the "expose board to your agent" action, shared by the button's
 * aria-label, its tooltip, and the test assertion so the three never drift.
 */
export const EXPOSE_BOARD_LABEL = "Expose this board to your agent";

interface BoardToolbarButtonProps {
  /** Spatial-nav moniker leaf for this button's `<Pressable>`. */
  moniker: string;
  /** Accessible name; also used as the visible tooltip when none is given. */
  ariaLabel: string;
  /** Lucide icon rendered inside the button. */
  icon: LucideIcon;
  /** Activation handler. */
  onPress: () => void;
  /** Tooltip text shown on hover/focus. */
  tooltip: string;
}

/**
 * A single icon button in the board toolbar (tear-off, expose, …).
 *
 * Every board-toolbar button shares the same `Pressable` + `Tooltip` + styled
 * `<button>` shell and differs only by its moniker, aria-label, icon, press
 * handler, and tooltip text — so they all render through this one component
 * rather than copy-pasted blocks.
 */
function BoardToolbarButton({
  moniker,
  ariaLabel,
  icon: Icon,
  onPress,
  tooltip,
}: BoardToolbarButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Pressable
          asChild
          moniker={asSegment(moniker)}
          ariaLabel={ariaLabel}
          onPress={onPress}
        >
          <button
            type="button"
            className="inline-flex items-center justify-center h-6 w-6 rounded-md text-muted-foreground/40 hover:bg-accent hover:text-accent-foreground transition-colors"
          >
            <Icon className="h-3.5 w-3.5" />
          </button>
        </Pressable>
      </TooltipTrigger>
      <TooltipContent side="bottom">{tooltip}</TooltipContent>
    </Tooltip>
  );
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

  // Register this board's MCP server into every detected agent's config so an
  // external coding agent (Claude Code, Codex, …) can talk to it. This is an
  // OS-level filesystem operation, so it invokes the plain Tauri command
  // directly (not a dispatched board command); per-agent results are surfaced
  // as toasts.
  const handleExpose = useCallback(async () => {
    if (!selectedPath) return;
    try {
      const results = await invoke<AgentExposeResult[]>(
        "expose_board_to_agents",
        { boardPath: selectedPath },
      );
      if (results.length === 0) {
        toast.info("No agents detected to expose this board to.");
        return;
      }
      for (const result of results) {
        if (result.ok) toast.success(result.message);
        else toast.error(result.message);
      }
    } catch (e) {
      toast.error(`Failed to expose board to your agent: ${String(e)}`);
    }
  }, [selectedPath]);

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
        <BoardToolbarButton
          moniker="board-selector.tear-off"
          ariaLabel="Open in new window"
          tooltip="Open in new window"
          icon={ExternalLink}
          onPress={() => {
            dispatchNewWindow({ args: { board_path: selectedPath } }).catch(
              console.error,
            );
          }}
        />
      )}

      {showTearOff && selectedPath && (
        <BoardToolbarButton
          moniker="board-selector.expose"
          ariaLabel={EXPOSE_BOARD_LABEL}
          tooltip={EXPOSE_BOARD_LABEL}
          icon={Share2}
          onPress={() => {
            handleExpose().catch(console.error);
          }}
        />
      )}
    </div>
  );
}
