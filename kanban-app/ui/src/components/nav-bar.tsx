import { forwardRef, useCallback, useMemo } from "react";
import { Info, Search } from "lucide-react";
import { BoardSelector } from "@/components/board-selector";
import { Field } from "@/components/fields/field";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { useSchema } from "@/lib/schema-context";
import {
  useDispatchCommand,
  useCommandBusy,
  type CommandDef,
} from "@/lib/command-scope";
import { FocusScope, useFocusScopeElementRef } from "@/components/focus-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
import {
  useBoardData,
  useOpenBoards,
  useActiveBoardPath,
  useHandleSwitchBoard,
} from "@/components/window-container";
import type { Entity, FieldDef, OpenBoard } from "@/types/kanban";

/**
 * Namespaced moniker for each toolbar FocusScope.
 *
 * All four toolbar targets live under a shared `toolbar:` prefix so
 * spatial-nav tests can match the top-edge contract with a single
 * `/^toolbar:/` regex, and so the monikers are globally unique from any
 * other `FocusScope` in the window layer (LeftNav uses `view:*`, the
 * perspective bar uses `perspective:*`).
 */
const TOOLBAR_BOARD_SELECTOR_MONIKER = "toolbar:board-selector";
const TOOLBAR_INSPECT_BOARD_MONIKER = "toolbar:inspect-board";
const TOOLBAR_PERCENT_COMPLETE_MONIKER = "toolbar:percent-complete";
const TOOLBAR_SEARCH_MONIKER = "toolbar:search";

/**
 * Inner `<button>` element for the Inspect Board toolbar target.
 *
 * Wired via `forwardRef` so Radix's `TooltipTrigger asChild` can forward
 * its Slot ref to the DOM node, and internally composes that forwarded
 * ref with the enclosing `FocusScope`'s `elementRef` (read from
 * `useFocusScopeElementRef()`) so `ResizeObserver` inside the scope can
 * measure the button's rect for spatial navigation and `useFocusDecoration`
 * can set `data-focused` directly on this `<button>` when the toolbar
 * moniker is claimed.
 *
 * Mirrors the `ViewButtonElement` pattern in `left-nav.tsx` — the scope's
 * `renderContainer={false}` means the button itself defines the scope's
 * spatial footprint.
 */
const InspectBoardButton = forwardRef<
  HTMLButtonElement,
  React.ButtonHTMLAttributes<HTMLButtonElement>
>(function InspectBoardButton({ onClick, ...rest }, forwardedRef) {
  const scopeElementRef = useFocusScopeElementRef();
  const refCallback = useCallback(
    (node: HTMLButtonElement | null) => {
      if (scopeElementRef) scopeElementRef.current = node;
      if (typeof forwardedRef === "function") forwardedRef(node);
      else if (forwardedRef) forwardedRef.current = node;
    },
    [scopeElementRef, forwardedRef],
  );

  return (
    <button
      ref={refCallback}
      type="button"
      aria-label="Inspect board"
      data-moniker={TOOLBAR_INSPECT_BOARD_MONIKER}
      data-testid={`data-moniker:${TOOLBAR_INSPECT_BOARD_MONIKER}`}
      className="p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
      onClick={onClick}
      {...rest}
    >
      <Info className="h-4 w-4" />
    </button>
  );
});

/**
 * Inner `<button>` element for the Search toolbar target.
 *
 * Same ref-composition pattern as {@link InspectBoardButton}.
 */
const SearchButton = forwardRef<
  HTMLButtonElement,
  React.ButtonHTMLAttributes<HTMLButtonElement>
>(function SearchButton({ onClick, ...rest }, forwardedRef) {
  const scopeElementRef = useFocusScopeElementRef();
  const refCallback = useCallback(
    (node: HTMLButtonElement | null) => {
      if (scopeElementRef) scopeElementRef.current = node;
      if (typeof forwardedRef === "function") forwardedRef(node);
      else if (forwardedRef) forwardedRef.current = node;
    },
    [scopeElementRef, forwardedRef],
  );

  return (
    <button
      ref={refCallback}
      type="button"
      aria-label="Search"
      data-moniker={TOOLBAR_SEARCH_MONIKER}
      data-testid={`data-moniker:${TOOLBAR_SEARCH_MONIKER}`}
      className="ml-auto p-1 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
      onClick={onClick}
      {...rest}
    >
      <Search className="h-4 w-4" />
    </button>
  );
});

/**
 * Wraps the `BoardSelector` in a FocusScope so the board name + dropdown
 * is reachable via spatial navigation. Attaches the enclosing scope's
 * `elementRef` to a thin `<div>` that contains the real selector so
 * `ResizeObserver` can measure its rect.
 *
 * Enter on this scope opens the Radix `Select` dropdown by firing a
 * synthetic click on the `SelectTrigger` — matching the click path.
 * The trigger is located inside the selector subtree via
 * `data-slot="select-trigger"` (the stable attribute Radix sets on its
 * trigger root).
 */
function ScopedBoardSelector({
  boards,
  selectedPath,
  onSelect,
  boardEntity,
}: {
  boards: OpenBoard[];
  selectedPath: string | null;
  onSelect: (path: string) => void;
  boardEntity: Entity | undefined;
}) {
  const scopeElementRef = useFocusScopeElementRef();
  const { setFocus } = useEntityFocus();
  const refCallback = useCallback(
    (node: HTMLDivElement | null) => {
      if (scopeElementRef) scopeElementRef.current = node;
    },
    [scopeElementRef],
  );
  // Capture-phase click handler: with `renderContainer={false}` on the
  // enclosing FocusScope, the scope's own click handler never runs — so
  // the consumer must move spatial focus onto the scope's moniker
  // before Radix's internal click target (SelectTrigger, Field inline
  // editor) consumes the event. Using capture rather than bubble mirrors
  // the `ScopedPerspectiveTab` pattern and avoids dropped clicks when
  // Radix components stopPropagation internally.
  const handleScopeClick = useCallback(() => {
    setFocus(TOOLBAR_BOARD_SELECTOR_MONIKER);
  }, [setFocus]);

  return (
    <div
      ref={refCallback}
      data-moniker={TOOLBAR_BOARD_SELECTOR_MONIKER}
      data-testid={`data-moniker:${TOOLBAR_BOARD_SELECTOR_MONIKER}`}
      onClickCapture={handleScopeClick}
      className="flex items-center min-w-0"
    >
      <BoardSelector
        boards={boards}
        selectedPath={selectedPath}
        onSelect={onSelect}
        boardEntity={boardEntity}
        showTearOff
      />
    </div>
  );
}

/**
 * Inner wrapper that registers the percent-complete `Field` as a spatial
 * nav target. The `Field` itself does not wrap its value cell in a
 * `FocusScope`, so the toolbar adds a thin `<div>` that attaches the
 * enclosing scope's `elementRef` — `ResizeObserver` measures this div's
 * rect for the beam-test graph.
 *
 * The scope is commands-less and not Enter-activatable: the percent is a
 * read-only display on the toolbar, so `h`/`l` passes through it as a
 * landing site without any activation semantics.
 */
function ScopedPercentComplete({
  entityId,
  // Accept the field def as a prop to avoid re-resolving it in the child;
  // `Field` takes the whole def verbatim.
  fieldDef,
}: {
  entityId: string;
  fieldDef: FieldDef;
}) {
  const scopeElementRef = useFocusScopeElementRef();
  const { setFocus } = useEntityFocus();
  const refCallback = useCallback(
    (node: HTMLDivElement | null) => {
      if (scopeElementRef) scopeElementRef.current = node;
    },
    [scopeElementRef],
  );
  const handleScopeClick = useCallback(() => {
    setFocus(TOOLBAR_PERCENT_COMPLETE_MONIKER);
  }, [setFocus]);
  return (
    <div
      ref={refCallback}
      data-moniker={TOOLBAR_PERCENT_COMPLETE_MONIKER}
      data-testid={`data-moniker:${TOOLBAR_PERCENT_COMPLETE_MONIKER}`}
      onClickCapture={handleScopeClick}
      className="flex items-center"
    >
      <Field
        fieldDef={fieldDef}
        entityType="board"
        entityId={entityId}
        mode="compact"
        editing={false}
      />
    </div>
  );
}

/**
 * Top-level navigation bar. Reads board data, open boards, active path,
 * and switch-board handler from WindowContainer context -- takes no props.
 *
 * Each interactive element is wrapped in a `FocusScope` with a
 * `toolbar:*` moniker so the spatial engine sees a rect for it and
 * `k` (up) from the perspective bar or LeftNav lands on the toolbar
 * rather than finding no candidate above. Enter on each scope mirrors
 * the corresponding click action:
 *
 * - `toolbar:board-selector` — opens the board-switch dropdown
 * - `toolbar:inspect-board` — dispatches `ui.inspect` on the board
 * - `toolbar:search`        — dispatches `app.search`
 * - `toolbar:percent-complete` — no Enter action; read-only landing site
 */
/**
 * Build an Enter-bound `CommandDef` excluded from the context menu.
 *
 * Every toolbar FocusScope exposes a single keyboard activation
 * command — this helper keeps the `id` / `name` / handler wiring
 * consistent and the context-menu noise suppressed.
 */
function buildEnterCommand(
  id: string,
  name: string,
  execute: () => void,
): CommandDef {
  return {
    id,
    name,
    keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
    execute,
    contextMenu: false,
  };
}

/**
 * Build the `{ handler, commands }` pair for each toolbar button: the
 * handler composes `setFocus(moniker)` + the click side effect so mouse
 * and keyboard Enter paths converge, and `commands` wires the same
 * handler behind Enter via an Enter-bound `CommandDef`.
 */
function useEnterCommand(id: string, name: string, execute: () => void) {
  return useMemo<CommandDef[]>(
    () => [buildEnterCommand(id, name, execute)],
    [id, name, execute],
  );
}

/**
 * Stable click handlers for each toolbar scope.
 *
 * Each handler composes `setFocus(moniker)` with the button's side effect
 * (open Radix dropdown, dispatch `ui.inspect`, dispatch `app.search`) so
 * mouse and keyboard Enter paths converge on the same code. Returned
 * callbacks are memoized for use as both `onClick` and as command
 * `execute` targets in `useToolbarActions`.
 */
function useToolbarHandlers(board: ReturnType<typeof useBoardData>) {
  const dispatchInspect = useDispatchCommand("ui.inspect");
  const dispatchSearch = useDispatchCommand("app.search");
  const { setFocus } = useEntityFocus();

  // Enter opens the Radix Select dropdown via a click on the SelectTrigger
  // (located by its stable `data-slot` attribute).
  const openBoardSelector = useCallback(() => {
    setFocus(TOOLBAR_BOARD_SELECTOR_MONIKER);
    document
      .querySelector<HTMLButtonElement>(
        `[data-moniker="${TOOLBAR_BOARD_SELECTOR_MONIKER}"] [data-slot="select-trigger"]`,
      )
      ?.click();
  }, [setFocus]);

  const inspectBoard = useCallback(() => {
    if (!board) return;
    setFocus(TOOLBAR_INSPECT_BOARD_MONIKER);
    dispatchInspect({ target: board.board.moniker }).catch(console.error);
  }, [board, dispatchInspect, setFocus]);

  const search = useCallback(() => {
    setFocus(TOOLBAR_SEARCH_MONIKER);
    dispatchSearch().catch(console.error);
  }, [dispatchSearch, setFocus]);

  return { openBoardSelector, inspectBoard, search };
}

/**
 * Build the `{ handler, commands }` pair for each toolbar button: handlers
 * compose `setFocus(moniker)` + the click side effect so mouse and keyboard
 * Enter paths converge, and `commands` wires the same handler behind Enter
 * via an Enter-bound `CommandDef`.
 */
function useToolbarActions(board: ReturnType<typeof useBoardData>) {
  const { openBoardSelector, inspectBoard, search } = useToolbarHandlers(board);
  const boardSelectorCommands = useEnterCommand(
    "toolbar.board-selector.activate",
    "Open board selector",
    openBoardSelector,
  );
  const inspectCommands = useEnterCommand(
    "toolbar.inspect-board.activate",
    "Inspect board",
    inspectBoard,
  );
  const searchCommands = useEnterCommand(
    "toolbar.search.activate",
    "Search",
    search,
  );
  return {
    inspectBoard,
    search,
    boardSelectorCommands,
    inspectCommands,
    searchCommands,
  };
}

function BusyIndicator() {
  return (
    <div
      role="progressbar"
      aria-label="Command in progress"
      className="absolute bottom-0 left-0 right-0 h-0.5 overflow-hidden"
    >
      <div className="h-full w-1/3 bg-primary animate-indeterminate" />
    </div>
  );
}

function InspectBoardScope({
  commands,
  onInspect,
}: {
  commands: CommandDef[];
  onInspect: () => void;
}) {
  return (
    <FocusScope
      moniker={TOOLBAR_INSPECT_BOARD_MONIKER}
      commands={commands}
      renderContainer={false}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <InspectBoardButton onClick={onInspect} />
        </TooltipTrigger>
        <TooltipContent side="bottom">Inspect board</TooltipContent>
      </Tooltip>
    </FocusScope>
  );
}

function SearchScope({
  commands,
  onSearch,
}: {
  commands: CommandDef[];
  onSearch: () => void;
}) {
  return (
    <FocusScope
      moniker={TOOLBAR_SEARCH_MONIKER}
      commands={commands}
      renderContainer={false}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <SearchButton onClick={onSearch} />
        </TooltipTrigger>
        <TooltipContent side="bottom">Search</TooltipContent>
      </Tooltip>
    </FocusScope>
  );
}

function PercentCompleteScope({
  entityId,
  fieldDef,
}: {
  entityId: string;
  fieldDef: FieldDef;
}) {
  return (
    <FocusScope
      moniker={TOOLBAR_PERCENT_COMPLETE_MONIKER}
      commands={[]}
      renderContainer={false}
    >
      <ScopedPercentComplete entityId={entityId} fieldDef={fieldDef} />
    </FocusScope>
  );
}

/**
 * Top application toolbar — fixed 48px-tall `<header>` with the board
 * selector, board inspect button, percent-complete readout, search
 * button, and an in-progress command indicator.
 *
 * Every interactive element is wrapped in a `FocusScope` under the
 * `toolbar:*` moniker namespace so the Rust spatial engine registers a
 * rect above the perspective bar and LeftNav — pressing `k` (Up) from
 * those regions reaches a toolbar button. Each scope binds Enter to the
 * button's click action through `useToolbarActions`, so mouse and
 * keyboard paths converge on the same dispatcher.
 *
 * Meant to be rendered once at the App level inside `AppShell`.
 */
export function NavBar() {
  const board = useBoardData();
  const openBoards = useOpenBoards();
  const activeBoardPath = useActiveBoardPath();
  const onSwitchBoard = useHandleSwitchBoard();
  const { getFieldDef } = useSchema();
  const percentFieldDef = getFieldDef("board", "percent_complete");
  const { isBusy } = useCommandBusy();
  const actions = useToolbarActions(board);
  const selectedPath =
    activeBoardPath ?? openBoards.find((b) => b.is_active)?.path ?? null;

  return (
    <header className="relative flex h-12 items-center border-b px-4 gap-3">
      <FocusScope
        moniker={TOOLBAR_BOARD_SELECTOR_MONIKER}
        commands={actions.boardSelectorCommands}
        renderContainer={false}
      >
        <ScopedBoardSelector
          boards={openBoards}
          selectedPath={selectedPath}
          onSelect={onSwitchBoard}
          boardEntity={board?.board}
        />
      </FocusScope>
      {board && (
        <InspectBoardScope
          commands={actions.inspectCommands}
          onInspect={actions.inspectBoard}
        />
      )}
      {board && percentFieldDef && (
        <PercentCompleteScope
          entityId={board.board.id}
          fieldDef={percentFieldDef}
        />
      )}
      <SearchScope
        commands={actions.searchCommands}
        onSearch={actions.search}
      />
      {isBusy && <BusyIndicator />}
    </header>
  );
}
