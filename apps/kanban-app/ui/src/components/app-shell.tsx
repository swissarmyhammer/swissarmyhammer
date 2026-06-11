import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import { useUIState } from "@/lib/ui-state-context";
import { useAppMode } from "@/lib/app-mode-context";
import {
  createKeyHandler,
  extractChainBindings,
  extractKeymapBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import { useCommandList } from "@/hooks/use-command-list";
import { reportDispatchError } from "@/lib/dispatch-error";
import { CommandPalette } from "@/components/command-palette";
import { FocusLayer } from "@/components/focus-layer";
import { JumpToOverlay } from "@/components/jump-to-overlay";
import { useEnclosingLayerFq } from "@/components/layer-fq-context";
import { asSegment } from "@/types/spatial";
import { triggerStartRename } from "@/components/perspective-tab-bar";
import { registerWebviewCommandHandler } from "@/lib/webview-command-bus";
import {
  aiStreaming,
  triggerAiCancel,
  triggerAiFocus,
  triggerAiModel,
  triggerAiNewChat,
  triggerAiToggle,
} from "@/ai/commands";

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

  // Global keybindings are sourced from the metadata-driven Command registry,
  // not a hardcoded table: every command that declares `keys[mode]`
  // contributes one binding. The list re-fetches on `commands/changed`, so a
  // newly-registered command's key is live without a reload. The effect below
  // re-creates the handler whenever the keymap mode or the derived table
  // changes (a keymap switch is itself a `settings.keymap.*` command).
  const { commands: registryCommands } = useCommandList();
  const globalBindings = useMemo(
    () => extractKeymapBindings(registryCommands, mode),
    [registryCommands, mode],
  );

  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;
  const focusedScopeRef = useRef(focusedScope);
  focusedScopeRef.current = focusedScope;
  // The scoped-registry binding layer (below) reads the live registry at
  // keystroke time; a ref keeps the handler effect's dependency list down to
  // the derived global table while still seeing every registry re-fetch.
  const registryCommandsRef = useRef(registryCommands);
  registryCommandsRef.current = registryCommands;

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
    // Pass scope bindings so command `keys` from the focused scope are
    // resolved through the same single key handler. Two binding sources merge
    // here in ONE depth-interleaved inner-first walk over the focused chain
    // (`extractChainBindings`):
    //
    //   1. Scope-level React `CommandDef`s (inspector close, pill untag,
    //      Inspectable Space, root `ai.*`) — component-owned; at any given
    //      chain depth they win over the registry layer for the same key
    //      (inner knowledge beats catalogue metadata).
    //   2. Scoped REGISTRY bindings — plugin-defined commands whose `scope`
    //      names a zone moniker literally present in the focused chain (the
    //      `grid-commands` plugin's `scope: ["ui:grid"]`, Card C; the
    //      `app-shell-commands` plugin's `ui:field` / `ui:pressable` markers,
    //      Card D). Their behaviors live on the webview command bus,
    //      registered by the zone's component, so a literal-moniker match
    //      implies the handler is live.
    //
    // The interleave (rather than two flat layers) is load-bearing: a focused
    // `<Pressable>`'s registry-bound Space (matched at its inner
    // `ui:pressable` marker) must beat any outer claim of the same key —
    // innermost wins across BOTH sources, exactly as it did when every
    // binding was a component def. (The GLOBAL Space → `entity.inspect`
    // binding is plugin-owned, Card G, and only fires when no chain scope
    // claims Space — scope beats global in `createKeyHandler`.)
    const handler = createKeyHandler(
      mode,
      executeCommand,
      () =>
        extractChainBindings(
          registryCommandsRef.current,
          mode,
          focusedScopeRef.current,
        ),
      globalBindings,
    );
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [mode, executeCommand, globalBindings]);

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

// There is NO static global command list here (Card I deleted
// `STATIC_GLOBAL_COMMANDS`). Every global command — app.*, settings.keymap.*,
// file.*, window.new, ai.* — is DEFINED by a builtin plugin
// (`builtin/plugins/app-shell-commands`, `file-commands`, `app-shell-commands`,
// `ai-commands`, …) and surfaces through the Command service catalogue: the
// palette and menus read `useCommandList`, and the hotkey layer derives its
// global table from the same registry via `extractKeymapBindings`. The
// registry `keys` use the canonical `normalizeKeyEvent` form, so no React-side
// duplicate definition is needed for a key to resolve.
//
// `app.dismiss` deliberately carries NO Escape binding anywhere (card
// `01KTPDTH772HSEV5F7R1DKYDNJ`): Escape is owned globally by `nav.drillOut`,
// whose backend fall-through performs the contextual dismiss. The id remains
// dispatchable programmatically (inspector backdrop click, quick-capture)
// against its plugin registration.
//
// (`app.resetWindows`, which the old static list still carried, was
// deliberately dropped with no replacement planned: it had no plugin
// definition and no backend implementation — dispatching it always errored,
// a brokenness that predates this cleanup. The command-cutover deleted the
// dispatch path the historical fix card 01KN2GX9ABPFFAFG536SMWN9MY targeted,
// and that card is closed; nothing tracks the feature. If Reset Windows
// should ever return, file a fresh card against the window plugin.)

/**
 * Build the dynamic global commands — currently just the
 * app.entity.startRename command, which exists in the backend registry for
 * palette discovery but runs locally via `triggerStartRename`.
 *
 * The directional / first-last / drill `nav.*` commands no longer live
 * here — they are owned by the `nav-commands` builtin plugin
 * (`builtin/plugins/nav-commands/index.ts`) and execute host-side through
 * the `focus` kernel, so `useDispatchCommand` routes a dispatched `nav.*`
 * id to the backend rather than a React closure.
 *
 * The root-scope `entity.inspect` (Space) no longer lives here either
 * (Card G): the plugin-owned `entity.inspect`
 * (`builtin/plugins/app-shell-commands/commands/ui.ts`) carries the Space keys
 * GLOBALLY, so the binding always resolves (the keybinding handler still
 * `preventDefault()`s and the browser never page-scrolls on Space), and
 * its execute resolves the focused entity SERVER-SIDE from the dispatched
 * scope chain — replacing the React-side `INSPECTABLE_ENTITY_PREFIXES`
 * filter that used to live in this file.
 */
function buildDynamicGlobalCommands(): CommandDef[] {
  return [
    {
      id: "app.entity.startRename",
      name: "Rename Perspective",
      execute: () => {
        triggerStartRename();
      },
    },
  ];
}

/**
 * Register the webview command-bus handlers for the five `ai.*` window-layer
 * commands.
 *
 * The commands are DEFINED by the `ai-commands` builtin plugin
 * (`builtin/plugins/ai-commands/index.ts` — id, name, keys, menu in the
 * unified registry; the plugin's backend `execute` is an inert no-op). Their
 * EFFECT lives in the AI panel React tree, so `AppShell` bridges the two on
 * the webview command bus (Card I, replacing the deleted client-side
 * `buildAiCommands` scope defs): `useDispatchCommand` runs a registered
 * handler and skips the backend, and each handler calls into the
 * `ai/commands.ts` module registry, where the AI panel components register
 * the live behaviors on mount. A command fired before the panel mounts is a
 * silent no-op, exactly as before.
 *
 * `ai.cancel` keeps its availability gate here: a generation can only be
 * stopped while it is in flight, so the handler reads {@link aiStreaming} at
 * dispatch time and no-ops when the conversation is idle — the same
 * observable behavior the legacy `available: false` scope def produced
 * (palette-side gating of the registry entry is tracked separately, kanban
 * 01KT7DB01HTR9SNRRG145F009P).
 */
function useAiCommandBusHandlers(): void {
  useEffect(() => {
    const cleanups = [
      registerWebviewCommandHandler("ai.toggle", () => {
        triggerAiToggle();
      }),
      registerWebviewCommandHandler("ai.focus", () => {
        triggerAiFocus();
      }),
      registerWebviewCommandHandler("ai.newChat", () => {
        triggerAiNewChat();
      }),
      // The `model` id rides in `opts.args` — palette rows that select a
      // model dispatch `ai.model` with `{ args: { model } }`.
      registerWebviewCommandHandler("ai.model", (opts) => {
        const model = opts?.args?.model;
        triggerAiModel(typeof model === "string" ? model : undefined);
      }),
      registerWebviewCommandHandler("ai.cancel", () => {
        if (aiStreaming()) {
          triggerAiCancel();
        }
      }),
    ];
    return () => {
      for (const cleanup of cleanups) cleanup();
    };
  }, []);
}

// `nav.focus` is registered in `<EntityFocusProvider>` rather than here.
// The command wraps the entity-focus `setFocus` primitive, and tests
// commonly mount `<EntityFocusProvider>` without `<AppShell>`. Colocating
// the registration with the primitive it wraps means every tree that
// mounts the focus provider gets `nav.focus` resolution — production
// trees through `<AppShell>` and isolated test harnesses alike.

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
  const dismiss = useDispatchCommand("app.dismiss");

  // The plugin-defined `ai.*` commands execute through webview-bus handlers
  // registered here at the window layer (their effect lives in the AI panel
  // subtree; the `ai.cancel` handler gates on the live streaming flag).
  useAiCommandBusHandlers();

  // Jump-To overlay open/close lives here so every entry point —
  // vim-mode `s`, cua/emacs `Mod+G`, the Navigation > Jump To menu
  // item, and the palette — opens the *same* overlay instance. The
  // `nav.jump` plugin command's webview-bus handler (registered below)
  // flips this; `<JumpToOverlay>` mounts when `jumpOpen` is true and
  // dismisses itself via the sentinel `app.dismiss` shadow on Escape /
  // backdrop click / blur.
  const [jumpOpen, setJumpOpen] = useState(false);

  // `nav.jump` is a plugin command (owned by the `nav-commands` bundle) with no
  // backend op: its effect is presentation-only — open the `<JumpToOverlay>`.
  // Register a webview handler for the id on the command bus (Card B); when the
  // id is dispatched (keybinding `s` / `Mod+G`, the Navigation > Jump To menu
  // item, or the palette), `useDispatchCommand` runs this handler and skips the
  // backend. The ownership-guarded cleanup runs on unmount so a stale closure
  // never lingers. This is pure presentation — it only flips local React state.
  useEffect(() => {
    return registerWebviewCommandHandler("nav.jump", () => {
      setJumpOpen(true);
    });
  }, []);

  // Window-root layer FQ — passed explicitly to the palette `<FocusLayer>`
  // because the command palette renders via `createPortal(document.body)`,
  // which severs the React ancestor chain a `<FocusLayer>` would otherwise
  // walk. Reading the FQ here, where `<FocusLayer name="window">` (mounted
  // in `App.tsx`) is still a direct ancestor, captures the right parent
  // regardless of how the palette portals out at render time.
  const windowLayerFq = useEnclosingLayerFq();

  usePaletteModeSync(paletteOpen);

  // The window-layer scope carries ONLY `app.entity.startRename` (see
  // `buildDynamicGlobalCommands`) — every other global command is
  // plugin-defined and resolves from the Command service catalogue (Card I
  // deleted the static client-side list and the `ai.*` scope defs; the ai
  // executions now ride the webview command bus, registered above).
  //
  // The directional / first-last / drill `nav.*` commands and `nav.jump` are
  // likewise not registered here — the `nav-commands` builtin plugin owns
  // them. The directional / drill commands execute host-side through the
  // `focus` kernel (so `useDispatchCommand` routes a dispatched `nav.*` id to
  // the backend), and `nav.jump`'s webview-bus handler (registered above)
  // opens the jump overlay. `entity.inspect` is likewise plugin-owned
  // (Card G, `builtin/plugins/app-shell-commands/commands/ui.ts`).
  const globalCommands: CommandDef[] = useMemo(
    () => buildDynamicGlobalCommands(),
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
      {/* Jump-To overlay (AceJump-style scope picker). Opened by the
          `nav.jump` global command from any entry point (keybinding,
          menu, palette). The overlay manages its own internal focus
          layer (`/jump-to`) and dismiss paths — Escape, backdrop
          click, no-match flash, and window blur all flow through its
          sentinel `app.dismiss` shadow back to `setJumpOpen(false)`. */}
      <JumpToOverlay open={jumpOpen} onClose={() => setJumpOpen(false)} />
    </CommandScopeProvider>
  );
}
