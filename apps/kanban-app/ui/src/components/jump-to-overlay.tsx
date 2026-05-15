/**
 * `<JumpToOverlay>` — visual + interactive overlay for the Jump-To
 * (vim-sneak / AceJump-style) feature.
 *
 * On open, the overlay enumerates every `<FocusScope>` registered in the
 * layer that contains the user's prior focus, generates short prefix-free
 * codes via the Rust kernel ({@link generateSneakCodes}), and paints one
 * label pill per scope at its on-screen rect. Typing a unique code
 * dispatches `setFocus` to the matching scope and closes; typing a non-
 * matching letter flashes red, restores prior focus, and closes; Escape
 * flows through `nav.drillOut → app.dismiss` and lands on the sentinel's
 * `app.dismiss` shadow command, which restores prior focus and closes.
 *
 * # Architecture: claim focus into the overlay layer so Escape cascades
 *
 * The overlay mounts its own `<FocusLayer name="jump-to">` containing one
 * sentinel `<FocusScope>`. On open, focus is claimed on the sentinel —
 * critical so `nav.drillOut` walks the jump-to layer's chain (sentinel →
 * layer-root edge → no descent → fall through to `app.dismiss`) rather
 * than the user's prior focus chain.
 *
 * The sentinel's `commands` prop registers an `app.dismiss` shadow whose
 * `execute` calls the overlay's `handleDismiss` — restore prior focus,
 * then `onClose`. This is the same shadow pattern the inspector layer
 * uses (`ui.inspector.close`) but keyed on `app.dismiss` because the
 * overlay has no "first close just one panel" notion.
 *
 * # Dismiss paths
 *
 * | Trigger                          | Path                                                              | Focus result        |
 * |---------------------------------|--------------------------------------------------------------------|---------------------|
 * | `Escape`                        | global keymap → `nav.drillOut` → no descent → `app.dismiss` →     | restore prior focus |
 * |                                 | sentinel shadow → `handleDismiss`                                  |                     |
 * | Backdrop click                  | backdrop `onClick={handleDismiss}`                                 | restore prior focus |
 * | Letter not extending any prefix | overlay's keydown → 150ms flash → `handleDismiss`                  | restore prior focus |
 * | Unique multi-letter match       | overlay's keydown → `actions.setFocus(fq)` → `onClose` (no restore)| focus on match      |
 * | `Backspace`                     | overlay's keydown — shrink buffer                                  | (no close)          |
 * | Window blur                     | `blur` listener → `handleDismiss`                                  | restore prior focus |
 * | 0 enumerable scopes on open     | open-effect immediate `onClose`                                    | unchanged           |
 *
 * The overlay's keydown handler explicitly does NOT see Escape — Escape
 * is owned by the global keymap and reaches `handleDismiss` via the
 * sentinel's `app.dismiss` shadow. This keeps drill-out semantics uniform
 * with the inspector / palette layers.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { type CommandDef, useDispatchCommand } from "@/lib/command-scope";
import { useSpatialFocusActions } from "@/lib/spatial-focus-context";
import { generateSneakCodes } from "@/lib/sneak-codes";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import { useOptionalFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import {
  asSegment,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

/** Identity-stable layer-name segment for the jump-to overlay layer. */
const JUMP_TO_LAYER_NAME = asSegment("jump-to");
/** Identity-stable sentinel-scope segment inside the jump-to layer. */
const JUMP_TO_SENTINEL_SEGMENT = asSegment("jump-to-sentinel");

/**
 * Duration (ms) the red "no-match" flash stays visible before the overlay
 * dismisses. Matches the user-perceptible timing referenced in the task —
 * long enough that a sighted user registers the rejection, short enough
 * that subsequent input is not noticeably blocked.
 */
const FLASH_MS = 150;

/** Public props for the `<JumpToOverlay>` component. */
export interface JumpToOverlayProps {
  /** Whether the overlay is currently visible. Owned by `app-shell.tsx`. */
  open: boolean;
  /** Called to dismiss the overlay. */
  onClose: () => void;
}

/** One enumerated scope paired with its assigned jump code. */
interface JumpTarget {
  fq: FullyQualifiedMoniker;
  rect: DOMRect;
  code: string;
}

/**
 * A focus-claim dispatcher specialised for the jump-to overlay.
 *
 * `nav.focus` is the global command; calling this routes the FQM through
 * the command-scope dispatcher so every focus claim flows through the
 * same auditable choke point. The overlay calls this for prior-focus
 * restore, sentinel auto-claim, and unique-match landing.
 */
type NavFocusDispatcher = (fq: FullyQualifiedMoniker) => void;

/**
 * Visual + interactive overlay for Jump-To. Renders nothing when
 * `open === false`.
 */
export function JumpToOverlay({ open, onClose }: JumpToOverlayProps) {
  const spatial = useSpatialFocusActions();
  // Dispatch focus claims through `nav.focus` — the single auditable
  // command that wraps the entity-focus `setFocus` primitive. Card
  // `01KR7CDEFWWVF4WH0BCHE8Y21J` consolidates every focus claim onto
  // this command so cross-cutting concerns hang off one closure.
  const dispatchNavFocus = useDispatchCommand("nav.focus");
  const navFocus = useCallback<NavFocusDispatcher>(
    (fq) => {
      void dispatchNavFocus({ args: { fq } }).catch((err) => {
        console.error("[JumpToOverlay] nav.focus dispatch failed", err);
      });
    },
    [dispatchNavFocus],
  );
  if (!open) return null;
  return (
    <JumpToOverlayBody
      spatial={spatial}
      navFocus={navFocus}
      onClose={onClose}
    />
  );
}

/** Bag of action surfaces the overlay calls into. */
interface OverlayActions {
  /** Spatial-focus actions — read-only `focusedFq` / enumeration. */
  spatial: ReturnType<typeof useSpatialFocusActions>;
  /** Focus-claim dispatcher — `nav.focus` wrapper. */
  navFocus: NavFocusDispatcher;
}

/** Props for the mounted overlay body — a stable set after the open gate. */
interface JumpToOverlayBodyProps extends OverlayActions {
  onClose: () => void;
}

/**
 * Mounted body of the overlay. Pulled out of {@link JumpToOverlay} so the
 * effects / state hooks only spin up when `open` is true and unmount on
 * close — the alternative (always-on hooks gated by `open`) would
 * complicate restoring prior focus and locking body scroll cleanly.
 */
function JumpToOverlayBody({
  spatial,
  navFocus,
  onClose,
}: JumpToOverlayBodyProps) {
  // Read the surrounding layer's FQM BEFORE we push our own jump-to
  // `<FocusLayer>` — once that layer mounts it overrides the FQM context
  // for descendants. Capturing the parent now is what lets the sentinel
  // FQM be deterministically computed (parent / jump-to / jump-to-sentinel)
  // without threading another consumer component just to extract it.
  // Default to `null` so a malformed test harness without any surrounding
  // layer still produces a deterministic result (`/jump-to/...`).
  const parentLayerFq = useOptionalFullyQualifiedMoniker();
  // Capture prior focus exactly once before any focus-claim runs. Stash in
  // a ref so the dismiss handler can restore it without taking it as a
  // dep (the value never changes for the lifetime of this mount).
  const priorFocusedFqRef = useRef<FullyQualifiedMoniker | null>(null);
  if (priorFocusedFqRef.current === null) {
    priorFocusedFqRef.current = spatial.focusedFq();
  }

  // Enumerate scopes in the topmost layer (the active modal layer).
  // `priorFocusedFq` is no longer the layer-selection input — see
  // `useJumpTargets` — but we still pass it through for symmetry with
  // future per-target restoration logic and so the call signature
  // remains documenting.
  const targets = useJumpTargets(spatial, priorFocusedFqRef.current);

  // Dismiss = restore prior focus then onClose. Memoized so the sentinel
  // commands array stays identity-stable across renders (the inner
  // `<FocusScope>` registers commands once per mount).
  const handleDismiss = useCallback(() => {
    const prior = priorFocusedFqRef.current;
    if (prior !== null) {
      navFocus(prior);
    }
    onClose();
  }, [navFocus, onClose]);

  // Empty enumeration → close immediately without claiming focus or
  // restoring (no-op restore — never happened). Defensive: covers the
  // window-root fallback case where the registry exists but has zero
  // entries (e.g. a freshly-launched app with no scopes mounted).
  useEffect(() => {
    if (targets !== null && targets.length === 0) {
      onClose();
    }
  }, [targets, onClose]);

  // Lock body scroll while the overlay is mounted with a non-empty
  // target set. Restored on unmount so a closed overlay never leaves
  // the page un-scrollable.
  useBodyScrollLock(targets !== null && targets.length > 0);

  // Window blur dismisses with prior-focus restore — standard modal
  // hygiene so swapping windows does not leave a stale overlay onscreen.
  useWindowBlurDismiss(handleDismiss);

  if (targets === null || targets.length === 0) return null;

  return (
    <JumpToLayerRoot
      parentLayerFq={parentLayerFq}
      navFocus={navFocus}
      targets={targets}
      handleDismiss={handleDismiss}
      onMatch={onClose}
    />
  );
}

/** Props threaded to the layer-root tree. */
interface JumpToLayerRootProps {
  /** FQM of the layer that surrounds this overlay (e.g. `/window`). */
  parentLayerFq: FullyQualifiedMoniker | null;
  navFocus: NavFocusDispatcher;
  targets: JumpTarget[];
  handleDismiss: () => void;
  /** Called after a unique-code match has set focus on the matched scope. */
  onMatch: () => void;
}

/**
 * Mounts the jump-to focus layer + portal-rendered overlay tree. Pulled
 * apart from the body so the keydown / focus-claim effects can read the
 * sentinel FQM from `<FocusLayer>`-derived context.
 */
function JumpToLayerRoot({
  parentLayerFq,
  navFocus,
  targets,
  handleDismiss,
  onMatch,
}: JumpToLayerRootProps) {
  // Compose the sentinel FQM the same way the kernel will see it: the
  // jump-to layer's FQM is `<parentFq>/jump-to` (or `/jump-to` when
  // there's no surrounding layer); the sentinel is one segment deeper.
  // Doing the composition here (rather than reading
  // `useFullyQualifiedMoniker()` from inside the layer) keeps the
  // sentinel identity available to the focus-claim effect without
  // having to thread a child component just to extract the FQM via
  // context.
  const layerFq = useMemo<FullyQualifiedMoniker>(
    () =>
      parentLayerFq === null
        ? fqRoot(JUMP_TO_LAYER_NAME)
        : composeFq(parentLayerFq, JUMP_TO_LAYER_NAME),
    [parentLayerFq],
  );
  const sentinelFq = useMemo(
    () => composeFq(layerFq, JUMP_TO_SENTINEL_SEGMENT),
    [layerFq],
  );

  const sentinelCommands = useMemo<readonly CommandDef[]>(
    () => [
      {
        id: "app.dismiss",
        name: "Dismiss Jump-To",
        execute: handleDismiss,
      },
    ],
    [handleDismiss],
  );

  return (
    <FocusLayer name={JUMP_TO_LAYER_NAME}>
      {createPortal(
        <FocusScope
          moniker={JUMP_TO_SENTINEL_SEGMENT}
          commands={sentinelCommands}
          // Suppress the visible focus indicator on the invisible
          // sentinel — the user never sees focus land here.
          showFocus={false}
        >
          <JumpToOverlayChrome
            navFocus={navFocus}
            sentinelFq={sentinelFq}
            targets={targets}
            handleDismiss={handleDismiss}
            onMatch={onMatch}
          />
        </FocusScope>,
        document.body,
      )}
    </FocusLayer>
  );
}

/** Props for the visible chrome (backdrop, pills, key handler). */
interface JumpToOverlayChromeProps {
  navFocus: NavFocusDispatcher;
  sentinelFq: FullyQualifiedMoniker;
  targets: JumpTarget[];
  handleDismiss: () => void;
  onMatch: () => void;
}

/**
 * Visible chrome for the overlay — backdrop, label pills, buffered key
 * matcher, and flash state.
 *
 * Mounted inside the sentinel `<FocusScope>` so the keydown handler
 * observes events that bubble from the sentinel's host `<div>`.
 */
function JumpToOverlayChrome({
  navFocus,
  sentinelFq,
  targets,
  handleDismiss,
  onMatch,
}: JumpToOverlayChromeProps) {
  const [buffer, setBuffer] = useState("");
  const [flashing, setFlashing] = useState(false);
  const codes = useMemo(() => targets.map((t) => t.code), [targets]);

  // Claim focus on the sentinel after mount. Without this, `nav.drillOut`
  // would walk the user's prior focus chain (e.g. card → column → board)
  // before reaching `app.dismiss`. With the claim, the kernel sees focus
  // inside the jump-to layer; drill-out hits the layer-root edge and
  // falls through to `app.dismiss` immediately.
  useEffect(() => {
    navFocus(sentinelFq);
    // Intentionally fire once — re-running this effect (e.g. on a target
    // re-enumeration) would steal focus back from the user mid-input.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sentinelFq]);

  const handleKeyDown = useKeyMatcher({
    navFocus,
    targets,
    codes,
    buffer,
    setBuffer,
    setFlashing,
    handleDismiss,
    onMatch,
  });

  // Attach the matcher at the document level in the capture phase. Two
  // reasons:
  //
  // 1. The sentinel scope claims focus, but the keydown actually fires on
  //    whatever DOM node holds focus — not on this chrome `<div>` — so a
  //    React `onKeyDown` here never observes the user's keystrokes.
  // 2. Capture phase runs before bubble phase, so this listener fires
  //    BEFORE the global keymap's bubble-phase listener registered in
  //    `KeybindingHandler`. Combined with `stopImmediatePropagation` in
  //    `useKeyMatcher`'s `claim()` helper, this makes the overlay a true
  //    modal — keystrokes routed through us never leak to global
  //    keybindings or focused-scope shortcuts while the overlay is open.
  useEffect(() => {
    const onDocKeyDown = (e: KeyboardEvent) => handleKeyDown(e);
    document.addEventListener("keydown", onDocKeyDown, { capture: true });
    return () =>
      document.removeEventListener("keydown", onDocKeyDown, { capture: true });
  }, [handleKeyDown]);

  return (
    <div
      data-testid="jump-to-overlay"
      // The matcher itself is wired on `document` (see effect above) so it
      // catches keystrokes regardless of which inner element has focus.
      // This wrapper exists for layout / `data-testid` only.
    >
      <div
        data-testid="jump-to-backdrop"
        // `z-[80]` so the backdrop paints above every panel z-index in
        // the app (inspector panel sits at z-30; the chrome that holds
        // it sits at z-40). Without this, an inspector-active jump
        // session would have its pills underdraw the panel they are
        // supposed to label.
        className={`fixed inset-0 z-[80] ${
          flashing ? "bg-red-500/30" : "bg-black/30"
        }`}
        // Stop wheel / touchmove from scrolling underlying scroll
        // containers while the overlay is up. `pointer-events: auto` is
        // implicit (default) — the backdrop intercepts both.
        onWheel={(e) => e.preventDefault()}
        onTouchMove={(e) => e.preventDefault()}
        // Stop propagation BEFORE dismissing so the click doesn't bubble
        // up to the surrounding sentinel `<FocusScope>`'s click handler
        // and re-claim focus on the sentinel after `handleDismiss` has
        // already restored the prior focus.
        onClick={(e) => {
          e.stopPropagation();
          handleDismiss();
        }}
      />
      {targets.map((t) => (
        <JumpPill key={t.fq} target={t} />
      ))}
    </div>
  );
}

/** Hook params for {@link useKeyMatcher}. */
interface UseKeyMatcherParams {
  navFocus: NavFocusDispatcher;
  targets: JumpTarget[];
  codes: string[];
  buffer: string;
  setBuffer: React.Dispatch<React.SetStateAction<string>>;
  setFlashing: React.Dispatch<React.SetStateAction<boolean>>;
  handleDismiss: () => void;
  onMatch: () => void;
}

/**
 * Build the keydown handler driving buffered code matching.
 *
 * Returns a stable-shape `KeyboardEventHandler<HTMLDivElement>` whose
 * behavior is:
 *
 *   - Printable letter: extend the buffer; on unique match, fire
 *     {@link UseKeyMatcherParams.onMatch}; on no-match flash and dismiss.
 *   - `Backspace`: shrink the buffer; never closes (an empty-buffer
 *     backspace is a no-op).
 *   - Other keys (including `Escape`, modifiers, arrows): ignored.
 */
function useKeyMatcher(params: UseKeyMatcherParams) {
  const {
    navFocus,
    targets,
    codes,
    buffer,
    setBuffer,
    setFlashing,
    handleDismiss,
    onMatch,
  } = params;

  return useCallback(
    (e: KeyboardEvent | React.KeyboardEvent<HTMLDivElement>) => {
      // Helper: claim the event so the global keymap and any focused
      // scope's keybindings don't ALSO see it while the overlay owns
      // input. `stopImmediatePropagation` is required (not just
      // `stopPropagation`) because the global keymap installs its own
      // document-level listener — both fire on the same target.
      const claim = () => {
        e.preventDefault();
        if ("stopImmediatePropagation" in e) {
          (e as KeyboardEvent).stopImmediatePropagation();
        }
      };
      // Escape dismisses directly. The original design routed Escape
      // through the global keymap's `nav.drillOut → app.dismiss → sentinel
      // shadow` cascade, but that proved flaky in practice — when the
      // sentinel hasn't fully claimed focus, drillOut walks the user's
      // prior chain and never reaches the shadow. Handling Escape here
      // (the overlay's own document listener, mounted only while open)
      // makes dismissal deterministic.
      if (e.key === "Escape") {
        claim();
        handleDismiss();
        return;
      }
      if (e.key === "Backspace") {
        claim();
        setBuffer((b) => b.slice(0, -1));
        return;
      }
      // Only printable single-letter keys participate in matching.
      if (e.key.length !== 1 || !/[a-zA-Z]/.test(e.key)) return;
      claim();
      const next = buffer + e.key.toLowerCase();
      const exact = codes.indexOf(next);
      if (exact >= 0) {
        const target = targets[exact];
        navFocus(target.fq);
        onMatch();
        return;
      }
      const isPrefix = codes.some((c) => c.startsWith(next));
      if (isPrefix) {
        setBuffer(next);
        return;
      }
      // No match — flash, then dismiss.
      setFlashing(true);
      setTimeout(() => {
        setFlashing(false);
        handleDismiss();
      }, FLASH_MS);
    },
    [
      navFocus,
      targets,
      codes,
      buffer,
      setBuffer,
      setFlashing,
      handleDismiss,
      onMatch,
    ],
  );
}

/** Props for one rendered code pill. */
interface JumpPillProps {
  target: JumpTarget;
}

/**
 * One absolutely-positioned label pill rendering its assigned code at
 * the top-left of the matched scope's rect. Carries `data-jump-code`
 * and `data-jump-fq` for deterministic e2e selection.
 */
function JumpPill({ target }: JumpPillProps) {
  const { code, fq, rect } = target;
  return (
    <div
      data-jump-code={code}
      data-jump-fq={fq}
      // `z-[80]` so the pill paints above the inspector panel (z-30)
      // and the panel's surrounding chrome (z-40) when the inspector
      // is the topmost (active) layer. The backdrop one level below
      // shares the same z so the pill's relative paint order stays
      // deterministic.
      className="fixed z-[80] bg-primary text-primary-foreground font-mono px-1 rounded shadow text-xs leading-tight"
      style={{
        left: rect.left + 4,
        top: rect.top + 4,
      }}
    >
      {code}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

/**
 * Enumerate scopes in the **topmost** layer (the active modal layer),
 * generate one code per scope, and pair them up. Returns `null` while
 * async code generation is in flight, an empty array when the layer
 * has no non-zero-area scopes (the empty-enumeration case the body
 * handles by closing immediately), or a populated `JumpTarget[]`
 * otherwise.
 *
 * Per the modal-layer model (card `01KR7CDEFWWVF4WH0BCHE8Y21J`),
 * jump-to enumerates against the **active layer** — the topmost
 * pushed layer — not the layer that owned `priorFocusedFq`. With no
 * inspector / palette mounted, the topmost layer IS the window, so
 * pills paint on cards / columns / navbar. With the inspector on top,
 * pills paint on inspector field scopes. The `priorFocusedFq` plumbing
 * remains for the dismiss-on-no-match restore-prior-focus path.
 *
 * The enumeration runs once on mount (the body remounts on each open),
 * so mid-overlay layout changes don't reshuffle pills — that's a
 * deliberate design choice: the user picks a code based on the layout
 * they saw when they opened, not on whatever the layout becomes
 * mid-input.
 */
function useJumpTargets(
  spatial: ReturnType<typeof useSpatialFocusActions>,
  _priorFocusedFq: FullyQualifiedMoniker | null,
): JumpTarget[] | null {
  const [targets, setTargets] = useState<JumpTarget[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    // Read the topmost layer FQM. The window layer is always pushed at
    // app boot, so under normal conditions this is non-null. Fall back
    // to `/window` defensively for the early-boot edge case where the
    // overlay opens before any layer has registered.
    const topLayerFq = spatial.topLayerFq() ?? fqRoot(asSegment("window"));
    const enumerated = spatial
      .enumerateScopesInLayer(topLayerFq)
      .filter((s) => s.rect.width > 0 && s.rect.height > 0);
    if (enumerated.length === 0) {
      setTargets([]);
      return;
    }
    generateSneakCodes(enumerated.length)
      .then((codes: string[]) => {
        if (cancelled) return;
        const paired: JumpTarget[] = enumerated.map((s, i) => ({
          fq: s.fq,
          rect: s.rect,
          code: codes[i],
        }));
        setTargets(paired);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error("[JumpToOverlay] generateSneakCodes failed", err);
        setTargets([]);
      });
    return () => {
      cancelled = true;
    };
    // Run-once on mount: the body remounts on each open so this captures
    // the layout the user saw when they pressed the Jump-To trigger.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return targets;
}

/**
 * Lock `document.body.style.overflow` to `"hidden"` while `active` is
 * `true`. Captures and restores the previous value on unmount so a
 * closed overlay never leaves the page un-scrollable.
 */
function useBodyScrollLock(active: boolean): void {
  useEffect(() => {
    if (!active) return;
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prev;
    };
  }, [active]);
}

/**
 * While mounted, attach a `blur` listener on `window` that calls
 * `handleDismiss`. Standard modal hygiene — switching windows should
 * not leave a stale overlay onscreen.
 */
function useWindowBlurDismiss(handleDismiss: () => void): void {
  // Read `handleDismiss` through a ref so re-binding the listener on
  // every render is unnecessary; the listener identity stays stable.
  const handleDismissRef = useRef(handleDismiss);
  handleDismissRef.current = handleDismiss;

  useEffect(() => {
    const onBlur = () => handleDismissRef.current();
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("blur", onBlur);
    };
  }, []);
}
