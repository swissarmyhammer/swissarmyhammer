/**
 * The Container that docks {@link AiPanel} into the main window layer.
 *
 * # A Container, not a View
 *
 * Per `ARCHITECTURE.md`'s Container/View split, this is the Container
 * counterpart to the `AiPanel` View. The View renders props and never touches
 * the backend; this Container owns every backend seam the View only reports:
 *
 * - **Model enumeration.** Fetches `ai_list_models` once on mount and feeds
 *   the result to `AiPanel` as the `models` prop.
 * - **Per-board persistence.** `AiPanel` reports the user's model choice via
 *   `onSelectModel`; this Container persists it — together with the panel's
 *   open-state and draggable width — per board (see below).
 * - **Layout.** Renders the right-docked, collapsible, resizable shell that
 *   hosts the View, a sibling of `ViewsContainer` inside `WindowContainer` and
 *   outside the inspector stack (see `App.tsx`).
 *
 * # Per-board UI state — `localStorage` keyed by board path
 *
 * The panel's open-state, width, and selected model are per-board UI state.
 * They persist in `localStorage` under a key derived from the active board
 * path — the same plain `localStorage`-backed per-board mechanism
 * `quick-capture.tsx` uses to remember its last board. This is webview-local
 * persistence only: there is no backend `UIState`/YAML store or event-sync
 * plumbing involved. The app shows one board per window, so a fresh window
 * reopening a board restores exactly the panel geometry and model it had last
 * time. The conversation transcript is deliberately NOT persisted — the chat
 * is stateless (see `ideas/kanban/ai_panel.md`).
 *
 * # Collapsible — driven by the `ai.toggle` window-layer command
 *
 * The panel is collapsible: this Container owns the open-state and renders an
 * in-header collapse/expand control. The window-layer `ai.toggle` / `ai.focus`
 * / `ai.model` commands (registered in `AppShell`'s global scope) drive it
 * through the `ai/commands.ts` module registry — this Container registers
 * `handleToggle`, `handleFocus`, and `handleSelectModel` as their handlers.
 * `ai.newChat` / `ai.cancel` and the streaming flag are owned by the
 * conversation, which registers them itself.
 *
 * # Quick-capture never shows the panel
 *
 * The borderless quick-capture popup is a minimal capture surface, not a board
 * workspace. When `isQuickCapture` is set the Container renders nothing — the
 * `App.tsx` quick-capture tree never even mounts it, and this guard keeps the
 * panel absent for any other caller too.
 */
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { SparklesIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useActiveBoardPath } from "@/lib/command-scope";
import {
  AiPanel,
  aiPanelConnectFactory,
  type AiModel,
  type AiPanelConnectFactory,
} from "@/components/ai-panel";
import {
  aiStreaming,
  registerAiCommandHandlers,
  subscribeAiStreaming,
} from "@/ai/commands";

/** Detect if this window instance is the quick-capture popup. */
const IS_QUICK_CAPTURE =
  new URLSearchParams(window.location.search).get("window") === "quick-capture";

/**
 * Default panel width applied when no width has been persisted for the board
 * yet (fresh board, never resized). Mirrors the inspector's 420 px default so
 * the two right-docked surfaces have a consistent resting size.
 */
export const AI_PANEL_DEFAULT_WIDTH = 420;

/**
 * Lower clamp on the user-resizable panel width (CSS pixels).
 *
 * Below this the conversation log and the model selector start wrapping; the
 * value matches the inspector's `MIN_PANEL_WIDTH` for visual consistency.
 */
const MIN_PANEL_WIDTH = 320;

/**
 * Absolute upper clamp on the user-resizable panel width (CSS pixels).
 *
 * The effective upper bound is `min(MAX_PANEL_WIDTH, 0.85 * viewport)` so the
 * panel can never swallow the whole window on a narrow display.
 */
const MAX_PANEL_WIDTH = 800;

/** Compute the upper clamp for the current viewport width. */
function maxAllowedWidth(viewportWidth: number): number {
  return Math.min(MAX_PANEL_WIDTH, Math.floor(viewportWidth * 0.85));
}

/** Clamp `n` into the valid resize range for the current viewport. */
function clampWidth(n: number, viewportWidth: number): number {
  const upper = Math.max(MIN_PANEL_WIDTH, maxAllowedWidth(viewportWidth));
  return Math.max(MIN_PANEL_WIDTH, Math.min(upper, n));
}

/**
 * The persisted per-board AI panel state — open-state, width, and the
 * selected model id. Each field is optional so a partial or legacy snapshot
 * degrades gracefully to the defaults.
 */
export interface AiPanelState {
  /** Whether the panel is expanded. Absent → expanded (the default). */
  open?: boolean;
  /** The user-dragged panel width in CSS pixels. Absent → the default. */
  width?: number;
  /** The per-board selected model id. Absent → no model chosen yet. */
  modelId?: string;
}

/**
 * The `localStorage` key under which a board's AI panel state is persisted.
 *
 * Keyed by the board path so each board reopens with its own panel geometry
 * and model — the per-board contract the task requires. Exported so tests can
 * assert the persisted shape.
 */
export function aiPanelStateStorageKey(boardPath: string): string {
  return `ai-panel-state:${boardPath}`;
}

/**
 * Read the persisted {@link AiPanelState} for a board.
 *
 * Returns an empty object when nothing is stored, the board path is unknown,
 * or the stored value is malformed — so callers always get a usable record.
 */
function loadAiPanelState(boardPath: string | undefined): AiPanelState {
  if (!boardPath) return {};
  try {
    const raw = localStorage.getItem(aiPanelStateStorageKey(boardPath));
    if (!raw) return {};
    const parsed = JSON.parse(raw) as unknown;
    if (parsed && typeof parsed === "object") {
      return parsed as AiPanelState;
    }
    return {};
  } catch {
    // A storage read can throw in locked-down webviews; treat as "no state".
    return {};
  }
}

/**
 * Persist a partial {@link AiPanelState} update for a board.
 *
 * Merges over whatever is already stored so writing one field (e.g. the width
 * on drag-end) never clobbers the others. A no-op when the board path is
 * unknown.
 */
function saveAiPanelState(
  boardPath: string | undefined,
  patch: AiPanelState,
): void {
  if (!boardPath) return;
  try {
    const next = { ...loadAiPanelState(boardPath), ...patch };
    localStorage.setItem(
      aiPanelStateStorageKey(boardPath),
      JSON.stringify(next),
    );
  } catch {
    // Persistence is best-effort; a failed write must not break the panel.
  }
}

/** The two endpoint URLs `ai_start_agent` hands back, camelCased on the wire. */
interface AgentEndpoint {
  /** Loopback `ws://127.0.0.1:<port>` URL for the in-process ACP agent. */
  wsUrl: string;
  /** The board's full-SAH-toolset MCP URL, or `null` when the board has none. */
  mcpUrl: string | null;
}

/** Props for {@link AiPanelContainer}. */
export interface AiPanelContainerProps {
  /**
   * Builds the {@link AiPanelConnectFactory} for the hosted `AiPanel`. In
   * production `App.tsx` passes a factory built from `aiPanelConnectFactory`;
   * tests inject a stub. When omitted the Container builds the production
   * factory itself from `ai_start_agent`.
   */
  createConnect?: AiPanelConnectFactory;
  /**
   * When `true` the Container renders nothing — the quick-capture window never
   * shows the panel. Defaults to the module-level `IS_QUICK_CAPTURE`
   * detection; overridable so the guard is testable.
   */
  isQuickCapture?: boolean;
}

/**
 * Build the production `createConnect` factory for a board.
 *
 * Composes the real handoff: `ai_start_agent(modelId)` yields the loopback
 * `ws://` agent URL and the board MCP URL, which `aiPanelConnectFactory` (the
 * View module's exported helper) wires into an ACP connection. Pulled into the
 * Container because starting the agent is a backend call — a View seam.
 */
function useProductionConnect(boardDir: string): AiPanelConnectFactory {
  return useMemo(() => {
    // `startAgent` is the Container's `ai_start_agent` backend seam; the View
    // module's `aiPanelConnectFactory` composes it into an ACP connection.
    const startAgent = (modelId: string): Promise<AgentEndpoint> =>
      invoke<AgentEndpoint>("ai_start_agent", {
        modelId,
        boardPath: boardDir,
      });
    return aiPanelConnectFactory(boardDir, startAgent);
  }, [boardDir]);
}

/**
 * The right-docked AI panel Container.
 *
 * Owns the per-board open/width/model state, fetches `ai_list_models`, and
 * renders the collapsible, resizable shell around the `AiPanel` View.
 */
export function AiPanelContainer({
  createConnect,
  isQuickCapture = IS_QUICK_CAPTURE,
}: AiPanelContainerProps): ReactNode {
  // The quick-capture popup is a minimal capture surface — never the panel.
  if (isQuickCapture) {
    return null;
  }
  return <AiPanelContainerBody createConnect={createConnect} />;
}

/** Props for {@link AiPanelContainerBody}. */
interface AiPanelContainerBodyProps {
  createConnect?: AiPanelConnectFactory;
}

/**
 * The mounted body of the Container — split out so the `isQuickCapture` guard
 * in {@link AiPanelContainer} is a clean early `return null` with no hooks
 * running in the quick-capture window.
 */
function AiPanelContainerBody({
  createConnect,
}: AiPanelContainerBodyProps): ReactNode {
  const boardPath = useActiveBoardPath();

  // Per-board state, seeded once from this board's persisted `localStorage`
  // snapshot. Re-seeded whenever the active board changes so switching boards
  // swaps in that board's own panel geometry and model.
  const persisted = useMemo(() => loadAiPanelState(boardPath), [boardPath]);
  const [open, setOpen] = useState<boolean>(persisted.open ?? true);
  const [width, setWidth] = useState<number>(
    persisted.width ?? AI_PANEL_DEFAULT_WIDTH,
  );
  const [modelId, setModelId] = useState<string | null>(
    persisted.modelId ?? null,
  );
  useEffect(() => {
    const next = loadAiPanelState(boardPath);
    setOpen(next.open ?? true);
    setWidth(next.width ?? AI_PANEL_DEFAULT_WIDTH);
    setModelId(next.modelId ?? null);
  }, [boardPath]);

  // Model list — the Container's one fetch seam. `undefined` while in flight.
  const [models, setModels] = useState<AiModel[] | undefined>(undefined);
  useEffect(() => {
    let cancelled = false;
    invoke<AiModel[]>("ai_list_models")
      .then((list) => {
        if (!cancelled) setModels(list);
      })
      .catch((err) => {
        console.error("ai_list_models failed:", err);
        if (!cancelled) setModels([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** Persist the user's model choice per board, then feed it back to the View. */
  const handleSelectModel = useCallback(
    (id: string) => {
      setModelId(id);
      saveAiPanelState(boardPath, { modelId: id });
    },
    [boardPath],
  );

  /**
   * Auto-select a sensible default model when none is persisted for the board.
   *
   * The panel can otherwise land in the dead-end `NoModelState` whenever a
   * board has no persisted `modelId` (every fresh board, or any board whose
   * `localStorage` snapshot was cleared) even though `ai_list_models` has
   * already returned a usable model. To avoid that, once the model list has
   * resolved and the per-board `modelId` is still `null`, pick the first
   * `available: true` entry and route it through `handleSelectModel` — the
   * same path a user click takes, so the choice is persisted via
   * `saveAiPanelState` and a remount reads it back from `localStorage`.
   *
   * Rules:
   *
   *   - Only runs when `modelId === null`. A persisted or user-picked id is
   *     never overwritten — even if that model is `available: false`, the
   *     user's explicit prior choice wins.
   *   - Picks the first `available: true` model in the list. The backend
   *     orders Claude Code first when its CLI is detected, then local llamas
   *     (see `apps/kanban-app/src/ai/models.rs::ai_list_models`), so the
   *     default reflects the same priority.
   *   - When every entry is `available: false`, leaves `modelId` as `null`
   *     so `NoModelState` continues to render — that is a genuine
   *     empty-config case, not a dead-end the user can fix by clicking the
   *     picker.
   */
  useEffect(() => {
    if (modelId !== null) return;
    if (!models) return;
    const firstAvailable = models.find((model) => model.available);
    if (firstAvailable) {
      handleSelectModel(firstAvailable.id);
    }
  }, [models, modelId, handleSelectModel]);

  /** Flip the panel open-state and persist it for this board. */
  const handleToggle = useCallback(() => {
    setOpen((prev) => {
      const next = !prev;
      saveAiPanelState(boardPath, { open: next });
      return next;
    });
  }, [boardPath]);

  /**
   * Expand the panel (if collapsed) and move keyboard focus into its prompt
   * input — the `ai.focus` window-layer command's handler.
   *
   * The panel is expanded first so focus has a rendered target; the prompt
   * editor is the AI composer's CodeMirror 6 content DOM, located by its
   * `role="textbox"` + accessible label. The focus is deferred a frame so the
   * expand has committed to the DOM before the lookup runs.
   */
  const handleFocus = useCallback(() => {
    setOpen((prev) => {
      if (!prev) saveAiPanelState(boardPath, { open: true });
      return true;
    });
    requestAnimationFrame(() => {
      const input = document.querySelector<HTMLElement>(
        "[data-slot='ai-panel'] [role='textbox'][aria-label='Message the AI agent']",
      );
      input?.focus();
    });
  }, [boardPath]);

  // Register the Container-owned `ai.*` command handlers into the
  // `ai/commands.ts` registry so the window-layer `ai.toggle` / `ai.focus` /
  // `ai.model` commands (in `AppShell`'s global scope) can drive the panel.
  // `ai.newChat` / `ai.cancel` and the streaming flag are owned by
  // `AiPanelConversation`, which registers them itself.
  useEffect(() => {
    return registerAiCommandHandlers({
      toggle: handleToggle,
      focus: handleFocus,
      setModel: handleSelectModel,
    });
  }, [handleToggle, handleFocus, handleSelectModel]);

  // Mirror the conversation's streaming flag to the backend `UIState` so the
  // `ai.cancel` palette entry is gated server-side too. `AiPanelConversation`
  // (a View) reports streaming into the `ai/commands.ts` registry; the
  // Container — which owns every backend seam — pushes it to `UIState` via
  // the `ai_set_streaming` Tauri command. Keeping the `invoke` here, not in
  // the View, preserves the Container/View split.
  const streaming = useSyncExternalStore(
    subscribeAiStreaming,
    aiStreaming,
    aiStreaming,
  );
  useEffect(() => {
    invoke("ai_set_streaming", { streaming }).catch((err) => {
      console.error("ai_set_streaming failed:", err);
    });
  }, [streaming]);

  /** Live drag: update the width without a persistence write. */
  const handleResize = useCallback((next: number) => {
    setWidth(next);
  }, []);

  /** Drag end: persist the final width once. */
  const handleResizeEnd = useCallback(
    (final: number) => {
      setWidth(final);
      saveAiPanelState(boardPath, { width: final });
    },
    [boardPath],
  );

  // The production connect factory — used only when no `createConnect` is
  // injected. Built unconditionally (hooks must not be conditional); the
  // injected prop takes precedence below.
  const productionConnect = useProductionConnect(boardPath ?? "");
  const effectiveConnect = createConnect ?? productionConnect;

  return (
    <AiPanelShell
      open={open}
      width={width}
      onToggle={handleToggle}
      onResize={handleResize}
      onResizeEnd={handleResizeEnd}
    >
      <AiPanel
        boardDir={boardPath ?? ""}
        models={models}
        modelId={modelId}
        onSelectModel={handleSelectModel}
        onCollapse={handleToggle}
        createConnect={effectiveConnect}
      />
    </AiPanelShell>
  );
}

/** Props for {@link AiPanelShell}. */
interface AiPanelShellProps {
  /** Whether the panel is expanded. */
  open: boolean;
  /** The current panel width in CSS pixels. */
  width: number;
  /** Flip the open-state. */
  onToggle: () => void;
  /** Fired on every `mousemove` during a width drag, with the clamped width. */
  onResize: (next: number) => void;
  /** Fired once on `mouseup` after a drag, with the final clamped width. */
  onResizeEnd: (final: number) => void;
  /** The hosted `AiPanel` View. */
  children: ReactNode;
}

/**
 * The right-docked panel shell — collapse rail, resize handle, and body.
 *
 * When collapsed the shell shrinks to a thin rail with the expand control;
 * when expanded it renders the left-edge resize handle and the hosted View
 * inside a fixed-width column.
 *
 * # The body stays mounted across toggles
 *
 * `children` (the hosted `AiPanel`) is rendered unconditionally — collapsing
 * the panel only hides the body (`hidden` → `display: none`), it never
 * unmounts it. Unmounting on collapse would tear down the `useConversation`
 * store and the live ACP session, so a toggle would silently destroy the
 * conversation. The "start fresh" path is the dedicated `ai.newChat` /
 * "New conversation" affordance — toggling the panel must never do that.
 *
 * The rail and the body live under one always-mounted outer container so the
 * body is never unmounted by a toggle. The outer container's width is the
 * rail width (`w-9`) when collapsed and the user-resizable `width` when
 * expanded.
 *
 * The resize handle reuses the `SlidePanel` drag pattern: window-level
 * `mousemove`/`mouseup` listeners installed only for the duration of a drag,
 * a transient `liveWidth` so the panel resizes at 60 fps, and a single
 * persistence call on release.
 */
function AiPanelShell({
  open,
  width,
  onToggle,
  onResize,
  onResizeEnd,
  children,
}: AiPanelShellProps): ReactNode {
  // Drag bookkeeping — held in a ref so the window-level move/up handlers read
  // the current start coordinates without a render cascade, mirroring
  // `slide-panel.tsx`. `moved` guards against persisting a no-op width when a
  // bare click on the handle never crossed a clamp boundary.
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
      // Dragging the LEFT edge left grows the panel: next = startWidth - deltaX.
      const deltaX = event.clientX - drag.startX;
      const next = clampWidth(drag.startWidth - deltaX, window.innerWidth);
      if (next !== drag.startWidth) {
        drag.moved = true;
      }
      drag.lastWidth = next;
      onResize(next);
    },
    [onResize],
  );

  const endDrag = useCallback(() => {
    const drag = dragRef.current;
    if (!drag || !drag.active) return;
    drag.active = false;
    window.removeEventListener("mousemove", drag.onMove);
    window.removeEventListener("mouseup", drag.onUp);
    if (drag.moved) {
      onResizeEnd(drag.lastWidth);
    }
    dragRef.current = null;
  }, [onResizeEnd]);

  // Keep the window-level listeners pointing at the freshest closures so a
  // mid-drag prop change is not ignored.
  const handleMouseMoveRef = useRef(handleMouseMove);
  const endDragRef = useRef(endDrag);
  useEffect(() => {
    handleMouseMoveRef.current = handleMouseMove;
  }, [handleMouseMove]);
  useEffect(() => {
    endDragRef.current = endDrag;
  }, [endDrag]);

  // Release any captured listeners if the shell unmounts mid-drag.
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

  // Memoized on `[width]`: a live `mousemove` re-renders with a fresh `width`,
  // so React re-attaches the handle's `onMouseDown` mid-drag. Harmless —
  // `onMouseDown` only fires on a fresh press and a drag is already in flight —
  // and kept on purpose so this stays a faithful mirror of `slide-panel.tsx`'s
  // handle. Reading `width` from a ref instead would tighten the deps to `[]`
  // but diverge from that intentionally-parallel sibling for no real gain.
  const handleMouseDown = useCallback(
    (event: React.MouseEvent) => {
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
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [width],
  );

  // One always-mounted outer container hosts both the collapsed rail and the
  // expanded body. The rail and the body are siblings; only their visibility
  // toggles. `children` is never unmounted by a toggle, so the hosted
  // `AiPanel`'s `useConversation` store and live ACP session survive any
  // number of collapse/expand cycles.
  //
  // Sizing: `w-9` (the rail width) when collapsed, the user-resizable `width`
  // when expanded. `maxWidth: 85vw` matches the resize clamp so the expanded
  // panel can never swallow the whole window on a narrow display.
  return (
    <div
      className={
        open
          ? "relative flex h-full shrink-0 flex-col border-l bg-background"
          : "relative flex h-full w-9 shrink-0 flex-col border-l bg-background"
      }
      data-testid="ai-panel-container"
      data-ai-panel-collapsed={open ? "false" : "true"}
      style={open ? { width, maxWidth: "85vw" } : undefined}
    >
      {/* Left-edge resize handle — only rendered when expanded; it has nothing
          to grab when the container is at the rail width. */}
      {open ? (
        <div
          data-ai-panel-resize-handle
          onMouseDown={handleMouseDown}
          className="group absolute top-0 left-0 z-10 h-full w-[6px] cursor-col-resize select-none"
          aria-hidden="true"
        >
          <div className="h-full w-px bg-transparent transition-colors group-hover:bg-border" />
        </div>
      ) : null}

      {/* The collapsed rail — a single AI-star toggle. Only shown when
          collapsed; the expanded panel has its own header with the matching
          star control wired through `onCollapse` on the hosted View. */}
      {open ? null : (
        <div className="flex flex-col items-center py-2">
          <Button
            aria-label="Expand AI panel"
            onClick={onToggle}
            size="icon"
            variant="ghost"
          >
            <SparklesIcon className="size-4" />
          </Button>
        </div>
      )}

      {/* The hosted `AiPanel` body — always rendered. When collapsed it is
          hidden (`hidden` → `display: none`) so it takes no layout space, but
          it stays in the tree so the conversation and ACP session survive a
          toggle. */}
      <div className="min-h-0 flex-1" hidden={!open}>
        {children}
      </div>
    </div>
  );
}
