import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import { useSpatialFocusActions } from "@/lib/spatial-focus-context";
import { useEntitiesByType } from "@/components/rust-engine-container";
import { InspectorFocusBridge } from "@/components/inspector-focus-bridge";
import { SlidePanel } from "@/components/slide-panel";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { FocusLayer, useCurrentLayerKey } from "@/components/focus-layer";
import { FocusZone, useParentZoneKey } from "@/components/focus-zone";
import type { Entity, EntityBag } from "@/types/kanban";
import { entityFromBag, getStr } from "@/types/kanban";
import { asLayerName, asMoniker } from "@/types/spatial";

const PANEL_WIDTH = 420;

/**
 * Identity-stable `LayerName` for the inspector overlay layer.
 *
 * Pulled to module scope so re-renders never mint a fresh value — the
 * `<FocusLayer>` push effect depends on `name`, and a fresh literal in JSX
 * would force an unnecessary tear-down / re-push of the inspector layer
 * on every parent render.
 */
const INSPECTOR_LAYER_NAME = asLayerName("inspector");

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
 * Spatial-nav structure: when the panel stack is non-empty, the rendered
 * panel list is wrapped in a single `<FocusLayer name="inspector">`
 * whose `parentLayerKey` is the enclosing window-root layer (read here
 * via `useCurrentLayerKey()` while the window layer is still a direct
 * React ancestor — the inspector layer is itself a sibling of the
 * board content, so the explicit parent is required to avoid the layer
 * being mistaken for a second window root). Each `InspectorPanel`
 * registers a `<FocusZone>` *inside* its `<SlidePanel>` whose moniker
 * is `panel:${entityType}:${entityId}` — that's the zone the Rust
 * spatial graph tracks per panel for `last_focused` memory and
 * cross-zone leaf fallback between adjacent panels.
 *
 * The panel zone wrap lives **inside** the SlidePanel rather than around
 * it because `SlidePanel` is `position: fixed`: a wrapper outside it
 * collapses to zero size and the `<FocusIndicator>` painted inside that
 * wrapper has no visible box to anchor to. Putting the zone inside the
 * panel body lets the indicator paint at the panel's own left edge, which
 * is the affordance users see when drill-out lands focus on the panel.
 *
 * Focus claim on mount: a `<ClaimPanelFocusOnMount>` helper rendered
 * inside each panel's `<FocusZone>` calls `spatial_focus(panelKey)` once
 * on first mount, advancing the kernel's focused key from the source
 * element (the navbar Inspect button, a card, a perspective tab, …)
 * into the new panel zone. Without this, drill-out from Escape walks
 * the source element's zone chain (e.g. `task:T1A` → `column:TODO` →
 * `ui:board` → null → dismiss) before dismiss fires — three Escapes
 * for a card-driven open, two for a navbar-driven open. With it, the
 * panel zone is at the layer root, drill-out returns null, and the
 * first Escape dismisses. See card 01KQ9Z9VN6EXM9JWJRNM5T7T19.
 *
 * Owns:
 * - panelStack state synced from UIState inspector_stack
 * - Close handlers dispatching ui.inspector.close and ui.inspector.close_all
 * - Backdrop overlay rendering
 * - Inspector layer mount when any panel is open
 * - Panel stack rendering with offset
 * - InspectorPanel component (entity resolution + SlidePanel + per-panel
 *   FocusZone registration + ClaimPanelFocusOnMount helper)
 */
export function InspectorsContainer() {
  const uiState = useUIState();
  const entitiesByType = useEntitiesByType();
  const entityStore = useMemo(() => entitiesByType, [entitiesByType]);

  // Read the window-root layer key here — this component is mounted as a
  // direct child of the window's `<FocusLayer name="window">` in App.tsx,
  // so the surrounding context has it. We forward it explicitly to the
  // inspector layer so the parent link survives even if the panels were
  // ever portaled (and so the Rust registry sees the inspector layer as a
  // child of the window root rather than minting a second root).
  const windowLayerKey = useCurrentLayerKey();

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

  /** Close the topmost inspector panel via the command architecture. */
  const dispatchInspectorClose = useDispatchCommand("ui.inspector.close");
  const closeTopPanel = useCallback(() => {
    dispatchInspectorClose().catch((e) =>
      console.error("ui.inspector.close failed:", e),
    );
  }, [dispatchInspectorClose]);

  /** Close all inspector panels via the command architecture. */
  const dispatchInspectorCloseAll = useDispatchCommand(
    "ui.inspector.close_all",
  );
  const closeAll = useCallback(() => {
    dispatchInspectorCloseAll().catch((e) =>
      console.error("ui.inspector.close_all failed:", e),
    );
  }, [dispatchInspectorCloseAll]);

  const hasPanels = panelStack.length > 0;

  /**
   * The rendered panel list. Each `<InspectorPanel>` mounts its own
   * `<FocusZone moniker="panel:${entityType}:${entityId}">` *inside*
   * the SlidePanel, so the zone becomes a navigable container inside
   * the inspector layer with a visible indicator anchored to the
   * panel body's left edge (see `InspectorPanel` for the rationale).
   * Cross-zone leaf fallback (beam rule 2) handles arrowing between
   * adjacent open panels — no special case needed.
   */
  const panelNodes = panelStack.map((entry, index) => {
    const rightOffset = (panelStack.length - 1 - index) * PANEL_WIDTH;
    return (
      <InspectorPanel
        key={`${entry.entityType}-${entry.entityId}`}
        entry={entry}
        entityStore={entityStore}
        onClose={closeTopPanel}
        style={{ right: rightOffset }}
      />
    );
  });

  return (
    <>
      {/* Backdrop — visible when any panel is open */}
      <div
        className={`fixed inset-0 z-20 bg-black/20 transition-opacity duration-200 ${
          hasPanels ? "opacity-100" : "opacity-0 pointer-events-none"
        }`}
        onClick={closeAll}
      />

      {/* The inspector layer mounts only while at least one panel is open.
          On unmount the Rust side pops the layer and emits the parent's
          `last_focused`, which restores focus to whatever was focused on
          the board before the first panel opened. */}
      {hasPanels && (
        <FocusLayer name={INSPECTOR_LAYER_NAME} parentLayerKey={windowLayerKey}>
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
}

/**
 * Resolves an entity for the inspector panel. Tries the local entity store
 * first, then falls back to fetching from the backend via get_entity.
 *
 * Spatial-nav: registers the per-panel `<FocusZone>` *inside* the
 * `<SlidePanel>` so it has a real layout box and the panel-edge focus
 * indicator can paint against the panel body's left edge. The
 * SlidePanel itself is `position: fixed` — wrapping it from outside
 * collapses the wrapper to zero size, leaving the indicator without a
 * visible host. The zone moniker is `panel:${entityType}:${entityId}`,
 * the same identity the inspector layer's panel stack uses for
 * `last_focused` memory and the cross-zone leaf fallback that picks up
 * an adjacent panel when the topmost one closes.
 *
 * The zone wrap is rendered identically in the loading and resolved
 * branches so the panel zone exists from the moment the panel mounts —
 * drill-out / cross-panel fallback never sees a window where the zone
 * is missing while the entity is being fetched.
 *
 * Focus claim on mount: a `<ClaimPanelFocusOnMount>` helper rendered
 * inside the `<FocusZone>` calls `spatial_focus(panelKey)` once on
 * first mount. See its component-level doc for the rationale and the
 * timing argument; see card 01KQ9Z9VN6EXM9JWJRNM5T7T19 for the bug it
 * pins.
 *
 * Focus restore on unmount is handled by the enclosing inspector
 * `<FocusLayer>` (which pops in the Rust registry and emits the parent
 * layer's `last_focused`) and by each panel's own `<FocusZone>` wrapper
 * (per-panel `last_focused` memory). The legacy `useRestoreFocus()`
 * hook that this component used to call is therefore no longer needed —
 * see card 01KNQXYC4RBQP1N2NQ33P8DPB9.
 */
function InspectorPanel({
  entry,
  entityStore,
  onClose,
  style,
}: InspectorPanelProps) {
  const { getSchema } = useSchema();
  const [fetchedEntity, setFetchedEntity] = useState<Entity | null>(null);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);

  // Per-panel zone moniker — identifies this panel's slot in the
  // inspector layer's child list for `last_focused` memory and
  // cross-zone leaf fallback. Memoised so the brand cast does not mint
  // a fresh-identity Moniker on every render (which would churn the
  // FocusZone's register effect).
  const panelMoniker = useMemo(
    () => asMoniker(`panel:${entry.entityType}:${entry.entityId}`),
    [entry.entityType, entry.entityId],
  );

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

  // Body content \u2014 either the loading/error message or the inspector
  // bridge. Both branches render inside the same FocusZone so the
  // panel zone is registered for the panel's full lifetime, not just
  // after the entity resolves.
  const body = !resolved ? (
    <p className="text-sm text-muted-foreground">
      {fetchError ? `Entity not found` : "Loading\u2026"}
    </p>
  ) : (
    <ErrorBoundary>
      <InspectorFocusBridge entity={resolved} />
    </ErrorBoundary>
  );

  return (
    <SlidePanel open={true} onClose={onClose} style={style}>
      {/* `min-h-full` lets the zone fill the SlidePanel body so a
          click anywhere on the panel content focuses the panel zone,
          not just where the inspector body itself paints. */}
      <FocusZone moniker={panelMoniker} className="min-h-full">
        <ClaimPanelFocusOnMount />
        {body}
      </FocusZone>
    </SlidePanel>
  );
}

/**
 * Side-effect-only child of an `<InspectorPanel>`'s `<FocusZone>` that
 * moves spatial focus to the enclosing panel zone once on first mount.
 *
 * Without this, dispatching `ui.inspect` from a source element (a card,
 * the navbar Inspect button, a perspective tab, …) leaves the kernel's
 * focused key on the source. Pressing Escape then walks the source
 * element's zone chain via drill-out before the chain falls through to
 * `app.dismiss` — three Escapes for a card-driven open, two for a
 * navbar-driven open. With this helper, the kernel's focused key
 * advances to the panel zone immediately after the panel mounts, so the
 * very first Escape's drill-out lands at the layer root (returns null)
 * and dismiss fires.
 *
 * # Why a child of the zone, not a sibling
 *
 * `<FocusZone>` mints its `SpatialKey` internally and exposes it only
 * via the `FocusZoneContext` it pushes around its children. A descendant
 * therefore reads the panel zone's `SpatialKey` through
 * `useParentZoneKey()`. A *sibling* would have no way to discover the
 * key.
 *
 * # Why the focus call is deferred via `queueMicrotask`
 *
 * React's effect ordering on mount is bottom-up: child effects fire
 * **before** parent effects. The parent `<FocusZone>` registers itself
 * with the spatial registry from its own `useEffect` (which calls
 * `invoke("spatial_register_zone", …)`). If this child effect were to
 * call `focus(panelKey)` synchronously, the resulting
 * `invoke("spatial_focus", …)` IPC message would be queued **before**
 * the register IPC, and the kernel would see a focus call for an
 * unknown key (no-op).
 *
 * `queueMicrotask` defers the focus call to the microtask queue. By the
 * time that microtask drains, every effect in the current commit phase
 * has fired — including the parent zone's register effect, which
 * synchronously enqueues `invoke("spatial_register_zone", …)` before
 * suspending at its first `await`. The focus IPC therefore lands on
 * Tauri's serial command channel **after** the register IPC, and the
 * Rust kernel processes them in order under the same `with_spatial`
 * mutex.
 *
 * # First-mount only
 *
 * The `useEffect` body fires once per mount (the dep list captures
 * stable identities — the panel zone's `SpatialKey` is minted once and
 * `useSpatialFocusActions()` returns an identity-stable bag for the
 * provider's lifetime). `<InspectorsContainer>` keys each panel
 * `<InspectorPanel>` by `${entityType}-${entityId}`, so opening a
 * different entity remounts a fresh panel and this helper fires again
 * for the new zone — which is exactly the behavior we want when the
 * user drills from one inspector panel to another. Re-renders that
 * don't change the panel identity (a UIState delta unrelated to the
 * inspector stack, an entity-store refresh, etc.) leave focus alone,
 * so the kernel's `last_focused` memory inside the panel is preserved
 * across them.
 */
function ClaimPanelFocusOnMount(): null {
  const panelKey = useParentZoneKey();
  const { focus } = useSpatialFocusActions();

  useEffect(() => {
    // No enclosing zone means we are mounted outside the spatial-nav
    // stack (degraded test or a misconfigured tree). Nothing to focus.
    if (!panelKey) return;

    // Defer to the next microtask so the parent `<FocusZone>`'s register
    // effect — which fires AFTER this child effect — has a chance to
    // synchronously enqueue `spatial_register_zone(panelKey, …)` before
    // we enqueue `spatial_focus(panelKey)`. See the component-level doc
    // comment for the full ordering argument.
    queueMicrotask(() => {
      focus(panelKey).catch((err) =>
        console.error("[InspectorPanel] focus on mount failed:", err),
      );
    });
  }, [panelKey, focus]);

  return null;
}
