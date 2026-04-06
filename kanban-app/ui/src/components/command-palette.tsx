import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { getCM, Vim } from "@replit/codemirror-vim";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useDispatchCommand, type CommandAtDepth } from "@/lib/command-scope";
import { useUIState } from "@/lib/ui-state-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { fuzzyMatch } from "@/lib/fuzzy-filter";
import { moniker } from "@/lib/moniker";
import { useEntityCommands } from "@/lib/entity-commands";
import { FocusScope } from "@/components/focus-scope";
import { EntityIcon } from "@/components/entity-icon";

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
  const { keymap_mode: mode, scope_chain: scopeChain } = useUIState();
  const dispatch = useDispatchCommand();

  /** Shape returned by the backend. */
  interface ResolvedCommand {
    id: string;
    name: string;
    target?: string;
    context_menu: boolean;
    keys?: { vim?: string; cua?: string; emacs?: string };
    available: boolean;
  }

  // Fetch commands from the backend when the palette opens or scope changes.
  const [backendCommands, setBackendCommands] = useState<ResolvedCommand[]>([]);
  useEffect(() => {
    if (!open) return;
    invoke<ResolvedCommand[]>("list_commands_for_scope", {
      scopeChain: scopeChain ?? [],
    })
      .then(setBackendCommands)
      .catch((e) => {
        console.error("list_commands_for_scope failed:", e);
        setBackendCommands([]);
      });
  }, [open, scopeChain]);

  // Adapt backend commands to the shape the palette expects (CommandAtDepth)
  const allCommands: CommandAtDepth[] = useMemo(
    () =>
      backendCommands.map((cmd) => ({
        command: {
          id: cmd.id,
          name: cmd.name,
          target: cmd.target,
          keys: cmd.keys,
        },
        depth: 0,
      })),
    [backendCommands],
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
    if (entry) {
      onClose();
      dispatch(entry.command.id, {
        target: entry.command.target,
      }).catch(console.error);
    }
  }, [filteredCommands, selectedIndex, onClose, dispatch]);

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
              return (
                <div
                  key={entry.command.id + ":" + (entry.command.target ?? "")}
                  role="option"
                  aria-selected={index === selectedIndex}
                  data-testid={`command-item-${entry.command.id}`}
                  className={`flex cursor-pointer items-center justify-between px-3 py-1.5 text-sm
                      ${index === selectedIndex ? "bg-accent text-accent-foreground" : "hover:bg-accent/50"}`}
                  onClick={() => {
                    onClose();
                    dispatch(entry.command.id, {
                      target: entry.command.target,
                    }).catch(console.error);
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

/**
 * Single search result row wrapped in a FocusScope.
 *
 * Extracted as a component so the `useEntityCommands` hook can be called
 * at the top level of a component (hooks cannot be called inside `.map()`).
 */
/** Props for the SearchResultItem component. */
interface ResultRowProps {
  result: SearchResult;
  index: number;
  selectedIndex: number;
  onClose: () => void;
  onHoverIndex: (index: number) => void;
}

function SearchResultItem({
  result,
  index,
  selectedIndex,
  onClose,
  onHoverIndex,
}: ResultRowProps) {
  const entityMoniker = moniker(result.entity_type, result.entity_id);
  const commands = useEntityCommands(result.entity_type, result.entity_id);

  return (
    <FocusScope moniker={entityMoniker} commands={commands}>
      <SearchResultRow
        entityMoniker={entityMoniker}
        result={result}
        index={index}
        selectedIndex={selectedIndex}
        onClose={onClose}
        onHoverIndex={onHoverIndex}
      />
    </FocusScope>
  );
}

/** Inner row that can access the FocusScope's CommandScopeContext. */
function SearchResultRow({
  entityMoniker,
  result,
  index,
  selectedIndex,
  onClose,
  onHoverIndex,
}: {
  entityMoniker: string;
  result: ResultRowProps["result"];
  index: number;
  selectedIndex: number;
  onClose: () => void;
  onHoverIndex: (index: number) => void;
}) {
  const dispatch = useDispatchCommand("entity.inspect");
  return (
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
  );
}
