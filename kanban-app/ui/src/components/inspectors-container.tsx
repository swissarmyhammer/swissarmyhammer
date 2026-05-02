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
import { FocusZone } from "@/components/focus-zone";
import { useFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import type { Entity, EntityBag } from "@/types/kanban";
import { entityFromBag, getStr } from "@/types/kanban";
import { asSegment } from "@/types/spatial";

const PANEL_WIDTH = 420;

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
 * in a single `<FocusLayer name="inspector">` whose `parentLayerKey` is
 * the enclosing window-root layer (read here via `useCurrentLayerKey()`
 * while the window layer is still a direct React ancestor — the
 * inspector layer is itself a sibling of the board content, so the
 * explicit parent is required to avoid the layer being mistaken for a
 * second window root).
 *
 * Each open `<InspectorPanel>` wraps its body in an entity-keyed
 * `<FocusZone moniker={asSegment(\`${entityType}:${entityId}\`)}>` —
 * see card `01KQFCQ9QMQKCDYVWGTXSVK5PZ`. The zone segment is the
 * entity moniker itself (e.g. `task:T1`); there is no `panel:*`
 * indirection. Field zones inside the inspector register with
 * `parentZone === <entity-zone FQM>`, so:
 *
 *   - Iter 0 of the kernel's beam-search cascade is confined to peers
 *     within the same entity (ArrowDown at the last field of inspector
 *     A stays put — does NOT enter inspector B).
 *   - Iter 1 escalates to the entity zones themselves (siblings under
 *     the inspector layer root with `parentZone === null`), so
 *     ArrowLeft/Right between adjacent panels still resolves to a
 *     field in the spatially-nearest entity.
 *
 * Predecessor card `01KQCTJY1QZ710A05SE975GHNR` deleted the previous
 * `panel:type:id` zone (along with `<InspectorFocusBridge>` and the
 * `inspector.edit/editEnter/exitEdit` commands); those stay deleted.
 * This card walks back only the structural barrier, with the entity
 * moniker as identity instead of a panel-prefixed wrapper.
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
   * The rendered panel list. Each `<InspectorPanel>` wraps its body in
   * an entity-keyed `<FocusZone moniker={asSegment(\`${type}:${id}\`)}>`
   * inside the `<SlidePanel>`. Field zones register with
   * `parentZone === <entity-zone FQM>`, so iter 0 of the cascade is
   * confined to peers within the same entity and cross-panel nav
   * escalates to iter 1 (entity-zone peers under the inspector layer
   * root). See card `01KQFCQ9QMQKCDYVWGTXSVK5PZ`.
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
          onClick={closeAll}
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
}

/**
 * Resolves an entity for the inspector panel. Tries the local entity store
 * first, then falls back to fetching from the backend via get_entity.
 *
 * # Spatial-nav participation
 *
 * The panel body is wrapped in a single entity-keyed `<FocusZone>`
 * whose `moniker` segment is the entity moniker itself (e.g.
 * `task:T1`, `tag:bug`, `project:spatial-nav`) — NOT a `panel:`
 * prefix. The zone is a pure structural barrier: no commands, no
 * `navOverride`. Each field zone inside `<EntityInspector>` registers
 * with `parentZone === <this entity zone's FQM>`, which:
 *
 *   - Confines iter 0 of the kernel's beam-search cascade to peers
 *     within the same entity (so ArrowDown at the last field of
 *     inspector A stays put rather than crossing into inspector B's
 *     field zones).
 *   - Lets cross-entity ArrowLeft/Right escalate to iter 1 (entity-zone
 *     peers under the inspector layer root with `parentZone === null`),
 *     where beam search picks the spatially-nearest other entity zone
 *     and descends into its field zones.
 *
 * The `<InspectorFocusBridge>` that previously wrapped the inspector
 * in a `<FocusScope moniker={entityMoniker}>` plus the
 * `inspector.edit/editEnter/exitEdit` commands stay deleted (per
 * `01KQCTJY1QZ710A05SE975GHNR`); this wrap is a structural zone only,
 * keyed by the entity moniker rather than a `panel:` prefix.
 * See card `01KQFCQ9QMQKCDYVWGTXSVK5PZ` for the entity-zone barrier
 * design.
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
  // `useFirstFieldFocus` hook \u2014 it captures the previously-focused
  // moniker on first mount and dispatches `setFocus(firstField)` once
  // the schema's first field is resolved, then restores prev focus on
  // unmount. The behavior the deleted `<ClaimPanelFocusOnMount>` was
  // built for (advance focus into the inspector layer immediately so
  // the first Escape dismisses) is now satisfied by that hook with no
  // panel-zone intermediary.

  // The entity-keyed zone segment. The kernel registers this zone
  // under the inspector layer root, and `<EntityInspector>`'s field
  // zones land underneath with `parentZone === <this zone's FQM>`.
  // The segment is the entity moniker itself \u2014 see card
  // `01KQFCQ9QMQKCDYVWGTXSVK5PZ` for why we don't use a `panel:`
  // prefix.
  const entityZoneSegment = useMemo(
    () => asSegment(`${entry.entityType}:${entry.entityId}`),
    [entry.entityType, entry.entityId],
  );

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
    <SlidePanel open={true} onClose={onClose} style={style}>
      <FocusZone moniker={entityZoneSegment}>{body}</FocusZone>
    </SlidePanel>
  );
}
