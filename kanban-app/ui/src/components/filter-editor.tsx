/**
 * Inline filter expression editor for the perspective formula bar.
 *
 * Wraps `TextEditor` with the filter DSL language and mention autocomplete,
 * layering validation and command dispatch on top. Exposes a `focus()` handle
 * via forwardRef so the parent can focus the editor programmatically (e.g. when
 * the filter button on the active tab is clicked).
 *
 * The filter expression uses the kanban filter DSL (`#tag && @user || !#done`).
 * Invalid expressions show a subtle error indicator inline. Clear button (×)
 * removes the filter via `perspective.clearFilter`.
 */

import { forwardRef, useCallback, useEffect, useRef, useState } from "react";
import { parser as filterParser } from "@/lang-filter/parser";
import { filterLanguage } from "@/lang-filter";
import { X } from "lucide-react";
import { useDispatchCommand } from "@/lib/command-scope";
import { useMentionExtensions } from "@/hooks/use-mention-extensions";
import { cn } from "@/lib/utils";
import {
  TextEditor,
  type TextEditorHandle,
} from "@/components/fields/text-editor";

/** Handle exposed via forwardRef so parents can programmatically focus the editor. */
export type FilterEditorHandle = TextEditorHandle;

/** Props for the FilterEditor component embedded in the perspective formula bar. */
interface FilterEditorProps {
  /** Current filter expression from the perspective. */
  filter: string;
  /** Perspective ID to update. */
  perspectiveId: string;
  /**
   * Called after a successful submit or clear.
   *
   * Optional — the formula bar usage has no popover to close, so this
   * defaults to a no-op.
   */
  onClose?: () => void;
}

/**
 * Validate a filter expression by parsing it with the Lezer grammar.
 *
 * Walks the parse tree looking for error nodes. Returns null if the expression
 * is valid, or an error message string describing the problem.
 */
function validateFilter(expression: string): string | null {
  if (!expression.trim()) return null;
  const tree = filterParser.parse(expression);
  let error: string | null = null;
  tree.iterate({
    enter(node) {
      if (node.type.isError && !error) {
        error = "Invalid filter expression";
      }
    },
  });
  return error;
}

/** Debounce delay for filter autosave, in milliseconds. */
const AUTOSAVE_DELAY_MS = 300;

/**
 * Validates and dispatches a filter expression.
 *
 * Empty text clears the filter; invalid text sets the error without dispatching;
 * valid text dispatches `perspective.filter`. Returns true if dispatch occurred
 * (or cleared), false if validation failed.
 */
function applyFilter(
  text: string,
  perspectiveId: string,
  dispatchFilter: (opts: { args: Record<string, unknown> }) => Promise<unknown>,
  dispatchClearFilter: (opts: {
    args: Record<string, unknown>;
  }) => Promise<unknown>,
  setError: (err: string | null) => void,
): boolean {
  const trimmed = text.trim();
  if (!trimmed) {
    setError(null);
    dispatchClearFilter({ args: { perspective_id: perspectiveId } }).catch(
      console.error,
    );
    return true;
  }
  const err = validateFilter(trimmed);
  if (err) {
    setError(err);
    return false;
  }
  setError(null);
  dispatchFilter({
    args: { filter: trimmed, perspective_id: perspectiveId },
  }).catch(console.error);
  return true;
}

/**
 * Manages filter dispatch, validation, error state, and debounced autosave.
 *
 * Typing auto-applies the filter after a short debounce. Enter commits
 * immediately and calls onClose. The clear button cancels any pending
 * debounce and dispatches clearFilter. Vim Esc from insert mode triggers
 * onChange (via TextEditor's saveInPlace) which feeds through the same
 * debounced autosave path.
 */
function useFilterDispatch(perspectiveId: string, onClose: () => void) {
  const dispatchFilter = useDispatchCommand("perspective.filter");
  const dispatchClearFilter = useDispatchCommand("perspective.clearFilter");
  const [error, setError] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  /** Cancel any pending debounced save. */
  const cancelPending = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  // Clean up timer on unmount.
  useEffect(() => cancelPending, [cancelPending]);

  /** Debounced autosave — called on every doc change and vim save-in-place. */
  const handleChange = useCallback(
    (text: string) => {
      cancelPending();
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        applyFilter(
          text,
          perspectiveId,
          dispatchFilter,
          dispatchClearFilter,
          setError,
        );
      }, AUTOSAVE_DELAY_MS);
    },
    [perspectiveId, dispatchFilter, dispatchClearFilter, cancelPending],
  );

  /** Immediate commit (Enter key) — cancels debounce, dispatches now, closes. */
  const handleCommit = useCallback(
    (text: string) => {
      cancelPending();
      applyFilter(
        text,
        perspectiveId,
        dispatchFilter,
        dispatchClearFilter,
        setError,
      );
      onClose();
    },
    [
      perspectiveId,
      dispatchFilter,
      dispatchClearFilter,
      cancelPending,
      onClose,
    ],
  );

  const handleCancel = useCallback(() => {
    onClose();
  }, [onClose]);

  /** Clear button — cancels debounce, clears filter immediately, closes. */
  const handleClear = useCallback(() => {
    cancelPending();
    dispatchClearFilter({ args: { perspective_id: perspectiveId } }).catch(
      console.error,
    );
    onClose();
  }, [perspectiveId, dispatchClearFilter, cancelPending, onClose]);

  return { error, handleCommit, handleCancel, handleChange, handleClear };
}

/**
 * CM6 editor for editing perspective filter DSL expressions.
 *
 * Renders as a borderless inline editor suitable for embedding in the
 * perspective tab bar formula bar. Uses `TextEditor` with `filterLanguage()`
 * so it shares all keymap, vim mode, and extension infrastructure. Exposes
 * `focus()` via forwardRef so the parent can focus the editor without managing
 * internal refs.
 *
 * - Enter saves the filter via `perspective.filter` command
 * - Escape cancels (no-op in formula bar context)
 * - Clear button (×) removes the filter via `perspective.clearFilter`
 */
export const FilterEditor = forwardRef<FilterEditorHandle, FilterEditorProps>(
  function FilterEditor({ filter, perspectiveId, onClose = () => {} }, ref) {
    const mentionExts = useMentionExtensions({
      includeVirtualTags: true,
      includeFilterSigils: true,
    });
    const { error, handleCommit, handleCancel, handleChange, handleClear } =
      useFilterDispatch(perspectiveId, onClose);

    return (
      <div
        data-testid="filter-editor"
        className={cn(
          "flex items-center gap-1 flex-1 min-w-0",
          error && "text-destructive",
        )}
      >
        <div className="flex-1 min-w-0">
          <TextEditor
            ref={ref}
            value={filter}
            onCommit={handleCommit}
            onCancel={handleCancel}
            onChange={handleChange}
            placeholder="Filter… e.g. #bug @alice"
            singleLine
            autoFocus={false}
            repeatable
            languageExtension={filterLanguage()}
            extraExtensions={mentionExts}
          />
        </div>
        {filter && (
          <button
            onClick={handleClear}
            className="inline-flex items-center justify-center h-4 w-4 shrink-0 text-muted-foreground hover:text-foreground transition-colors"
            aria-label="Clear filter"
          >
            <X className="h-3 w-3" />
          </button>
        )}
      </div>
    );
  },
);
