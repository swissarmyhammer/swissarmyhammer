import { invoke } from "@tauri-apps/api/core";
import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { BoardProgress } from "@/components/board-progress";
import { useExecuteCommand } from "@/lib/command-scope";
import type { BoardData, OpenBoard } from "@/types/kanban";

interface NavBarProps {
  board: BoardData | null;
  openBoards: OpenBoard[];
  onBoardSwitched: () => void;
  onBoardInspect?: () => void;
}

export function NavBar({
  board,
  openBoards,
  onBoardSwitched,
  onBoardInspect,
}: NavBarProps) {
  const executeCommand = useExecuteCommand();

  const handleSwitchBoard = async (path: string) => {
    try {
      await invoke("set_active_board", { path });
      onBoardSwitched();
    } catch (error) {
      console.error("Failed to switch board:", error);
    }
  };

  return (
    <header className="flex h-12 items-center border-b px-4 gap-3">
      <BoardSelector
        boards={openBoards}
        selectedPath={openBoards.find((b) => b.is_active)?.path ?? null}
        onSelect={handleSwitchBoard}
        boardEntity={board?.board}
      />
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
      {board && <BoardProgress board={board} />}
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
