import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { CommandScopeProvider, useExecuteCommand, type CommandDef } from "@/lib/command-scope";
import { useKeymap, type KeymapMode } from "@/lib/keymap-context";
import { useAppMode } from "@/lib/app-mode-context";
import { useUndoStack } from "@/lib/undo-context";
import { createKeyHandler } from "@/lib/keybindings";
import { CommandPalette } from "@/components/command-palette";

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
  const { mode: keymapMode } = useKeymap();
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
    ],
    [canUndo, canRedo, undo, redo, setMode],
  );

  /** Close the command palette and return to normal mode. */
  const closePalette = useCallback(() => {
    setPaletteOpen(false);
    setMode("normal");
  }, [setMode]);

  return (
    <CommandScopeProvider commands={globalCommands}>
      <KeybindingHandler mode={keymapMode} />
      {children}
      <CommandPalette open={paletteOpen} onClose={closePalette} />
    </CommandScopeProvider>
  );
}
