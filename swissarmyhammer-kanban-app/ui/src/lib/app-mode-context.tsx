import { createContext, useContext, useState, type ReactNode } from "react";

/** The three application-level interaction modes. */
export type AppMode = "normal" | "command" | "search";

interface AppModeContextValue {
  /** The current interaction mode. */
  mode: AppMode;
  /** Switch to a different interaction mode. */
  setMode: (mode: AppMode) => void;
}

const AppModeContext = createContext<AppModeContextValue>({
  mode: "normal",
  setMode: () => {},
});

/**
 * Provides application-level interaction mode state to the component tree.
 *
 * Wraps children with an AppModeContext that tracks whether the app is in
 * normal, command, or search mode.
 */
export function AppModeProvider({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<AppMode>("normal");

  return (
    <AppModeContext.Provider value={{ mode, setMode }}>
      {children}
    </AppModeContext.Provider>
  );
}

/**
 * Returns the current app mode and a setter to change it.
 *
 * Must be used within an AppModeProvider.
 */
export function useAppMode(): AppModeContextValue {
  return useContext(AppModeContext);
}
