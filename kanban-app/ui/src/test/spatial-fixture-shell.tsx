/**
 * Shared keybinding + nav-command shell for vitest-browser spatial-nav
 * fixtures.
 *
 * ## Why this exists
 *
 * Three fixtures (`spatial-grid-fixture.tsx`, `spatial-inspector-fixture.tsx`,
 * and `spatial-board-fixture.tsx`) all need the same keyboard wiring:
 *
 * - A keydown handler that routes vim-mode keys through the focused scope's
 *   binding chain, matching `AppShell`'s `KeybindingHandler` in production.
 * - A `CommandScopeProvider` populated with `nav.up`/`nav.down`/`nav.left`/
 *   `nav.right` (and their `nav.first`/`nav.last` siblings) carrying no
 *   local `execute:` — identical to `AppShell`'s `NAV_COMMAND_DEFS`. The
 *   dispatcher routes each keypress through `dispatch_command` to the
 *   Rust nav handler, exactly like production.
 * - A `FocusLayer name="window"` at the root so every scope has a layer to
 *   register with.
 *
 * Each fixture previously defined its own copy of this wiring. Keeping the
 * three copies in sync by hand is a drift hazard — if production's
 * `AppShell` keybinding shape changes, every fixture has to be updated
 * independently or the test harness silently diverges from production.
 *
 * Extracting the shell here gives us one source of truth. Fixtures compose
 * this component with their own body content and, if needed, extra commands
 * (e.g. the inspector fixture adds a `ui.inspect` handler).
 *
 * ## What this shell does NOT own
 *
 * - `EntityFocusProvider` — each fixture still mounts its own provider at
 *   the root so tests can observe focus state if they want. The shell
 *   assumes it renders inside an `EntityFocusProvider`.
 * - The fixture body — callers pass their column/row/card tree as
 *   `children`.
 * - Extra focus layers (e.g. the inspector's inner layer) — callers render
 *   those inside `children`.
 */

import { useContext, useEffect, useRef, type ReactNode } from "react";
import {
  CommandScopeContext,
  CommandScopeProvider,
  type CommandDef,
  useDispatchCommand,
} from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";

/**
 * Optional vim-mode overrides for the nav commands.
 *
 * The grid and board fixtures leave `nav.first`/`nav.last` unbound in vim
 * mode (they only expose cua-mode `Home`/`End`), while the inspector
 * fixture needs `g g` and `Shift+G` to match production. The shell accepts
 * these overrides rather than hard-coding both patterns.
 */
export interface NavVimOverrides {
  /** Vim keybinding for `nav.first`. Default: no vim binding. */
  navFirstVim?: string;
  /** Vim keybinding for `nav.last`. Default: no vim binding. */
  navLastVim?: string;
}

/**
 * Build the navigation commands with keybindings only — no local
 * `execute:` handler.
 *
 * Matches production's `NAV_COMMAND_DEFS` exactly: every nav keypress
 * falls through to `dispatch_command`, which the test's `tauriCoreMock`
 * intercepts and routes back into the shim's `SpatialState::navigate`.
 * There is no JS-side broadcaster in the fixture's dependency graph,
 * so a regression that re-adds one would fail the tests instead of
 * passing on the broken side-channel.
 */
export function useFixtureNavCommands(
  overrides: NavVimOverrides = {},
): CommandDef[] {
  const firstKeys: CommandDef["keys"] = overrides.navFirstVim
    ? { vim: overrides.navFirstVim, cua: "Home" }
    : { cua: "Home" };
  const lastKeys: CommandDef["keys"] = overrides.navLastVim
    ? { vim: overrides.navLastVim, cua: "End" }
    : { cua: "End" };

  return [
    {
      id: "nav.up",
      name: "Navigate Up",
      keys: { vim: "k", cua: "ArrowUp" },
    },
    {
      id: "nav.down",
      name: "Navigate Down",
      keys: { vim: "j", cua: "ArrowDown" },
    },
    {
      id: "nav.left",
      name: "Navigate Left",
      keys: { vim: "h", cua: "ArrowLeft" },
    },
    {
      id: "nav.right",
      name: "Navigate Right",
      keys: { vim: "l", cua: "ArrowRight" },
    },
    {
      id: "nav.first",
      name: "First",
      keys: firstKeys,
    },
    {
      id: "nav.last",
      name: "Last",
      keys: lastKeys,
    },
  ];
}

/**
 * Wire a real keydown handler to the document for vim/cua/emacs bindings.
 *
 * Must run inside `CommandScopeProvider` (so `useDispatchCommand` can reach
 * nav commands) and `EntityFocusProvider` (so focused-scope resolution
 * works). The handler uses the production `createKeyHandler` — same code
 * path the real `AppShell` uses — so 'j' → `nav.down` is identical between
 * fixture and production.
 *
 * Passes `extractScopeBindings` as the third argument so vim-mode
 * `h`/`j`/`k`/`l` (defined on `CommandDef.keys.vim`) resolve through the
 * focused scope chain — same wiring as `AppShell`'s `KeybindingHandler` in
 * production. Without this, `j` has no global binding and would do nothing.
 *
 * Falls back to the tree (ambient) `CommandScopeContext` when no scope is
 * focused, mirroring `AppShell`'s `focusedScope ?? treeScope` pattern so the
 * "something is always focused" invariant can recover from a null focus via
 * a nav key (the key must resolve to `nav.*` even without a focused scope).
 */
export function FixtureKeybindingHandler({ mode }: { mode: KeymapMode }) {
  const dispatch = useDispatchCommand();
  const focusedScope = useFocusedScope();
  const treeScope = useContext(CommandScopeContext);

  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;
  const focusedScopeRef = useRef(focusedScope);
  focusedScopeRef.current = focusedScope;
  const treeScopeRef = useRef(treeScope);
  treeScopeRef.current = treeScope;

  useEffect(() => {
    const handler = createKeyHandler(
      mode,
      async (id) => {
        await dispatchRef.current(id);
        return true;
      },
      () =>
        extractScopeBindings(
          focusedScopeRef.current ?? treeScopeRef.current,
          mode,
        ),
    );
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [mode]);

  return null;
}

/** Props for `FixtureShell`. */
export interface FixtureShellProps {
  /** Fixture body — column/row/card tree rendered inside the scope. */
  children: ReactNode;
  /**
   * Additional commands to merge alongside the standard `nav.*` set.
   *
   * Fixtures that need extra bindings (e.g. the inspector's `ui.inspect`
   * handler) pass them here rather than wrapping the shell in yet another
   * provider. Commands are concatenated after the nav set.
   */
  extraCommands?: CommandDef[];
  /**
   * Optional vim-mode overrides for `nav.first`/`nav.last`.
   *
   * Defaults to cua-only bindings, which matches the grid and board
   * fixtures. The inspector fixture passes `g g` / `Shift+G` to match
   * production's inspector nav.
   */
  navOverrides?: NavVimOverrides;
  /**
   * Keybinding mode to wire into the document handler. Defaults to `"vim"`
   * because all three spatial fixtures drive h/j/k/l.
   */
  mode?: KeymapMode;
}

/**
 * Root shell around the fixture body: provides the window `FocusLayer`,
 * wires nav commands (plus any fixture-specific extras) into the
 * `CommandScope`, and installs the keybinding handler.
 *
 * Assumes it renders inside an `EntityFocusProvider` — the shell does not
 * mount one itself so fixtures retain control over the focus root.
 */
export function FixtureShell({
  children,
  extraCommands,
  navOverrides,
  mode = "vim",
}: FixtureShellProps) {
  const navCommands = useFixtureNavCommands(navOverrides);
  const commands = extraCommands
    ? [...navCommands, ...extraCommands]
    : navCommands;
  return (
    <FocusLayer name="window">
      <CommandScopeProvider commands={commands}>
        <FixtureKeybindingHandler mode={mode} />
        {children}
      </CommandScopeProvider>
    </FocusLayer>
  );
}
