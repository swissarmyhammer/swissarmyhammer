/**
 * StoreContainer binds a filesystem path (`.kanban` directory) to the scope
 * chain via a `store:{path}` moniker.
 *
 * Sits between RustEngineContainer and BoardContainer in the hierarchy:
 *
 * ```
 * WindowContainer (window:main)
 *   └─ RustEngineContainer (engine)
 *        └─ StoreContainer (store:/path/to/.kanban)  ← this component
 *             └─ BoardContainer (board:01ABC)
 *                  └─ ...
 * ```
 *
 * The backend extracts the store path from the scope chain via
 * `resolve_store_path()`, removing the need for an explicit `boardPath`
 * IPC parameter.
 */

import { useMemo, type ReactNode } from "react";
import { CommandScopeProvider } from "@/lib/command-scope";

interface StoreContainerProps {
  /** The canonical filesystem path to the `.kanban` directory. */
  path: string;
  children: ReactNode;
}

/**
 * Provides a `store:{path}` moniker in the scope chain.
 *
 * Uses {@link CommandScopeProvider} rather than `<FocusScope>` because the
 * store scope is purely structural — it contributes a moniker to the
 * scope chain so the backend can resolve the store path, and nothing
 * more. There is no entity to focus, no spatial-nav rect to register,
 * and no DOM surface to draw a focus bar around. Sibling structural
 * containers (`AppModeContainer`, `WindowContainer`,
 * `BoardContainer`) follow the same `<CommandScopeProvider>`-only
 * pattern.
 */
export function StoreContainer({ path, children }: StoreContainerProps) {
  const moniker = useMemo(() => `store:${path}`, [path]);

  return (
    <CommandScopeProvider moniker={moniker}>{children}</CommandScopeProvider>
  );
}
