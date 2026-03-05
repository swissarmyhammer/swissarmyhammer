import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CommandScopeProvider, useExecuteCommand, type CommandDef } from "@/lib/command-scope";
import { useKeymap, type KeymapMode } from "@/lib/keymap-context";
import { useAppMode } from "@/lib/app-mode-context";
import { useUndoStack } from "@/lib/undo-context";
import { createKeyHandler } from "@/lib/keybindings";
import { CommandPalette } from "@/components/command-palette";
import { syncMenuToNative } from "@/lib/menu-sync";

/**
 * Internal component that attaches a global keydown listener.
 *
 * Must be rendered inside a CommandScopeProvider so that useExecuteCommand
 * resolves commands from the scope AppShell just created.
 */
function KeybindingHandler({ mode }: { mode: KeymapMode }) {
  const executeCommand = useExecuteCommand();

  useEffect(() => {
    const handler = createKeyHandler(mode, executeCommand);
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

  return null;
}

/**
 * Top-level shell that wires global commands, keybindings, and the command
 * palette around the application content.
 *
 * Must be rendered inside KeymapProvider, AppModeProvider, and
 * UndoStackProvider (it reads from all three). It provides a
 * CommandScopeProvider to its children.
 *
 * Provider nesting order:
 *   KeymapProvider > AppModeProvider > UndoStackProvider > AppShell > children
 */
export function AppShell({ children }: { children: ReactNode }) {
  const [paletteOpen, setPaletteOpen] = useState(false);
  const { mode: keymapMode, setMode: setKeymapMode } = useKeymap();
  const { setMode } = useAppMode();
  const { undo, redo, canUndo, canRedo } = useUndoStack();

  /** Global commands available throughout the app. */
  const globalCommands: CommandDef[] = useMemo(
    () => [
      {
        id: "app.command",
        name: "Command Palette",
        keys: { vim: ":", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
        execute: () => {
          setPaletteOpen(true);
          setMode("command");
        },
      },
      {
        id: "app.palette",
        name: "Command Palette",
        keys: { vim: "Mod+Shift+P", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
        execute: () => {
          setPaletteOpen(true);
          setMode("command");
        },
      },
      {
        id: "app.undo",
        name: "Undo",
        keys: { vim: "u", cua: "Mod+Z", emacs: "C-/" },
        execute: () => undo(),
        available: canUndo,
      },
      {
        id: "app.redo",
        name: "Redo",
        keys: { vim: "Mod+R", cua: "Mod+Shift+Z" },
        execute: () => redo(),
        available: canRedo,
      },
      {
        id: "app.dismiss",
        name: "Dismiss",
        keys: { vim: "Escape", cua: "Escape", emacs: "Escape" },
        execute: () => {
          setPaletteOpen(false);
          setMode("normal");
        },
      },
      // Placeholders for future implementation
      {
        id: "app.search",
        name: "Search",
        keys: { vim: "/", cua: "Mod+F" },
        execute: () => {},
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
        name: "Switch to Vim Keymap",
        menuPlacement: { menu: "settings", group: 0, order: 1, radioGroup: "keymap", checked: keymapMode === "vim" },
        execute: async () => {
          await invoke("set_keymap_mode", { mode: "vim" });
          setKeymapMode("vim");
        },
      },
      {
        id: "settings.keymap.cua",
        name: "Switch to CUA Keymap",
        menuPlacement: { menu: "settings", group: 0, order: 0, radioGroup: "keymap", checked: keymapMode === "cua" },
        execute: async () => {
          await invoke("set_keymap_mode", { mode: "cua" });
          setKeymapMode("cua");
        },
      },
      {
        id: "settings.keymap.emacs",
        name: "Switch to Emacs Keymap",
        menuPlacement: { menu: "settings", group: 0, order: 2, radioGroup: "keymap", checked: keymapMode === "emacs" },
        execute: async () => {
          await invoke("set_keymap_mode", { mode: "emacs" });
          setKeymapMode("emacs");
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
        id: "app.about",
        name: "About",
        menuPlacement: { menu: "app", group: 0, order: 0 },
        execute: () => {
          // Tauri about dialog -- placeholder for now
        },
      },
    ],
    [canUndo, canRedo, undo, redo, setMode, setKeymapMode, keymapMode],
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
      <CommandPalette open={paletteOpen} onClose={closePalette} />
    </CommandScopeProvider>
  );
}
