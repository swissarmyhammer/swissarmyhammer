import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { Field } from "@/components/fields/field";
import { FocusScope } from "@/components/focus-scope";
import { FocusZone } from "@/components/focus-zone";
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
 * The container renders as a `<FocusZone moniker="ui:navbar">` so the spatial
 * navigator can drill into the bar and remember a last-focused leaf for
 * fallback. Each actionable child (board selector, inspect button, search
 * button) registers as a `<FocusScope>` leaf with a `ui:navbar.{name}` moniker
 * — but only when its content is actually rendered, so we never publish a
 * zero-rect leaf for a button that is currently hidden behind a conditional.
 *
 * # Focus indicator layout
 *
 * The single `<FocusIndicator>` cursor-bar paints a 4px-wide vertical stripe
 * 8px to the LEFT of its host (`-left-2 w-1`). For that stripe to read as
 * "this nav button has focus" the bar needs room to live without colliding
 * with the previous sibling — the failure mode the historic ring variant
 * was reaching for. The layout addresses that here without a second variant:
 *
 *   - The row uses `gap-2` (8px between siblings), which matches the bar's
 *     `-left-2` offset. The bar lands in the gap immediately to the right
 *     of the previous sibling, visually pointing at the focused button —
 *     the same pattern `<PerspectiveTabBar>` ships.
 *   - The row's `px-4` provides 16px of left padding before the leftmost
 *     leaf, well over the 8px the bar needs to remain on-screen.
 *   - Each leaf wraps its child in a `<FocusScope>` with no extra padding —
 *     the cursor-bar lives at `-left-2` outside the wrapper's box, in the
 *     gap region, exactly the way every other column-strip consumer lays
 *     it out.
 *
 * # Zone-level focus
 *
 * `<FocusZone moniker="ui:navbar">` keeps `showFocusBar={false}`: the bar
 * spans the entire viewport width, so a focus indicator covering the whole
 * row would be visual noise without telling the user anything they don't
 * already know. The zone exists to be the parent of its leaves and to
 * remember a last-focused leaf for drill-out fallback — its leaves own the
 * visible focus signal. `data-focused` still flips on the wrapper so e2e
 * selectors and debugging tooling can observe the claim.
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
      moniker={asSegment("ui:navbar")}
      showFocusBar={false}
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
      <FocusZone
        moniker={asSegment("ui:navbar.board-selector")}
        showFocusBar={false}
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
      </FocusZone>
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
      <FocusScope
        moniker={asSegment("ui:navbar.search")}
        className="ml-auto"
      >
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
      </FocusScope>
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
