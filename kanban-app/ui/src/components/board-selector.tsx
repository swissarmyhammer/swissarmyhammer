/**
 * Shared board selector — native <select> with two size variants.
 *
 * - `compact`: small text, used in quick-capture footer
 * - `full`: larger text, used in nav-bar header
 *
 * Displays "Board Name — path/stem" for each option.
 */

import type { OpenBoard } from "@/types/kanban";

/** Extract the last meaningful path segment (parent of .kanban). */
function pathStem(path: string): string {
  const parts = path.split("/").filter(Boolean);
  const last = parts[parts.length - 1];
  return last === ".kanban" && parts.length > 1
    ? parts[parts.length - 2]
    : last || path;
}

/** Format a board for display: "Name — path/stem" or just the path stem if unnamed. */
export function boardDisplayLabel(board: OpenBoard): string {
  const stem = pathStem(board.path);
  if (board.name && board.name !== stem) {
    return `${board.name} — ${stem}`;
  }
  return stem;
}

interface BoardSelectorProps {
  boards: OpenBoard[];
  selectedPath: string | null;
  onSelect: (path: string) => void;
  /** "full" = larger text for nav-bar, "compact" = small text for quick-capture. */
  variant?: "full" | "compact";
  className?: string;
}

/** Native select-based board picker with compact and full variants. */
export function BoardSelector({ boards, selectedPath, onSelect, variant = "full", className }: BoardSelectorProps) {
  if (boards.length === 0) return null;

  const sizeClass = variant === "full"
    ? "text-sm font-semibold"
    : "text-xs";

  return (
    <select
      value={selectedPath ?? ""}
      onChange={(e) => onSelect(e.target.value)}
      className={`bg-transparent text-foreground focus:outline-none cursor-pointer truncate ${sizeClass} ${className ?? ""}`}
    >
      {boards.map((b) => (
        <option key={b.path} value={b.path}>
          {boardDisplayLabel(b)}
        </option>
      ))}
    </select>
  );
}
