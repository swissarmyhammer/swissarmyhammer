import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CommandScopeContext,
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
  type DispatchOptions,
} from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { useUIState } from "@/lib/ui-state-context";
import { useAppMode } from "@/lib/app-mode-context";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import { CommandPalette } from "@/components/command-palette";
import { triggerStartRename } from "@/components/perspective-tab-bar";

/** Route native menu commands through the command scope dispatch. */
function useMenuCommandListener(
  executeCommand: (id: string) => Promise<boolean>,
) {
  useEffect(() => {
    const unlisten = listen<string>("menu-command", async (event) => {
      const executed = await executeCommand(event.payload);
      if (!executed) console.warn(`Menu command not found: ${event.payload}`);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [executeCommand]);
}

/** Route native context-menu commands through useDispatchCommand. */
type AdHocDispatch = (cmd: string, opts?: DispatchOptions) => Promise<unknown>;
function useContextMenuCommandListener(
  dispatchRef: React.RefObject<AdHocDispatch>,
) {
  useEffect(() => {
    const unlisten = listen<{
      cmd: string;
      target?: string;
      scope_chain?: string[];
    }>("context-menu-command", async (event) => {
      const { cmd, target, scope_chain } = event.payload;
      if (!cmd) return;
      await dispatchRef.current(cmd, { target, scopeChain: scope_chain });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
    // dispatchRef is stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

/**
 * Internal component that attaches a global keydown listener.
 *
 * Must be rendered inside a CommandScopeProvider so that useDispatchCommand
 * resolves commands from the scope AppShell just created.
 *
 * When a FocusScope is focused, commands resolve from the focused scope
 * first, falling back to the tree (ambient) scope when nothing is focused.
 * This matches the `focusedScope ?? treeScope` pattern in `useDispatchCommand`
 * and keeps global bindings like `h`/`j`/`k`/`l` (`nav.*`) alive even when
 * spatial focus is momentarily null — without it, the "something is always
 * focused" invariant can never recover via a nav key because the key never
 * resolves to a command.
 */
function KeybindingHandler({ mode }: { mode: KeymapMode }) {
  const dispatch: AdHocDispatch = useDispatchCommand();
  const focusedScope = useFocusedScope();
  const treeScope = useContext(CommandScopeContext);

  const dispatchRef = useRef<AdHocDispatch>(dispatch);
  dispatchRef.current = dispatch;
  const focusedScopeRef = useRef(focusedScope);
  focusedScopeRef.current = focusedScope;
  const treeScopeRef = useRef(treeScope);
  treeScopeRef.current = treeScope;

  const executeCommand = useCallback(async (id: string): Promise<boolean> => {
    if (
      (id === "app.undo" || id === "app.redo") &&
      document.activeElement?.closest(".cm-editor")
    ) {
      return false;
    }
    await dispatchRef.current(id);
    return true;
  }, []);

  useEffect(() => {
    const handler = createKeyHandler(mode, executeCommand, () =>
      extractScopeBindings(
        focusedScopeRef.current ?? treeScopeRef.current,
        mode,
      ),
    );
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [mode, executeCommand]);

  useMenuCommandListener(executeCommand);
  useContextMenuCommandListener(dispatchRef);

  return null;
}

/**
 * Static global commands with no `execute` handler — dispatched to the Rust
 * backend on invocation.
 *
 * Kept at module scope so the `AppShell` component body stays small and the
 * array identity is stable across renders.
 */
const STATIC_GLOBAL_COMMANDS: CommandDef[] = [
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
  { id: "app.help", name: "Help", keys: { vim: "F1", cua: "F1" } },
  {
    id: "app.quit",
    name: "Quit",
    keys: { cua: "Mod+Q", vim: "Mod+Q", emacs: "Mod+Q" },
  },
  { id: "settings.keymap.vim", name: "Keymap Vim" },
  { id: "settings.keymap.cua", name: "Keymap CUA" },
  { id: "settings.keymap.emacs", name: "Keymap Emacs" },
  { id: "app.resetWindows", name: "Reset Windows" },
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
  { id: "app.about", name: "About" },
];

/**
 * Key binding + display metadata for each universal navigation command.
 *
 * These entries carry no `execute` handler on purpose: with no local
 * handler, the dispatcher routes each nav keypress through
 * `dispatch_command` to the Rust `NavigateCmd` impl, which drives the
 * per-window `SpatialState::navigate` and emits `focus-changed`. The
 * React focus store subscribes to that event and re-renders FocusScope
 * decorations — the same round-trip `ui.inspect` uses. A local `execute`
 * would short-circuit the dispatcher and break the round-trip.
 *
 * This list is kept in sync with the `nav.*` entries in
 * `swissarmyhammer-commands/builtin/commands/nav.yaml`. The YAML is the
 * registry source of truth for the backend; this array supplies the
 * React-facing `CommandDef` shape so the palette and keybinding layers
 * can see the same commands.
 */
const NAV_COMMAND_DEFS: CommandDef[] = [
  {
    id: "nav.up",
    name: "Navigate Up",
    keys: { vim: "k", cua: "ArrowUp", emacs: "Ctrl+p" },
  },
  {
    id: "nav.down",
    name: "Navigate Down",
    keys: { vim: "j", cua: "ArrowDown", emacs: "Ctrl+n" },
  },
  {
    id: "nav.left",
    name: "Navigate Left",
    keys: { vim: "h", cua: "ArrowLeft", emacs: "Ctrl+b" },
  },
  {
    id: "nav.right",
    name: "Navigate Right",
    keys: { vim: "l", cua: "ArrowRight", emacs: "Ctrl+f" },
  },
  {
    id: "nav.first",
    name: "Navigate to First",
    keys: { cua: "Home", emacs: "Alt+<" },
  },
  {
    id: "nav.last",
    name: "Navigate to Last",
    keys: { vim: "Shift+G", cua: "End", emacs: "Alt+>" },
  },
];

/**
 * Sync app mode to the palette-open flag in backend UIState.
 *
 * When the palette opens, switch to "command" mode; when it closes, return
 * to "normal". Encapsulated as a hook so `AppShell` stays compact.
 */
function usePaletteModeSync(paletteOpen: boolean): void {
  const { setMode } = useAppMode();
  const prevPaletteOpenRef = useRef(paletteOpen);
  useEffect(() => {
    if (paletteOpen && !prevPaletteOpenRef.current) {
      setMode("command");
    } else if (!paletteOpen && prevPaletteOpenRef.current) {
      setMode("normal");
    }
    prevPaletteOpenRef.current = paletteOpen;
  }, [paletteOpen, setMode]);
}

/**
 * Collect per-window UI state (keymap, palette, window label) that AppShell
 * reads from the backend UIState context.
 *
 * Extracted so the component body stays under the 50-line function budget.
 */
function useAppShellUIState() {
  const uiState = useUIState();
  const windowLabel = getCurrentWindow().label;
  const winState = uiState.windows?.[windowLabel];
  const paletteOpen = winState?.palette_open ?? false;
  const paletteMode = winState?.palette_mode ?? "command";
  // Normalize to a valid KeymapMode, defaulting to "cua" for unknown values
  const keymapModeRaw = uiState.keymap_mode;
  const keymapMode: KeymapMode =
    keymapModeRaw === "vim" || keymapModeRaw === "emacs"
      ? keymapModeRaw
      : "cua";
  return { paletteOpen, paletteMode, keymapMode };
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

/** The single `ui.perspective.startRename` CommandDef wired locally. */
const PERSPECTIVE_RENAME_COMMAND: CommandDef = {
  id: "ui.perspective.startRename",
  name: "Rename Perspective",
  execute: () => {
    triggerStartRename();
  },
};

/** Top-level shell that wires global commands, keybindings, and the command palette. */
export function AppShell({ children, onSwitchBoard }: AppShellProps) {
  const { paletteOpen, paletteMode, keymapMode } = useAppShellUIState();
  const dismiss = useDispatchCommand("app.dismiss");

  usePaletteModeSync(paletteOpen);

  // All module-scope arrays are stable; the memo has no dependencies.
  const globalCommands: CommandDef[] = useMemo(
    () => [
      ...STATIC_GLOBAL_COMMANDS,
      ...NAV_COMMAND_DEFS,
      PERSPECTIVE_RENAME_COMMAND,
    ],
    [],
  );

  /** Close the command palette (dispatch to backend) and return to normal mode. */
  const closePalette = useCallback(() => {
    dismiss();
  }, [dismiss]);

  return (
    <FocusLayer name="window">
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
    </FocusLayer>
  );
}
