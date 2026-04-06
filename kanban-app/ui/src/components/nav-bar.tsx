import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { Field } from "@/components/fields/field";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import {
  useBoardData,
  useOpenBoards,
  useActiveBoardPath,
  useHandleSwitchBoard,
} from "@/components/window-container";

/**
 * Top-level navigation bar. Reads board data, open boards, active path,
 * and switch-board handler from WindowContainer context -- takes no props.
 */
export function NavBar() {
  const board = useBoardData();
  const openBoards = useOpenBoards();
  const activeBoardPath = useActiveBoardPath();
  const onSwitchBoard = useHandleSwitchBoard();
  const dispatchInspect = useDispatchCommand("ui.inspect");
  const dispatchSearch = useDispatchCommand("app.search");
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
      {board && (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              aria-label="Inspect board"
              className="p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
              onClick={() => {
                dispatchInspect({ target: board.board.moniker }).catch(
                  console.error,
                );
              }}
            >
              <Info className="h-4 w-4" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom">Inspect board</TooltipContent>
        </Tooltip>
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
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label="Search"
            className="ml-auto p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
            onClick={() => dispatchSearch().catch(console.error)}
          >
            <Search className="h-4 w-4" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="bottom">Search</TooltipContent>
      </Tooltip>
    </header>
  );
}
