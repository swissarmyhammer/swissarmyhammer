import {
  createContext,
  useCallback,
  useContext,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { UndoStack, type UndoableCommand } from "./undo-stack";

interface UndoStackContextValue {
  /** Execute a command and push it onto the undo stack. */
  push: (cmd: UndoableCommand) => Promise<void>;
  /** Undo the most recently executed command. No-op if nothing to undo. */
  undo: () => Promise<void>;
  /** Redo the most recently undone command. No-op if nothing to redo. */
  redo: () => Promise<void>;
  /** Whether there is at least one command that can be undone. */
  canUndo: boolean;
  /** Whether there is at least one command that can be redone. */
  canRedo: boolean;
}

const UndoStackContext = createContext<UndoStackContextValue>({
  push: async () => {},
  undo: async () => {},
  redo: async () => {},
  canUndo: false,
  canRedo: false,
});

/**
 * Provides an UndoStack instance to the component tree.
 *
 * Holds a single UndoStack in a ref so the instance persists across renders.
 * A counter state is bumped after every mutating operation to trigger
 * re-renders so that `canUndo` and `canRedo` stay current.
 */
export function UndoStackProvider({ children }: { children: ReactNode }) {
  const stackRef = useRef(new UndoStack());
  // Bump this counter after every mutation to force consumers to re-render
  // with fresh canUndo/canRedo values.
  const [, setRevision] = useState(0);
  const bump = useCallback(() => setRevision((r) => r + 1), []);

  const push = useCallback(
    async (cmd: UndoableCommand) => {
      await stackRef.current.push(cmd);
      bump();
    },
    [bump],
  );

  const undo = useCallback(async () => {
    await stackRef.current.undo();
    bump();
  }, [bump]);

  const redo = useCallback(async () => {
    await stackRef.current.redo();
    bump();
  }, [bump]);

  const value: UndoStackContextValue = {
    push,
    undo,
    redo,
    canUndo: stackRef.current.canUndo,
    canRedo: stackRef.current.canRedo,
  };

  return (
    <UndoStackContext.Provider value={value}>
      {children}
    </UndoStackContext.Provider>
  );
}

/**
 * Returns the undo stack operations and state flags.
 *
 * Must be used within an UndoStackProvider.
 */
export function useUndoStack(): UndoStackContextValue {
  return useContext(UndoStackContext);
}
