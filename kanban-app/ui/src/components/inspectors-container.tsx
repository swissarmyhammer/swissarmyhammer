import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import { useEntitiesByType } from "@/components/rust-engine-container";
import { InspectorFocusBridge } from "@/components/inspector-focus-bridge";
import { SlidePanel } from "@/components/slide-panel";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { FocusLayer, useCurrentLayerKey } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
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
 * inside that layer is wrapped in a `<FocusScope kind="zone">` whose
 * moniker is `panel:${entityType}:${entityId}` — that's the zone the
 * Rust spatial graph tracks per panel for `last_focused` memory and
 * cross-zone leaf fallback between adjacent panels.
 *
 * Owns:
 * - panelStack state synced from UIState inspector_stack
 * - Close handlers dispatching ui.inspector.close and ui.inspector.close_all
 * - Backdrop overlay rendering
 * - Inspector layer mount when any panel is open
 * - Panel stack rendering with offset, each wrapped as its own zone
 * - InspectorPanel component (entity resolution + SlidePanel)
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
   * The rendered panel list, with each panel wrapped in a
   * `<FocusScope kind="zone">` so it becomes a navigable zone inside the
   * inspector layer. Cross-zone leaf fallback (beam rule 2) handles
   * arrowing between adjacent open panels — no special case needed.
   */
  const panelNodes = panelStack.map((entry, index) => {
    const rightOffset = (panelStack.length - 1 - index) * PANEL_WIDTH;
    const panelMoniker = asMoniker(
      `panel:${entry.entityType}:${entry.entityId}`,
    );
    return (
      <FocusScope
        key={`${entry.entityType}-${entry.entityId}`}
        kind="zone"
        moniker={panelMoniker}
        showFocusBar={false}
      >
        <InspectorPanel
          entry={entry}
          entityStore={entityStore}
          onClose={closeTopPanel}
          style={{ right: rightOffset }}
        />
      </FocusScope>
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
 * Focus restore on unmount is now handled by the enclosing inspector
 * `<FocusLayer>` (which pops in the Rust registry and emits the parent
 * layer's `last_focused`) and by each panel's `<FocusScope kind="zone">`
 * wrapper (per-panel `last_focused` memory). The legacy
 * `useRestoreFocus()` hook that this component used to call is therefore
 * no longer needed — see card 01KNQXYC4RBQP1N2NQ33P8DPB9.
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

  if (!resolved) {
    return (
      <SlidePanel open={true} onClose={onClose} style={style}>
        <p className="text-sm text-muted-foreground">
          {fetchError ? `Entity not found` : "Loading\u2026"}
        </p>
      </SlidePanel>
    );
  }

  return (
    <SlidePanel open={true} onClose={onClose} style={style}>
      <ErrorBoundary>
        <InspectorFocusBridge entity={resolved} />
      </ErrorBoundary>
    </SlidePanel>
  );
}
