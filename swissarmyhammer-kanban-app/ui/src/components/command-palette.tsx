import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap, EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { getCM, Vim } from "@replit/codemirror-vim";
import { useAvailableCommands, collectAvailableCommands, type CommandAtDepth } from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import { useKeymap } from "@/lib/keymap-context";
import { minimalTheme, keymapExtension } from "@/lib/cm-keymap";
import { fuzzyMatch } from "@/lib/fuzzy-filter";

interface CommandPaletteProps {
  /** Whether the palette is currently visible. */
  open: boolean;
  /** Called to dismiss the palette. */
  onClose: () => void;
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
 */
export function CommandPalette({ open, onClose }: CommandPaletteProps) {
  const [filter, setFilter] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
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

  // Reset state when palette opens
  useEffect(() => {
    if (open) {
      setFilter("");
      setSelectedIndex(0);
    }
  }, [open]);

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

  // Filter and sort commands by fuzzy match score
  const filtered = useMemo(() => {
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
  }, [filter, allCommands]);

  // Clamp selection when filtered list changes
  useEffect(() => {
    setSelectedIndex((prev) => Math.min(prev, Math.max(0, filtered.length - 1)));
  }, [filtered]);

  // Execute the selected command
  const executeSelected = useCallback(() => {
    const entry = filtered[selectedIndex];
    if (entry) {
      onClose();
      entry.command.execute?.();
    }
  }, [filtered, selectedIndex, onClose]);

  // Ref so CM6 extensions always see the latest closures
  const executeSelectedRef = useRef(executeSelected);
  executeSelectedRef.current = executeSelected;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  /** Move selection up or down, clamped to list bounds. */
  const moveSelection = useCallback(
    (delta: number) => {
      setSelectedIndex((prev) =>
        Math.max(0, Math.min(filtered.length - 1, prev + delta))
      );
    },
    [filtered.length]
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
      minimalTheme,
      keymapCompartment.current.of(keymapExtension(mode)),
      // Prevent line wrapping — single-line input
      EditorView.theme({
        ".cm-content": { whiteSpace: "nowrap" },
        ".cm-scroller": { overflow: "hidden" },
      }),
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
            basicSetup={false}
            placeholder="Type a command..."
            className="text-sm"
          />
        </div>

        {/* Command list */}
        <div
          ref={listRef}
          data-testid="command-palette-list"
          className="max-h-64 overflow-y-auto py-1"
          role="listbox"
        >
          {filtered.length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">
              No matching commands
            </div>
          ) : (
            filtered.map((entry, index) => {
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
                    entry.command.execute?.();
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
    document.body
  );
}
