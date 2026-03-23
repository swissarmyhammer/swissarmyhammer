import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  CommandScopeProvider,
  useExecuteCommand,
  resolveCommand,
  dispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import { useUIState } from "@/lib/ui-state-context";
import { useAppMode } from "@/lib/app-mode-context";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import { CommandPalette } from "@/components/command-palette";
import { pathStem } from "@/components/board-selector";
import { syncMenuToNative } from "@/lib/menu-sync";
import { dispatchContextMenuCommand } from "@/lib/context-menu";
import { useInspectDismiss } from "@/lib/inspect-context";

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

export function AppShell({
  children,
  openBoards,
  onSwitchBoard,
}: AppShellProps) {
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteMode, setPaletteMode] = useState<"command" | "search">(
    "command",
  );
  const paletteOpenRef = useRef(false);
  paletteOpenRef.current = paletteOpen;
  const { keymap_mode: keymapModeRaw } = useUIState();
  // Normalize to a valid KeymapMode, defaulting to "cua" for unknown values
  const keymapMode: KeymapMode =
    keymapModeRaw === "vim" || keymapModeRaw === "emacs"
      ? keymapModeRaw
      : "cua";
  const { setMode } = useAppMode();
  const dismissInspector = useInspectDismiss();

  /** Global commands available throughout the app. */
  const globalCommands: CommandDef[] = useMemo(
    () => [
      {
        id: "app.command",
        name: "Command Palette",
        keys: { vim: ":", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
        execute: () => {
          setPaletteMode("command");
          setPaletteOpen(true);
          setMode("command");
        },
      },
      {
        id: "app.palette",
        name: "Command Palette",
        keys: { vim: "Mod+Shift+P", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
        execute: () => {
          setPaletteMode("command");
          setPaletteOpen(true);
          setMode("command");
        },
      },
      {
        id: "app.undo",
        name: "Undo",
        keys: { vim: "u", cua: "Mod+Z", emacs: "C-/" },
        // No execute -- dispatches to Rust via dispatch_command
      },
      {
        id: "app.redo",
        name: "Redo",
        keys: { vim: "Mod+R", cua: "Mod+Shift+Z" },
        // No execute -- dispatches to Rust via dispatch_command
      },
      {
        id: "app.dismiss",
        name: "Dismiss",
        keys: { vim: "Escape", cua: "Escape", emacs: "Escape" },
        execute: () => {
          // Layered dismiss: palette first, then inspector stack, then clear focus.
          // Read paletteOpen from the ref to avoid adding it to useMemo deps.
          if (paletteOpenRef.current) {
            setPaletteOpen(false);
            setMode("normal");
            return;
          }
          if (dismissInspector()) {
            return;
          }
          // Nothing to dismiss — just ensure we're in normal mode
          setMode("normal");
        },
      },
      {
        id: "app.search",
        name: "Find",
        keys: { vim: "/", cua: "Mod+F", emacs: "Mod+F" },
        menuPlacement: { menu: "edit", group: 0, order: 0 },
        execute: () => {
          setPaletteMode("search");
          setPaletteOpen(true);
          setMode("command");
        },
      },
      {
        id: "app.help",
        name: "Help",
        keys: { vim: "F1", cua: "F1" },
        execute: () => {},
      },
      {
        id: "app.quit",
        name: "Quit",
        keys: { cua: "Mod+Q", vim: "Mod+Q", emacs: "Mod+Q" },
        menuPlacement: { menu: "app", group: 2, order: 0 },
        execute: async () => {
          await invoke("quit_app");
        },
      },
      {
        id: "settings.keymap.vim",
        name: "Keymap Vim",
        menuPlacement: {
          menu: "settings",
          group: 0,
          order: 1,
          radioGroup: "keymap",
          checked: keymapMode === "vim",
        },
        // No execute — dispatches to Rust via dispatch_command which mutates UIState
      },
      {
        id: "settings.keymap.cua",
        name: "Keymap CUA",
        menuPlacement: {
          menu: "settings",
          group: 0,
          order: 0,
          radioGroup: "keymap",
          checked: keymapMode === "cua",
        },
        // No execute — dispatches to Rust via dispatch_command which mutates UIState
      },
      {
        id: "settings.keymap.emacs",
        name: "Keymap Emacs",
        menuPlacement: {
          menu: "settings",
          group: 0,
          order: 2,
          radioGroup: "keymap",
          checked: keymapMode === "emacs",
        },
        // No execute — dispatches to Rust via dispatch_command which mutates UIState
      },
      {
        id: "app.resetWindows",
        name: "Reset Windows",
        menuPlacement: { menu: "settings", group: 1, order: 0 },
        execute: async () => {
          await invoke("reset_windows");
        },
      },
      {
        id: "file.newBoard",
        name: "New Board",
        keys: { cua: "Mod+N", vim: "Mod+N" },
        menuPlacement: { menu: "file", group: 0, order: 0 },
        execute: async () => {
          await invoke("new_board_dialog");
        },
      },
      {
        id: "file.openBoard",
        name: "Open Board",
        keys: { cua: "Mod+O", vim: "Mod+O" },
        menuPlacement: { menu: "file", group: 0, order: 1 },
        execute: async () => {
          await invoke("open_board_dialog");
        },
      },
      {
        id: "file.closeBoard",
        name: "Close Board",
        keys: { cua: "Mod+W", vim: "Mod+W" },
        menuPlacement: { menu: "file", group: 0, order: 2 },
        // No execute — dispatches to Rust via dispatch_command which calls file.closeBoard
      },
      {
        id: "window.new",
        name: "New Window",
        keys: { cua: "Mod+Shift+N", vim: "Mod+Shift+N", emacs: "Mod+Shift+N" },
        menuPlacement: { menu: "window", group: 0, order: 0 },
        execute: async () => {
          await invoke("create_window");
        },
      },
      {
        id: "app.about",
        name: "About",
        menuPlacement: { menu: "app", group: 0, order: 0 },
        execute: () => {
          // Tauri about dialog -- placeholder for now
        },
      },
      // Dynamic board switch commands — one per open board.
      // Uses index as suffix to avoid filesystem paths in command IDs.
      ...(openBoards ?? []).map((b, i) => ({
        id: `board.switch.${i}`,
        name: `Switch to Board ${b.name || pathStem(b.path)}`,
        execute: () => onSwitchBoard?.(b.path),
      })),
    ],
    [setMode, keymapMode, dismissInspector, openBoards, onSwitchBoard],
  );

  /** Close the command palette and return to normal mode. */
  const closePalette = useCallback(() => {
    setPaletteOpen(false);
    setMode("normal");
  }, [setMode]);

  // Sync native menu bar whenever global commands or keymap mode change.
  useEffect(() => {
    syncMenuToNative(globalCommands, keymapMode);
  }, [globalCommands, keymapMode]);

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
