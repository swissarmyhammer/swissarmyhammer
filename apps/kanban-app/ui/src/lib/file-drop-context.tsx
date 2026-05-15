/**
 * File drop context — HTML5 drag events + save_dropped_file Tauri command.
 *
 * All windows have dragDropEnabled:false so Tauri's native handler is off.
 * File drops use HTML5 dataTransfer.files → save_dropped_file (writes to
 * temp file) → callback receives temp path → existing attachment copy.
 * Task card drag uses HTML5 DnD natively. No conflict.
 *
 * LIFO callback stack: most recently registered target receives drops.
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
import { invoke } from "@tauri-apps/api/core";

/** Callback receives temp file paths after save_dropped_file. */
export type DropCallback = (paths: string[]) => void;

interface FileDropContextValue {
  isDragging: boolean;
  paths: string[] | null;
  registerDropTarget: (callback: DropCallback) => void;
  unregisterDropTarget: (callback: DropCallback) => void;
}

const FileDropContext = createContext<FileDropContextValue>({
  isDragging: false,
  paths: null,
  registerDropTarget: () => {},
  unregisterDropTarget: () => {},
});

/** Hook to access the file drop context. */
export function useFileDrop(): FileDropContextValue {
  return useContext(FileDropContext);
}

interface FileDropProviderProps {
  children: ReactNode;
  _testOverride?: { isDragging: boolean };
}

/**
 * Provider that handles file drops via HTML5 drag events.
 *
 * Prevents browser navigation for file drags while letting task card
 * drags pass through to DropZone components.
 */
export function FileDropProvider({
  children,
  _testOverride,
}: FileDropProviderProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [paths, setPaths] = useState<string[] | null>(null);
  const callbackStackRef = useRef<DropCallback[]>([]);
  const dragCounterRef = useRef(0);

  useEffect(() => {
    const handleDragEnter = (e: DragEvent) => {
      if (!e.dataTransfer?.types.includes("Files")) return;
      e.preventDefault();
      dragCounterRef.current++;
      if (dragCounterRef.current === 1) {
        setIsDragging(true);
        const names: string[] = [];
        if (e.dataTransfer.items) {
          for (const item of e.dataTransfer.items) {
            if (item.kind === "file") {
              const file = item.getAsFile();
              if (file) names.push(file.name);
            }
          }
        }
        setPaths(names.length > 0 ? names : null);
      }
    };

    const handleDragLeave = (e: DragEvent) => {
      if (!e.dataTransfer?.types.includes("Files")) return;
      dragCounterRef.current--;
      if (dragCounterRef.current <= 0) {
        dragCounterRef.current = 0;
        setIsDragging(false);
        setPaths(null);
      }
    };

    const handleDragOver = (e: DragEvent) => {
      if (e.dataTransfer?.types.includes("Files")) e.preventDefault();
    };

    const handleDrop = async (e: DragEvent) => {
      if (!e.dataTransfer?.types.includes("Files")) return;
      e.preventDefault();
      dragCounterRef.current = 0;
      setIsDragging(false);
      setPaths(null);

      const stack = callbackStackRef.current;
      const cb = stack.length > 0 ? stack[stack.length - 1] : null;
      if (!cb || !e.dataTransfer.files.length) return;

      const tempPaths: string[] = [];
      for (const file of e.dataTransfer.files) {
        try {
          const buffer = await file.arrayBuffer();
          const path = await invoke<string>("save_dropped_file", {
            filename: file.name,
            data: Array.from(new Uint8Array(buffer)),
          });
          tempPaths.push(path);
        } catch (err) {
          console.error("save_dropped_file failed:", err);
        }
      }
      if (tempPaths.length > 0) cb(tempPaths);
    };

    document.addEventListener("dragenter", handleDragEnter);
    document.addEventListener("dragleave", handleDragLeave);
    document.addEventListener("dragover", handleDragOver);
    document.addEventListener("drop", handleDrop);
    return () => {
      document.removeEventListener("dragenter", handleDragEnter);
      document.removeEventListener("dragleave", handleDragLeave);
      document.removeEventListener("dragover", handleDragOver);
      document.removeEventListener("drop", handleDrop);
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
