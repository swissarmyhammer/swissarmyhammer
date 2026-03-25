import {
  createContext,
  useContext,
  useRef,
  useMemo,
  type ReactNode,
} from "react";

/**
 * Stable, ref-backed actions for board cursor navigation.
 *
 * ColumnView reads from this context instead of receiving callback props,
 * eliminating unstable function references that would defeat React.memo.
 */
interface BoardNavActions {
  /** A card was clicked — set cursor to (columnId, cardIndex). */
  onCardClick: (columnId: string, cardIndex: number) => void;
  /** A column header was clicked — set cursor to (columnId, -1). */
  onHeaderClick: (columnId: string) => void;
  /** A card was double-clicked — set cursor and open inspector. */
  onCardDoubleClick: (columnId: string, cardIndex: number) => void;
}

const BoardNavContext = createContext<BoardNavActions | null>(null);

interface BoardNavProviderProps {
  onCardClick: (columnId: string, cardIndex: number) => void;
  onHeaderClick: (columnId: string) => void;
  onCardDoubleClick: (columnId: string, cardIndex: number) => void;
  children: ReactNode;
}

/**
 * Provides stable board navigation actions to ColumnView descendants.
 *
 * The context value is backed by refs so it never changes identity,
 * which means consuming components never re-render due to context changes.
 */
export function BoardNavProvider({
  onCardClick,
  onHeaderClick,
  onCardDoubleClick,
  children,
}: BoardNavProviderProps) {
  const cardClickRef = useRef(onCardClick);
  cardClickRef.current = onCardClick;
  const headerClickRef = useRef(onHeaderClick);
  headerClickRef.current = onHeaderClick;
  const dblClickRef = useRef(onCardDoubleClick);
  dblClickRef.current = onCardDoubleClick;

  // Stable value — never changes identity, delegates to refs at call time
  const value = useMemo<BoardNavActions>(
    () => ({
      onCardClick: (colId, idx) => cardClickRef.current(colId, idx),
      onHeaderClick: (colId) => headerClickRef.current(colId),
      onCardDoubleClick: (colId, idx) => dblClickRef.current(colId, idx),
    }),
    [],
  );

  return (
    <BoardNavContext.Provider value={value}>
      {children}
    </BoardNavContext.Provider>
  );
}

/**
 * Read board navigation actions from the nearest BoardNavProvider.
 *
 * Returns null when outside a BoardNavProvider (e.g., standalone ColumnView in tests).
 */
export function useBoardNavActions(): BoardNavActions | null {
  return useContext(BoardNavContext);
}
