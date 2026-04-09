/**
 * AppModeContainer owns the application interaction mode (normal, command, search).
 *
 * Wraps immediately inside WindowContainer, ABOVE everything else. The mode
 * governs keybinding interpretation, visual indicators, and command availability.
 *
 * Owns:
 * - AppModeProvider context (mode + setMode)
 * - CommandScopeProvider moniker="mode:{current_mode}"
 *
 * Hierarchy:
 * ```
 * WindowContainer          window:{label}
 *   AppModeContainer       mode:{mode}    <- this component
 *     BoardContainer       board:{id}
 *       ...
 * ```
 */

import { type ReactNode } from "react";
import { CommandScopeProvider } from "@/lib/command-scope";
import { AppModeProvider, useAppMode } from "@/lib/app-mode-context";

// Re-export useAppMode so consumers can import from this container
export { useAppMode, type AppMode } from "@/lib/app-mode-context";

// ---------------------------------------------------------------------------
// AppModeContainer
// ---------------------------------------------------------------------------

interface AppModeContainerProps {
  children: ReactNode;
}

/**
 * Wraps children with the app mode context and a command scope moniker
 * that reflects the current mode (e.g. "mode:normal", "mode:command").
 *
 * Must render inside WindowContainer so it has access to the window scope chain.
 */
export function AppModeContainer({ children }: AppModeContainerProps) {
  return (
    <AppModeProvider>
      <AppModeScopeWrapper>{children}</AppModeScopeWrapper>
    </AppModeProvider>
  );
}

/**
 * Inner wrapper that reads the current mode from context and provides
 * a CommandScopeProvider with the corresponding moniker.
 *
 * Separated from AppModeContainer so that useAppMode() is called inside
 * AppModeProvider's subtree.
 */
function AppModeScopeWrapper({ children }: { children: ReactNode }) {
  const { mode } = useAppMode();

  return (
    <CommandScopeProvider commands={[]} moniker={`mode:${mode}`}>
      {children}
    </CommandScopeProvider>
  );
}
