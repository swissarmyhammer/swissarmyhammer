import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
  type DispatchOptions,
} from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import {
  useSpatialFocusActions,
  type SpatialFocusActions,
} from "@/lib/spatial-focus-context";
import { useUIState } from "@/lib/ui-state-context";
import { useAppMode } from "@/lib/app-mode-context";
import {
  createKeyHandler,
  extractKeymapBindings,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import { useCommandList } from "@/hooks/use-command-list";
import { reportDispatchError } from "@/lib/dispatch-error";
import { CommandPalette } from "@/components/command-palette";
import { FocusLayer } from "@/components/focus-layer";
import { JumpToOverlay } from "@/components/jump-to-overlay";
import { useEnclosingLayerFq } from "@/components/layer-fq-context";
import { asSegment, fqLastSegment } from "@/types/spatial";
import { triggerStartRename } from "@/components/perspective-tab-bar";
import { registerWebviewCommandHandler } from "@/lib/webview-command-bus";
import {
  aiStreaming,
  subscribeAiStreaming,
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
    const handler = createKeyHandler(
      mode,
      executeCommand,
      () => extractScopeBindings(focusedScopeRef.current, mode),
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
 * Read-only ref bag for the root-scope `entity.inspect` command closure.
 *
 * The closure is minted once into the `globalCommands` memo and lives for the
 * AppShell's lifetime, but it needs to read the *latest* spatial focus actions
 * on every keystroke. Holding the actions in a ref lets the memo dependency
 * list stay empty without staling on context updates.
 *
 * The directional / first-last / drill `nav.*` commands no longer live here —
 * they are owned by the `nav-commands` builtin plugin
 * (`builtin/plugins/nav-commands/index.ts`) and execute host-side through the
 * `focus` kernel, so `useDispatchCommand` routes a dispatched `nav.*` id to the
 * backend rather than a React closure.
 */
interface DrillRefs {
  spatialActionsRef: React.MutableRefObject<SpatialFocusActions>;
}

/**
 * Inspectable-entity SegmentMoniker prefixes — the kinds of focused FQMs
 * for which the root-scope `entity.inspect` command actually dispatches
 * `ui.inspect`. UI chrome (`ui:*`, `perspective_tab:`, `cell:*`,
 * `grid_cell:*`, `row_label:`, etc.) is not inspectable.
 *
 * Mirrors the `ENTITY_PREFIXES` list pinned by the architectural guard
 * (`focus-architecture.guards.node.test.ts`, Guards B + C) — keep the
 * two lists in sync. The duplication is intentional: the guard's list
 * is derived from `<Inspectable>` JSX call sites, this list is the
 * runtime filter on focused FQMs, and an outright import would create
 * a test-source coupling.
 */
const INSPECTABLE_ENTITY_PREFIXES = [
  "task:",
  "tag:",
  "column:",
  "board:",
  "field:",
  "attachment:",
] as const;

/** True if the leaf segment of an FQM identifies an inspectable entity. */
function isInspectableSegment(segment: string): boolean {
  return INSPECTABLE_ENTITY_PREFIXES.some((p) => segment.startsWith(p));
}

/**
 * Build the root-scope `entity.inspect` command — the global Space
 * binding that fires when no per-`<Inspectable>` scope is in the
 * focused chain to shadow it.
 *
 * The per-Inspectable scope command (`inspectable.tsx`) registers the
 * same id at scope level with the same `keys`. `extractScopeBindings`
 * walks the focused scope chain inner-first and returns the closest
 * binding for a given key, so when an Inspectable wraps the focused
 * leaf its scope-level command wins and the root one never runs. When
 * the chain has no Inspectable — at app open with `<body>` focus, on a
 * focused chrome scope (perspective tab, filter editor), or after the
 * inspector closes and focus is parked off any entity — this root
 * binding takes over.
 *
 * Behavior:
 *   - `focusedFq() === null`: no-op. The keybinding handler still
 *     calls `preventDefault()` because the binding lookup succeeded,
 *     which is the load-bearing effect (the browser does not scroll
 *     the page).
 *   - `focusedFq()` resolves to a non-Inspectable kind (e.g. a
 *     `perspective_tab:`): no-op. Same reasoning — preventDefault
 *     fires from the binding-resolution path; the execute closure
 *     filters by `INSPECTABLE_ENTITY_PREFIXES` so chrome focus does
 *     not synthesize a bogus `ui.inspect` against a non-entity
 *     moniker.
 *   - `focusedFq()` resolves to an inspectable kind (`task:`, `tag:`,
 *     `column:`, `board:`, `field:`, `attachment:`): dispatches
 *     `ui.inspect` with the leaf segment as `target` — same shape the
 *     per-Inspectable scope command uses, so the backend handler sees
 *     a uniform payload across paths.
 *
 * The DOM `<body>` / `<input>` / `[contenteditable]` distinction lives
 * upstream in `createKeyHandler`'s `isEditableTarget` gate, which
 * short-circuits before the binding map is consulted — so this
 * command never fires when DOM focus is on an editable surface, and
 * `preventDefault()` is correctly NOT called there.
 *
 * Pinned by `inspectable.space.browser.test.tsx` (cards
 * `01KQJHFX0HADZH74P7KJQRFM4E` — root-scope Space binding).
 */
function buildRootInspectCommand(
  spatialActionsRef: React.MutableRefObject<SpatialFocusActions>,
  inspectDispatchRef: React.MutableRefObject<
    (opts?: DispatchOptions) => Promise<unknown>
  >,
): CommandDef {
  return {
    id: "entity.inspect",
    name: "Inspect",
    keys: { vim: "Space", cua: "Space", emacs: "Space" },
    execute: () => {
      const focusedFq = spatialActionsRef.current.focusedFq();
      if (focusedFq === null) return;
      const segment = fqLastSegment(focusedFq);
      if (!isInspectableSegment(segment)) return;
      inspectDispatchRef.current({ target: segment }).catch(console.error);
    },
  };
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
 *
 * The root-scope `entity.inspect` (Space) lives here too — same
 * reasoning: shadowed by the per-`<Inspectable>` scope command when an
 * inspectable entity is in the focused chain, but always present at
 * the root so Space never falls through to the browser's page-scroll
 * default.
 */
function buildDynamicGlobalCommands(
  drillRefs: DrillRefs,
  inspectDispatchRef: React.MutableRefObject<
    (opts?: DispatchOptions) => Promise<unknown>
  >,
): CommandDef[] {
  return [
    buildRootInspectCommand(drillRefs.spatialActionsRef, inspectDispatchRef),
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
 * Build the window-layer `ai.*` commands that drive the AI panel.
 *
 * These are registered in `AppShell`'s global command scope — the window
 * layer — so their keybindings fire app-wide, even when focus is on a board
 * card outside the AI panel (matching `ARCHITECTURE.md`'s scope model). Each
 * `execute` closure calls into the `ai/commands.ts` module registry, where the
 * AI panel components have registered the live handlers; a command fired
 * before the panel mounts is a silent no-op.
 *
 * `ai.cancel` is the one availability-gated command: a generation can only be
 * stopped while it is in flight, so its `available` flag tracks the
 * `streaming` argument. When `streaming` is `false` the `CommandDef` is
 * `available: false`, which both hides it from the palette
 * (`collectAvailableCommands`) and makes its keybinding a no-op
 * (`resolveCommand` returns `null` on a blocked command).
 *
 * The `ai.*` `keys` blocks here mirror `swissarmyhammer-kanban`'s
 * `builtin/commands/ai.yaml` — the YAML side feeds the palette's keybinding
 * hints and the backend completeness guard; this React side feeds
 * `extractScopeBindings`. The static `BINDING_TABLES` entries cover the
 * no-focus case where the scope walk yields nothing.
 *
 * @param streaming - Whether the AI conversation is currently streaming.
 * @returns The five `ai.*` command definitions.
 */
function buildAiCommands(streaming: boolean): CommandDef[] {
  return [
    {
      // `keys` use the canonical lowercase form `normalizeKeyEvent`
      // emits for a non-shifted letter (e.g. `Mod+j`), matching the
      // `BINDING_TABLES` entries and the rest of `STATIC_GLOBAL_COMMANDS`
      // (`file.closeBoard` is `Mod+w`). The YAML mirror keeps `Mod+J`
      // uppercase — that side feeds menu accelerators / palette hints.
      id: "ai.toggle",
      name: "Toggle AI Panel",
      keys: { vim: "Mod+j", cua: "Mod+j", emacs: "Mod+j" },
      execute: () => {
        triggerAiToggle();
      },
    },
    {
      id: "ai.focus",
      name: "Focus AI Panel",
      keys: { vim: "Mod+i", cua: "Mod+i", emacs: "Mod+i" },
      execute: () => {
        triggerAiFocus();
      },
    },
    {
      id: "ai.newChat",
      name: "New AI Chat",
      keys: { vim: "Mod+Shift+J", cua: "Mod+Shift+J", emacs: "Mod+Shift+J" },
      execute: () => {
        triggerAiNewChat();
      },
    },
    {
      id: "ai.model",
      name: "Set AI Model",
      // The `model` id rides in `opts.args` — palette rows that select a
      // model dispatch `ai.model` with `{ args: { model } }`.
      execute: (opts) => {
        const model = opts?.args?.model;
        triggerAiModel(typeof model === "string" ? model : undefined);
      },
    },
    {
      id: "ai.cancel",
      name: "Stop AI Generation",
      keys: { vim: "Mod+.", cua: "Mod+.", emacs: "Mod+." },
      // Available only mid-stream — `available: false` blocks both the
      // palette entry and the keybinding when the conversation is idle.
      available: streaming,
      execute: () => {
        triggerAiCancel();
      },
    },
  ];
}

/**
 * Subscribe to the AI conversation's streaming flag.
 *
 * A `useSyncExternalStore` binding over the `ai/commands.ts` registry: when
 * the conversation enters or leaves the streaming state, `AppShell` re-renders
 * and rebuilds `ai.cancel` with the fresh `available` flag.
 *
 * @returns `true` while the AI conversation is streaming a turn.
 */
function useAiStreaming(): boolean {
  return useSyncExternalStore(subscribeAiStreaming, aiStreaming, aiStreaming);
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
  // Tracks the AI conversation's streaming flag so `ai.cancel`'s `available`
  // is rebuilt whenever a turn starts or ends.
  const aiIsStreaming = useAiStreaming();
  const spatialActions = useSpatialFocusActions();
  const dismiss = useDispatchCommand("app.dismiss");
  // Pre-bound dispatcher for the root-scope `entity.inspect` command
  // (`buildRootInspectCommand`). The closure that owns Space at the
  // root needs a stable handle to dispatch `ui.inspect` against the
  // currently-focused entity moniker; reading the dispatcher here
  // anchors it inside the same React tree the per-Inspectable scope
  // command resolves through.
  const inspectDispatch = useDispatchCommand("ui.inspect");

  // Jump-To overlay open/close lives here so every entry point —
  // vim-mode `s`, cua/emacs `Mod+G`, the Navigation > Jump To menu
  // item, and the palette — opens the *same* overlay instance. The
  // `nav.jump` plugin command's webview-bus handler (registered below)
  // flips this; `<JumpToOverlay>` mounts when `jumpOpen` is true and
  // dismisses itself via the sentinel `app.dismiss` shadow on Escape /
  // backdrop click / blur.
  const [jumpOpen, setJumpOpen] = useState(false);

  // The root-scope `entity.inspect` command needs read-on-demand access to
  // spatial focus. Holding the actions in a ref keeps the `globalCommands`
  // memo dependency list empty while still letting the closure see the latest
  // context value at keystroke time. The actions bag from
  // `useSpatialFocusActions` is itself identity-stable (built once per provider
  // lifetime), so the ref is belt-and-braces — a future refactor that turns it
  // into a per-render value still survives.
  const spatialActionsRef = useRef(spatialActions);
  spatialActionsRef.current = spatialActions;
  const inspectDispatchRef = useRef(inspectDispatch);
  inspectDispatchRef.current = inspectDispatch;

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

  // Static commands come from module scope; dynamic ones close over the
  // spatial-actions ref. Both batches are stable, so the memo depends only on
  // `aiIsStreaming` (which rebuilds the `ai.cancel` availability gate).
  //
  // The directional / first-last / drill `nav.*` commands and `nav.jump` are
  // no longer registered here — the `nav-commands` builtin plugin owns them.
  // The directional / drill commands execute host-side through the `focus`
  // kernel (so `useDispatchCommand` routes a dispatched `nav.*` id to the
  // backend), and `nav.jump`'s webview-bus handler (registered above) opens the
  // jump overlay.
  const globalCommands: CommandDef[] = useMemo(
    () => [
      ...buildDynamicGlobalCommands({ spatialActionsRef }, inspectDispatchRef),
      // The window-layer `ai.*` commands — registered here so their
      // keybindings fire app-wide. Rebuilt when `aiIsStreaming` flips
      // so `ai.cancel`'s `available` tracks the live conversation.
      ...buildAiCommands(aiIsStreaming),
      ...STATIC_GLOBAL_COMMANDS,
    ],
    [aiIsStreaming],
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
