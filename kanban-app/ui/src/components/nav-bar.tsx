import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { Field } from "@/components/fields/field";
import { FocusScope } from "@/components/focus-scope";
import { Pressable } from "@/components/pressable";
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
import { asSegment } from "@/types/spatial";

/**
 * Top-level navigation bar.
 *
 * Reads board data, open boards, active path, and switch-board handler from
 * `WindowContainer` context — takes no props.
 *
 * The bar is a plain `<div role="banner">` — NOT a `<FocusScope>`. Each
 * actionable child (board selector, inspect button, search button) registers
 * as its own `<FocusScope>` leaf with a `ui:navbar.{name}` moniker, and
 * those leaves register as **peer top-level scopes** under the surrounding
 * `<FocusLayer name="window">` — siblings of `ui:left-nav` and
 * `ui:perspective-bar`. Inner scopes only mount when their content is
 * actually rendered, so we never publish a zero-rect leaf for a button
 * currently hidden behind a conditional.
 *
 * # Why the outer `<FocusScope moniker="ui:navbar">` is gone
 *
 * The bar spans the full viewport width. When the navbar was wrapped in a
 * `<FocusScope>`, the kernel saw a viewport-spanning rectangle that
 * swallowed clicks landing on bar whitespace AND beam-search candidates
 * arriving from below — focus resolved to the parent `ui:navbar` rather
 * than to any inner leaf, so clicks on the board-name field, arrow-nav
 * from the left-nav, and arrow-nav from the perspective-bar all failed to
 * reach the inner leaves. This is the same class of bug
 * commit `8232b25cc` fixed for `ui:board`: a redundant container scope
 * sitting at the same rect as something else, swallowing focus that
 * belongs to the inner content.
 *
 * Promoting the inner scopes to peers of `ui:left-nav` /
 * `ui:perspective-bar` lets the kernel hit-test them directly and lets
 * beam-search treat them as first-class navigation candidates.
 *
 * # Focus indicator layout
 *
 * `<FocusIndicator>` paints a dotted border inside each leaf's box; no
 * special gap or padding is required to make room for it.
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
    <div
      role="banner"
      className="relative flex h-12 items-center border-b px-4 gap-2"
    >
      {/*
        `<BoardSelector>` is itself a multi-leaf surface (editable name
        Field, dropdown trigger, tear-off button), so the navbar wraps
        it in a `<FocusZone>` rather than a `<FocusScope>`. The selector
        component places `<FocusScope>` leaves around the dropdown and
        tear-off; the editable name `<Field>` is its own zone. This
        keeps the kernel's scope-is-leaf invariant intact — see
        `swissarmyhammer-focus/tests/scope_is_leaf.rs`.
      */}
      <FocusScope
        moniker={asSegment("ui:navbar.board-selector")}
        // showFocus=false: container zone (BoardSelector's inner Field / dropdown / tear-off paint their own focus).
        showFocus={false}
      >
        <BoardSelector
          boards={openBoards}
          selectedPath={
            activeBoardPath ?? openBoards.find((b) => b.is_active)?.path ?? null
          }
          onSelect={onSwitchBoard}
          boardEntity={board?.board}
          showTearOff
        />
      </FocusScope>
      {board && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Pressable
              asChild
              moniker={asSegment("ui:navbar.inspect")}
              ariaLabel="Inspect board"
              onPress={() => {
                dispatchInspect({ target: board.board.moniker }).catch(
                  console.error,
                );
              }}
            >
              <button
                type="button"
                className="p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
              >
                <Info className="h-4 w-4" />
              </button>
            </Pressable>
          </TooltipTrigger>
          <TooltipContent side="bottom">Inspect board</TooltipContent>
        </Tooltip>
      )}
      {/*
        `<Field>` is itself a `<FocusZone>` keyed by
        `field:{type}:{id}.{name}` (see `fields/field.tsx`), so the
        percent-complete Field already participates in the spatial graph
        as a peer zone of the navbar's leaf scopes — no extra `<FocusScope>`
        wrap is needed here.
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
      {/*
        `ml-auto` lives on a wrapping flex child so the search button is
        pushed to the right edge of the navbar. `<Pressable asChild>`
        spreads its className onto the inner `<button>` host (via
        `Slot.Root` mergeProps), not onto the `<FocusScope>` `<div>` it
        mounts — so ml-auto on the Pressable would land on the button
        and fail to push the FocusScope wrapper right. The wrapper div
        keeps the layout invariant intact while leaving Pressable's
        contract untouched.
      */}
      <div className="ml-auto">
        <Tooltip>
          <TooltipTrigger asChild>
            <Pressable
              asChild
              moniker={asSegment("ui:navbar.search")}
              ariaLabel="Search"
              onPress={() => dispatchSearch().catch(console.error)}
            >
              <button
                type="button"
                className="p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
              >
                <Search className="h-4 w-4" />
              </button>
            </Pressable>
          </TooltipTrigger>
          <TooltipContent side="bottom">Search</TooltipContent>
        </Tooltip>
      </div>
      {isBusy && (
        <div
          role="progressbar"
          aria-label="Command in progress"
          className="absolute bottom-0 left-0 right-0 h-0.5 overflow-hidden"
        >
          <div className="h-full w-1/3 bg-primary animate-indeterminate" />
        </div>
      )}
    </div>
  );
}
