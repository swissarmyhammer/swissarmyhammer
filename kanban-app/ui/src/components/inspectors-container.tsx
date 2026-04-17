import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import { useEntitiesByType } from "@/components/rust-engine-container";
import { InspectorFocusBridge } from "@/components/inspector-focus-bridge";
import { SlidePanel } from "@/components/slide-panel";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import type { Entity, EntityBag } from "@/types/kanban";
import { entityFromBag, getStr } from "@/types/kanban";

const PANEL_WIDTH = 420;

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
 * Owns:
 * - panelStack state synced from UIState inspector_stack
 * - Close handlers dispatching ui.inspector.close and ui.inspector.close_all
 * - Backdrop overlay rendering
 * - Panel stack rendering with offset
 * - InspectorPanel component (entity resolution + SlidePanel)
 */
export function InspectorsContainer() {
  const uiState = useUIState();
  const entityStore = useEntitiesByType();

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

  return (
    <>
      {/* Backdrop — visible when any panel is open */}
      <div
        className={`fixed inset-0 z-20 bg-black/20 transition-opacity duration-200 ${
          panelStack.length > 0
            ? "opacity-100"
            : "opacity-0 pointer-events-none"
        }`}
        onClick={closeAll}
      />

      {/* Render inspector panels from the stack */}
      {panelStack.map((entry, index) => {
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
      })}
    </>
  );
}

/**
 * Resolve an entity by type+id: try the local entity store first,
 * then fall back to fetching from the Rust backend via `get_entity`.
 */
function useResolvedEntity(
  entityType: string,
  entityId: string,
  entityStore: Record<string, Entity[]>,
): { entity: Entity | null; fetchError: string | null } {
  const { getSchema } = useSchema();
  const [fetchedEntity, setFetchedEntity] = useState<Entity | null>(null);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);

  const entities = entityStore[entityType];
  let localEntity = entities?.find((e) => e.id === entityId);
  if (!localEntity) {
    const displayField = getSchema(entityType)?.entity.search_display_field;
    if (displayField) {
      localEntity = entities?.find((e) => getStr(e, displayField) === entityId);
    }
  }
  const resolved = localEntity ?? fetchedEntity;
  const fetchKey = `${entityType}:${entityId}`;

  useEffect(() => { fetchedRef.current = null; }, [fetchKey]);

  useEffect(() => {
    if (resolved || fetchedRef.current === fetchKey) return;
    fetchedRef.current = fetchKey;
    setFetchError(null);
    invoke<Record<string, unknown>>("get_entity", { entityType, id: entityId })
      .then((bag) => setFetchedEntity(entityFromBag(bag as EntityBag)))
      .catch((err) => {
        console.error(`[InspectorPanel] Failed to fetch entity: ${fetchKey}`, err);
        setFetchError(String(err));
      });
  }, [resolved, fetchKey, entityType, entityId]);

  return { entity: resolved, fetchError };
}

/** Props for the InspectorPanel component. */
interface InspectorPanelProps {
  entry: PanelEntry;
  entityStore: Record<string, Entity[]>;
  onClose: () => void;
  style?: React.CSSProperties;
}

/** Renders a single inspector panel with entity resolution and error handling. */
function InspectorPanel({ entry, entityStore, onClose, style }: InspectorPanelProps) {
  const { entity, fetchError } = useResolvedEntity(entry.entityType, entry.entityId, entityStore);

  if (!entity) {
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
        <InspectorFocusBridge entity={entity} />
      </ErrorBoundary>
    </SlidePanel>
  );
}
