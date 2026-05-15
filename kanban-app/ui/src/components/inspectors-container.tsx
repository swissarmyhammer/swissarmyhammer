import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import { useEntitiesByType } from "@/components/rust-engine-container";
import { EntityInspector } from "@/components/entity-inspector";
import { SlidePanel } from "@/components/slide-panel";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { FocusLayer } from "@/components/focus-layer";
import { useFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import type { Entity, EntityBag } from "@/types/kanban";
import { entityFromBag, getStr } from "@/types/kanban";
import { asSegment } from "@/types/spatial";

/**
 * Default panel width applied when `WindowState.inspector_width` is
 * `undefined` (fresh window, never resized). Mirrors the previous
 * fixed `w-[420px]` Tailwind class used before the resizable inspector
 * landed in 01KQSE8TT79XC3KJGEHX6DW99G.
 */
const DEFAULT_PANEL_WIDTH = 420;

/**
 * Identity-stable `LayerName` for the inspector overlay layer.
 *
 * Pulled to module scope so re-renders never mint a fresh value — the
 * `<FocusLayer>` push effect depends on `name`, and a fresh literal in JSX
 * would force an unnecessary tear-down / re-push of the inspector layer
 * on every parent render.
 */
const INSPECTOR_LAYER_NAME = asSegment("inspector");

/** Window label for per-window state persistence. */
const WINDOW_LABEL = getCurrentWindow().label;

/** A panel entry is just an entity reference — entity type + id. */
interface PanelEntry {
  entityType: string;
  entityId: string;
}

/**
 * Parses the backend inspector_stack monikers (e.g. "task:t1") into
 * PanelEntry objects. Returns an empty array if the stack is undefined.
 */
function parsePanelStack(inspectorStack: string[] | undefined): PanelEntry[] {
  if (!inspectorStack) return [];
  const entries: PanelEntry[] = [];
  for (const m of inspectorStack) {
    const sep = m.indexOf(":");
    if (sep < 0) continue;
    entries.push({
      entityType: m.slice(0, sep),
      entityId: m.slice(sep + 1),
    });
  }
  return entries;
}

/**
 * Overlay container that owns the inspector panel stack.
 *
 * Reads the UIState inspector_stack for the current window and renders
 * a backdrop + stacked SlidePanel components for each entry. This is a
 * sibling/overlay alongside the main content, NOT wrapping it.
 *
 * # Spatial-nav structure
 *
 * When the panel stack is non-empty, the rendered panel list is wrapped
 * in a single `<FocusLayer name="inspector">` whose `parentLayerFq` is
 * the enclosing window-root layer (read here via `useFullyQualifiedMoniker()`
 * while the window layer is still a direct React ancestor — the
 * inspector layer is itself a sibling of the board content, so the
 * explicit parent is required to avoid the layer being mistaken for a
 * second window root).
 *
 * Card `01KR7CDEFWWVF4WH0BCHE8Y21J` flattened the inspector layer:
 * panel bodies are NO LONGER wrapped in a per-panel `<FocusScope>` /
 * `<FocusZone>` / `<FocusLayer>`. Field rows' `<FocusScope>`s register
 * directly under the inspector layer's `LayerScopeRegistry`. With the
 * modal-layer model, jump-to enumerates against the topmost layer (the
 * inspector layer when it's on top), and cardinal nav is confined to
 * that layer's flat scope list — no per-entity barrier is needed.
 *
 * Predecessor cards `01KQCTJY1QZ710A05SE975GHNR` and
 * `01KQFCQ9QMQKCDYVWGTXSVK5PZ` introduced and tweaked per-panel zones;
 * the iter-1 attempt under this card promoted those to per-panel
 * layers. Both shapes are now reverted: only the single
 * `<FocusLayer name="inspector">` boundary remains. The
 * `<InspectorFocusBridge>` and the
 * `inspector.edit/editEnter/exitEdit` commands stay deleted (per
 * `01KQCTJY1QZ710A05SE975GHNR`).
 *
 * # First-field focus on panel mount
 *
 * `<EntityInspector>`'s `useFirstFieldFocus` hook runs once on first
 * mount and dispatches `setFocus(field:type:id.<first-field-name>)`
 * via the entity-focus bridge. Without it, drill-out from Escape
 * walks the source element's zone chain (e.g. `task:T1A` → `column:TODO`
 * → `ui:board` → null → dismiss) before dismiss fires. With it, focus
 * advances to a field at the layer root, drill-out hits the layer
 * boundary, and the first Escape dismisses (`nav.drillOut` echoes,
 * the React chain falls through to `app.dismiss`).
 *
 * Owns:
 * - panelStack state synced from UIState inspector_stack
 * - Close handlers dispatching ui.inspector.close and ui.inspector.close_all
 * - Backdrop overlay rendering
 * - Inspector layer mount when any panel is open
 * - Panel stack rendering with offset
 * - InspectorPanel component (entity resolution + SlidePanel)
 */
export function InspectorsContainer() {
  const uiState = useUIState();
  const entitiesByType = useEntitiesByType();
  const entityStore = useMemo(() => entitiesByType, [entitiesByType]);

  // Read the window-root layer FQM here — this component is mounted as a
  // direct child of the window's `<FocusLayer name="window">` in App.tsx,
  // so the surrounding context has it. We forward it explicitly to the
  // inspector layer so the parent link survives even if the panels were
  // ever portaled (and so the Rust registry sees the inspector layer as a
  // child of the window root rather than minting a second root).
  const windowLayerFq = useFullyQualifiedMoniker();

  // Derive panel stack from UIState
  const winState = uiState.windows?.[WINDOW_LABEL];
  const inspectorStack = winState?.inspector_stack;
  const [panelStack, setPanelStack] = useState<PanelEntry[]>([]);

  // Sync backend inspector_stack to local panelStack state.
  // Context menu and palette dispatches go directly to the Rust backend
  // (bypassing React command callbacks), so we reactively read UIState.
  useEffect(() => {
    setPanelStack(parsePanelStack(inspectorStack));
  }, [inspectorStack]);

  /**
   * Live panel width applied to every inspector in the stack.
   *
   * Reads the persisted `inspector_width` from `WindowState`, falling
   * back to the historical 420 px default when the user hasn't resized
   * yet. During a drag, transient updates flow through `liveWidth`
   * (`onResize` callback) so every panel re-renders at 60 fps without
   * any backend round-trip; only the final value is dispatched on
   * `mouseup` via `ui.inspector.set_width`. The dispatch echoes back as
   * an `inspector_width` `ui-state-changed` event, which updates
   * `winState.inspector_width` and clears the transient state on the
   * next render.
   */
  const persistedWidth = winState?.inspector_width ?? DEFAULT_PANEL_WIDTH;
  const [liveWidth, setLiveWidth] = useState<number | null>(null);
  // Tracks whether a drag is currently in progress. Held in a ref —
  // not reactive state — because the reset effect below must read the
  // *current* drag status without re-triggering itself when the flag
  // flips. `handleResize` sets it on the first move; `handleResizeEnd`
  // clears it on release.
  const isDraggingRef = useRef(false);
  // Reset transient state whenever the persisted value changes (e.g.
  // after dispatch echoes back through ui-state-changed) — otherwise a
  // stale `liveWidth` would shadow the new persisted value. Skip the
  // reset while a drag is in progress so a backend echo arriving
  // mid-drag (e.g. a future cross-window broadcast) does not clobber
  // the in-flight drag with a snap-back to the persisted value.
  useEffect(() => {
    if (isDraggingRef.current) return;
    setLiveWidth(null);
  }, [persistedWidth]);
  const effectiveWidth = liveWidth ?? persistedWidth;

  const dispatchSetWidth = useDispatchCommand("ui.inspector.set_width");
  const handleResize = useCallback((next: number) => {
    isDraggingRef.current = true;
    setLiveWidth(next);
  }, []);
  const handleResizeEnd = useCallback(
    (final: number) => {
      isDraggingRef.current = false;
      dispatchSetWidth({ args: { width: final } }).catch((e) =>
        console.error("ui.inspector.set_width failed:", e),
      );
    },
    [dispatchSetWidth],
  );

  /** Close the topmost inspector panel via the command architecture. */
  const dispatchInspectorClose = useDispatchCommand("ui.inspector.close");
  const closeTopPanel = useCallback(() => {
    dispatchInspectorClose().catch((e) =>
      console.error("ui.inspector.close failed:", e),
    );
  }, [dispatchInspectorClose]);

  /**
   * Backdrop click → dispatch `app.dismiss`.
   *
   * The backdrop covers the area outside the active panel — clicking
   * there is the modal-layer equivalent of "click outside, close the
   * topmost layer". Per the modal-layer model (card
   * `01KR7CDEFWWVF4WH0BCHE8Y21J`), `app.dismiss` is the single
   * top-layer-aware command: with the inspector on top it pops the
   * topmost panel; with another layer above it (palette, jump-to)
   * the dismiss closes that layer first.
   *
   * Replaces the earlier hard-coded `dispatchInspectorCloseAll` —
   * which assumed the inspector was always the active layer — so the
   * backend's topmost-layer routing decides the actual close.
   */
  const dispatchAppDismiss = useDispatchCommand("app.dismiss");
  const handleBackdropClick = useCallback(() => {
    dispatchAppDismiss().catch((e) =>
      console.error("app.dismiss (inspector backdrop) failed:", e),
    );
  }, [dispatchAppDismiss]);

  const hasPanels = panelStack.length > 0;

  /**
   * The rendered panel list. Each `<InspectorPanel>` renders its body
   * directly inside a `<SlidePanel>` — no per-panel `<FocusScope>` /
   * `<FocusZone>` / `<FocusLayer>` wrap. Field rows inside register
   * their own `<FocusScope>`s directly under the surrounding
   * `<FocusLayer name="inspector">`'s `LayerScopeRegistry`.
   *
   * Cardinal nav, jump-to enumeration, and drill-out all operate on
   * the inspector layer's flat scope list. See card
   * `01KR7CDEFWWVF4WH0BCHE8Y21J`.
   */
  const panelNodes = panelStack.map((entry, index) => {
    const rightOffset = (panelStack.length - 1 - index) * effectiveWidth;
    return (
      <InspectorPanel
        key={`${entry.entityType}-${entry.entityId}`}
        entry={entry}
        entityStore={entityStore}
        onClose={closeTopPanel}
        style={{ right: rightOffset }}
        width={effectiveWidth}
        onResize={handleResize}
        onResizeEnd={handleResizeEnd}
      />
    );
  });

  return (
    <>
      {/* Backdrop — only mounted while a panel is open.
          `position: fixed` + numeric `z-index` always creates a stacking
          context (per CSS spec), so an always-mounted z-20 transparent
          backdrop covering the viewport would suppress sibling overlays
          at lower z-indices in the closed-inspector state — including
          the navbar's window-layer focus-debug overlays at z-15. The
          fade-in on open is preserved by `transition-opacity` plus the
          initial render at `opacity-100`; the fade-out on close is
          intentionally dropped (the SlidePanel's slide-out animation
          is the user-visible signal). */}
      {hasPanels && (
        <div
          className="fixed inset-0 z-20 bg-black/20 opacity-100 transition-opacity duration-200"
          onClick={handleBackdropClick}
        />
      )}

      {/* The inspector layer mounts only while at least one panel is open.
          On unmount the Rust side pops the layer and emits the parent's
          `last_focused`, which restores focus to whatever was focused on
          the board before the first panel opened. */}
      {hasPanels && (
        <FocusLayer name={INSPECTOR_LAYER_NAME} parentLayerFq={windowLayerFq}>
          {panelNodes}
        </FocusLayer>
      )}
    </>
  );
}

/** Props for the InspectorPanel component. */
interface InspectorPanelProps {
  entry: PanelEntry;
  entityStore: Record<string, Entity[]>;
  onClose: () => void;
  style?: React.CSSProperties;
  /** Current panel width in CSS pixels. Forwarded to the SlidePanel. */
  width: number;
  /** Forwarded to the SlidePanel's left-edge resize handle. */
  onResize: (next: number) => void;
  /** Forwarded to the SlidePanel — fires once on mouseup with the final width. */
  onResizeEnd: (final: number) => void;
}

/**
 * Resolves an entity for the inspector panel. Tries the local entity store
 * first, then falls back to fetching from the backend via get_entity.
 *
 * # Spatial-nav participation
 *
 * The panel body renders directly inside the `<SlidePanel>` — there
 * is NO per-panel `<FocusScope>` / `<FocusZone>` / `<FocusLayer>`
 * wrap. Field zones inside `<EntityInspector>` register their
 * `<FocusScope>`s directly under the surrounding
 * `<FocusLayer name="inspector">`'s `LayerScopeRegistry`. Cardinal
 * navigation between fields resolves entirely within that single
 * registry's flat scope list, jump-to enumeration runs against that
 * same registry, and drill-out walks straight to the layer root.
 *
 * Per-panel structural barriers (initially `<FocusScope>`, then
 * iter-1's `<FocusLayer>` per panel) were removed by card
 * `01KR7CDEFWWVF4WH0BCHE8Y21J` once the modal-layer model put the
 * containment promise on the topmost-layer rule rather than per-panel
 * fences. The `<InspectorFocusBridge>` and the
 * `inspector.edit/editEnter/exitEdit` commands stay deleted (per
 * `01KQCTJY1QZ710A05SE975GHNR`).
 *
 * # First-field focus claim on mount
 *
 * `<EntityInspector>`'s own `useFirstFieldFocus` hook captures the
 * previously-focused moniker once on first mount and dispatches
 * `setFocus(field:type:id.<first-field-name>)` to the entity-focus
 * bridge. Without that claim, drill-out from Escape would walk the
 * source element's zone chain (a card, the navbar, a perspective tab)
 * before hitting the layer boundary; with it, focus is at the
 * inspector layer root already and the first Escape dismisses (per
 * `01KQ9Z9VN6EXM9JWJRNM5T7T19`).
 *
 * # Focus restore on unmount
 *
 * Closing the topmost panel unmounts its `<FocusLayer>` (when it was
 * the only panel) or unregisters its field zones (when other panels
 * remain). The Rust kernel emits the parent layer's `last_focused` on
 * pop, restoring focus to whatever was focused before the panel stack
 * opened. See card `01KNQXYC4RBQP1N2NQ33P8DPB9` for the prior cleanup
 * that removed `useRestoreFocus()`.
 */
function InspectorPanel({
  entry,
  entityStore,
  onClose,
  style,
  width,
  onResize,
  onResizeEnd,
}: InspectorPanelProps) {
  const { getSchema } = useSchema();
  const [fetchedEntity, setFetchedEntity] = useState<Entity | null>(null);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);

  // Try local store first — match by ID, then by search_display_field from schema
  const entities = entityStore[entry.entityType];
  let localEntity = entities?.find((e) => e.id === entry.entityId);
  if (!localEntity) {
    const displayField = getSchema(entry.entityType)?.entity
      .search_display_field;
    if (displayField) {
      localEntity = entities?.find(
        (e) => getStr(e, displayField) === entry.entityId,
      );
    }
  }
  const resolved = localEntity ?? fetchedEntity;

  // Fetch from backend if not found locally
  const fetchKey = `${entry.entityType}:${entry.entityId}`;

  // Reset fetch dedup ref when the target entity changes so a new
  // fetch can be attempted (e.g. after a failed fetch for a different entity).
  useEffect(() => {
    fetchedRef.current = null;
  }, [fetchKey]);

  useEffect(() => {
    if (resolved || fetchedRef.current === fetchKey) return;
    fetchedRef.current = fetchKey;
    setFetchError(null);
    invoke<Record<string, unknown>>("get_entity", {
      entityType: entry.entityType,
      id: entry.entityId,
    })
      .then((bag) => {
        setFetchedEntity(entityFromBag(bag as EntityBag));
      })
      .catch((err) => {
        const msg = String(err);
        console.error(
          `[InspectorPanel] Failed to fetch entity: ${fetchKey}`,
          err,
        );
        setFetchError(msg);
      });
  }, [resolved, fetchKey, entry.entityType, entry.entityId]);

  // First-field focus on mount is owned by `<EntityInspector>`'s
  // `useFirstFieldFocus` hook — it captures the previously-focused
  // moniker on first mount and dispatches `setFocus(firstField)` once
  // the schema's first field is resolved, then restores prev focus on
  // unmount. The behavior the deleted `<ClaimPanelFocusOnMount>` was
  // built for (advance focus into the inspector layer immediately so
  // the first Escape dismisses) is now satisfied by that hook with no
  // panel-zone intermediary.

  const body = !resolved ? (
    <p className="text-sm text-muted-foreground">
      {fetchError ? `Entity not found` : "Loading\u2026"}
    </p>
  ) : (
    <ErrorBoundary>
      <EntityInspector entity={resolved} />
    </ErrorBoundary>
  );

  return (
    <SlidePanel
      open={true}
      onClose={onClose}
      style={style}
      width={width}
      onResize={onResize}
      onResizeEnd={onResizeEnd}
    >
      {body}
    </SlidePanel>
  );
}
