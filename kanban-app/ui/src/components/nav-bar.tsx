import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { Field } from "@/components/fields/field";
import { FocusZone } from "@/components/focus-zone";
import { Focusable } from "@/components/focusable";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand, useCommandBusy } from "@/lib/command-scope";
import {
  useBoardData,
  useOpenBoards,
  useActiveBoardPath,
  useHandleSwitchBoard,
} from "@/components/window-container";
import { asMoniker } from "@/types/spatial";

/**
 * Top-level navigation bar.
 *
 * Reads board data, open boards, active path, and switch-board handler from
 * `WindowContainer` context — takes no props.
 *
 * The container renders as a `<FocusZone moniker="ui:navbar">` so the spatial
 * navigator can drill into the bar and remember a last-focused leaf for
 * fallback. Each actionable child (board selector, inspect button, search
 * button) registers as a `<Focusable>` leaf with a `ui:navbar.{name}` moniker
 * — but only when its content is actually rendered, so we never publish a
 * zero-rect leaf for a button that is currently hidden behind a conditional.
 *
 * No keyboard listeners live here: arrow-key traversal is owned by the Rust
 * spatial navigator. The buttons keep their click handlers so mouse / pointer
 * activation continues to work.
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
  const { isBusy } = useCommandBusy();

  return (
    <FocusZone
      moniker={asMoniker("ui:navbar")}
      showFocusBar={false}
      role="banner"
      className="relative flex h-12 items-center border-b px-4 gap-3"
    >
      <Focusable moniker={asMoniker("ui:navbar.board-selector")}>
        <BoardSelector
          boards={openBoards}
          selectedPath={
            activeBoardPath ?? openBoards.find((b) => b.is_active)?.path ?? null
          }
          onSelect={onSwitchBoard}
          boardEntity={board?.board}
          showTearOff
        />
      </Focusable>
      {board && (
        <Focusable moniker={asMoniker("ui:navbar.inspect")}>
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
        </Focusable>
      )}
      {/*
        Field is a composite that owns its own focus model — not wrapped as a
        Focusable leaf here. Field-as-a-spatial-nav citizen is covered by a
        separate spatial-nav card.
      */}
      {board && percentFieldDef && (
        <Field
          fieldDef={percentFieldDef}
          entityType="board"
          entityId={board.board.id}
          mode="compact"
          editing={false}
        />
      )}
      <Focusable moniker={asMoniker("ui:navbar.search")} className="ml-auto">
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              aria-label="Search"
              className="p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
              onClick={() => dispatchSearch().catch(console.error)}
            >
              <Search className="h-4 w-4" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom">Search</TooltipContent>
        </Tooltip>
      </Focusable>
      {isBusy && (
        <div
          role="progressbar"
          aria-label="Command in progress"
          className="absolute bottom-0 left-0 right-0 h-0.5 overflow-hidden"
        >
          <div className="h-full w-1/3 bg-primary animate-indeterminate" />
        </div>
      )}
    </FocusZone>
  );
}
