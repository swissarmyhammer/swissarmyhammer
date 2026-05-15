import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { X } from "lucide-react";
import { useDispatchCommand } from "@/lib/command-scope";

/**
 * Default panel width applied when no `width` prop is passed.
 *
 * Mirrors the historical `w-[420px]` Tailwind class so unaware callers
 * keep their previous geometry.
 */
const DEFAULT_PANEL_WIDTH = 420;

/**
 * Lower clamp on the user-resizable panel width (CSS pixels).
 *
 * Below this the inspector form fields stop being legible — the
 * markdown editors and field labels start wrapping on every word.
 */
const MIN_PANEL_WIDTH = 320;

/**
 * Absolute upper clamp on the user-resizable panel width (CSS pixels).
 *
 * The actual upper bound is `min(MAX_PANEL_WIDTH, 0.85 * viewport)` —
 * the 0.85 fraction preserves the historical `max-w-[85vw]` cap so the
 * panel never swallows the entire viewport on narrow windows.
 */
const MAX_PANEL_WIDTH = 800;

/**
 * Compute the upper clamp for the current viewport width.
 *
 * Returns the smaller of `MAX_PANEL_WIDTH` and 85% of the viewport so
 * the inspector cannot eclipse the underlying board on narrow windows.
 */
function maxAllowedWidth(viewportWidth: number): number {
  return Math.min(MAX_PANEL_WIDTH, Math.floor(viewportWidth * 0.85));
}

/** Clamp `n` into the valid resize range for the current viewport. */
function clampWidth(n: number, viewportWidth: number): number {
  const upper = Math.max(MIN_PANEL_WIDTH, maxAllowedWidth(viewportWidth));
  return Math.max(MIN_PANEL_WIDTH, Math.min(upper, n));
}

interface SlidePanelProps {
  open: boolean;
  onClose: () => void;
  style?: React.CSSProperties;
  children: ReactNode;
  /**
   * Current width of the panel in CSS pixels. Defaults to 420 px when
   * omitted so callers that don't care about resize keep the historical
   * geometry. The width is applied via inline `style.width` so it can
   * be driven from React state during a drag.
   */
  width?: number;
  /**
   * Called on every `mousemove` while the user drags the left-edge
   * handle. Receives the clamped, in-bounds candidate width. Callers
   * use this to update transient drag state (so the panel visibly
   * resizes at 60 fps) without round-tripping through the backend.
   */
  onResize?: (nextWidth: number) => void;
  /**
   * Called once on `mouseup` after a drag with the final clamped width.
   * Callers dispatch the persistence command (`ui.inspector.set_width`)
   * here — mirrors the column-resize / window-geometry pattern of
   * "transient state in React, only the final value round-trips".
   */
  onResizeEnd?: (finalWidth: number) => void;
}

/**
 * Generic slide-out panel shell — fixed to the right edge.
 *
 * Renders children inside a panel (default 420 px wide) with a close
 * button and a 6 px-wide invisible left-edge resize handle. The handle
 * shows `cursor-col-resize` on hover and a hairline `bg-border`
 * indicator. Dragging the handle emits `onResize` continuously and
 * `onResizeEnd` once on release.
 *
 * Width is applied via inline `style.width`; the historical
 * `max-w-[85vw]` cap is preserved as `style.maxWidth: '85vw'` so the
 * panel can never visually exceed the viewport even if a stale
 * `width` prop drifts out of bounds.
 */
export function SlidePanel({
  open,
  onClose: _onClose,
  style,
  children,
  width = DEFAULT_PANEL_WIDTH,
  onResize,
  onResizeEnd,
}: SlidePanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const dispatchClose = useDispatchCommand("ui.inspector.close");

  // Drag state lives in a ref because mousemove handlers need to read
  // the start coordinates without triggering a render cascade. The
  // last clamped value is kept here too so `mouseup` can pass it to
  // `onResizeEnd` without depending on closure capture of stale state.
  //
  // `moved` flips to true the first time `mousemove` produces a clamped
  // width different from `startWidth`. A tap-without-movement (mousedown
  // → mouseup with zero intervening movement, or movement that never
  // crossed a clamp boundary) leaves it false, so `endDrag` skips
  // `onResizeEnd` and we don't persist a no-op width to disk. Without
  // this guard, a stray tap on a never-resized panel would flip
  // `WindowState.inspector_width` from `None` to `Some(420)` (the React
  // default) and write that to disk — see review finding 2026-05-09.
  //
  // The window-level `mousemove`/`mouseup` listeners are also held in
  // this ref so they can be installed in `handleMouseDown` (drag start)
  // and cleaned up in `endDrag` (drag end). Binding them only for the
  // lifetime of an active drag matches the pattern used elsewhere in
  // the codebase and avoids every mounted `<SlidePanel>` walking every
  // pointer event over the entire window.
  const dragRef = useRef<{
    startX: number;
    startWidth: number;
    lastWidth: number;
    moved: boolean;
    active: boolean;
    onMove: (e: MouseEvent) => void;
    onUp: () => void;
  } | null>(null);

  const handleMouseMove = useCallback(
    (event: MouseEvent) => {
      const drag = dragRef.current;
      if (!drag || !drag.active) return;
      // Dragging the LEFT edge to the right shrinks the panel; to the
      // left grows it. So `nextWidth = startWidth - deltaX`.
      const deltaX = event.clientX - drag.startX;
      const raw = drag.startWidth - deltaX;
      const next = clampWidth(raw, window.innerWidth);
      if (next !== drag.startWidth) {
        drag.moved = true;
      }
      drag.lastWidth = next;
      onResize?.(next);
    },
    [onResize],
  );

  const endDrag = useCallback(() => {
    const drag = dragRef.current;
    if (!drag || !drag.active) return;
    drag.active = false;
    // Tear down the window-level listeners installed in handleMouseDown.
    window.removeEventListener("mousemove", drag.onMove);
    window.removeEventListener("mouseup", drag.onUp);
    // Tap-without-movement: don't persist a no-op width.
    if (drag.moved) {
      onResizeEnd?.(drag.lastWidth);
    }
    dragRef.current = null;
  }, [onResizeEnd]);

  // Stash refs for the latest handlers so the `mousemove`/`mouseup`
  // listeners installed in `handleMouseDown` always invoke the freshest
  // closure (otherwise a `width` prop change mid-drag would be ignored).
  const handleMouseMoveRef = useRef(handleMouseMove);
  const endDragRef = useRef(endDrag);
  useEffect(() => {
    handleMouseMoveRef.current = handleMouseMove;
  }, [handleMouseMove]);
  useEffect(() => {
    endDragRef.current = endDrag;
  }, [endDrag]);

  // On unmount in the middle of a drag, release the captured listeners
  // so they don't outlive the component.
  useEffect(() => {
    return () => {
      const drag = dragRef.current;
      if (drag?.active) {
        window.removeEventListener("mousemove", drag.onMove);
        window.removeEventListener("mouseup", drag.onUp);
        dragRef.current = null;
      }
    };
  }, []);

  const handleMouseDown = useCallback(
    (event: React.MouseEvent) => {
      // Only the primary button starts a drag. Suppress text selection
      // for the duration via `event.preventDefault()`.
      if (event.button !== 0) return;
      event.preventDefault();
      const onMove = (e: MouseEvent) => handleMouseMoveRef.current(e);
      const onUp = () => endDragRef.current();
      dragRef.current = {
        startX: event.clientX,
        startWidth: width,
        lastWidth: width,
        moved: false,
        active: true,
        onMove,
        onUp,
      };
      // Install window-level listeners only for the duration of the
      // drag. They are removed in `endDrag` (or on unmount).
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [width],
  );

  // Track hover so the resize-handle indicator paints subtly without
  // requiring a Tailwind `group-hover:*` chain on the parent.
  const [handleHover, setHandleHover] = useState(false);

  return (
    <div
      ref={panelRef}
      data-slide-panel
      className={`fixed top-0 z-30 h-full bg-background border-l border-border shadow-xl flex flex-col transition-transform duration-200 ease-out ${
        open ? "translate-x-0" : "translate-x-full"
      }`}
      style={{ ...style, width, maxWidth: "85vw" }}
    >
      {/* Left-edge resize handle — 6 px-wide invisible hit zone.
          A hairline `bg-border` indicator paints on hover so the user
          discovers the handle without any persistent visual weight on
          the panel edge. */}
      <div
        data-inspector-resize-handle
        onMouseDown={handleMouseDown}
        onMouseEnter={() => setHandleHover(true)}
        onMouseLeave={() => setHandleHover(false)}
        className="absolute top-0 left-0 z-40 h-full w-[6px] cursor-col-resize select-none"
        aria-hidden="true"
      >
        <div
          className={`h-full w-px transition-colors ${
            handleHover ? "bg-border" : "bg-transparent"
          }`}
        />
      </div>
      <div className="flex items-center justify-end px-3 pt-3">
        <button
          onClick={() => {
            dispatchClose().catch(console.error);
          }}
          className="shrink-0 p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto px-5 pb-5">{children}</div>
    </div>
  );
}
