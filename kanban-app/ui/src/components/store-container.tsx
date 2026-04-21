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
import { FocusScope } from "@/components/focus-scope";

interface StoreContainerProps {
  /** The canonical filesystem path to the `.kanban` directory. */
  path: string;
  children: ReactNode;
}

/**
 * Provides a `store:{path}` FocusScope in the scope chain.
 *
 * Uses `renderContainer={false}` to avoid adding a wrapping DOM element —
 * the store scope is purely structural, not visual.
 */
export function StoreContainer({ path, children }: StoreContainerProps) {
  const moniker = useMemo(() => `store:${path}`, [path]);

  return (
    <FocusScope
      moniker={moniker}
      renderContainer={false}
      showFocusBar={false}
    >
      {children}
    </FocusScope>
  );
}
