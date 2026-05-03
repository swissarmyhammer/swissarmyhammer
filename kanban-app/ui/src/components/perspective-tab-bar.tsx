import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Filter, Group, Plus } from "lucide-react";
import { cn } from "@/lib/utils";
import { usePerspectives } from "@/lib/perspective-context";
import { useViews } from "@/lib/views-context";
import {
  useDispatchCommand,
  CommandScopeProvider,
  type CommandDef,
} from "@/lib/command-scope";
import { useContextMenu } from "@/lib/context-menu";
import { moniker } from "@/lib/moniker";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  FilterEditor,
  type FilterEditorHandle,
} from "@/components/filter-editor";
import { GroupSelector } from "@/components/group-selector";
import { TextEditor } from "@/components/fields/text-editor";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { FocusScope } from "@/components/focus-scope";
import { FocusZone } from "@/components/focus-zone";
import { Pressable } from "@/components/pressable";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";
import type { FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Start-rename callback registry — bridges AppShell command dispatch to the
// PerspectiveTabBar component that owns the rename state.
// ---------------------------------------------------------------------------

/**
 * Subscriber callback invoked when a "start rename" signal is broadcast.
 *
 * Receives an optional explicit perspective id. When `id` is undefined the
 * subscriber falls back to the active perspective — this is the path taken
 * by the global command palette's `ui.entity.startRename`, which has no
 * specific tab in mind. When `id` is supplied it targets that perspective
 * directly — this is the path taken by per-tab Enter, where the focused tab
 * (active or inactive) is the explicit rename target.
 */
type StartRenameCallback = (id?: string) => void;

/** Module-level subscriber set broadcasting rename signals to all mounted PerspectiveTabBar instances. */
const startRenameCallbacks = new Set<StartRenameCallback>();

/**
 * Subscribe to "start rename" signals.
 *
 * Called by `usePerspectiveTabBar` to enter rename mode when the command
 * palette (or any other source) dispatches `ui.entity.startRename`.
 *
 * @returns An unsubscribe function.
 */
export function onStartRename(cb: StartRenameCallback): () => void {
  startRenameCallbacks.add(cb);
  return () => {
    startRenameCallbacks.delete(cb);
  };
}

/**
 * Trigger all registered "start rename" callbacks.
 *
 * Pass an explicit `id` to start renaming a specific perspective (the
 * focused-tab path). Omit `id` to fall back to the active perspective (the
 * command-palette path).
 *
 * Intended to be called from AppShell's global command handler, the
 * focused-tab scope command, or tests.
 */
export function triggerStartRename(id?: string): void {
  for (const cb of startRenameCallbacks) cb(id);
}

// ---------------------------------------------------------------------------
// Rename hook — encapsulates inline rename state and dispatch
// ---------------------------------------------------------------------------

/** Manages inline rename state and commit logic for perspective tabs. */
function usePerspectiveRename() {
  const dispatchPerspectiveRename = useDispatchCommand("perspective.rename");
  const { refresh } = usePerspectives();
  const [renamingId, setRenamingId] = useState<string | null>(null);

  const startRename = useCallback((id: string) => {
    setRenamingId(id);
  }, []);

  const commitRename = useCallback(
    async (id: string, oldName: string, newName: string) => {
      console.warn("[rename] commitRename called", { id, oldName, newName });
      setRenamingId(null);
      const trimmed = newName.trim();
      if (!trimmed || trimmed === oldName) {
        console.warn("[rename] skipped — name unchanged or empty", {
          trimmed,
          oldName,
        });
        return;
      }
      try {
        console.warn("[rename] dispatching perspective.rename", {
          id,
          new_name: trimmed,
        });
        await dispatchPerspectiveRename({ args: { id, new_name: trimmed } });
        await refresh();
        console.warn("[rename] dispatch succeeded");
      } catch (e) {
        console.warn("[rename] dispatch FAILED", e);
      }
    },
    [dispatchPerspectiveRename, refresh],
  );

  const cancelRename = useCallback(() => setRenamingId(null), []);
  return { renamingId, startRename, commitRename, cancelRename };
}

// ---------------------------------------------------------------------------
// Tab bar state hook — derived state and refs for PerspectiveTabBar
// ---------------------------------------------------------------------------

/**
 * Collects all derived state, refs, and callbacks needed by PerspectiveTabBar.
 *
 * Extracted so the component JSX stays within readable length.
 */
function usePerspectiveTabBar() {
  const { perspectives, activePerspective, setActivePerspectiveId } =
    usePerspectives();
  const { activeView } = useViews();
  const { renamingId, startRename, commitRename, cancelRename } =
    usePerspectiveRename();
  const filterEditorRef = useRef<FilterEditorHandle>(null);
  const handleFilterFocus = useCallback(() => {
    filterEditorRef.current?.focus();
  }, []);
  const viewKind = activeView?.kind ?? "board";
  const filteredPerspectives = useMemo(
    () => perspectives.filter((p) => p.view === viewKind),
    [perspectives, viewKind],
  );

  // Subscribe to the module-level start-rename signal so the command palette
  // (via AppShell's global command) AND per-tab Enter (via the scope-pinned
  // `ui.entity.startRename` on each `<ScopedPerspectiveTab>`) can trigger
  // inline rename mode.
  //
  // When the broadcaster supplies an explicit `id` (per-tab path) we honor
  // it — that is the focused tab, active or not. When no id is supplied
  // (command-palette path) we fall back to the active perspective.
  useEffect(() => {
    return onStartRename((id) => {
      if (id) {
        startRename(id);
        return;
      }
      if (activePerspective) {
        startRename(activePerspective.id);
      }
    });
  }, [activePerspective, startRename]);

  return {
    activeView,
    activePerspective,
    setActivePerspectiveId,
    filteredPerspectives,
    viewKind,
    filterEditorRef,
    handleFilterFocus,
    renamingId,
    startRename,
    commitRename,
    cancelRename,
  };
}

// ---------------------------------------------------------------------------
// Main tab bar
// ---------------------------------------------------------------------------

/**
 * A compact tab bar that shows perspectives for the current view kind.
 *
 * Layout: tabs on the left (scrollable) + filter formula bar on the right.
 * The formula bar is an always-visible CM6 editor for the active perspective's
 * filter expression — analogous to Excel's formula bar. Clicking the filter
 * icon button on the active tab moves focus into the formula bar.
 *
 * Sits between the NavBar and the view content area.
 */
export function PerspectiveTabBar() {
  const {
    activeView,
    activePerspective,
    setActivePerspectiveId,
    filteredPerspectives,
    viewKind,
    filterEditorRef,
    handleFilterFocus,
    renamingId,
    startRename,
    commitRename,
    cancelRename,
  } = usePerspectiveTabBar();

  if (!activeView) return null;

  return (
    <PerspectiveBarSpatialZone>
      {/*
        Left: scrollable perspective tabs + add button.

        `pl-2` and `gap-2` are load-bearing — each tab is a `<FocusScope>`
        leaf and `<FocusIndicator>` paints an absolutely-positioned bar at
        `-left-2` (8px) of its host. The inner `overflow-x-auto` clips
        anything that overflows horizontally, so without `pl-2` the
        leftmost tab's indicator is clipped (and without `gap-2` the
        indicator on tabs 2..N would overlap the previous tab). Same
        pattern as the board's column strip — see `BoardDndWrapper` in
        `board-view.tsx` for the analogous `overflow-x-auto pl-2`.
      */}
      <div className="flex items-center gap-2 overflow-x-auto shrink-0 max-w-[60%] pl-2">
        {filteredPerspectives.map((p) => (
          <ScopedPerspectiveTab
            key={p.id}
            perspective={p}
            activePerspectiveId={activePerspective?.id}
            renamingId={renamingId}
            onSelect={() => setActivePerspectiveId(p.id)}
            onDoubleClick={() => startRename(p.id)}
            onRenameCommit={(text) => commitRename(p.id, p.name, text)}
            onRenameCancel={cancelRename}
            onFilterFocus={handleFilterFocus}
          />
        ))}
        <AddPerspectiveButton
          filteredPerspectives={filteredPerspectives}
          viewKind={viewKind}
        />
      </div>
      {/* Right: filter formula bar — always visible when a perspective is active */}
      {activePerspective && (
        <FilterFormulaBarFocusable
          key={activePerspective.id}
          perspectiveId={activePerspective.id}
        >
          <FilterFormulaBar
            ref={filterEditorRef}
            filter={activePerspective.filter}
            perspectiveId={activePerspective.id}
          />
        </FilterFormulaBarFocusable>
      )}
    </PerspectiveBarSpatialZone>
  );
}

/** Layout className shared by the spatial-zone wrapper and its plain-div fallback. */
const PERSPECTIVE_BAR_LAYOUT =
  "flex items-center border-b bg-muted/20 px-1 h-8 shrink-0";

/**
 * Wrap the perspective tab bar in a `<FocusZone moniker={asSegment("ui:perspective-bar")}>`
 * when the surrounding tree mounts the spatial-nav stack.
 *
 * `<FocusZone>` enforces a strict contract — it throws when no `<FocusLayer>`
 * ancestor is present. That contract is correct for the production tree
 * (`App.tsx` always mounts the providers) but would force every
 * `PerspectiveTabBar` unit test that doesn't care about spatial nav to
 * set up the providers. Conditionally rendering the zone when both context
 * lookups succeed keeps the strict contract intact for direct
 * `<FocusZone>` usage while letting the existing test suite keep its narrow
 * provider tree.
 *
 * The zone (or plain div fallback) carries the same layout class so the
 * `h-8 shrink-0` chain stays intact whether or not the spatial-nav stack is
 * present.
 */
function PerspectiveBarSpatialZone({ children }: { children: ReactNode }) {
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return <div className={PERSPECTIVE_BAR_LAYOUT}>{children}</div>;
  }
  return (
    <FocusZone
      moniker={asSegment("ui:perspective-bar")}
      // The bar is viewport-spanning chrome (full window width × 32px high) —
      // a focus indicator running across its entire row would be visual
      // noise. The bar's job in the spatial graph is to be the parent zone
      // for its tab leaves; the leaves themselves render the visible bar
      // when claimed. `data-focused` still flips on the wrapper for e2e
      // selectors / debugging.
      showFocusBar={false}
      className={PERSPECTIVE_BAR_LAYOUT}
    >
      {children}
    </FocusZone>
  );
}

// ---------------------------------------------------------------------------
// Scoped perspective tab — CommandScopeProvider + PerspectiveTab together
// ---------------------------------------------------------------------------

/** Minimal perspective shape used within the tab bar render tree. */
interface Perspective {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
}

/** Props for a perspective tab rendered inside its own CommandScopeProvider. */
interface ScopedPerspectiveTabProps {
  perspective: Perspective;
  /** ID of the currently active perspective, used to compute `isActive`. */
  activePerspectiveId: string | undefined;
  /** ID of the perspective currently being renamed, or null if none. */
  renamingId: string | null;
  onSelect: () => void;
  onDoubleClick: () => void;
  onRenameCommit: (newName: string) => void;
  onRenameCancel: () => void;
  /** Called when the filter icon button is clicked — focuses the formula bar. */
  onFilterFocus: () => void;
}

/**
 * Wraps a single perspective tab in its CommandScopeProvider.
 *
 * Extracted from the PerspectiveTabBar map to keep the parent component concise.
 *
 * The tab's render also goes through `<PerspectiveTabFocusable>`, which mounts
 * a `<FocusZone moniker={asSegment(`perspective_tab:${id}`)} showFocusBar={false}>`
 * wrapper when the spatial-nav stack is mounted. Each tab is therefore a
 * sibling zone inside the surrounding `ui:perspective-bar` zone, and the
 * interactive controls inside the tab — name, filter icon, group icon — are
 * `<FocusScope>` leaves at `perspective_tab.name:{id}`,
 * `perspective_tab.filter:{id}`, and `perspective_tab.group:{id}`. See
 * `PerspectiveTabFocusable` below for the structural rationale.
 *
 * Per-tab rename binding: every perspective tab — active or inactive —
 * registers a `ui.entity.startRename` `CommandDef` whose `keys` block
 * (Enter for cua / vim / emacs) is picked up by `extractScopeBindings`
 * when this tab is the spatial focus. That binding shadows the global
 * `nav.drillIn: Enter` for the perspective scope only, matching the YAML
 * `scope: "entity:perspective"` filter on the same id in
 * `swissarmyhammer-commands/builtin/commands/ui.yaml`. The execute path:
 *
 *   - On the active tab: trigger rename on the active perspective.
 *   - On an inactive tab: dispatch `perspective.set` to activate the tab,
 *     then trigger rename targeted at this tab's id. The broadcaster
 *     accepts an explicit id so the rename is independent of the
 *     async UI-state propagation that would otherwise leave the
 *     subscriber's `activePerspective` snapshot stale.
 */
function ScopedPerspectiveTab({
  perspective,
  activePerspectiveId,
  renamingId,
  onSelect,
  onDoubleClick,
  onRenameCommit,
  onRenameCancel,
  onFilterFocus,
}: ScopedPerspectiveTabProps) {
  const isActive = activePerspectiveId === perspective.id;
  const dispatchPerspectiveSet = useDispatchCommand("perspective.set");
  const startRenameCommands = useMemo<readonly CommandDef[]>(() => {
    return [
      {
        id: "ui.entity.startRename",
        name: "Rename Perspective",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
        execute: async () => {
          if (!isActive) {
            // Activate the focused tab before mounting the rename editor —
            // the user's mental model is "Enter edits the name of the tab
            // I am on", which implies the tab also becomes active.
            await dispatchPerspectiveSet({
              args: { perspective_id: perspective.id },
            });
          }
          // Pass the explicit id so the rename targets this tab regardless
          // of whether the activate dispatch's UI-state event has reached
          // the subscriber yet.
          triggerStartRename(perspective.id);
        },
      },
    ];
  }, [isActive, perspective.id, dispatchPerspectiveSet]);
  return (
    <CommandScopeProvider
      moniker={moniker("perspective", perspective.id)}
      commands={startRenameCommands}
    >
      <PerspectiveTabFocusable id={perspective.id}>
        <PerspectiveTab
          id={perspective.id}
          name={perspective.name}
          filter={perspective.filter}
          group={perspective.group}
          isActive={isActive}
          isRenaming={renamingId === perspective.id}
          onSelect={onSelect}
          onDoubleClick={onDoubleClick}
          onRenameCommit={onRenameCommit}
          onRenameCancel={onRenameCancel}
          onFilterFocus={onFilterFocus}
        />
      </PerspectiveTabFocusable>
    </CommandScopeProvider>
  );
}

/**
 * Wrap a perspective tab in `<FocusZone moniker={asSegment(`perspective_tab:${id}`)}>`
 * when the spatial-nav stack is mounted; otherwise fall through.
 *
 * # Why a zone, not a leaf
 *
 * Pre-iteration the tab wrapper was a `<FocusScope>` leaf with the
 * `<TabButton>`, `<FilterFocusButton>`, and `<GroupPopoverButton>`
 * rendered as plain `<button>` children inside it. Migrating the
 * inner controls to `<Pressable>` (which mounts its own
 * `<FocusScope>` leaf) creates a Scope-inside-Scope violation that the
 * kernel's iteration-3 `scope-not-leaf` enforcement detects.
 *
 * The fix mirrors entity-card's iteration-2 reshape (card
 * `01KQJDYJ4SDKK2G8FTAQ348ZHG`): promote the wrapper to `<FocusZone>`
 * and wrap each interactive child in its own `<FocusScope>` leaf —
 * `perspective_tab.name:{id}` for the name button,
 * `perspective_tab.filter:{id}` for the filter icon (via Pressable),
 * `perspective_tab.group:{id}` for the group icon (via Pressable).
 * `showFocusBar={false}` because the inner leaves carry the focus
 * signal — no visible bar across the whole tab is wanted.
 *
 * Same conditional pattern as `PerspectiveBarSpatialZone` —
 * the strict primitive contract is preserved for production while
 * keeping the test surface narrow.
 *
 * See also: `ScopedPerspectiveTab` above — it explains how this wrapper
 * composes with the `<CommandScopeProvider moniker="perspective:{id}">`
 * that surrounds it, so right-click context-menu chains pick up both.
 */
function PerspectiveTabFocusable({
  id,
  children,
}: {
  id: string;
  children: ReactNode;
}) {
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return <>{children}</>;
  }
  return (
    <FocusZone
      moniker={asSegment(`perspective_tab:${id}`)}
      showFocusBar={false}
    >
      {children}
    </FocusZone>
  );
}

/**
 * Wrap the always-visible filter formula bar in
 * `<FocusScope moniker={asSegment(`filter_editor:${perspectiveId}`)}>` when
 * the spatial-nav stack is mounted; otherwise fall through.
 *
 * Without this leaf the kernel's beam-search has no scope to land on for
 * the formula bar — it would skip the editor entirely on `nav.left` /
 * `nav.right`. The per-perspective segment matches the
 * `key={activePerspective.id}` remount on the outer component so the kernel
 * sees a distinct leaf per perspective and runs through a clean
 * unregister → register cycle when the active perspective switches, rather
 * than aliasing across perspectives via a shared moniker.
 *
 * `<FocusScope>`'s click handler skips clicks landing on `INPUT`,
 * `TEXTAREA`, `SELECT`, or `[contenteditable]` (focus-scope.tsx
 * `handleClick`), which preserves the existing
 * `onClick={() => editorRef.current?.focus()}` behaviour on the bar's
 * interior — clicks on the CM6 contenteditable surface route through to the
 * editor's own caret placement instead of being intercepted by the leaf.
 *
 * Same conditional pattern as `PerspectiveBarSpatialZone` and
 * `PerspectiveTabFocusable` — the strict primitive contract is preserved
 * for production while keeping the existing narrow-provider tests passing.
 */
function FilterFormulaBarFocusable({
  perspectiveId,
  children,
}: {
  perspectiveId: string;
  children: ReactNode;
}) {
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return <>{children}</>;
  }
  return (
    <FocusScope moniker={asSegment(`filter_editor:${perspectiveId}`)}>
      {children}
    </FocusScope>
  );
}

// ---------------------------------------------------------------------------
// Add-perspective button
// ---------------------------------------------------------------------------

/** "+" button that creates a new perspective for the current view kind. */
function AddPerspectiveButton({
  filteredPerspectives,
  viewKind,
}: {
  filteredPerspectives: Array<{ name: string }>;
  viewKind: string;
}) {
  const dispatchPerspectiveSave = useDispatchCommand("perspective.save");

  const handleAdd = useCallback(() => {
    const untitledCount = filteredPerspectives.filter((p) =>
      p.name.startsWith("Untitled"),
    ).length;
    const name =
      untitledCount === 0 ? "Untitled" : `Untitled ${untitledCount + 1}`;
    dispatchPerspectiveSave({ args: { name, view: viewKind } }).catch(
      console.error,
    );
  }, [filteredPerspectives, viewKind, dispatchPerspectiveSave]);

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Pressable
          asChild
          moniker={asSegment("ui:perspective-bar.add")}
          ariaLabel="Add perspective"
          onPress={handleAdd}
        >
          <button
            type="button"
            className="inline-flex items-center justify-center h-7 w-7 text-muted-foreground hover:text-foreground hover:bg-muted/50 rounded-md transition-colors"
          >
            <Plus className="h-3.5 w-3.5" />
          </button>
        </Pressable>
      </TooltipTrigger>
      <TooltipContent>New perspective</TooltipContent>
    </Tooltip>
  );
}

// ---------------------------------------------------------------------------
// Inner tab component — rendered inside CommandScopeProvider so
// useContextMenu sees the perspective scope and builds the correct chain.
// ---------------------------------------------------------------------------

/** Props for an individual perspective tab button and its inline action buttons. */
interface PerspectiveTabProps {
  id: string;
  name: string;
  filter?: string;
  group?: string;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: () => void;
  onDoubleClick: () => void;
  /** Called with the new name text when the rename editor commits. */
  onRenameCommit: (newName: string) => void;
  onRenameCancel: () => void;
  /** Called when the filter icon button is clicked — focuses the formula bar. */
  onFilterFocus: () => void;
}

/**
 * Individual perspective tab that uses the backend command system for
 * context menus. Renders an inline filter focus button for the active tab
 * that moves keyboard focus into the formula bar (no popover).
 *
 * Must be rendered inside a CommandScopeProvider with a perspective
 * moniker so the scope chain is correct.
 */
function PerspectiveTab({
  id,
  name,
  filter,
  group,
  isActive,
  isRenaming,
  onSelect,
  onDoubleClick,
  onRenameCommit,
  onRenameCancel,
  onFilterFocus,
}: PerspectiveTabProps) {
  const handleContextMenu = useContextMenu();
  const [groupOpen, setGroupOpen] = useState(false);

  const { getSchema } = useSchema();
  const { activeView } = useViews();
  const entityType = activeView?.entity_type ?? "";
  const schemaFields = useMemo(
    () => getSchema(entityType)?.fields ?? [],
    [getSchema, entityType],
  );

  return (
    <div className="inline-flex items-center">
      <FocusScope moniker={asSegment(`perspective_tab.name:${id}`)}>
        <TabButton
          name={name}
          isActive={isActive}
          isRenaming={isRenaming}
          onSelect={onSelect}
          onDoubleClick={onDoubleClick}
          onContextMenu={handleContextMenu}
          onRenameCommit={onRenameCommit}
          onRenameCancel={onRenameCancel}
        />
      </FocusScope>
      {isActive && (
        <FilterFocusButton
          perspectiveId={id}
          filter={filter}
          onFocus={onFilterFocus}
        />
      )}
      {isActive && (
        <GroupPopoverButton
          group={group}
          perspectiveId={id}
          fields={schemaFields}
          open={groupOpen}
          onOpenChange={setGroupOpen}
        />
      )}
    </div>
  );
}

/**
 * The clickable tab button — shows the perspective name or a CM6 rename editor.
 *
 * When `isRenaming` is true, renders `InlineRenameEditor` in place of the
 * name text so the user can type a new name directly in the tab.
 */
function TabButton({
  name,
  isActive,
  isRenaming,
  onSelect,
  onDoubleClick,
  onContextMenu,
  onRenameCommit,
  onRenameCancel,
}: {
  name: string;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: () => void;
  onDoubleClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  onRenameCommit: (newName: string) => void;
  onRenameCancel: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      className={cn(
        "inline-flex items-center px-2.5 h-7 text-xs font-medium rounded-t-md border-b-2 transition-colors whitespace-nowrap",
        isActive
          ? "border-primary text-foreground bg-background"
          : "border-transparent text-muted-foreground hover:text-foreground hover:bg-muted/50",
      )}
    >
      {isRenaming ? (
        <InlineRenameEditor
          name={name}
          onCommit={onRenameCommit}
          onCancel={onRenameCancel}
        />
      ) : (
        name
      )}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Rename guard hook and inline rename editor
// ---------------------------------------------------------------------------

/**
 * Creates guarded commit and cancel callbacks for inline rename.
 *
 * The committedRef prevents double-fire from concurrent blur + Enter events.
 */
function useRenameGuards(
  onCommit: (text: string) => void,
  onCancel: () => void,
) {
  const committedRef = useRef(false);

  const guardedCommit = useCallback(
    (text: string) => {
      console.warn("[rename] guardedCommit called", {
        text,
        alreadyCommitted: committedRef.current,
      });
      if (committedRef.current) return;
      committedRef.current = true;
      onCommit(text);
    },
    [onCommit],
  );

  const guardedCancel = useCallback(() => {
    console.warn("[rename] guardedCancel called", {
      alreadyCommitted: committedRef.current,
    });
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return { guardedCommit, guardedCancel };
}

/**
 * Builds the extensions, refs, and callbacks for the inline rename editor.
 *
 * Vim semantics: Escape from normal mode routes to COMMIT, not cancel — the
 * vim idiom treats Escape as "done editing, save what I have." CUA/emacs treat
 * Escape as the explicit cancel/discard shortcut.
 */
function useInlineRenamePolicy(
  name: string,
  onCommit: (newName: string) => void,
  onCancel: () => void,
) {
  const latestTextRef = useRef(name);
  const { guardedCommit, guardedCancel } = useRenameGuards(onCommit, onCancel);
  const { keymap_mode: keymapMode } = useUIState();

  const trackText = useCallback((text: string) => {
    latestTextRef.current = text;
  }, []);

  const submitRef = useRef<(() => void) | null>(() => {});
  submitRef.current = () => guardedCommit(latestTextRef.current);
  const cancelRef = useRef<(() => void) | null>(() => {});
  cancelRef.current =
    keymapMode === "vim"
      ? () => guardedCommit(latestTextRef.current)
      : () => guardedCancel();

  const extensions = useMemo(
    () =>
      buildSubmitCancelExtensions({
        mode: keymapMode,
        onSubmitRef: submitRef,
        onCancelRef: cancelRef,
        singleLine: true,
        alwaysSubmitOnEnter: true,
      }),
    [keymapMode],
  );

  return { trackText, extensions, guardedCommit, latestTextRef };
}

/**
 * Inline CM6 rename editor — uses the pure {@link TextEditor} primitive and
 * wires its own Enter-commit, Escape-cancel, and blur-commit policy.
 *
 * Enter is bound via a keymap extension (`alwaysSubmitOnEnter: true`, so it
 * fires even in vim insert mode). Escape cancels. Blur commits via the
 * wrapper div's `onBlur`. The `committedRef` guard prevents double-fire from
 * concurrent Enter + blur events.
 *
 * Exported for integration testing; production usage is internal to the tab bar.
 */
export function InlineRenameEditor({
  name,
  onCommit,
  onCancel,
}: {
  name: string;
  onCommit: (newName: string) => void;
  onCancel: () => void;
}) {
  const { trackText, extensions, guardedCommit, latestTextRef } =
    useInlineRenamePolicy(name, onCommit, onCancel);

  return (
    <div
      className="min-w-[5rem]"
      onClick={(e) => e.stopPropagation()}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) {
          guardedCommit(latestTextRef.current);
        }
      }}
    >
      <TextEditor
        value={name}
        onChange={trackText}
        extensions={extensions}
        singleLine
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Filter focus button — clicking moves focus into the formula bar (no popover)
// ---------------------------------------------------------------------------

/**
 * Filter icon button on the active tab.
 *
 * Does NOT open a popover. Instead, clicking calls `onFocus` which focuses
 * the formula bar CM6 editor to the right of the tabs.
 *
 * Migrates to `<Pressable asChild>` so the icon gains both keyboard
 * reachability (the inner `<FocusScope>` provided by Pressable) AND
 * scope-level CommandDefs that bind Enter (vim/cua) and Space (cua) to
 * the same `onFocus` callback as a pointer click. The leaf moniker is
 * `perspective_tab.filter:{id}` — its parent zone is the surrounding
 * `perspective_tab:{id}` `<FocusZone>` so the kernel sees it as a
 * sibling leaf of the name and group leaves.
 *
 * The inner `<button>`'s `onClick={(e) => e.stopPropagation()}` is
 * preserved: a click on the filter icon must NOT bubble to the tab's
 * own click-to-activate handler. Radix Slot's `mergeProps` runs the
 * child's `onClick` first, then the slot's — so `e.stopPropagation()`
 * lands BEFORE Pressable's `handleClick` triggers `onPress`.
 */
function FilterFocusButton({
  perspectiveId,
  filter,
  onFocus,
}: {
  perspectiveId: string;
  filter?: string;
  onFocus: () => void;
}) {
  const hasFilter = Boolean(filter);
  return (
    <Pressable
      asChild
      moniker={asSegment(`perspective_tab.filter:${perspectiveId}`)}
      ariaLabel="Filter"
      onPress={onFocus}
    >
      <button
        type="button"
        className={cn(
          "inline-flex items-center justify-center h-5 w-5 rounded transition-colors -ml-1",
          hasFilter
            ? "text-primary"
            : "text-muted-foreground/50 hover:text-muted-foreground",
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <Filter className="h-3 w-3" fill={hasFilter ? "currentColor" : "none"} />
      </button>
    </Pressable>
  );
}

// ---------------------------------------------------------------------------
// Filter formula bar — always-visible CM6 filter editor in the right region
// ---------------------------------------------------------------------------

/** Props for the always-visible filter formula bar in the tab bar's right region. */
interface FilterFormulaBarProps {
  /** Current filter expression for the active perspective. */
  filter?: string;
  /** Perspective ID to dispatch filter commands against. */
  perspectiveId: string;
}

/**
 * Excel-style filter formula bar embedded in the right side of the tab bar.
 *
 * Always visible when a perspective is active. Contains a Filter icon and
 * a borderless CM6 filter editor with placeholder text. Exposes `focus()`
 * via forwardRef so the parent can focus the editor when the filter button
 * is clicked.
 *
 * Uses `key={perspectiveId}` on the parent to remount when switching
 * perspectives, ensuring the CM6 initial value reflects the new perspective.
 */
const FilterFormulaBar = forwardRef<FilterEditorHandle, FilterFormulaBarProps>(
  function FilterFormulaBar({ filter, perspectiveId }, ref) {
    const editorRef = useRef<FilterEditorHandle>(null);

    // Forward the inner editor handle so parents can call focus(), setValue(),
    // or getValue() (the last is used by reconciliation logic, not by the tab
    // bar itself — but the handle shape must stay aligned with TextEditorHandle
    // so the type remains substitutable through the ref chain).
    useImperativeHandle(
      ref,
      () => ({
        focus() {
          editorRef.current?.focus();
        },
        setValue(text: string) {
          editorRef.current?.setValue(text);
        },
        getValue() {
          return editorRef.current?.getValue() ?? "";
        },
      }),
      [],
    );

    return (
      <div
        data-testid="filter-formula-bar"
        className="flex items-center gap-1.5 flex-1 min-w-0 border-l border-border/50 pl-2 ml-1 cursor-text"
        onClick={() => editorRef.current?.focus()}
      >
        <Filter
          className="h-3.5 w-3.5 shrink-0 text-muted-foreground/60"
          aria-hidden="true"
        />
        <FilterEditor
          ref={editorRef}
          filter={filter ?? ""}
          perspectiveId={perspectiveId}
        />
      </div>
    );
  },
);

/**
 * Group-by icon button with popover for the active perspective.
 *
 * Opens a `GroupSelector` in a Radix Popover anchored below the button.
 * Highlighted in primary color when a group is active on the perspective.
 *
 * Migrates to `<Pressable asChild>` (inside the existing
 * `<PopoverTrigger asChild>` slot) so the icon gains both keyboard
 * reachability (the inner `<FocusScope>` provided by Pressable) AND
 * scope-level CommandDefs that bind Enter (vim/cua) and Space (cua) to
 * `onOpenChange(true)` — the same effect as a pointer click that opens
 * the popover. The leaf moniker is `perspective_tab.group:{id}` — its
 * parent zone is the surrounding `perspective_tab:{id}` `<FocusZone>`
 * so the kernel sees it as a sibling leaf of the name and filter
 * leaves.
 *
 * Trigger composition is `<PopoverTrigger asChild><Pressable
 * asChild><button>`. Radix Slot's `mergeProps` composes the chain so
 * exactly one `<button>` lands in the DOM, the trigger's onClick fires
 * (toggling open state), Pressable's `handleClick` fires `onPress`
 * (which calls `onOpenChange(true)`), and the inner button's
 * `e.stopPropagation()` keeps the click from bubbling to the tab's
 * activate handler.
 */
function GroupPopoverButton({
  group,
  perspectiveId,
  fields,
  open,
  onOpenChange,
}: {
  group?: string;
  perspectiveId: string;
  fields: FieldDef[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const hasGroup = Boolean(group);
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <Pressable
          asChild
          moniker={asSegment(`perspective_tab.group:${perspectiveId}`)}
          ariaLabel="Group"
          onPress={() => onOpenChange(true)}
        >
          <button
            type="button"
            className={cn(
              "inline-flex items-center justify-center h-5 w-5 rounded transition-colors -ml-0.5",
              hasGroup
                ? "text-primary"
                : "text-muted-foreground/50 hover:text-muted-foreground",
            )}
            onClick={(e) => e.stopPropagation()}
          >
            <Group
              className="h-3 w-3"
              fill={hasGroup ? "currentColor" : "none"}
            />
          </button>
        </Pressable>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={4}
        className="p-3 w-auto"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <GroupSelector
          group={group}
          perspectiveId={perspectiveId}
          fields={fields}
          onClose={() => onOpenChange(false)}
        />
      </PopoverContent>
    </Popover>
  );
}
