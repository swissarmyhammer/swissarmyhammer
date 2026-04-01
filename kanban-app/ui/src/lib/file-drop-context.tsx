/**
 * File drop context — provides global drag-drop prevention and routes
 * Tauri native file drops to a registered callback.
 *
 * Listens to `getCurrentWebview().onDragDropEvent()` for native file paths
 * and prevents the browser default dragover/drop behavior so files dragged
 * from Finder never take over the webview.
 *
 * Uses a stack-based (LIFO) model: multiple components can register as drop
 * targets simultaneously. The most recently registered callback (top of stack)
 * receives drops. `unregisterDropTarget` removes by reference, so unmounting
 * a component correctly restores the previous target.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/** Callback type for drop target registration. */
export type DropCallback = (paths: string[]) => void;

interface FileDropContextValue {
  /** Whether a file drag is currently over the window. */
  isDragging: boolean;
  /** The paths being dragged (available on enter), null when not dragging. */
  paths: string[] | null;
  /** Push a callback onto the drop target stack. The top of stack receives drops. */
  registerDropTarget: (callback: DropCallback) => void;
  /** Remove a callback from the stack by reference. */
  unregisterDropTarget: (callback: DropCallback) => void;
}

const FileDropContext = createContext<FileDropContextValue>({
  isDragging: false,
  paths: null,
  registerDropTarget: () => {},
  unregisterDropTarget: () => {},
});

/**
 * Hook to access the file drop context.
 *
 * @returns The current file drop state and registration functions
 */
export function useFileDrop(): FileDropContextValue {
  return useContext(FileDropContext);
}

/** Props for FileDropProvider, including an optional test override. */
interface FileDropProviderProps {
  children: ReactNode;
  /** Test-only override for isDragging state. Do not use in production. */
  _testOverride?: { isDragging: boolean };
}

/**
 * Provider that manages global file drop prevention and Tauri drag-drop events.
 *
 * Wrap the app in this provider to:
 * 1. Prevent the browser from opening dropped files
 * 2. Route file drops to registered attachment editors
 */
export function FileDropProvider({
  children,
  _testOverride,
}: FileDropProviderProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [paths, setPaths] = useState<string[] | null>(null);
  const callbackStackRef = useRef<DropCallback[]>([]);

  // Prevent browser default drag-drop behavior globally
  useEffect(() => {
    const preventDragOver = (e: DragEvent) => e.preventDefault();
    const preventDrop = (e: DragEvent) => e.preventDefault();
    document.addEventListener("dragover", preventDragOver);
    document.addEventListener("drop", preventDrop);
    return () => {
      document.removeEventListener("dragover", preventDragOver);
      document.removeEventListener("drop", preventDrop);
    };
  }, []);

  // Listen to Tauri native drag-drop events
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;

        if (payload.type === "enter") {
          setIsDragging(true);
          setPaths(payload.paths ?? null);
        } else if (payload.type === "leave") {
          setIsDragging(false);
          setPaths(null);
        } else if (payload.type === "drop") {
          const droppedPaths = payload.paths ?? [];
          const stack = callbackStackRef.current;
          const cb = stack.length > 0 ? stack[stack.length - 1] : null;
          if (cb && droppedPaths.length > 0) {
            cb(droppedPaths);
          }
          setIsDragging(false);
          setPaths(null);
        }
        // "over" events are ignored — we only need enter/leave/drop
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, []);

  const registerDropTarget = useCallback((callback: DropCallback) => {
    callbackStackRef.current = [...callbackStackRef.current, callback];
  }, []);

  const unregisterDropTarget = useCallback((callback: DropCallback) => {
    const idx = callbackStackRef.current.lastIndexOf(callback);
    if (idx !== -1) {
      const next = [...callbackStackRef.current];
      next.splice(idx, 1);
      callbackStackRef.current = next;
    }
  }, []);

  // Allow test overrides for isDragging
  const effectiveIsDragging = _testOverride
    ? _testOverride.isDragging
    : isDragging;

  return (
    <FileDropContext.Provider
      value={{
        isDragging: effectiveIsDragging,
        paths,
        registerDropTarget,
        unregisterDropTarget,
      }}
    >
      {children}
    </FileDropContext.Provider>
  );
}
