import { createContext, useContext, useCallback, type ReactNode } from "react";
import { parseMoniker } from "@/lib/moniker";

type InspectFn = (moniker: string) => void;

const InspectContext = createContext<InspectFn | null>(null);

interface InspectProviderProps {
  /** Called with (entityType, entityId) parsed from the moniker. */
  onInspect: (entityType: string, entityId: string) => void;
  children: ReactNode;
}

/**
 * Provides an inspect function that accepts a moniker string.
 * Parses the moniker and delegates to onInspect(type, id).
 */
export function InspectProvider({ onInspect, children }: InspectProviderProps) {
  const inspect = useCallback(
    (m: string) => {
      const { type, id } = parseMoniker(m);
      onInspect(type, id);
    },
    [onInspect],
  );

  return (
    <InspectContext.Provider value={inspect}>
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
  return ctx;
}
