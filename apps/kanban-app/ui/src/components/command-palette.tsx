import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { getCM, Vim } from "@replit/codemirror-vim";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import {
  FocusedScopeContext,
  scopeChainFromScope,
  useDispatchCommand,
  type CommandAtDepth,
} from "@/lib/command-scope";
import { useUIState } from "@/lib/ui-state-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { useCommandList } from "@/hooks/use-command-list";
import { useCommandAvailability } from "@/hooks/use-command-availability";
import { fuzzyMatch } from "@/lib/fuzzy-filter";
import { moniker } from "@/lib/moniker";
import { FocusScope } from "@/components/focus-scope";
import { EntityIcon } from "@/components/entity-icon";
import { asSegment } from "@/types/spatial";

/** Result shape returned by the backend `search_entities` command. */
interface SearchResult {
  entity_type: string;
  entity_id: string;
  display_name: string;
  score: number;
}

interface CommandPaletteProps {
  /** Whether the palette is currently visible. */
  open: boolean;
  /** Called to dismiss the palette. */
  onClose: () => void;
  /** Palette mode: "command" (default) filters commands; "search" calls backend search. */
  mode?: "command" | "search";
  /** Optional handler for switching board — used when a search result is a board entity. */
  onSwitchBoard?: (path: string) => void;
}

/**
 * A command palette overlay that lets the user search and execute commands.
 *
 * Renders as a portal to document.body with a semi-transparent backdrop.
 * Uses a CM6 single-line editor for the filter input, respecting the user's
 * keymap mode (vim/emacs/CUA). In vim mode, the editor auto-enters insert
 * mode when the palette opens (like vim's : command line). Below the input,
 * a scrollable list of matching commands is shown with keybinding hints.
 *
 * Navigation: Arrow keys or j/k to move selection, Enter to execute, Escape
 * to close. In vim mode, Escape in insert mode returns to normal mode;
 * Escape in normal mode closes the palette.
 *
 * In "search" mode, the backend `search_entities` command is invoked with
 * the debounced query. Each result is wrapped in a FocusScope so entity.inspect
 * commands are available. Selecting a result opens the entity inspector.
 */
export function CommandPalette({
  open,
  onClose,
  mode: paletteMode = "command",
  onSwitchBoard,
}: CommandPaletteProps) {
  const [filter, setFilter] = useState("");
  const [debouncedFilter, setDebouncedFilter] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const keymapCompartment = useRef(new Compartment());
  const listRef = useRef<HTMLDivElement>(null);
  const { keymap_mode: mode } = useUIState();
  // Scope chain is sourced from `FocusedScopeContext` — the frontend-
  // authoritative focus tree — rather than from `useUIState().scope_chain`.
  // The backend echoes scope_chain on every `ui.setFocus`, but the
  // `UIStateProvider` suppresses those events to keep `useUIState()`
  // reference-stable. Reading the chain directly from the focus context
  // preserves the "refetch commands when focus moves while palette is
  // open" semantic without the round-trip.
  const focusedScope = useContext(FocusedScopeContext);
  const scopeChain = useMemo(
    () => scopeChainFromScope(focusedScope),
    [focusedScope],
  );
  const dispatch = useDispatchCommand();

  // The innermost scope moniker is the palette's `useCommandList` filter:
  // `list command` keeps global commands plus those whose `scope` contains it,
  // matching the per-scope set the registry surfaces for this focus point.
  const currentScope = scopeChain[0];

  // Source commands from the metadata-driven Command registry rather than a
  // hardcoded list — re-fetched live on `commands/changed`. The palette only
  // renders in command mode, so the list is unused (but cheap) in search mode.
  const { commands: registryCommands, epoch: registryEpoch } = useCommandList(
    currentScope !== undefined ? { scope: currentScope } : {},
  );

  // Adapt registry commands to the shape the palette renders (CommandAtDepth).
  // Hidden commands (`visible: false`) never reach a surface.
  const allCommands: CommandAtDepth[] = useMemo(
    () =>
      registryCommands
        .filter((cmd) => cmd.visible !== false)
        .map((cmd) => ({
          command: {
            id: cmd.id,
            name: cmd.name,
            keys: cmd.keys,
          },
          depth: 0,
        })),
    [registryCommands],
  );

  // Dispatch inspect for search mode — dispatches to the backend via
  // the standard command system, which updates UIState inspector_stack.
  const dispatchInspect = useDispatchCommand("ui.inspect");

  // Reset state when palette opens.
  useEffect(() => {
    if (open) {
      setFilter("");
      setDebouncedFilter("");
      setSelectedIndex(0);
      setSearchResults([]);
    }
  }, [open]);

  // 150ms debounce on filter for search mode
  useEffect(() => {
    if (paletteMode !== "search") {
      setDebouncedFilter(filter);
      return;
    }
    const timer = setTimeout(() => {
      setDebouncedFilter(filter);
    }, 150);
    return () => clearTimeout(timer);
  }, [filter, paletteMode]);

  // Call backend search_entities when debouncedFilter changes (search mode only)
  useEffect(() => {
    if (paletteMode !== "search") return;
    if (!debouncedFilter.trim()) {
      setSearchResults([]);
      return;
    }
    let cancelled = false;
    invoke<SearchResult[]>("search_entities", {
      query: debouncedFilter,
      limit: 50,
    })
      .then((results) => {
        if (!cancelled) {
          setSearchResults(results);
        }
      })
      .catch((err) => {
        console.error("search_entities error:", err);
        if (!cancelled) {
          setSearchResults([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [paletteMode, debouncedFilter]);

  // Auto-enter insert mode in vim when the palette opens.
  // Retries until getCM() returns a value — the vim extension may take
  // a few frames to initialize after mount.
  useEffect(() => {
    if (!open || mode !== "vim") return;
    let cancelled = false;
    let attempts = 0;
    const tryEnterInsert = () => {
      if (cancelled || attempts > 20) return;
      attempts++;
      const view = editorRef.current?.view;
      if (!view) {
        requestAnimationFrame(tryEnterInsert);
        return;
      }
      const cm = getCM(view);
      if (!cm) {
        requestAnimationFrame(tryEnterInsert);
        return;
      }
      Vim.handleKey(cm as any, "i", "mapping");
    };
    requestAnimationFrame(tryEnterInsert);
    return () => {
      cancelled = true;
    };
  }, [open, mode]);

  // Filter and sort commands by fuzzy match score (command mode)
  const filteredCommands = useMemo(() => {
    if (paletteMode !== "command") return [];
    if (filter.length === 0) {
      return allCommands;
    }
    const scored: { entry: CommandAtDepth; score: number }[] = [];
    for (const entry of allCommands) {
      const result = fuzzyMatch(filter, entry.command.name);
      if (result.match) {
        scored.push({ entry, score: result.score });
      }
    }
    scored.sort((a, b) => a.score - b.score);
    return scored.map((s) => s.entry);
  }, [filter, allCommands, paletteMode]);

  // Evaluate `available command` for every visible row. The hook batches the
  // ids into one concurrency-limited fan-out and caches the verdicts until the
  // scope chain changes — so re-opening or typing does not re-hit the service.
  // Unevaluated ids are absent from the map and treated as available, so a
  // command never flickers grayed-out before its verdict resolves.
  const visibleIds = useMemo(
    () => filteredCommands.map((entry) => entry.command.id),
    [filteredCommands],
  );
  const { availability } = useCommandAvailability({
    enabled: open && paletteMode === "command",
    ids: visibleIds,
    scopeChain,
    // Invalidate cached verdicts when the registry changes (commands/changed),
    // so an already-open palette re-evaluates instead of showing stale rows.
    epoch: registryEpoch,
  });

  // Combined length for selection clamping
  const filteredLength =
    paletteMode === "search" ? searchResults.length : filteredCommands.length;

  // Clamp selection when filtered list changes
  useEffect(() => {
    setSelectedIndex((prev) => Math.min(prev, Math.max(0, filteredLength - 1)));
  }, [filteredLength]);

  // Execute the selected command (command mode)
  const executeSelectedCommand = useCallback(() => {
    const entry = filteredCommands[selectedIndex];
    if (!entry) return;
    // Unavailable commands are inert — Enter on a grayed-out row is a no-op,
    // matching the grayed-out click affordance.
    if (availability[entry.command.id]?.available === false) return;
    onClose();
    dispatch(entry.command.id).catch(console.error);
  }, [filteredCommands, selectedIndex, onClose, dispatch, availability]);

  // Execute the selected entity result (search mode)
  const executeSelectedResult = useCallback(() => {
    const result = searchResults[selectedIndex];
    if (!result) return;
    onClose();
    // Board is the one entity type whose primary action is "switch to it"
    // rather than "inspect it". This is correct domain behavior: selecting a
    // board in search results opens (switches to) that board, while every
    // other entity type gets inspected. The check is intentionally hardcoded
    // because no other entity type shares this navigation semantic.
    if (result.entity_type === "board" && onSwitchBoard) {
      onSwitchBoard(result.entity_id);
    }
    const entityMoniker = moniker(result.entity_type, result.entity_id);
    dispatchInspect({ target: entityMoniker }).catch(console.error);
  }, [searchResults, selectedIndex, onClose, dispatchInspect, onSwitchBoard]);

  const executeSelected =
    paletteMode === "search" ? executeSelectedResult : executeSelectedCommand;

  // Ref so CM6 extensions always see the latest closures
  const executeSelectedRef = useRef(executeSelected);
  executeSelectedRef.current = executeSelected;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  /** Move selection up or down, clamped to list bounds. */
  const moveSelection = useCallback(
    (delta: number) => {
      setSelectedIndex((prev) =>
        Math.max(0, Math.min(filteredLength - 1, prev + delta)),
      );
    },
    [filteredLength],
  );
  const moveSelectionRef = useRef(moveSelection);
  moveSelectionRef.current = moveSelection;

  // Scroll the selected item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const item = list.children[selectedIndex] as HTMLElement | undefined;
    // Guard: jsdom does not implement scrollIntoView
    if (item && typeof item.scrollIntoView === "function") {
      item.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);

  // Hot-swap keymap when mode changes while palette is open
  const prevModeRef = useRef<string | null>(null);
  useEffect(() => {
    if (!open || !editorRef.current?.view) {
      prevModeRef.current = null;
      return;
    }
    if (prevModeRef.current !== null && prevModeRef.current !== mode) {
      editorRef.current.view.dispatch({
        effects: keymapCompartment.current.reconfigure(keymapExtension(mode)),
      });
    }
    prevModeRef.current = mode;
  }, [mode, open]);

  // CM6 extensions for the single-line filter input.
  // Submit/cancel (Enter/Escape) is handled by the shared helper which
  // correctly supports vim's two-phase Escape (insert → normal, normal → cancel).
  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: executeSelectedRef,
        onCancelRef: onCloseRef,
        singleLine: true,
        alwaysSubmitOnEnter: true,
      }),
      // Arrow key navigation is palette-specific, not submit/cancel
      keymap.of([
        {
          key: "ArrowDown",
          run: () => {
            moveSelectionRef.current(1);
            return true;
          },
        },
        {
          key: "ArrowUp",
          run: () => {
            moveSelectionRef.current(-1);
            return true;
          },
        },
      ]),
    ],
    [mode],
  );

  // Get the keybinding hint for the current keymap mode
  const keyHint = useCallback(
    (cmd: CommandAtDepth): string | undefined => {
      const keys = cmd.command.keys;
      if (!keys) return undefined;
      return keys[mode as keyof typeof keys];
    },
    [mode],
  );

  if (!open) return null;

  return createPortal(
    <div
      data-testid="command-palette-backdrop"
      className="fixed inset-0 z-50 bg-black/50"
      onClick={onClose}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          console.warn("[palette] ESC on backdrop — dismissing");
          onClose();
        }
      }}
    >
      <div
        data-testid="command-palette"
        className="mx-auto mt-[25vh] w-full max-w-lg rounded-lg border border-border
          bg-popover text-popover-foreground shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* CM6 single-line filter input */}
        <div className="border-b border-border px-3 py-2">
          <CodeMirror
            ref={editorRef}
            autoFocus
            value={filter}
            onChange={setFilter}
            extensions={extensions}
            theme={shadcnTheme}
            basicSetup={false}
            placeholder={
              paletteMode === "search"
                ? "Type to search..."
                : "Type a command..."
            }
            className="text-sm"
          />
        </div>

        {/* Results list */}
        <div
          ref={listRef}
          data-testid="command-palette-list"
          className="max-h-64 overflow-y-auto py-1"
          role="listbox"
        >
          {paletteMode === "search" ? (
            <SearchResults
              results={searchResults}
              selectedIndex={selectedIndex}
              hasQuery={debouncedFilter.length > 0}
              onClose={onClose}
              onHoverIndex={setSelectedIndex}
            />
          ) : filteredCommands.length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">
              No matching commands
            </div>
          ) : (
            filteredCommands.map((entry, index) => {
              const hint = keyHint(entry);
              // Availability is consulted per visible row. A missing verdict
              // (not yet resolved) defaults to available so commands never
              // flicker grayed-out; an explicit `available: false` grays the
              // row out and surfaces `reason` as the tooltip.
              const verdict = availability[entry.command.id];
              const isUnavailable = verdict?.available === false;
              const reason = verdict?.reason;
              return (
                <div
                  key={entry.command.id}
                  role="option"
                  aria-selected={index === selectedIndex}
                  aria-disabled={isUnavailable}
                  title={isUnavailable ? reason : undefined}
                  data-testid={`command-item-${entry.command.id}`}
                  data-available={isUnavailable ? "false" : "true"}
                  className={`flex items-center justify-between px-3 py-1.5 text-sm
                      ${
                        isUnavailable
                          ? "cursor-not-allowed text-muted-foreground/50"
                          : "cursor-pointer"
                      }
                      ${
                        index === selectedIndex
                          ? "bg-accent text-accent-foreground"
                          : "hover:bg-accent/50"
                      }`}
                  onClick={() => {
                    // Unavailable commands are inert — match the grayed-out
                    // affordance by swallowing the click.
                    if (isUnavailable) return;
                    onClose();
                    dispatch(entry.command.id).catch(console.error);
                  }}
                  onMouseEnter={() => setSelectedIndex(index)}
                >
                  <span>{entry.command.name}</span>
                  {hint && (
                    <kbd className="ml-4 shrink-0 rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
                      {hint}
                    </kbd>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}

/** Props for the SearchResults component. */
interface ResultListProps {
  results: SearchResult[];
  selectedIndex: number;
  hasQuery: boolean;
  onClose: () => void;
  onHoverIndex: (index: number) => void;
}

/** Renders search results in search mode. Extracted to keep CommandPalette readable. */
function SearchResults({
  results,
  selectedIndex,
  hasQuery,
  onClose,
  onHoverIndex,
}: ResultListProps) {
  if (!hasQuery) {
    return (
      <div className="px-3 py-2 text-sm text-muted-foreground">
        Type to search...
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div className="px-3 py-2 text-sm text-muted-foreground">
        No matching entities
      </div>
    );
  }

  return (
    <>
      {results.map((result, index) => (
        <SearchResultItem
          key={moniker(result.entity_type, result.entity_id)}
          result={result}
          index={index}
          selectedIndex={selectedIndex}
          onClose={onClose}
          onHoverIndex={onHoverIndex}
        />
      ))}
    </>
  );
}

/** Props for the SearchResultItem component. */
interface ResultRowProps {
  result: SearchResult;
  index: number;
  selectedIndex: number;
  onClose: () => void;
  onHoverIndex: (index: number) => void;
}

/**
 * Single search result row wrapped in a FocusScope.
 *
 * Extracted as a top-level component so per-row scope registration (which
 * relies on `useEffect` inside `FocusScope`) is not called from within
 * `.map()`. The row's click handler dispatches `entity.inspect` against
 * the result's moniker; `useDispatchCommand` is a hook and therefore must
 * live at the top level of this component.
 */
function SearchResultItem({
  result,
  index,
  selectedIndex,
  onClose,
  onHoverIndex,
}: ResultRowProps) {
  const entityMoniker = asSegment(
    moniker(result.entity_type, result.entity_id),
  );
  const dispatch = useDispatchCommand("entity.inspect");

  return (
    <FocusScope moniker={entityMoniker}>
      <div
        role="option"
        aria-selected={index === selectedIndex}
        data-testid={`search-result-${entityMoniker}`}
        className={`flex cursor-pointer items-center gap-2 px-3 py-1.5 text-sm
          ${index === selectedIndex ? "bg-accent text-accent-foreground" : "hover:bg-accent/50"}`}
        onClick={() => {
          onClose();
          dispatch({ target: entityMoniker }).catch(console.error);
        }}
        onMouseEnter={() => onHoverIndex(index)}
      >
        <EntityIcon
          entityType={result.entity_type}
          className="h-3.5 w-3.5 shrink-0 text-muted-foreground"
        />
        <span className="min-w-0 truncate">{result.display_name}</span>
      </div>
    </FocusScope>
  );
}
