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

import {
  forwardRef,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { parser as filterParser } from "@/lang-filter/parser";
import { filterLanguage } from "@/lang-filter";
import { X } from "lucide-react";
import { EditorView } from "@codemirror/view";
import { pickedCompletion } from "@codemirror/autocomplete";
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
 * Debounced timer with cancel, flush, and unmount flush-cleanup.
 *
 * - `schedule(fn, delayMs)` — (re)start the debounce with a new callback.
 * - `cancel()` — drop any pending callback without invoking it (used by the
 *   clear-button path where the filter is being replaced, so running the
 *   stale pending callback would clobber the clear).
 * - `flush()` — if a timer is pending, clear it and invoke the stored
 *   callback synchronously. Used to commit a pending save when the user
 *   signals commit (completion accept) or the component unmounts.
 *
 * On unmount, `flush` is called (not `cancel`) so that a pending autosave
 * still fires — otherwise React reconciliation can silently drop a save
 * scheduled just before the component is keyed away (e.g. perspective toggle).
 */
function useDebouncedTimer() {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingFnRef = useRef<(() => void) | null>(null);

  const cancel = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    pendingFnRef.current = null;
  }, []);

  const flush = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const fn = pendingFnRef.current;
    pendingFnRef.current = null;
    if (fn) fn();
  }, []);

  // Flush (not cancel) on unmount so a pending autosave still fires when the
  // component is unmounted before the debounce elapses.
  useEffect(() => flush, [flush]);

  const schedule = useCallback(
    (fn: () => void, delayMs: number) => {
      if (timerRef.current !== null) clearTimeout(timerRef.current);
      pendingFnRef.current = fn;
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        pendingFnRef.current = null;
        fn();
      }, delayMs);
    },
    [],
  );

  return { schedule, cancel, flush };
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
  const { schedule, cancel, flush } = useDebouncedTimer();

  const apply = useCallback(
    (text: string) =>
      applyFilter(
        text,
        perspectiveId,
        dispatchFilter,
        dispatchClearFilter,
        setError,
      ),
    [perspectiveId, dispatchFilter, dispatchClearFilter],
  );

  /** Debounced autosave — called on every doc change and vim save-in-place. */
  const handleChange = useCallback(
    (text: string) => schedule(() => apply(text), AUTOSAVE_DELAY_MS),
    [schedule, apply],
  );

  /** Immediate commit (Enter key) — cancels debounce, dispatches now, closes. */
  const handleCommit = useCallback(
    (text: string) => {
      cancel();
      apply(text);
      onClose();
    },
    [cancel, apply, onClose],
  );

  const handleCancel = useCallback(() => onClose(), [onClose]);

  /** Clear button — cancels debounce, clears filter immediately, closes. */
  const handleClear = useCallback(() => {
    cancel();
    dispatchClearFilter({ args: { perspective_id: perspectiveId } }).catch(
      console.error,
    );
    onClose();
  }, [perspectiveId, dispatchClearFilter, cancel, onClose]);

  /**
   * Flush any pending debounced save immediately.
   *
   * Used by the completion-accept updateListener so that picking a tag from
   * the autocomplete dropdown commits the filter without waiting for the
   * 300ms debounce (which may never fire if the user then toggles perspective).
   */
  const handleFlush = useCallback(() => flush(), [flush]);

  return {
    error,
    handleCommit,
    handleCancel,
    handleChange,
    handleClear,
    handleFlush,
  };
}

/**
 * Build a CM6 extension that flushes the debounced autosave whenever a
 * transaction carrying the `pickedCompletion` annotation is applied.
 *
 * The flush is scheduled via `queueMicrotask` so it runs *after* the change
 * extension's own updateListener (which schedules the debounced save for the
 * just-inserted text). Net effect: completion accept → schedule save for the
 * new text → immediately flush it → synchronous dispatch.
 *
 * Specific to the formula-bar autosave model. Other mention-autocomplete
 * consumers (task description, etc.) save via explicit commit and must not
 * get this immediate-dispatch behavior, so this extension is intentionally
 * NOT added to `useMentionExtensions`.
 */
function buildCompletionFlushExtension(flush: () => void) {
  return EditorView.updateListener.of((update) => {
    for (const tr of update.transactions) {
      if (tr.annotation(pickedCompletion)) {
        queueMicrotask(flush);
        return;
      }
    }
  });
}

/**
 * Compose the CM6 extensions passed to the formula-bar editor.
 *
 * Combines the shared mention-autocomplete extensions (tags, virtual tags,
 * filter sigils) with a local flush-on-completion-accept extension. The flush
 * extension is intentionally scoped to this hook so other mention-autocomplete
 * consumers (task description, etc.) don't inherit immediate-dispatch behavior.
 */
function useFilterEditorExtensions(handleFlush: () => void) {
  const mentionExts = useMentionExtensions({
    includeVirtualTags: true,
    includeFilterSigils: true,
  });
  const completionFlushExt = useMemo(
    () => buildCompletionFlushExtension(handleFlush),
    [handleFlush],
  );
  return useMemo(
    () => [...mentionExts, completionFlushExt],
    [mentionExts, completionFlushExt],
  );
}

/** Clear-filter button (×) shown when the formula bar has a non-empty filter. */
function ClearFilterButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="inline-flex items-center justify-center h-4 w-4 shrink-0 text-muted-foreground hover:text-foreground transition-colors"
      aria-label="Clear filter"
    >
      <X className="h-3 w-3" />
    </button>
  );
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
    const {
      error,
      handleCommit,
      handleCancel,
      handleChange,
      handleClear,
      handleFlush,
    } = useFilterDispatch(perspectiveId, onClose);
    const extraExtensions = useFilterEditorExtensions(handleFlush);

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
            placeholder="Filter… e.g. #bug @alice $spatial-nav"
            singleLine
            autoFocus={false}
            repeatable
            languageExtension={filterLanguage()}
            extraExtensions={extraExtensions}
          />
        </div>
        {filter && <ClearFilterButton onClick={handleClear} />}
      </div>
    );
  },
);
