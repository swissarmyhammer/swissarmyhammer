import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { Filter } from "lucide-react";
// `Filter` is retained for the formula-bar's static prefix glyph;
// the per-tab `<FilterFocusButton>` was deleted by the migration in
// 01KRE1YA65MMG29RDQDQ0VPJQG (the registry-rendered `<CommandButton>`
// now owns that affordance). The `<GroupPopoverButton>` was deleted
// in 01KRE1ZTYJ5PPTQ29K72KE88B5 — its replacement is the
// registry-rendered `<CommandButton>` driven by the
// `perspective.group` YAML entry's `tab_button.icon: group`
// annotation. The `<AddPerspectiveButton>` (which used a hardcoded
// `Plus` icon import) was deleted in 01KRE21GJMPP289N1HSTMJG5HE —
// the `+` affordance is now a registry-rendered `<CommandButton>`
// for `perspective.save` rendered by `<BarRegistryTabButtons>` at
// the tab-bar level. All three icons are resolved at render time by
// `commandIconFor` in `command-icon-registry.ts`.
import { cn } from "@/lib/utils";
import { usePerspectives } from "@/lib/perspective-context";
import { useViews } from "@/lib/views-context";
import {
  useDispatchCommand,
  CommandScopeProvider,
  type CommandDef,
} from "@/lib/command-scope";
import { useContextMenu } from "@/lib/context-menu";
import { useBoardData } from "@/components/window-container";
import { moniker } from "@/lib/moniker";
import { CommandButton } from "@/components/command-button";
import type {
  CommandDef as RegistryCommandDef,
  TabButtonDef,
} from "@/types/kanban";
import {
  FilterEditor,
  type FilterEditorHandle,
} from "@/components/filter-editor";
import { TextEditor } from "@/components/fields/text-editor";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useUIState } from "@/lib/ui-state-context";
import { FocusScope } from "@/components/focus-scope";
import { Pressable } from "@/components/pressable";
import { useFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";
import { commandIconFor } from "@/components/command-icon-registry";

// ---------------------------------------------------------------------------
// Filter-editor FQM context — carries the spatial-nav FQM of the active
// perspective's filter editor scope up from the formula bar (which sits
// inside the `filter_editor:${id}` `<FocusScope>` and can compose the
// FQM via `useFullyQualifiedMoniker()`) to the Filter tab button (which
// sits OUTSIDE that scope and needs the FQM to dispatch `nav.focus`).
//
// The context value is a mutable ref so the writer (the formula bar
// wiring) can update it without re-rendering every consumer; the reader
// (the Filter tab button's click handler) snapshots it at click time.
// A null current means no active perspective has mounted its formula
// bar yet — the click handler no-ops in that case.
//
// Refactor history: card `01KRGZY33P99J7CGG0XRQGZ352` rewired the
// Filter tab-button click from a parallel `FocusFilter` → Tauri-event
// channel to the canonical `nav.focus` command. The FQM the click
// needs is composed inside `<FilterFormulaBarFocusable>`'s
// `<FocusScope>`, which is mounted alongside (not above) the
// `<RegistryTabButtons>` slot — so the writer and reader live as
// sibling subtrees and exchange the FQM through this shared ref.
// ---------------------------------------------------------------------------

/** Ref carrier shape — `.current` is the latest captured FQM (or null). */
type FilterEditorFqRef = React.MutableRefObject<FullyQualifiedMoniker | null>;

/**
 * Context holding the mutable ref. `null` outside the tab bar
 * (defensive — every render path that mounts the tab buttons also
 * provides this).
 */
const FilterEditorFqContext = createContext<FilterEditorFqRef | null>(null);

/** Read the FQM-ref context, or `null` outside the provider. */
function useOptionalFilterEditorFqRef(): FilterEditorFqRef | null {
  return useContext(FilterEditorFqContext);
}

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
  // The per-tab Filter button used to take a `handleFilterFocus`
  // callback wired through `<FilterFocusButton>`. After the
  // command-driven migration (01KRE1YA65MMG29RDQDQ0VPJQG) the
  // affordance became a registry-rendered tab button; card
  // `01KRGZY33P99J7CGG0XRQGZ352` rewired the click to dispatch the
  // frontend `nav.focus` against the `filter_editor:${id}` spatial-nav
  // scope (see `FilterFocusCommandButton` and
  // `FilterEditorDrillOutWiring`). The `filterEditorRef` is still held
  // here so the formula-bar wrapper's click-to-focus path (`onClick`
  // on the bar div in `FilterFormulaBar`) and the `nav.drillIn` Enter
  // command on `<FilterFormulaBarFocusable>` can still imperatively
  // focus the CM6 editor when the spatial-nav scope drives down.
  const activeViewId = activeView?.id;
  const viewKind = activeView?.kind ?? "board";
  // view_id-first / kind-fallback rule (see `PerspectiveDef` JSDoc in
  // `kanban-app/ui/src/types/kanban.ts`): a perspective with `view_id` is
  // pinned to that specific view instance; a perspective without `view_id`
  // is the legacy shared-by-kind shape and appears in every view whose
  // kind matches.
  const filteredPerspectives = useMemo(
    () =>
      perspectives.filter((p) => {
        if (p.view_id != null) return p.view_id === activeViewId;
        return p.view === viewKind;
      }),
    [perspectives, activeViewId, viewKind],
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
    filterEditorRef,
    renamingId,
    startRename,
    commitRename,
    cancelRename,
  };
}

// ---------------------------------------------------------------------------
// Registry-driven tab buttons — `<CommandButton>` per `tab_button`-tagged command
// ---------------------------------------------------------------------------

/**
 * Wire-format shape returned by the backend `list_commands_for_scope`
 * Tauri command. Mirrors `swissarmyhammer-kanban::scope_commands::ResolvedCommand`
 * — only the fields the tab bar reads are declared here; the dispatcher and
 * other consumers see more fields and use their own narrower shapes.
 *
 * Two fields drive the tab bar render path:
 *
 *   - `tab_button` — when set, the command renders as a `<CommandButton>`
 *     on the per-perspective tab. When absent, the command surfaces in
 *     palettes / context menus per its other metadata but contributes
 *     nothing to the tab bar.
 *   - `params` — carried through to `<CommandButton>` so the popover knows
 *     which fields to render. Backend-supplied `options` for enum-shaped
 *     params are already populated.
 */
interface ResolvedTabCommand {
  readonly id: string;
  readonly name: string;
  readonly target?: string;
  readonly group?: string;
  readonly context_menu?: boolean;
  readonly available?: boolean;
  readonly keys?: RegistryCommandDef["keys"];
  readonly args?: Record<string, unknown>;
  readonly params?: RegistryCommandDef["params"];
  readonly tab_button?: TabButtonDef;
}

/**
 * Query the live command registry for every command in scope and return
 * only those flagged for tab-button rendering.
 *
 * # Scope chain shape
 *
 * Built innermost → outermost from three monikers that together describe
 * the perspective + view + board the tab belongs to:
 *
 *   `["perspective:${perspectiveId}", "view:${activeView.id}", "board:${activeBoardId}"]`
 *
 * The backend's downstream passes consume each segment:
 *
 *   - `perspective:` — `PerspectiveFieldsResolver` looks up the perspective
 *     by id to populate enum picker options.
 *   - `view:` — `filter_by_view_kind` joins against `DynamicSources.views`
 *     to drop commands whose `view_kinds` array doesn't admit the active
 *     view's kind.
 *   - `board:` — present so cross-cutting board-level commands (e.g.
 *     archive / delete on the board itself) resolve correctly.
 *
 * The chain is built explicitly rather than read from `FocusedScopeContext`
 * because the tab bar's per-perspective queries are decoupled from
 * spatial focus — the active perspective hosts the inline tab-button
 * affordances regardless of which leaf the user has navigated to.
 * Walking the focus tree would only describe whichever tab is currently
 * focused.
 *
 * # Filtering
 *
 * The frontend filter is a single `tab_button != null` predicate. The
 * backend already runs `filter_by_view_kind` and availability checks
 * before emission — the frontend trusts that work and only narrows
 * further to "render me as a tab-button".
 *
 * @param perspectiveId The id of the perspective whose tab this hook
 *   queries for. Used as the innermost moniker in the scope chain.
 * @param activeViewId The id of the active view. Becomes the middle
 *   moniker in the scope chain.
 * @param activeBoardId The id of the active board. Becomes the
 *   outermost moniker in the scope chain. May be `undefined` when no
 *   board is loaded — in that case the hook returns an empty list
 *   without invoking the backend.
 * @returns Commands ready to feed to `<CommandButton>`. Empty until the
 *   first `list_commands_for_scope` response resolves.
 */
function useScopedTabCommands(
  perspectiveId: string,
  activeViewId: string,
  activeBoardId: string | undefined,
): ResolvedTabCommand[] {
  const [commands, setCommands] = useState<ResolvedTabCommand[]>([]);

  useEffect(() => {
    if (!activeBoardId) {
      setCommands([]);
      return;
    }
    let cancelled = false;
    const scopeChain = [
      `perspective:${perspectiveId}`,
      `view:${activeViewId}`,
      `board:${activeBoardId}`,
    ];
    invoke<ResolvedTabCommand[]>("list_commands_for_scope", { scopeChain })
      .then((resolved) => {
        if (cancelled) return;
        // Per-tab slot renders ENTITY-scoped tab buttons only.
        // Unscoped (`group: "global"`) tab-button commands — today only
        // `perspective.save` — are picked up by `<BarRegistryTabButtons>`
        // at the bar level so the `+` affordance sits outside any
        // individual perspective tab (matching the legacy
        // `<AddPerspectiveButton>` placement and surviving the no-active-
        // perspective edge case). Filtering here keeps a perspective.save
        // row that incidentally ships with every scope-chain query from
        // double-rendering inside the active tab.
        const tabCommands = resolved.filter(
          (c) => c.tab_button != null && c.group !== "global",
        );
        setCommands(tabCommands);
      })
      .catch((e) => {
        console.error("[PerspectiveTabBar] list_commands_for_scope failed:", e);
        if (!cancelled) setCommands([]);
      });
    return () => {
      cancelled = true;
    };
  }, [perspectiveId, activeViewId, activeBoardId]);

  return commands;
}

/**
 * Decide whether a registry-rendered tab-button should paint its
 * `isActive` highlight given the host perspective's current state.
 *
 * Keeping the per-command logic in one switch lets the YAML stay
 * declarative — adding a new tab-button command without a state-driven
 * highlight is a no-op here. Today only `perspective.filter.focus`
 * needs the highlight (mirrors the legacy `<FilterFocusButton>`'s
 * `hasFilter` indicator); Group / Sort migrations will land their own
 * cases as they migrate.
 *
 * @param commandId The command id from the registry payload.
 * @param perspective The perspective hosting the tab; reads `filter`,
 *   `group`, and (later) `sort` directly so per-command checks live
 *   inside the switch without per-call prop plumbing.
 */
function isCommandActiveForPerspective(
  commandId: string,
  perspective: Perspective,
): boolean {
  switch (commandId) {
    case "perspective.filter.focus":
      return Boolean(perspective.filter);
    case "perspective.group":
      return Boolean(perspective.group);
    default:
      return false;
  }
}

/**
 * Render one `<CommandButton>` per tab-button-tagged command for the
 * given perspective. Sits alongside any remaining hardcoded inline
 * affordances on the active tab until per-command migrations remove
 * their hardcoded counterparts.
 *
 * The slot grows as each per-command migration flips a YAML
 * `tab_button` on; the first migration is
 * `perspective.filter.focus` (task 01KRE1YA65MMG29RDQDQ0VPJQG) which
 * also wires the `isActive` highlight from the perspective's filter
 * state. Subsequent migrations (group, sort) extend
 * `isCommandActiveForPerspective` to read their own per-command state.
 */
function RegistryTabButtons({
  perspective,
  activeViewId,
  activeBoardId,
}: {
  perspective: Perspective;
  activeViewId: string;
  activeBoardId: string | undefined;
}) {
  const tabCommands = useScopedTabCommands(
    perspective.id,
    activeViewId,
    activeBoardId,
  );
  // Adapt the wire-format shape to the `CommandDef` `<CommandButton>`
  // consumes from `@/types/kanban`. Most fields pass through unchanged;
  // a default `visible: true` keeps the type alignment minimal without
  // forcing the wire format to mirror every optional UI flag.
  //
  // `perspective.filter.focus` is special-cased: its click must claim
  // spatial-nav focus on the formula bar's `filter_editor:${id}` scope
  // via `nav.focus` (the single auditable focus primitive — see card
  // `01KR7CDEFWWVF4WH0BCHE8Y21J`). The generic `<CommandButton>`
  // dispatches the command's own id, which is wrong for this case;
  // `<FilterFocusCommandButton>` keeps the registry-driven render
  // (icon, isActive, moniker) but overrides the dispatch.
  return (
    <>
      {tabCommands.map((cmd) => {
        const isActive = isCommandActiveForPerspective(cmd.id, perspective);
        if (cmd.id === "perspective.filter.focus") {
          return (
            <FilterFocusCommandButton
              key={cmd.id}
              command={
                {
                  id: cmd.id,
                  name: cmd.name,
                  keys: cmd.keys,
                  params: cmd.params,
                  tab_button: cmd.tab_button,
                } as RegistryCommandDef
              }
              perspectiveId={perspective.id}
              isActive={isActive}
            />
          );
        }
        return (
          <CommandButton
            key={cmd.id}
            command={
              {
                id: cmd.id,
                name: cmd.name,
                keys: cmd.keys,
                params: cmd.params,
                tab_button: cmd.tab_button,
              } as RegistryCommandDef
            }
            surface="perspective_tab"
            surfaceId={perspective.id}
            isActive={isActive}
          />
        );
      })}
    </>
  );
}

/**
 * Tab-button affordance for `perspective.filter.focus` that dispatches
 * `nav.focus` instead of the command's own id.
 *
 * Why a dedicated adapter (not the generic `<CommandButton>`): focus
 * claims in this app flow through the single `nav.focus` command (card
 * `01KR7CDEFWWVF4WH0BCHE8Y21J`). The previous wiring invented a
 * parallel `FocusFilter` → Tauri-event channel that bypassed the
 * spatial-nav kernel; card `01KRGZY33P99J7CGG0XRQGZ352` deleted that
 * channel and routed the click through `nav.focus({ args: { fq } })`.
 *
 * The FQM is composed inside `<FilterFormulaBarFocusable>`'s
 * `filter_editor:${id}` scope and stashed into the
 * `<FilterEditorFqContext>` ref by `<FilterEditorDrillOutWiring>`. This
 * adapter reads the ref at click time so it always picks up the
 * currently-active perspective's editor FQM, not a stale snapshot.
 *
 * Visual / spatial-nav contract: matches `<CommandButton>` exactly —
 * same icon, same `text-primary` highlight when `isActive`, same
 * `${surface}.${command.id}:${surfaceId}` moniker built from the
 * registry payload. Only the click semantics differ.
 */
function FilterFocusCommandButton({
  command,
  perspectiveId,
  isActive,
}: {
  command: RegistryCommandDef;
  perspectiveId: string;
  isActive: boolean;
}) {
  const Icon = commandIconFor(command.tab_button?.icon ?? "");
  const dispatchNavFocus = useDispatchCommand("nav.focus");
  const fqRef = useOptionalFilterEditorFqRef();

  const handlePress = useCallback(() => {
    const fq = fqRef?.current;
    if (!fq) {
      // Defensive: the formula bar hasn't mounted its scope yet (e.g.
      // no active perspective, or this fired in a degraded test
      // harness without `<SpatialFocusProvider>`). A click with no
      // target FQM is a no-op — there is nothing to focus.
      console.warn(
        "[FilterFocusCommandButton] no filter_editor FQM available; skipping nav.focus",
      );
      return;
    }
    void dispatchNavFocus({ args: { fq } }).catch((err) =>
      console.error(
        "[FilterFocusCommandButton] nav.focus dispatch failed",
        err,
      ),
    );
  }, [dispatchNavFocus, fqRef]);

  const monikerSegment = asSegment(
    `perspective_tab.${command.id}:${perspectiveId}`,
  );

  return (
    <Pressable
      asChild
      moniker={monikerSegment}
      ariaLabel={command.name}
      onPress={handlePress}
    >
      <button
        type="button"
        className={cn(
          "inline-flex items-center justify-center h-5 w-5 rounded transition-colors -ml-1",
          isActive
            ? "text-primary"
            : "text-muted-foreground/50 hover:text-muted-foreground",
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <Icon className="h-3 w-3" fill={isActive ? "currentColor" : "none"} />
      </button>
    </Pressable>
  );
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
    filterEditorRef,
    renamingId,
    startRename,
    commitRename,
    cancelRename,
  } = usePerspectiveTabBar();
  const boardData = useBoardData();
  const activeBoardId = boardData?.board?.id;

  // Shared ref carrying the active perspective's filter-editor FQM.
  // Written by `<FilterEditorDrillOutWiring>` (inside the
  // `filter_editor:${id}` scope) and read by the Filter tab button's
  // click handler (`<FilterFocusCommandButton>`), which sits in a
  // sibling subtree without a `useFullyQualifiedMoniker()` ancestor of
  // its own. See `FilterEditorFqContext` for the rationale.
  const filterEditorFqRef = useRef<FullyQualifiedMoniker | null>(null);

  if (!activeView) return null;

  return (
    <FilterEditorFqContext.Provider value={filterEditorFqRef}>
      <PerspectiveBarSpatialZone>
        {/*
          Left: scrollable perspective tabs + add button. `<FocusIndicator>`
          paints inside each tab's box, so no special gap or padding is
          required to make room for it.
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
            />
          ))}
          <BarRegistryTabButtons
            activeViewId={activeView.id}
            activeBoardId={activeBoardId}
          />
        </div>
        {/* Right: filter formula bar — always visible when a perspective is active */}
        {activePerspective && (
          <FilterFormulaBarFocusable
            key={activePerspective.id}
            perspectiveId={activePerspective.id}
            editorRef={filterEditorRef}
          >
            <FilterFormulaBar
              ref={filterEditorRef}
              filter={activePerspective.filter}
              perspectiveId={activePerspective.id}
            />
          </FilterFormulaBarFocusable>
        )}
      </PerspectiveBarSpatialZone>
    </FilterEditorFqContext.Provider>
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
    <FocusScope
      moniker={asSegment("ui:perspective-bar")}
      // The bar is viewport-spanning chrome (full window width × 32px high) —
      // a focus indicator running across its entire row would be visual
      // noise. The bar's job in the spatial graph is to be the parent zone
      // for its tab leaves; the leaves themselves render the visible bar
      // when claimed. `data-focused` still flips on the wrapper for e2e
      // selectors / debugging.
      // showFocus=false: viewport-spanning bar chrome; tab leaves own the visible focus signal.
      showFocus={false}
      className={PERSPECTIVE_BAR_LAYOUT}
    >
      {children}
    </FocusScope>
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
}

/**
 * Wraps a single perspective tab in its CommandScopeProvider.
 *
 * Extracted from the PerspectiveTabBar map to keep the parent component concise.
 *
 * The tab's render also goes through `<PerspectiveTabFocusable>`, which
 * mounts a `<FocusScope moniker={asSegment(`perspective_tab:${id}`)}>`
 * wrapper when the spatial-nav stack is mounted. The wrapper inherits
 * `<FocusScope>`'s default `showFocus={true}` so the dashed-border
 * indicator paints on the focused tab. Each tab is itself the focusable
 * spatial-nav target — there is no inner `perspective_tab.name` leaf
 * because that would register at the exact same rect as the outer
 * wrapper and trip the kernel's needless-nesting warning. Enter on a
 * focused tab triggers rename via the `ui.entity.startRename` command
 * this component registers, which shadows the global `nav.drillIn:
 * Enter` on the perspective scope.
 *
 * The filter icon and group icon to the right of the name remain as
 * inner focus leaves with distinct monikers — both affordances are now
 * `<CommandButton>` leaves (`perspective_tab.perspective.filter.focus:{id}`
 * and `perspective_tab.perspective.group:{id}`, built by
 * `<CommandButton>` from `${surface}.${command.id}:${surfaceId}`) rather
 * than Pressables. Both have distinct rects from the tab name and are
 * independently navigable.
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
 *   - On an inactive tab: dispatch `perspective.switch` to activate the tab,
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
}: ScopedPerspectiveTabProps) {
  const isActive = activePerspectiveId === perspective.id;
  const dispatchPerspectiveSwitch = useDispatchCommand("perspective.switch");
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
            await dispatchPerspectiveSwitch({
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
  }, [isActive, perspective.id, dispatchPerspectiveSwitch]);
  return (
    <CommandScopeProvider
      moniker={moniker("perspective", perspective.id)}
      commands={startRenameCommands}
    >
      <PerspectiveTabFocusable id={perspective.id}>
        <PerspectiveTab
          perspective={perspective}
          isActive={isActive}
          isRenaming={renamingId === perspective.id}
          onSelect={onSelect}
          onDoubleClick={onDoubleClick}
          onRenameCommit={onRenameCommit}
          onRenameCancel={onRenameCancel}
        />
      </PerspectiveTabFocusable>
    </CommandScopeProvider>
  );
}

/**
 * Wrap a perspective tab in `<FocusScope moniker={asSegment(`perspective_tab:${id}`)}>`
 * when the spatial-nav stack is mounted; otherwise fall through.
 *
 * # Single scope, no inner name leaf
 *
 * The tab IS the focusable target — clicking anywhere on the tab area
 * focuses `perspective_tab:${id}`, and Enter triggers rename via the
 * `ui.entity.startRename` command on the surrounding `perspective:${id}`
 * CommandScope (which shadows the global `nav.drillIn: Enter`). There is
 * no inner `perspective_tab.name` FocusScope because it would register
 * at the same rect as this wrapper and trigger the kernel's
 * needless-nesting warning.
 *
 * The icon affordances on an active tab are registry-rendered
 * `<CommandButton>`s, so each mounts its own inner FocusScope leaf
 * with a distinct moniker built by `<CommandButton>` from
 * `${surface}.${command.id}:${surfaceId}`:
 *
 *   - `perspective.filter.focus` → `perspective_tab.perspective.filter.focus:${id}`
 *   - `perspective.group` → `perspective_tab.perspective.group:${id}`
 *
 * All have distinct rects from the tab name and are independently
 * navigable. The legacy `perspective_tab.filter:${id}` (from the
 * deleted `<FilterFocusButton>`) and `perspective_tab.group:${id}`
 * (from the deleted `<GroupPopoverButton>`) monikers are gone — the
 * registry-driven monikers are the new spatial-nav targets.
 *
 * The wrapper inherits `<FocusScope>`'s default `showFocus={true}` so
 * the dashed focus indicator paints on the focused tab — the visible
 * indicator is the user-facing signal that arrow nav landed here. The
 * existing active/inactive border styling reflects which tab is the
 * active perspective, which is orthogonal to focus.
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
    <FocusScope moniker={asSegment(`perspective_tab:${id}`)}>
      {children}
    </FocusScope>
  );
}

/**
 * Wrap the always-visible filter formula bar in
 * `<FocusScope moniker={asSegment(`filter_editor:${perspectiveId}`)}>` when
 * the spatial-nav stack is mounted; otherwise fall through.
 *
 * Without this scope the kernel's beam-search has no target to land on
 * for the formula bar — it would skip the editor entirely on `nav.left`
 * / `nav.right`. The per-perspective segment matches the
 * `key={activePerspective.id}` remount on the outer component so the
 * kernel sees a distinct scope per perspective and runs through a clean
 * unregister → register cycle when the active perspective switches.
 *
 * # Drill-in / drill-out via the existing nav.drillIn / nav.drillOut
 *
 * When the spatial focus is on `filter_editor:${id}`, pressing Enter
 * fires the global `nav.drillIn` keybinding. We register a per-scope
 * `filter_editor.drillIn` CommandDef with `keys: { Enter }` that
 * shadows the global handler and focuses the inner CM6 editor.
 *
 * When the inner CM6 editor has DOM focus and the user presses
 * Escape, CM6's own keymap (already wired in
 * `useFilterEditorExtensions` via `buildSubmitCancelExtensions`) calls
 * the editor's `onClose` callback. We route `onClose` through
 * `FilterFormulaBar` so it lands here as the drill-out handler:
 * blur the contenteditable and `setFocus(filterFq)` to put kernel
 * focus back on this scope. Same `nav.drillOut` concept as
 * everywhere else, just routed through CM6's existing cancel path
 * (the global `nav.drillOut: Escape` doesn't fire while CM6 has DOM
 * focus because the keybinding handler short-circuits on editable
 * surfaces).
 *
 * `<FocusScope>`'s click handler skips clicks landing on `INPUT`,
 * `TEXTAREA`, `SELECT`, or `[contenteditable]` (focus-scope.tsx
 * `handleClick`), which preserves the existing
 * `onClick={() => editorRef.current?.focus()}` behaviour on the bar's
 * interior — clicks on the CM6 contenteditable surface route through
 * to the editor's own caret placement instead of being intercepted by
 * the scope.
 */
function FilterFormulaBarFocusable({
  perspectiveId,
  editorRef,
  children,
}: {
  perspectiveId: string;
  editorRef: React.RefObject<FilterEditorHandle | null>;
  children: ReactNode;
}) {
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();

  const drillCommands = useMemo<readonly CommandDef[]>(
    () => [
      {
        id: "filter_editor.drillIn",
        name: "Edit Filter",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
        execute: () => {
          editorRef.current?.focus();
        },
      },
    ],
    [editorRef],
  );

  if (!layerKey || !actions) {
    return <>{children}</>;
  }
  return (
    <FocusScope
      moniker={asSegment(`filter_editor:${perspectiveId}`)}
      commands={drillCommands}
    >
      <FilterEditorDrillOutWiring>{children}</FilterEditorDrillOutWiring>
    </FocusScope>
  );
}

/**
 * Inner wrapper rendered INSIDE `<FocusScope>` so it can read the
 * filter-editor scope's composed FQM via `useFullyQualifiedMoniker()`
 * and provide the `nav.drillOut` semantics (Escape from CM6 → blur +
 * refocus the spatial scope).
 *
 * Receives the existing children unchanged but passes a stable
 * `onEditorEscape` callback through React context (via
 * `FilterEditorEscapeContext`) so `FilterFormulaBar`'s descendant CM6
 * editor can wire it as its `onClose` without prop-drilling through
 * every intermediate component.
 */
function FilterEditorDrillOutWiring({ children }: { children: ReactNode }) {
  const filterFq = useFullyQualifiedMoniker();
  // Card `01KR7CDEFWWVF4WH0BCHE8Y21J`: focus claims flow through
  // `nav.focus`, the single auditable command that wraps the
  // entity-focus `setFocus` primitive. The filter-editor Escape
  // handler claims focus on the filter-editor scope itself after
  // blurring the CM6 contenteditable.
  const dispatchNavFocus = useDispatchCommand("nav.focus");
  const onEditorEscape = useCallback(() => {
    // Drop DOM focus from the CM6 contenteditable so the cursor stops
    // blinking visually — the kernel's spatial focus update alone
    // doesn't move DOM focus.
    if (
      typeof document !== "undefined" &&
      document.activeElement instanceof HTMLElement
    ) {
      document.activeElement.blur();
    }
    void dispatchNavFocus({ args: { fq: filterFq } }).catch((err) =>
      console.error(
        "[FilterEditorDrillOutWiring] nav.focus dispatch failed",
        err,
      ),
    );
  }, [filterFq, dispatchNavFocus]);

  // Publish the filter editor's FQM up to the Filter tab button (card
  // `01KRGZY33P99J7CGG0XRQGZ352`). The button's click site is a sibling
  // subtree of this wiring — both mount when the active perspective is
  // known. The shared ref lets the click handler read the most recently
  // captured FQM at click time without prop-drilling through the
  // intermediate `<PerspectiveBarSpatialZone>` / `<RegistryTabButtons>`
  // boundary. Cleared on unmount so a stale FQM from a torn-down
  // formula bar can't be reused after the active perspective switches.
  const filterEditorFqRef = useOptionalFilterEditorFqRef();
  useEffect(() => {
    if (!filterEditorFqRef) return;
    filterEditorFqRef.current = filterFq;
    return () => {
      // Only clear if our value is still current — another
      // FilterEditorDrillOutWiring instance for a different active
      // perspective may have written its own FQM between this effect
      // running and this cleanup. The strict identity check prevents
      // clobbering the successor's value.
      if (filterEditorFqRef.current === filterFq) {
        filterEditorFqRef.current = null;
      }
    };
  }, [filterEditorFqRef, filterFq]);

  return (
    <FilterEditorEscapeContext.Provider value={onEditorEscape}>
      {children}
    </FilterEditorEscapeContext.Provider>
  );
}

/**
 * Carries the filter-editor scope's "Escape from inside CM6" handler
 * down to the descendant `<FilterEditor>` without prop-drilling.
 *
 * Wired by `FilterEditorDrillOutWiring` (which sits inside the
 * `filter_editor:${id}` `<FocusScope>` so it can compose the scope's
 * FQM). Consumed by `FilterFormulaBar` to pass as `onClose` to
 * `<FilterEditor>` — that callback fires from CM6's existing
 * Escape-cancel keymap.
 */
const FilterEditorEscapeContext = createContext<(() => void) | null>(null);

// ---------------------------------------------------------------------------
// Bar-level registry-rendered tab buttons (global, unscoped commands)
// ---------------------------------------------------------------------------

/**
 * Render `<CommandButton>`s for tab-button-tagged GLOBAL (unscoped)
 * commands at the perspective tab bar level — outside any individual
 * perspective tab.
 *
 * Today this slot owns the `perspective.save` (`+` Add Perspective)
 * affordance. The legacy hardcoded `<AddPerspectiveButton>` lived at
 * the same DOM location; this component is its registry-driven
 * replacement (card `01KRE21GJMPP289N1HSTMJG5HE`).
 *
 * # Why a separate component from `<RegistryTabButtons>`
 *
 * `<RegistryTabButtons>` is mounted INSIDE the active perspective's
 * tab and queries the registry with a perspective-bearing scope
 * chain. That's the right placement for entity-scoped tab buttons
 * (filter, group, sort — all `scope: "entity:perspective"`) but
 * wrong for the `+` button: the user needs it even when there is no
 * active perspective yet, and it would be visually confusing inside
 * a tab. Splitting the bar-level slot into its own component lets
 * the `+` survive the empty-perspective edge case and live in the
 * gap immediately after the perspective tab list, matching the
 * legacy layout.
 *
 * # Scope chain
 *
 * Built from `view:` + `board:` only — there is no `perspective:`
 * moniker because the bar-level slot is not pinned to any one
 * perspective. The backend's `emit_global_registry_commands` pass
 * surfaces global tab-button commands for any non-empty scope chain;
 * the per-tab slot's `group !== "global"` filter keeps the same row
 * from double-rendering inside the active perspective's tab.
 *
 * # Filtering
 *
 * `tab_button != null && group === "global"` — only true global
 * (unscoped) tab-button commands surface here. Entity-scoped tab
 * buttons (which carry `group: "<entity_type>"`) are picked up by
 * `<RegistryTabButtons>` inside their owning tab instead.
 */
function BarRegistryTabButtons({
  activeViewId,
  activeBoardId,
}: {
  activeViewId: string;
  activeBoardId: string | undefined;
}) {
  const [commands, setCommands] = useState<ResolvedTabCommand[]>([]);

  useEffect(() => {
    if (!activeBoardId) {
      setCommands([]);
      return;
    }
    let cancelled = false;
    const scopeChain = [`view:${activeViewId}`, `board:${activeBoardId}`];
    invoke<ResolvedTabCommand[]>("list_commands_for_scope", { scopeChain })
      .then((resolved) => {
        if (cancelled) return;
        const tabCommands = resolved.filter(
          (c) => c.tab_button != null && c.group === "global",
        );
        setCommands(tabCommands);
      })
      .catch((e) => {
        console.error(
          "[PerspectiveTabBar] list_commands_for_scope (bar) failed:",
          e,
        );
        if (!cancelled) setCommands([]);
      });
    return () => {
      cancelled = true;
    };
  }, [activeViewId, activeBoardId]);

  // The bar-level surface uses the view id as the per-instance moniker
  // suffix so the `<CommandButton>`'s spatial-nav leaf is stable across
  // perspective switches AND distinct between sibling views (e.g. two
  // grid views in the same window would each get their own
  // `perspective_bar.perspective.save:<view_id>` leaf).
  //
  // Naming: the surface key uses underscore (`perspective_bar`) while
  // the surrounding spatial-nav zone segment uses hyphen
  // (`ui:perspective-bar`). The two strings live in different
  // namespaces — the surface is a `<CommandButton>` registry key, the
  // zone is a `FocusLayer` segment — and the existing call sites for
  // both shapes follow this convention, so they are deliberately not
  // unified here.
  return (
    <>
      {commands.map((cmd) => (
        <CommandButton
          key={cmd.id}
          command={
            {
              id: cmd.id,
              name: cmd.name,
              keys: cmd.keys,
              params: cmd.params,
              tab_button: cmd.tab_button,
            } as RegistryCommandDef
          }
          surface="perspective_bar"
          surfaceId={activeViewId}
        />
      ))}
    </>
  );
}

// ---------------------------------------------------------------------------
// Inner tab component — rendered inside CommandScopeProvider so
// useContextMenu sees the perspective scope and builds the correct chain.
// ---------------------------------------------------------------------------

/** Props for an individual perspective tab button and its inline action buttons. */
interface PerspectiveTabProps {
  /**
   * The perspective hosting this tab. Passed whole (rather than as
   * individual fields) so `<RegistryTabButtons>` can hand `perspective`
   * to `isCommandActiveForPerspective` without prop-drilling each
   * state field separately as new command migrations land.
   */
  perspective: Perspective;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: () => void;
  onDoubleClick: () => void;
  /** Called with the new name text when the rename editor commits. */
  onRenameCommit: (newName: string) => void;
  onRenameCancel: () => void;
}

/**
 * Individual perspective tab that uses the backend command system for
 * context menus. The registry-driven `<RegistryTabButtons>` renders any
 * inline affordances; today that's the Filter button (rendered by the
 * `<FilterFocusCommandButton>` adapter, which dispatches `nav.focus`
 * against the formula bar's `filter_editor:${id}` spatial-nav scope —
 * see card `01KRGZY33P99J7CGG0XRQGZ352`) plus the Group `<CommandButton>`
 * driven by the generic YAML tab-button rendering. There is no more
 * hardcoded inline filter button.
 *
 * Must be rendered inside a CommandScopeProvider with a perspective
 * moniker so the scope chain is correct.
 */
function PerspectiveTab({
  perspective,
  isActive,
  isRenaming,
  onSelect,
  onDoubleClick,
  onRenameCommit,
  onRenameCancel,
}: PerspectiveTabProps) {
  const { name } = perspective;
  const handleContextMenu = useContextMenu();

  // The TabButton is rendered as a plain `<button>` — NOT wrapped in its
  // own FocusScope. The outer `<FocusScope moniker={`perspective_tab:${id}`}>`
  // (PerspectiveTabFocusable) already covers the same rect: an inactive tab
  // is just the name text plus padding, so an inner `perspective_tab.name`
  // leaf would register at the exact same (x, y) and trigger the kernel's
  // needless-nesting warning. Enter on a focused tab triggers rename via
  // the `ui.entity.startRename` command registered by `ScopedPerspectiveTab`'s
  // CommandScopeProvider — that binding shadows the global `nav.drillIn:
  // Enter`, so the focused-component-knows-it's-focused contract holds via
  // command-scope chain resolution, not a separate inner scope.
  //
  // The registry-rendered `<CommandButton>`s inside `<RegistryTabButtons>`
  // below each own their own inner FocusScope leaf — they live to the
  // right of the tab name and have distinct rects, so they are
  // independently navigable with arrow keys.
  const { activeView } = useViews();
  const boardData = useBoardData();
  const activeBoardId = boardData?.board?.id;
  const activeViewId = activeView?.id;

  return (
    <div className="inline-flex items-center">
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
      {/*
        Registry-driven tab buttons — renders the migrated
        `perspective.filter.focus` and `perspective.group`
        `<CommandButton>`s (plus any future per-command migrations:
        sort, add, …) for the active tab. Gated on `isActive` to match
        the visual layout the legacy `<FilterFocusButton>` /
        `<GroupPopoverButton>` produced: inline affordances only
        appear on the active perspective's tab, where their state is
        meaningful and where their spatial-nav leaves live next to the
        formula bar. Mounted only when the active view is known so the
        scope chain (`perspective:` + `view:` + `board:`) is
        well-formed for `list_commands_for_scope`.
      */}
      {isActive && activeViewId && (
        <RegistryTabButtons
          perspective={perspective}
          activeViewId={activeViewId}
          activeBoardId={activeBoardId}
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
// Filter focus button — DELETED by 01KRE1YA65MMG29RDQDQ0VPJQG, rewired
// to `nav.focus` by 01KRGZY33P99J7CGG0XRQGZ352.
//
// The hardcoded `<FilterFocusButton>` was replaced by a registry-rendered
// tab button in `<RegistryTabButtons>` (above). Today the click site is
// the `<FilterFocusCommandButton>` adapter — it mirrors `<CommandButton>`
// for icon / isActive / moniker rendering but overrides the dispatch to
// issue `nav.focus({ args: { fq: <filter_editor FQM> } })` against the
// formula bar's `filter_editor:${id}` spatial-nav scope. The kernel then
// claims focus on the scope and the scope's own `nav.drillIn` (Enter)
// drives CM6 to take DOM focus — the same path arrow-nav uses when the
// user lands on the formula bar from a neighbouring leaf. This routes
// every focus claim through the single auditable `nav.focus` command
// (card `01KR7CDEFWWVF4WH0BCHE8Y21J`); the previous `FocusFilter` marker
// envelope + `ui.focus.filter` Tauri event channel was deleted alongside
// this rewire.
// ---------------------------------------------------------------------------

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

    // CM6's existing Escape-cancel keymap calls `onClose`. When the
    // surrounding `<FilterFormulaBarFocusable>` is mounted, that
    // callback drills-out to the spatial scope (blur + setFocus). When
    // we are mounted bare in narrow tests, the context is null and
    // Escape is a CM6-internal no-op as before.
    const onEditorEscape = useContext(FilterEditorEscapeContext);

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
          onClose={onEditorEscape ?? undefined}
        />
      </div>
    );
  },
);

// ---------------------------------------------------------------------------
// Group popover button — DELETED by 01KRE1ZTYJ5PPTQ29K72KE88B5.
//
// The hardcoded `<GroupPopoverButton>` + `<GroupSelector>` chain was
// replaced by a registry-rendered `<CommandButton>` for the
// command-driven `perspective.group` entry. The YAML now carries
// `tab_button: { icon: "group" }` and an enum-shaped `group` param
// with `options_from: "perspective.fields"`; the generic
// `<CommandPopover>` renders the picker `<select>` populated by the
// backend `PerspectiveFieldsResolver`. Picking a field dispatches
// `perspective.group` with the picked value plus a scope-chain-
// resolved `perspective_id` — same dispatcher path as before, now
// driven through the unified command pipeline so palette /
// keybindings / cross-window paths all converge on the same
// behaviour.
//
// The spatial-nav moniker for this affordance changed shape too:
// `perspective_tab.group:{id}` → `perspective_tab.perspective.group:{id}`
// (built by `<CommandButton>` from `${surface}.${command.id}:${surfaceId}`).
// ---------------------------------------------------------------------------
