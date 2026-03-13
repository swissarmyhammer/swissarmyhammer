import { createContext, useContext, useCallback, useMemo, type ReactNode } from "react";
import { parseMoniker } from "@/lib/moniker";

type InspectFn = (moniker: string) => void;
type DismissFn = () => boolean;

interface InspectContextValue {
  inspect: InspectFn;
  /** Close the topmost inspector panel. Returns true if a panel was closed. */
  dismiss: DismissFn;
}

const InspectContext = createContext<InspectContextValue | null>(null);

interface InspectProviderProps {
  /** Called with (entityType, entityId) parsed from the moniker. */
  onInspect: (entityType: string, entityId: string) => void;
  /** Called to close the topmost inspector panel. Returns true if a panel was closed. */
  onDismiss: () => boolean;
  children: ReactNode;
}

/**
 * Provides inspect and dismiss functions for the inspector panel stack.
 */
export function InspectProvider({ onInspect, onDismiss, children }: InspectProviderProps) {
  const inspect = useCallback(
    (m: string) => {
      const { type, id } = parseMoniker(m);
      onInspect(type, id);
    },
    [onInspect],
  );

  const value = useMemo(
    () => ({ inspect, dismiss: onDismiss }),
    [inspect, onDismiss],
  );

  return (
    <InspectContext.Provider value={value}>
      {children}
    </InspectContext.Provider>
  );
}

/**
 * Returns the inspect function from the nearest InspectProvider.
 * Call with a moniker string: inspectEntity("task:01JAB")
 */
export function useInspect(): InspectFn {
  const ctx = useContext(InspectContext);
  if (!ctx) throw new Error("useInspect must be used within an InspectProvider");
  return ctx.inspect;
}

/**
 * Returns a dismiss function that closes the topmost inspector panel.
 * Returns true if a panel was actually closed, false if stack was empty.
 */
export function useInspectDismiss(): DismissFn {
  const ctx = useContext(InspectContext);
  if (!ctx) throw new Error("useInspectDismiss must be used within an InspectProvider");
  return ctx.dismiss;
}
