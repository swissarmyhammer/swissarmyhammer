import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { Field } from "@/components/fields/field";
import { useSchema } from "@/lib/schema-context";
import { useExecuteCommand } from "@/lib/command-scope";
import type { BoardData, OpenBoard } from "@/types/kanban";

interface NavBarProps {
  board: BoardData | null;
  openBoards: OpenBoard[];
  /** Currently active board path for this window. */
  activeBoardPath?: string;
  /** Switch this window to a different board. */
  onSwitchBoard: (path: string) => void;
  onBoardInspect?: () => void;
}

export function NavBar({
  board,
  openBoards,
  activeBoardPath,
  onSwitchBoard,
  onBoardInspect,
}: NavBarProps) {
  const executeCommand = useExecuteCommand();
  const { getFieldDef } = useSchema();
  const percentFieldDef = getFieldDef("board", "percent_complete");

  return (
    <header className="flex h-12 items-center border-b px-4 gap-3">
      <BoardSelector
        boards={openBoards}
        selectedPath={
          activeBoardPath ?? openBoards.find((b) => b.is_active)?.path ?? null
        }
        onSelect={onSwitchBoard}
        boardEntity={board?.board}
        showTearOff
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
      {board && percentFieldDef && (
        <Field
          fieldDef={percentFieldDef}
          entityType="board"
          entityId={board.board.id}
          mode="compact"
          editing={false}
        />
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
