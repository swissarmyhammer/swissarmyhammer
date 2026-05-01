/**
 * Inline filter expression editor for the perspective formula bar.
 *
 * Wraps the pure {@link TextEditor} primitive with the filter DSL language,
 * mention autocomplete, debounced autosave, Enter-flush, completion-accept
 * flush, and clear-button handling. The formula bar is always-open and
 * always-live: there is no popover, no draft, no commit-and-close semantics.
 * Typing schedules a debounced save; Enter hurries it along; nothing else
 * about the editor changes on Enter.
 *
 * Exposes a `focus()` handle via forwardRef so the parent can focus the
 * editor programmatically (e.g. when the filter button on the active tab is
 * clicked).
 */

import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import { parser as filterParser } from "@/lang-filter/parser";
import { filterLanguage } from "@/lang-filter";
import { X } from "lucide-react";
import { EditorView } from "@codemirror/view";
import { pickedCompletion } from "@codemirror/autocomplete";
import { CommandScopeProvider, useDispatchCommand } from "@/lib/command-scope";
import { useUIState } from "@/lib/ui-state-context";
import { useMentionExtensions } from "@/hooks/use-mention-extensions";
import { cn } from "@/lib/utils";
import {
  TextEditor,
  type TextEditorHandle,
} from "@/components/fields/text-editor";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";

/** Handle exposed via forwardRef so parents can programmatically focus the editor. */
export type FilterEditorHandle = TextEditorHandle;

/** Props for the FilterEditor component embedded in the perspective formula bar. */
interface FilterEditorProps {
  /** Current filter expression from the perspective. */
  filter: string;
  /** Perspective ID to update. */
  perspectiveId: string;
  /**
   * Called when the user explicitly dismisses the editor (Escape or clear ×).
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
 *
 * Also stamps `lastDispatchedRef.current` with the effective value we send —
 * empty string on clear, trimmed text on set. The reconciliation effect in
 * `FilterEditorBody` compares incoming `filter` props against this stamp to
 * decide whether a prop change is the echo of our own dispatch (no-op) or a
 * genuinely external mutation (reset the buffer).
 */
function applyFilter(
  text: string,
  perspectiveId: string,
  dispatchFilter: (opts: { args: Record<string, unknown> }) => Promise<unknown>,
  dispatchClearFilter: (opts: {
    args: Record<string, unknown>;
  }) => Promise<unknown>,
  setError: (err: string | null) => void,
  lastDispatchedRef: { current: string },
): boolean {
  const trimmed = text.trim();
  if (!trimmed) {
    console.warn("[filter-diag] applyFilter clear", { perspectiveId });
    setError(null);
    lastDispatchedRef.current = "";
    dispatchClearFilter({ args: { perspective_id: perspectiveId } }).catch(
      console.error,
    );
    return true;
  }
  // The buffer is the source of truth. Always dispatch, even if the parser
  // rejects the expression — intermediate edit states are frequently invalid
  // (trailing operator, half-typed tag) and silently refusing to save would
  // desync the saved filter from what the user sees. The error is surfaced
  // to the editor via `setError` for visual indication only.
  const err = validateFilter(trimmed);
  setError(err);
  console.warn("[filter-diag] applyFilter DISPATCH", {
    perspectiveId,
    filter: trimmed,
    validationError: err,
  });
  lastDispatchedRef.current = trimmed;
  dispatchFilter({
    args: { filter: trimmed, perspective_id: perspectiveId },
  })
    .then(() =>
      console.warn("[filter-diag] applyFilter DISPATCH_RESOLVED", {
        perspectiveId,
        filter: trimmed,
      }),
    )
    .catch((e) => {
      console.warn("[filter-diag] applyFilter DISPATCH_FAILED", {
        perspectiveId,
        filter: trimmed,
        error: String(e),
      });
    });
  return true;
}

/**
 * Debounced timer with cancel, flush, and unmount flush-cleanup.
 *
 * - `schedule(fn, delayMs)` — (re)start the debounce with a new callback.
 * - `cancel()` — drop any pending callback without invoking it.
 * - `flush()` — if a timer is pending, clear it and invoke the stored callback
 *   synchronously. Used to commit a pending save on Enter, on completion
 *   accept, or on unmount.
 *
 * On unmount, `flush` is called (not `cancel`) so that a pending autosave
 * still fires — otherwise React reconciliation can silently drop a save
 * scheduled just before the component is keyed away (e.g. perspective toggle).
 */
function useDebouncedTimer() {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingFnRef = useRef<(() => void) | null>(null);

  const cancel = useCallback(() => {
    console.warn("[filter-diag] debounce CANCEL", {
      hadPending: pendingFnRef.current !== null,
    });
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

  // Flush (not cancel) on unmount so a pending autosave still fires.
  useEffect(() => flush, [flush]);

  const schedule = useCallback((fn: () => void, delayMs: number) => {
    if (timerRef.current !== null) clearTimeout(timerRef.current);
    pendingFnRef.current = fn;
    timerRef.current = setTimeout(() => {
      timerRef.current = null;
      pendingFnRef.current = null;
      fn();
    }, delayMs);
  }, []);

  return { schedule, cancel, flush };
}

/**
 * Build a CM6 extension that flushes the debounced autosave whenever a
 * transaction carrying the `pickedCompletion` annotation is applied.
 *
 * The flush is scheduled via `queueMicrotask` so it runs *after* the change
 * extension's own updateListener (which schedules the debounced save for the
 * just-inserted text). Net effect: completion accept → schedule save → flush
 * it synchronously. Reads the flush function through a ref so the extension
 * identity stays stable.
 */
function buildCompletionFlushExtension(flushRef: { current: () => void }) {
  return EditorView.updateListener.of((update) => {
    for (const tr of update.transactions) {
      if (tr.annotation(pickedCompletion)) {
        queueMicrotask(() => flushRef.current?.());
        return;
      }
    }
  });
}

/**
 * Builds the filter-apply function in a ref and returns it alongside error state.
 *
 * The ref prevents stale captures inside extension-hosted callbacks when
 * dispatch identities churn across renders.
 *
 * Also exposes `lastDispatchedRef` — the effective filter value this editor
 * most recently dispatched (via autosave, Enter flush, completion flush, or
 * the inline clear button). `FilterEditorBody` reads this ref in its
 * prop-to-buffer reconciliation effect to distinguish echoes of our own
 * dispatches (no-op) from genuinely external mutations (reset the CM6 buffer).
 */
function useApplyFilterRef(perspectiveId: string) {
  const dispatchFilter = useDispatchCommand("perspective.filter");
  const dispatchClearFilter = useDispatchCommand("perspective.clearFilter");
  const [error, setError] = useState<string | null>(null);
  const lastDispatchedRef = useRef<string>("");

  const applyRef = useRef<(text: string) => boolean>(() => false);
  applyRef.current = (text: string) =>
    applyFilter(
      text,
      perspectiveId,
      dispatchFilter,
      dispatchClearFilter,
      setError,
      lastDispatchedRef,
    );

  return { applyRef, error, dispatchClearFilter, lastDispatchedRef };
}

/**
 * Manages filter dispatch, validation, error state, and debounced autosave.
 *
 * Typing auto-applies the filter after a short debounce. Enter flushes the
 * pending debounced save immediately (never destroys it) so completion-accept
 * transactions still in flight can land without being clobbered. The clear
 * button cancels any pending debounce and dispatches clearFilter.
 */
function useFilterDispatch(perspectiveId: string, onClose: () => void) {
  const { applyRef, error, dispatchClearFilter, lastDispatchedRef } =
    useApplyFilterRef(perspectiveId);
  const { schedule, cancel, flush } = useDebouncedTimer();

  const handleChange = useCallback(
    (text: string) => {
      console.warn("[filter-diag] handleChange", {
        perspectiveId,
        text,
        len: text.length,
      });
      // Suppress the scheduled apply when the new text matches the ref stamp.
      //
      // Two call sites stamp the ref BEFORE triggering a doc change that
      // re-enters this handler: the reconciliation effect below in
      // `FilterEditorBody` (for external mutations) and `handleClear` (for
      // the × button). In both cases the ensuing `onChange` would otherwise
      // schedule a redundant apply that dispatches the SAME value back to
      // the backend — round-tripping a single external mutation as two
      // undo-stack entries, or N-plicating across multi-window setups.
      //
      // Real keystrokes fall through because the ref still holds the
      // PREVIOUS dispatched value (e.g. ref="#bug" while the user types
      // "#bug a"), so the equality check fails and the debounce runs
      // normally. The trim mirrors `applyFilter`'s own canonicalisation so
      // whitespace-only variants (e.g. "#bug ") are treated as no-ops
      // exactly as `applyFilter` would.
      if (text.trim() === lastDispatchedRef.current) {
        console.warn("[filter-diag] handleChange SUPPRESS (ref match)", {
          perspectiveId,
          text,
        });
        return;
      }
      schedule(() => applyRef.current(text), AUTOSAVE_DELAY_MS);
    },
    [schedule, applyRef, perspectiveId, lastDispatchedRef],
  );

  // Enter handler: flush pending save synchronously. Must NOT cancel — that
  // would poison a save just scheduled by a completion-accept transaction
  // still propagating through update listeners. Does NOT close the formula bar.
  const handleFlush = useCallback(() => {
    console.warn("[filter-diag] handleFlush", { perspectiveId });
    flush();
  }, [flush, perspectiveId]);
  const handleCancel = useCallback(() => onClose(), [onClose]);

  /** Clear × button — cancels debounce, clears filter immediately, closes. */
  const handleClear = useCallback(() => {
    cancel();
    // Stamp the ref BEFORE dispatch so the backend's echoed entity-field
    // event (which re-renders us with filter="") is recognised as our own
    // and suppressed by the reconciliation effect in FilterEditorBody.
    lastDispatchedRef.current = "";
    dispatchClearFilter({ args: { perspective_id: perspectiveId } }).catch(
      console.error,
    );
    onClose();
  }, [perspectiveId, dispatchClearFilter, cancel, onClose, lastDispatchedRef]);

  return {
    error,
    handleFlush,
    handleCancel,
    handleChange,
    handleClear,
    lastDispatchedRef,
  };
}

/**
 * Compose the CM6 extensions passed to the formula-bar editor.
 *
 * Combines the shared mention-autocomplete extensions with a local flush-on-
 * completion-accept extension and Enter-flush / Escape-dismiss keymaps. The
 * flush extension is intentionally scoped to this hook so other mention-
 * autocomplete consumers don't inherit immediate-dispatch behavior.
 *
 * The Enter/Escape keymaps are built once per mode and use stable refs to the
 * flush and cancel callbacks — that keeps the extension array identity stable
 * across re-renders so the CM6 EditorView is not reconfigured mid-typing.
 */
function useFilterEditorExtensions(
  flushRef: { current: (() => void) | null },
  cancelRef: { current: (() => void) | null },
) {
  const { keymap_mode: mode } = useUIState();
  const mentionExts = useMentionExtensions({
    includeVirtualTags: true,
    includeFilterSigils: true,
  });
  const completionFlushExt = useMemo(
    () =>
      buildCompletionFlushExtension({
        current: () => flushRef.current?.(),
      }),
    [flushRef],
  );
  const submitCancelExts = useMemo(
    () =>
      buildSubmitCancelExtensions({
        mode,
        onSubmitRef: flushRef,
        onCancelRef: cancelRef,
        singleLine: true,
        alwaysSubmitOnEnter: true,
      }),
    [mode, flushRef, cancelRef],
  );
  return useMemo(
    () => [...mentionExts, completionFlushExt, ...submitCancelExts],
    [mentionExts, completionFlushExt, submitCancelExts],
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
 * perspective tab bar formula bar. Uses the pure {@link TextEditor} primitive
 * and wires its own debounced-autosave, Enter-flush, completion-flush, and
 * clear-button policy.
 */
/**
 * Inner editor body — assumes a `CommandScopeProvider` with the perspective
 * moniker is already in the tree above it, so every dispatch carries
 * `perspective:{id}` in its scope chain regardless of focus state.
 */
const FilterEditorBody = forwardRef<FilterEditorHandle, FilterEditorProps>(
  function FilterEditorBody(
    { filter, perspectiveId, onClose = () => {} },
    ref,
  ) {
    const {
      error,
      handleFlush,
      handleCancel,
      handleChange,
      handleClear,
      lastDispatchedRef,
    } = useFilterDispatch(perspectiveId, onClose);

    // Stable refs for extensions — callback identities churn, but ref
    // identities never do.
    const flushRef = useRef<(() => void) | null>(handleFlush);
    flushRef.current = handleFlush;
    const cancelRef = useRef<(() => void) | null>(handleCancel);
    cancelRef.current = handleCancel;

    const extensions = useFilterEditorExtensions(flushRef, cancelRef);
    const languageExtension = useMemo(() => filterLanguage(), []);

    // Inner ref so the clear button can imperatively empty the CM6 buffer.
    // Forward `focus`, `setValue`, and `getValue` out to the external consumer.
    const innerRef = useRef<TextEditorHandle>(null);
    useImperativeHandle(
      ref,
      () => ({
        focus() {
          innerRef.current?.focus();
        },
        setValue(text: string) {
          innerRef.current?.setValue(text);
        },
        getValue() {
          return innerRef.current?.getValue() ?? "";
        },
      }),
      [],
    );

    // ---------------------------------------------------------------------
    // External-change reconciliation
    // ---------------------------------------------------------------------
    // The TextEditor primitive captures `value` at mount and never reapplies
    // it (doc is the source of truth after mount). That is correct for the
    // keystroke-driven path — we must never echo a backend refresh back into
    // the buffer mid-typing or we would clobber characters in flight.
    //
    // But some mutations originate *outside* this editor: context-menu
    // "Clear Filter", command palette, keybindings, undo/redo of
    // `perspective.clearFilter`, or filter changes pushed from another
    // window. Those arrive as a `filter` prop change on the active
    // perspective, with no keystroke to drive the buffer. Without a
    // reconciliation path the CM6 buffer would continue to display stale
    // text even though the backend filter was cleared.
    //
    // The guards below respond ONLY to truly external changes:
    //
    //   1. `filter !== lastDispatchedRef.current` — our own dispatches
    //      (autosave, Enter flush, completion flush, × button) stamp the
    //      ref BEFORE dispatch, so the echoed prop matches and is ignored.
    //
    //   2. `filter !== innerRef.current?.getValue()` — if the user has
    //      typed ahead of the round-trip, the buffer already holds the
    //      newer text; resetting it here would clobber keystrokes in
    //      flight.
    //
    // When both guards pass we rewrite the buffer via the imperative
    // `setValue` handle — the same mechanism the inline × button uses.
    //
    // Adversarial edge case — filter flaps back to `lastDispatchedRef`
    // mid-typing: if an external source asserts the filter back to the
    // value we most recently dispatched (e.g. window B sets filter to
    // what window A last dispatched) while the user is typing in this
    // window, guard (1) trips and the effect deliberately no-ops. The
    // pending debounced save will later overwrite that external
    // assertion with whatever the user has typed. This is a defensible
    // trade-off (typing priority beats stale-but-equal-to-our-last-stamp
    // external assertions) and flows naturally from the two guards.
    useEffect(() => {
      const next = filter ?? "";
      if (next === lastDispatchedRef.current) return;
      if (next === innerRef.current?.getValue()) return;
      // Stamp first, then setValue. The setValue triggers onChange which
      // schedules a debounced applyFilter; that apply path also stamps the
      // ref to the same value and dispatches an idempotent echo. Stamping
      // up-front keeps the effect's own guards honest if React re-runs
      // before the debounce fires.
      lastDispatchedRef.current = next;
      innerRef.current?.setValue(next);
    }, [filter, lastDispatchedRef, perspectiveId]);

    const handleClearAndReset = useCallback(() => {
      handleClear();
      innerRef.current?.setValue("");
    }, [handleClear]);

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
            ref={innerRef}
            value={filter}
            onChange={handleChange}
            extensions={extensions}
            languageExtension={languageExtension}
            placeholder="Filter… e.g. #bug @alice $spatial-nav"
            singleLine
            autoFocus={false}
          />
        </div>
        {filter && <ClearFilterButton onClick={handleClearAndReset} />}
      </div>
    );
  },
);

/**
 * CM6 editor for editing perspective filter DSL expressions.
 *
 * Wraps {@link FilterEditorBody} in a `CommandScopeProvider` with moniker
 * `perspective:{id}` so every `perspective.filter` / `perspective.clearFilter`
 * dispatch carries that scope on its chain. Without this wrapper, dispatches
 * made while the editor has no focus (autocomplete pick, unmount-flush, etc.)
 * travel with a scope chain missing `perspective:` and the backend rejects
 * them with "command not available in current context".
 */
export const FilterEditor = forwardRef<FilterEditorHandle, FilterEditorProps>(
  function FilterEditor(props, ref) {
    return (
      <CommandScopeProvider moniker={`perspective:${props.perspectiveId}`}>
        <FilterEditorBody ref={ref} {...props} />
      </CommandScopeProvider>
    );
  },
);
