import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap, EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { getCM, Vim } from "@replit/codemirror-vim";
import { invoke } from "@tauri-apps/api/core";
import { useAvailableCommands, collectAvailableCommands, dispatchCommand, type CommandAtDepth } from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import { useKeymap } from "@/lib/keymap-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { fuzzyMatch } from "@/lib/fuzzy-filter";
import { useInspectOptional } from "@/lib/inspect-context";
import { moniker } from "@/lib/moniker";
import { FocusScope } from "@/components/focus-scope";
import { CheckSquare, Tag, Columns, User, type LucideIcon } from "lucide-react";

/** Map entity_type to a Lucide icon. Returns undefined for unknown types. */
const entityTypeIcons: Record<string, LucideIcon> = {
  task: CheckSquare,
  tag: Tag,
  column: Columns,
  actor: User,
};

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
export function CommandPalette({ open, onClose, mode: paletteMode = "command" }: CommandPaletteProps) {
  const [filter, setFilter] = useState("");
  const [debouncedFilter, setDebouncedFilter] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const keymapCompartment = useRef(new Compartment());
  const listRef = useRef<HTMLDivElement>(null);
  const { mode } = useKeymap();
  const focusedScope = useFocusedScope();
  const rootCommands = useAvailableCommands();
  // When a scope is focused, collect commands from it (which includes its ancestor chain).
  // Otherwise fall back to commands from the root scope context.
  const allCommands = useMemo(
    () => focusedScope ? collectAvailableCommands(focusedScope) : rootCommands,
    [focusedScope, rootCommands],
  );

  // Inspect hook for search mode (only used in search mode)
  const inspectEntity = useInspectOptional();

  // Reset state when palette opens
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
    invoke<SearchResult[]>("search_entities", { query: debouncedFilter, limit: 50 })
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
    return () => { cancelled = true; };
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
    return () => { cancelled = true; };
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
  const filteredLength = paletteMode === "search" ? searchResults.length : filteredCommands.length;

  // Clamp selection when filtered list changes
  useEffect(() => {
    setSelectedIndex((prev) => Math.min(prev, Math.max(0, filteredLength - 1)));
  }, [filteredLength]);

  // Execute the selected command (command mode)
  const executeSelectedCommand = useCallback(() => {
    const entry = filteredCommands[selectedIndex];
    if (entry) {
      onClose();
      dispatchCommand(entry.command);
    }
  }, [filteredCommands, selectedIndex, onClose]);

  // Execute the selected entity result (search mode)
  const executeSelectedResult = useCallback(() => {
    const result = searchResults[selectedIndex];
    if (result && inspectEntity) {
      const entityMoniker = moniker(result.entity_type, result.entity_id);
      onClose();
      inspectEntity(entityMoniker);
    }
  }, [searchResults, selectedIndex, onClose, inspectEntity]);

  const executeSelected = paletteMode === "search" ? executeSelectedResult : executeSelectedCommand;

  // Ref so CM6 extensions always see the latest closures
  const executeSelectedRef = useRef(executeSelected);
  executeSelectedRef.current = executeSelected;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  /** Move selection up or down, clamped to list bounds. */
  const moveSelection = useCallback(
    (delta: number) => {
      setSelectedIndex((prev) =>
        Math.max(0, Math.min(filteredLength - 1, prev + delta))
      );
    },
    [filteredLength]
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

  // CM6 extensions for the single-line filter input
  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      // Navigation and execution keybindings (highest priority)
      keymap.of([
        {
          key: "Enter",
          run: () => {
            executeSelectedRef.current();
            return true;
          },
        },
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
      // Escape handling: in vim mode, check vim state — if in insert mode,
      // let vim handle Escape (exits to normal mode). If already in normal
      // mode, close the palette. In CUA/emacs, Escape always closes.
      EditorView.domEventHandlers({
        keydown(event, view) {
          if (event.key === "Escape") {
            if (mode !== "vim") {
              onCloseRef.current();
              return true;
            }
            // Check vim state: if in insert mode, let vim exit to normal
            const cm = getCM(view);
            const vimState = cm?.state?.vim;
            if (vimState?.insertMode) {
              // Let vim handle it — will exit insert mode
              return false;
            }
            // Already in normal mode — close the palette
            onCloseRef.current();
            return true;
          }
          return false;
        },
      }),
    ],
    [mode]
  );

  // Get the keybinding hint for the current keymap mode
  const keyHint = useCallback(
    (cmd: CommandAtDepth): string | undefined => {
      const keys = cmd.command.keys;
      if (!keys) return undefined;
      return keys[mode as keyof typeof keys];
    },
    [mode]
  );

  if (!open) return null;

  return createPortal(
    <div
      data-testid="command-palette-backdrop"
      className="fixed inset-0 z-50 bg-black/50"
      onClick={onClose}
      onKeyDown={(e) => {
        // Catch Escape on the backdrop itself (e.g. when focus is outside CM6)
        if (e.key === "Escape") {
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
            placeholder={paletteMode === "search" ? "Type to search..." : "Type a command..."}
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
              inspectEntity={inspectEntity}
            />
          ) : (
            filteredCommands.length === 0 ? (
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
                      dispatchCommand(entry.command);
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
            )
          )}
        </div>
      </div>
    </div>,
    document.body
  );
}

/** Renders search results in search mode. Extracted to keep CommandPalette readable. */
function SearchResults({
  results,
  selectedIndex,
  hasQuery,
  onClose,
  onHoverIndex,
  inspectEntity,
}: {
  results: SearchResult[];
  selectedIndex: number;
  hasQuery: boolean;
  onClose: () => void;
  onHoverIndex: (index: number) => void;
  inspectEntity: ((moniker: string) => void) | null;
}) {
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
      {results.map((result, index) => {
        const entityMoniker = moniker(result.entity_type, result.entity_id);
        const Icon = entityTypeIcons[result.entity_type];
        const typeLabel = result.entity_type.charAt(0).toUpperCase() + result.entity_type.slice(1);

        const commands = [
          {
            id: "entity.inspect",
            name: `Inspect ${result.entity_type}`,
            target: entityMoniker,
            contextMenu: true,
            execute: () => {
              if (inspectEntity) {
                onClose();
                inspectEntity(entityMoniker);
              }
            },
          },
        ];

        return (
          <FocusScope
            key={entityMoniker}
            moniker={entityMoniker}
            commands={commands}
          >
            <div
              role="option"
              aria-selected={index === selectedIndex}
              data-testid={`search-result-${entityMoniker}`}
              className={`flex cursor-pointer items-center gap-2 px-3 py-1.5 text-sm
                ${index === selectedIndex ? "bg-accent text-accent-foreground" : "hover:bg-accent/50"}`}
              onClick={() => {
                if (inspectEntity) {
                  onClose();
                  inspectEntity(entityMoniker);
                }
              }}
              onMouseEnter={() => onHoverIndex(index)}
            >
              {Icon
                ? <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                : <span className="shrink-0 text-xs text-muted-foreground">{typeLabel}</span>
              }
              <span className="min-w-0 truncate">{result.display_name}</span>
            </div>
          </FocusScope>
        );
      })}
    </>
  );
}
