import { useCallback, useEffect, useMemo, useRef, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CommandScopeProvider,
  useExecuteCommand,
  resolveCommand,
  dispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import { useFocusedScope, useEntityFocus } from "@/lib/entity-focus-context";
import { useUIState } from "@/lib/ui-state-context";
import { useAppMode } from "@/lib/app-mode-context";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import { CommandPalette } from "@/components/command-palette";
import { dispatchContextMenuCommand } from "@/lib/context-menu";

/**
 * Internal component that attaches a global keydown listener.
 *
 * Must be rendered inside a CommandScopeProvider so that useExecuteCommand
 * resolves commands from the scope AppShell just created.
 *
 * When a FocusScope is focused, commands resolve from the focused scope
 * first, falling back to the root scope (current context) if not found.
 */
function KeybindingHandler({ mode }: { mode: KeymapMode }) {
  const rootExecuteCommand = useExecuteCommand();
  const focusedScope = useFocusedScope();

  // Store focused scope in a ref so the key handler callback always sees
  // the latest value without re-creating the handler on every focus change.
  const focusedScopeRef = useRef(focusedScope);
  focusedScopeRef.current = focusedScope;
  const rootExecuteRef = useRef(rootExecuteCommand);
  rootExecuteRef.current = rootExecuteCommand;

  /** Execute a command, preferring the focused scope when available. */
  const executeCommand = useCallback(async (id: string): Promise<boolean> => {
    // When a CM6 editor has focus, let it handle its own undo/redo
    if (
      (id === "app.undo" || id === "app.redo") &&
      document.activeElement?.closest(".cm-editor")
    ) {
      return false;
    }

    const scope = focusedScopeRef.current;
    if (scope) {
      const cmd = resolveCommand(scope, id);
      if (cmd) {
        await dispatchCommand(cmd);
        return true;
      }
    }
    // Fall back to root scope
    return rootExecuteRef.current(id);
  }, []);

  useEffect(() => {
    // Pass scope bindings so command `keys` from the focused scope (inspector,
    // grid, board nav) are resolved through the same single key handler.
    const handler = createKeyHandler(mode, executeCommand, () =>
      extractScopeBindings(focusedScopeRef.current, mode),
    );
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [mode, executeCommand]);

  // Listen for menu-command events from the native menu and route them
  // through the command scope so they behave identically to keybindings
  // and palette invocations.
  useEffect(() => {
    const unlisten = listen<string>("menu-command", async (event) => {
      const commandId = event.payload;
      const executed = await executeCommand(commandId);
      if (!executed) {
        console.warn(`Menu command not found: ${commandId}`);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [executeCommand]);

  // Listen for context-menu-command events from the native context menu
  // and dispatch from the pending handlers map (populated by useContextMenu).
  useEffect(() => {
    const unlisten = listen<string>("context-menu-command", async (event) => {
      const commandId = event.payload;
      const dispatched = await dispatchContextMenuCommand(commandId);
      if (!dispatched) {
        console.warn(`Context menu command not found: ${commandId}`);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return null;
}

/**
 * Top-level shell that wires global commands, keybindings, and the command
 * palette around the application content.
 *
 * Must be rendered inside UIStateProvider, AppModeProvider, and
 * UndoStackProvider (it reads from all three). It provides a
 * CommandScopeProvider to its children.
 *
 * Provider nesting order:
 *   UIStateProvider > AppModeProvider > UndoStackProvider > AppShell > children
 */
interface AppShellProps {
  children: ReactNode;
  /** Currently open boards — used to generate board.switch commands. */
  openBoards?: Array<{ path: string; name: string; is_active: boolean }>;
  /** Handler to switch the current window to a different board. */
  onSwitchBoard?: (path: string) => void;
}

export function AppShell({ children, onSwitchBoard }: AppShellProps) {
  const uiState = useUIState();
  const keymapModeRaw = uiState.keymap_mode;
  const windowLabel = getCurrentWindow().label;
  const winState = uiState.windows?.[windowLabel];
  const paletteOpen = winState?.palette_open ?? false;
  const paletteMode = winState?.palette_mode ?? "command";
  // Normalize to a valid KeymapMode, defaulting to "cua" for unknown values
  const keymapMode: KeymapMode =
    keymapModeRaw === "vim" || keymapModeRaw === "emacs"
      ? keymapModeRaw
      : "cua";
  const { setMode } = useAppMode();
  const { broadcastNavCommand } = useEntityFocus();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;

  // Sync app mode from palette state driven by backend UIState.
  const prevPaletteOpenRef = useRef(paletteOpen);
  useEffect(() => {
    if (paletteOpen && !prevPaletteOpenRef.current) {
      setMode("command");
    } else if (!paletteOpen && prevPaletteOpenRef.current) {
      setMode("normal");
    }
    prevPaletteOpenRef.current = paletteOpen;
  }, [paletteOpen, setMode]);

  /** Global commands available throughout the app.
   *
   * Most commands have no `execute` callback and dispatch to the Rust backend.
   * Only navigation commands retain frontend `execute` handlers because they
   * are purely UI focus movement that broadcasts through EntityFocusContext.
   */
  const globalCommands: CommandDef[] = useMemo(
    () => [
      // --- Commands dispatched to backend (no execute callback) ---
      {
        id: "app.command",
        name: "Command Palette",
        keys: { vim: ":", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
      },
      {
        id: "app.palette",
        name: "Command Palette",
        keys: { vim: "Mod+Shift+P", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
      },
      {
        id: "app.undo",
        name: "Undo",
        keys: { vim: "u", cua: "Mod+Z", emacs: "Ctrl+/" },
      },
      {
        id: "app.redo",
        name: "Redo",
        keys: { vim: "Mod+R", cua: "Mod+Shift+Z" },
      },
      {
        id: "app.dismiss",
        name: "Dismiss",
        keys: { vim: "Escape", cua: "Escape", emacs: "Escape" },
      },
      {
        id: "app.search",
        name: "Find",
        keys: { vim: "/", cua: "Mod+F", emacs: "Mod+F" },
      },
      {
        id: "app.help",
        name: "Help",
        keys: { vim: "F1", cua: "F1" },
      },
      {
        id: "app.quit",
        name: "Quit",
        keys: { cua: "Mod+Q", vim: "Mod+Q", emacs: "Mod+Q" },
      },
      {
        id: "settings.keymap.vim",
        name: "Keymap Vim",
      },
      {
        id: "settings.keymap.cua",
        name: "Keymap CUA",
      },
      {
        id: "settings.keymap.emacs",
        name: "Keymap Emacs",
      },
      {
        id: "app.resetWindows",
        name: "Reset Windows",
      },
      {
        id: "file.newBoard",
        name: "New Board",
        keys: { cua: "Mod+N", vim: "Mod+N" },
      },
      {
        id: "file.openBoard",
        name: "Open Board",
        keys: { cua: "Mod+O", vim: "Mod+O" },
      },
      {
        id: "file.closeBoard",
        name: "Close Board",
        keys: { cua: "Mod+w", vim: "Mod+w" },
      },
      {
        id: "window.new",
        name: "New Window",
        keys: { cua: "Mod+Shift+N", vim: "Mod+Shift+N", emacs: "Mod+Shift+N" },
      },
      {
        id: "app.about",
        name: "About",
      },
      // --- Universal navigation commands ---
      // These broadcast to all registered claimWhen predicates.
      // Each FocusScope with a matching predicate can pull focus.
      {
        id: "nav.up",
        name: "Navigate Up",
        keys: { vim: "k", cua: "ArrowUp", emacs: "Ctrl+p" },
        execute: () => {
          broadcastRef.current("nav.up");
        },
      },
      {
        id: "nav.down",
        name: "Navigate Down",
        keys: { vim: "j", cua: "ArrowDown", emacs: "Ctrl+n" },
        execute: () => {
          broadcastRef.current("nav.down");
        },
      },
      {
        id: "nav.left",
        name: "Navigate Left",
        keys: { vim: "h", cua: "ArrowLeft", emacs: "Ctrl+b" },
        execute: () => {
          broadcastRef.current("nav.left");
        },
      },
      {
        id: "nav.right",
        name: "Navigate Right",
        keys: { vim: "l", cua: "ArrowRight", emacs: "Ctrl+f" },
        execute: () => {
          broadcastRef.current("nav.right");
        },
      },
      {
        id: "nav.first",
        name: "Navigate to First",
        keys: { cua: "Home", emacs: "Alt+<" },
        execute: () => {
          broadcastRef.current("nav.first");
        },
      },
      {
        id: "nav.last",
        name: "Navigate to Last",
        keys: { vim: "Shift+G", cua: "End", emacs: "Alt+>" },
        execute: () => {
          broadcastRef.current("nav.last");
        },
      },
    ],
    [],
  );

  /** Close the command palette (dispatch to backend) and return to normal mode. */
  const closePalette = useCallback(() => {
    // Dispatch app.dismiss to backend to close palette
    dispatchCommand({ id: "app.dismiss", name: "Dismiss" });
  }, []);

  return (
    <CommandScopeProvider commands={globalCommands}>
      <KeybindingHandler mode={keymapMode} />
      {children}
      <CommandPalette
        open={paletteOpen}
        onClose={closePalette}
        mode={paletteMode}
        onSwitchBoard={onSwitchBoard}
      />
    </CommandScopeProvider>
  );
}
