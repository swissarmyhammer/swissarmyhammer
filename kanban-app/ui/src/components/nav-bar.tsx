import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronDown, Info, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { EditableMarkdown } from "@/components/editable-markdown";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useExecuteCommand } from "@/lib/command-scope";
import type { BoardData, OpenBoard } from "@/types/kanban";
import { getStr } from "@/types/kanban";

interface NavBarProps {
  board: BoardData | null;
  openBoards: OpenBoard[];
  /** Called after switching the active board to reload board data. */
  onBoardSwitched: () => void;
  onBoardInspect?: () => void;
}

export function NavBar({
  board,
  openBoards,
  onBoardSwitched,
  onBoardInspect,
}: NavBarProps) {
  const { updateField } = useFieldUpdate();
  const executeCommand = useExecuteCommand();

  const handleSwitchBoard = async (path: string) => {
    try {
      await invoke("set_active_board", { path });
      onBoardSwitched();
    } catch (error) {
      console.error("Failed to switch board:", error);
    }
  };

  const boardName = board ? getStr(board.board, "name", "No Board") : "No Board";

  return (
    <header className="flex h-12 items-center border-b px-4 gap-3">
      <EditableMarkdown
        value={boardName}
        onCommit={(name) => {
          if (board) updateField("board", board.board.id, "name", name).catch(() => {});
        }}
        className="text-sm font-semibold cursor-text"
        inputClassName="text-sm font-semibold bg-transparent border-b border-ring"
      />
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="sm" className="px-1">
            <ChevronDown className="h-4 w-4 opacity-50" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-64">
          {openBoards.length > 0 && (
            <>
              <DropdownMenuLabel>Open</DropdownMenuLabel>
              {openBoards.map((ob) => (
                <DropdownMenuItem
                  key={ob.path}
                  onClick={() => handleSwitchBoard(ob.path)}
                >
                  {ob.is_active && <Check className="h-4 w-4" />}
                  <span className={ob.is_active ? "font-medium" : ""}>
                    {(() => {
                      const parts = ob.path.split("/").filter(Boolean);
                      const last = parts[parts.length - 1];
                      return last === ".kanban" && parts.length > 1
                        ? parts[parts.length - 2]
                        : last || ob.path;
                    })()}
                  </span>
                </DropdownMenuItem>
              ))}
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
      {board && onBoardInspect && (
        <button
          type="button"
          className="p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
          onClick={onBoardInspect}
          title="Inspect board"
        >
          <Info className="h-4 w-4" />
        </button>
      )}
      <button
        type="button"
        className="ml-auto p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
        onClick={() => executeCommand("app.search")}
        title="Search"
      >
        <Search className="h-4 w-4" />
      </button>
    </header>
  );
}
