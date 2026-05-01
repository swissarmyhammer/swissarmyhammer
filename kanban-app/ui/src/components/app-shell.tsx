import { useCallback, useEffect, useMemo, useRef, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
  type DispatchOptions,
} from "@/lib/command-scope";
import { useFocusActions, useFocusedScope } from "@/lib/entity-focus-context";
import {
  useSpatialFocusActions,
  type SpatialFocusActions,
} from "@/lib/spatial-focus-context";
import { useUIState } from "@/lib/ui-state-context";
import { useAppMode } from "@/lib/app-mode-context";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import { reportDispatchError } from "@/lib/dispatch-error";
import { CommandPalette } from "@/components/command-palette";
import { FocusLayer } from "@/components/focus-layer";
import { useEnclosingLayerFq } from "@/components/layer-fq-context";
import {
  asSegment,
  type Direction,
  type FullyQualifiedMoniker,
} from "@/types/spatial";
import { triggerStartRename } from "@/components/perspective-tab-bar";

/**
 * Identity-stable `SegmentMoniker` for the command-palette overlay layer.
 *
 * Pulled to module scope so re-renders never mint a fresh value — the
 * `<FocusLayer>` push effect depends on `name`, and a fresh-identity literal
 * in JSX would force a tear-down / re-push cycle on every parent render.
 */
const PALETTE_LAYER_NAME = asSegment("palette");

/**
 * Internal component that attaches a global keydown listener.
 *
 * Must be rendered inside a CommandScopeProvider so that useDispatchCommand
 * resolves commands from the scope AppShell just created.
 *
 * When a FocusScope is focused, commands resolve from the focused scope
 * first, falling back to the root scope (current context) if not found.
 */
function KeybindingHandler({ mode }: { mode: KeymapMode }) {
  const dispatch = useDispatchCommand();
  const focusedScope = useFocusedScope();

  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;
  const focusedScopeRef = useRef(focusedScope);
  focusedScopeRef.current = focusedScope;

  /** Execute a command via useDispatchCommand (focused scope preferred). */
  const executeCommand = useCallback(async (id: string): Promise<boolean> => {
    // When a CM6 editor has focus, let it handle its own undo/redo
    if (
      (id === "app.undo" || id === "app.redo") &&
      document.activeElement?.closest(".cm-editor")
    ) {
      return false;
    }

    // Generic dispatch entry point: keybindings have no per-command UI
    // around them, so a backend failure must be surfaced as a toast or
    // it vanishes into an unhandled promise rejection. Sites that own
    // their own contextual error UI (e.g. `useAddTaskHandler`) already
    // catch the rejection and toast their own message — they call
    // dispatch directly and never go through this generic path.
    try {
      await dispatchRef.current(id);
    } catch (e) {
      reportDispatchError(id, e);
    }
    return true;
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

  // Listen for context-menu-command events from native context menus.
  // These carry the full ContextMenuItem payload (cmd, target, scope_chain)
  // from the right-click point. Dispatched through useDispatchCommand so
  // they get busy tracking, client-side resolution, and the same dispatch
  // path as keybindings/palette/drag.
  useEffect(() => {
    const unlisten = listen<{
      cmd: string;
      target?: string;
      scope_chain?: string[];
    }>("context-menu-command", async (event) => {
      const { cmd, target, scope_chain } = event.payload;
      if (!cmd) return;
      try {
        await dispatchRef.current(cmd, {
          target,
          scopeChain: scope_chain,
        });
      } catch (e) {
        // Same rationale as the keybinding handler: a context-menu
        // dispatch has no per-command UI around it, so a failure that
        // isn't surfaced as a toast vanishes silently.
        reportDispatchError(cmd, e);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

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
 * The `direction` field is the wire-shape literal that `spatial_navigate`
 * accepts (matches `Direction` in `types/spatial.ts`). Each command's
 * `execute` closure threads the currently-focused [`FullyQualifiedMoniker`]
 * (read via `actions.focusedFq()`) plus this direction string into
 * `spatial_navigate` via the spatial-actions ref.
 *
 * Kept as a data table so `buildNavCommands` can produce the CommandDef[] in
 * a single pass without repetitive object literals.
 */
const NAV_COMMAND_SPEC: ReadonlyArray<{
  id: string;
  name: string;
  keys: CommandDef["keys"];
  direction: Direction;
}> = [
  {
    id: "nav.up",
    name: "Navigate Up",
    keys: { vim: "k", cua: "ArrowUp", emacs: "Ctrl+p" },
    direction: "up",
  },
  {
    id: "nav.down",
    name: "Navigate Down",
    keys: { vim: "j", cua: "ArrowDown", emacs: "Ctrl+n" },
    direction: "down",
  },
  {
    id: "nav.left",
    name: "Navigate Left",
    keys: { vim: "h", cua: "ArrowLeft", emacs: "Ctrl+b" },
    direction: "left",
  },
  {
    id: "nav.right",
    name: "Navigate Right",
    keys: { vim: "l", cua: "ArrowRight", emacs: "Ctrl+f" },
    direction: "right",
  },
  {
    id: "nav.first",
    name: "Navigate to First",
    keys: { cua: "Home", emacs: "Alt+<" },
    direction: "first",
  },
  {
    id: "nav.last",
    name: "Navigate to Last",
    keys: { vim: "Shift+G", cua: "End", emacs: "Alt+>" },
    direction: "last",
  },
];

/**
 * Build universal navigation CommandDefs that dispatch `spatial_navigate`.
 *
 * Each command reads the currently-focused [`FullyQualifiedMoniker`] from
 * the `SpatialFocusProvider` via `actions.focusedFq()`, then awaits the
 * matching Tauri command (`spatial_navigate`) with the per-spec `direction`
 * literal. When the registry has nothing focused (`focusedFq() === null`)
 * the command is a no-op — there is nothing to navigate from.
 *
 * Historically this was the entry point for the pull-based predicate
 * registry: each FocusScope with a matching `claimWhen` predicate would
 * claim focus when these commands fired. The predicate registry has been
 * replaced by the Rust spatial-nav kernel (beam search plus per-direction
 * `overrides`), so the React side now pushes the focused key + direction
 * into the kernel and the kernel emits `focus-changed` for the new
 * target, which the React tree picks up via `useFocusClaim`.
 */
function buildNavCommands(
  spatialActionsRef: React.MutableRefObject<SpatialFocusActions>,
): CommandDef[] {
  return NAV_COMMAND_SPEC.map((spec) => ({
    id: spec.id,
    name: spec.name,
    keys: spec.keys,
    execute: async () => {
      const actions = spatialActionsRef.current;
      const fq = actions.focusedFq();
      if (fq === null) return;
      await actions.navigate(fq, spec.direction);
    },
  }));
}

/**
 * Read-only ref bag for the drill-in / drill-out command closures.
 *
 * The closures are minted once into the `globalCommands` memo and live
 * for the AppShell's lifetime, but they need to read the *latest*
 * spatial focus actions, entity setFocus callback, and `app.dismiss`
 * dispatcher on every keystroke. Holding all three in refs lets the
 * memo dependency list stay empty without staling on context updates.
 */
interface DrillRefs {
  spatialActionsRef: React.MutableRefObject<SpatialFocusActions>;
  setFocusRef: React.MutableRefObject<
    (fq: FullyQualifiedMoniker | null) => void
  >;
  dismissRef: React.MutableRefObject<
    (opts?: DispatchOptions) => Promise<unknown>
  >;
}

/**
 * Build the `nav.drillIn` (Enter) and `nav.drillOut` (Escape) commands.
 *
 * Both read the currently-focused [`FullyQualifiedMoniker`] from the
 * `SpatialFocusProvider` via `actions.focusedFq()`, await the matching
 * Tauri command (`spatial_drill_in` / `spatial_drill_out`), and dispatch
 * `setFocus(result)` against the entity focus store on every result.
 * Under the no-silent-dropout contract the kernel always returns a
 * [`FullyQualifiedMoniker`]; the caller compares the result to the focused
 * FQM to detect the "no descent / no drill happened" case:
 *
 * - `nav.drillIn` falls through implicitly — `setFocus(focusedMoniker)`
 *   is idempotent on the entity-focus store, so a leaf without an
 *   inline-edit affordance is a visible no-op. Leaves with an editor
 *   handle Enter via their own scope-level command (e.g.
 *   `field.edit`, card-name rename) which shadows this binding.
 * - `nav.drillOut` falls through to `app.dismiss` when the result
 *   equals the focused moniker, so the existing Escape chain (close
 *   the topmost modal layer) still fires at a layer root.
 *
 * Mirrors the React contract documented on `SpatialFocusActions.drillIn`
 * / `drillOut` — purely a registry query, no focus-state mutation; the
 * caller wires the resulting moniker into the entity focus store.
 */
function buildDrillCommands(refs: DrillRefs): CommandDef[] {
  return [
    {
      id: "nav.drillIn",
      name: "Drill In",
      keys: { vim: "Enter", cua: "Enter" },
      execute: async () => {
        const actions = refs.spatialActionsRef.current;
        const focusedFq = actions.focusedFq();
        if (focusedFq === null) return;
        const result = await actions.drillIn(focusedFq, focusedFq);
        // The kernel always returns an FQM. When `result === focusedFq`
        // the caller's setFocus call is idempotent (entity-focus store
        // detects identity-stable FQMs and emits no event), which
        // visually matches the legacy "null → no-op" behavior. When
        // `result !== focusedFq` setFocus moves focus to the new
        // target.
        refs.setFocusRef.current(result);
      },
    },
    {
      id: "nav.drillOut",
      name: "Drill Out",
      keys: { vim: "Escape", cua: "Escape" },
      execute: async () => {
        const actions = refs.spatialActionsRef.current;
        const focusedFq = actions.focusedFq();
        if (focusedFq === null) {
          // No spatial focus → nothing to drill out of; honour the
          // existing Escape chain (close topmost modal layer).
          await refs.dismissRef.current();
          return;
        }
        const result = await actions.drillOut(focusedFq, focusedFq);
        if (result === focusedFq) {
          // Kernel echoed the focused FQM — layer-root edge or
          // torn state. Fall through to `app.dismiss` to close the
          // topmost modal layer; the user-observable behavior is
          // identical to the legacy `null` fall-through.
          await refs.dismissRef.current();
        } else {
          refs.setFocusRef.current(result);
        }
      },
    },
  ];
}

/**
 * Build the dynamic global commands — drill commands first (so they
 * shadow the static `app.dismiss: Escape` binding when their
 * scope-level `keys` are merged into the global key handler), nav
 * commands next, plus the ui.entity.startRename command which exists in
 * the backend registry for palette discovery but runs locally via
 * `triggerStartRename`.
 *
 * Drill commands MUST come before `STATIC_GLOBAL_COMMANDS`-derived
 * entries in the iteration order seen by `extractScopeBindings`: that
 * walk uses "first key wins per scope", so to claim Escape away from
 * `app.dismiss` the drill command's `CommandDef` must reach the scope
 * map first. Putting them at the head of the dynamic batch — which
 * AppShell prepends to the static batch in the spread — orders them
 * correctly.
 */
function buildDynamicGlobalCommands(drillRefs: DrillRefs): CommandDef[] {
  return [
    ...buildDrillCommands(drillRefs),
    ...buildNavCommands(drillRefs.spatialActionsRef),
    {
      id: "ui.entity.startRename",
      name: "Rename Perspective",
      execute: () => {
        triggerStartRename();
      },
    },
  ];
}

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

export function AppShell({ children, onSwitchBoard }: AppShellProps) {
  const { paletteOpen, paletteMode, keymapMode } = useAppShellUIState();
  const { setFocus } = useFocusActions();
  const spatialActions = useSpatialFocusActions();
  const dismiss = useDispatchCommand("app.dismiss");

  // Drill + nav commands need read-on-demand access to spatial focus, entity
  // setFocus, and the `app.dismiss` dispatcher. Holding each in a ref keeps
  // the `globalCommands` memo dependency list empty while still letting the
  // closures see the latest context values at keystroke time. The actions
  // bag from `useSpatialFocusActions` is itself identity-stable (built once
  // per provider lifetime), so the ref is belt-and-braces — a future
  // refactor that turns it into a per-render value still survives.
  const spatialActionsRef = useRef(spatialActions);
  spatialActionsRef.current = spatialActions;
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;
  const dismissRef = useRef(dismiss);
  dismissRef.current = dismiss;

  // Window-root layer FQ — passed explicitly to the palette `<FocusLayer>`
  // because the command palette renders via `createPortal(document.body)`,
  // which severs the React ancestor chain a `<FocusLayer>` would otherwise
  // walk. Reading the FQ here, where `<FocusLayer name="window">` (mounted
  // in `App.tsx`) is still a direct ancestor, captures the right parent
  // regardless of how the palette portals out at render time.
  const windowLayerFq = useEnclosingLayerFq();

  usePaletteModeSync(paletteOpen);

  // Static commands come from module scope; dynamic ones close over the
  // spatial-actions / setFocus / dismiss refs. Both batches are stable, so
  // the memo has no dependencies.
  //
  // Dynamic commands precede static ones in the array so the drill
  // commands' `keys: { cua: "Escape" }` reaches the `CommandScope` map
  // before the static `app.dismiss: Escape`. `extractScopeBindings`
  // walks the map in insertion order with first-key-wins semantics, so
  // the `nav.drillOut` binding has to register first to claim Escape
  // away from `app.dismiss` while a scope is focused. The drill
  // execute closures fall through to `app.dismiss` themselves on a
  // null kernel result, so the user-facing Escape behavior at a layer
  // root is preserved.
  const globalCommands: CommandDef[] = useMemo(
    () => [
      ...buildDynamicGlobalCommands({
        spatialActionsRef,
        setFocusRef,
        dismissRef,
      }),
      ...STATIC_GLOBAL_COMMANDS,
    ],
    [],
  );

  /** Close the command palette (dispatch to backend) and return to normal mode. */
  const closePalette = useCallback(() => {
    dismiss();
  }, [dismiss]);

  return (
    <CommandScopeProvider commands={globalCommands}>
      <KeybindingHandler mode={keymapMode} />
      {children}
      {/* The palette is its own modal layer: arrow keys move only between
          palette rows, never bleeding back to whatever was beneath. The
          layer mounts when the palette opens (from the backend UIState
          `palette_open` flag) and unmounts when it closes — pop on
          unmount restores `last_focused` on the parent (window-root)
          layer, so dismissing the palette with Escape returns focus to
          whichever leaf was focused before the palette opened.

          `parentLayerFq={windowLayerFq}` is required because the
          palette portals to `document.body`; without an explicit parent
          the FocusLayer would compute `parent=null` and mint a second
          window-root, which the Rust registry rejects as a corruption. */}
      {paletteOpen && (
        <FocusLayer name={PALETTE_LAYER_NAME} parentLayerFq={windowLayerFq}>
          <CommandPalette
            open={paletteOpen}
            onClose={closePalette}
            mode={paletteMode}
            onSwitchBoard={onSwitchBoard}
          />
        </FocusLayer>
      )}
    </CommandScopeProvider>
  );
}
