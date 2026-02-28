import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type KeymapMode = "cua" | "vim" | "emacs";

interface KeymapContextValue {
  mode: KeymapMode;
  setMode: (mode: KeymapMode) => void;
}

const KeymapContext = createContext<KeymapContextValue>({
  mode: "cua",
  setMode: () => {},
});

function isValidMode(v: unknown): v is KeymapMode {
  return v === "cua" || v === "vim" || v === "emacs";
}

export function KeymapProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<KeymapMode>("cua");

  // Read initial mode from Tauri backend
  useEffect(() => {
    invoke<string>("get_keymap_mode").then((m) => {
      if (isValidMode(m)) setModeState(m);
    }).catch(() => {
      // Fallback: stay on default "cua"
    });
  }, []);

  // Listen for keymap changes from the native menu
  useEffect(() => {
    const unlisten = listen<string>("keymap-changed", (event) => {
      if (isValidMode(event.payload)) {
        setModeState(event.payload);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // When the user changes mode from the UI (nav bar), persist via Tauri command
  const setMode = useCallback((next: KeymapMode) => {
    setModeState(next);
    invoke("set_keymap_mode", { mode: next }).catch(() => {
      // ignore — UI already updated optimistically
    });
  }, []);

  return (
    <KeymapContext.Provider value={{ mode, setMode }}>
      {children}
    </KeymapContext.Provider>
  );
}

export function useKeymap(): KeymapContextValue {
  return useContext(KeymapContext);
}
