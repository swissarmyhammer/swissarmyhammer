import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { Field } from "@/components/fields/field";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import { moniker } from "@/lib/moniker";
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
        <button
          type="button"
          className="p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
          onClick={() => {
            dispatchInspect({ target: moniker("board", "board") }).catch(
              console.error,
            );
          }}
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
        onClick={() => dispatchSearch().catch(console.error)}
        title="Search"
      >
        <Search className="h-4 w-4" />
      </button>
    </header>
  );
}
