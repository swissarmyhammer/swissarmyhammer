/**
 * `<Inspectable>` — entity-aware wrapper that opens the inspector on
 * double-click.
 *
 * # Why this is its own component
 *
 * The spatial-nav primitives `<FocusScope>` and `<FocusZone>` are pure
 * spatial-nav infrastructure: they register rects with the Rust-side
 * spatial graph, subscribe to focus claims, and own click / right-click
 * for spatial focus and context menus. Inspect-on-double-click is a
 * **kanban-domain** concern — only entities
 * (`task:`, `tag:`, `column:`, `board:`, `field:`, `attachment:`) are
 * inspectable. UI chrome (`ui:*`, `perspective_tab:`, `cell:*`,
 * `grid_cell:*`) is not.
 *
 * Earlier revisions threaded a boolean `inspectOnDoubleClick` prop into
 * the primitives and registered `useDispatchCommand("app.inspect")` from
 * inside their bodies. Space lived even further afield, on a `board.inspect`
 * command at the BoardView's `<CommandScopeProvider>`. Both arrangements
 * shared three smells:
 *
 *   1. **Domain leakage.** A generic spatial-nav primitive (or the
 *      board view's command list) imported kanban-domain knowledge.
 *   2. **Implicit naming.** `inspectOnDoubleClick={true}` on a
 *      `<FocusScope moniker="task:01">` is a flag whose meaning is
 *      buried in conventions; `board.inspect` claimed Space at a scope
 *      that had no architectural relationship to "inspectable entity".
 *   3. **Composability.** A focused inspector field zone (mounted in
 *      the inspector layer, a sibling of BoardView) had no path to
 *      `board.inspect` — its scope chain didn't reach the BoardView's
 *      scope, so Space did nothing on a focused field.
 *
 * `<Inspectable>` *names* the architectural concept ("this DOM subtree
 * is an inspectable entity") and is the **single source** of the
 * double-click → `app.inspect` dispatch. The primitives are smaller and
 * pure-spatial; the dblclick inspect plumbing lives in exactly one place
 * that can be reasoned about, audited, and replaced as a unit.
 *
 * # Where Space went (Card G)
 *
 * The Space → inspect gesture is no longer wired here. Earlier revisions
 * mounted a scope-level `entity.inspect` `CommandDef` per `<Inspectable>`
 * (plus a root-scope fallback in `app-shell.tsx`); Card G consolidated
 * those into the SINGLE plugin-owned `entity.inspect`
 * (`builtin/plugins/app-shell-commands/commands/ui.ts`): a global Space command whose
 * execute resolves the focused entity SERVER-SIDE from the dispatched
 * scope chain (innermost inspectable moniker wins — the same
 * closest-`<Inspectable>` semantics the per-scope defs provided, because
 * the chain is leaf-first). The plugin-owned guard
 * (`inspect-and-focus-commands.plugin-owned.node.test.ts`) keeps any
 * client-side `entity.inspect` `CommandDef` from reappearing.
 *
 * # Usage — wrapper component
 *
 *   ```tsx
 *   <Inspectable moniker={asSegment(`task:${task.id}`)}>
 *     <FocusScope moniker={asSegment(`task:${task.id}`)}>
 *       {cardBody}
 *     </FocusScope>
 *   </Inspectable>
 *   ```
 *
 * Wrap every entity call site (`task:`/`tag:` cards, `column:` zones,
 * `board:` zones, `field:` rows, `attachment:` items, mention pills).
 * Do NOT wrap chrome (`ui:*`, `perspective_tab:`, `cell:*`,
 * `grid_cell:*`) — chrome is not inspectable. The architectural guard
 * (`focus-architecture.guards.node.test.ts`, Guards B + C) enforces
 * both directions.
 *
 * # Usage — hook escape hatch for non-`<div>` hosts
 *
 * `<Inspectable>` renders a `<div className="contents">`. That works
 * for nearly every call site, but DOM rules forbid `<div>` between
 * `<tbody>` and `<tr>` (the browser parser moves the div outside the
 * table). For table rows and similar restricted contexts, use
 * `useInspectOnDoubleClick(moniker)` and attach the returned handler
 * to the host element directly:
 *
 *   ```tsx
 *   function EntityRow({ entityMk, children }: Props) {
 *     const onDoubleClick = useInspectOnDoubleClick(asSegment(entityMk));
 *     return <tr onDoubleClick={onDoubleClick}>{children}</tr>;
 *   }
 *   ```
 *
 * Both `<Inspectable>` and the hook resolve the same
 * `useDispatchCommand("app.inspect")` call from this single file, so
 * Guard A continues to hold (one non-test file owns the inspect
 * dispatch). The two paths share a private `useInspectDoubleClickHandler`
 * helper to keep the editable-surface skip logic identical.
 *
 * # Behavior
 *
 *   - On double-click within the wrapper / on the host element,
 *     dispatches `app.inspect` against the wrapper's `moniker`.
 *   - Skips the dispatch when the gesture lands on an editable surface
 *     (`<input>`, `<textarea>`, `<select>`, or any `[contenteditable]`
 *     ancestor) — the editor owns the gesture.
 *   - Calls `e.stopPropagation()` after a successful dispatch so
 *     ancestors do not also see the gesture.
 *
 * # Inner-handler propagation contract
 *
 * Inner buttons that own the double-click gesture for their own purpose
 * (e.g. a perspective tab's `<button onDoubleClick={startRename}>`)
 * should call `e.stopPropagation()` to keep the gesture from reaching
 * the wrapping `<Inspectable>`. This wrapper does not check whether the
 * gesture originated from a button — propagation is the standard React
 * mechanism, and inner handlers are expected to stop the event when
 * they consume it.
 *
 * # DOM shape
 *
 * `<Inspectable>` renders a single `<div className="contents">`. The
 * Tailwind `contents` class makes the wrapper itself layout-transparent:
 * children participate in the parent's flex/grid box as if no wrapper
 * existed. That preserves the layout of every existing call site that
 * previously had `<FocusScope>` / `<FocusZone>` directly under a flex
 * parent.
 */

import { useCallback, type ReactNode } from "react";
import { useDispatchCommand } from "@/lib/command-scope";
import type { SegmentMoniker } from "@/types/spatial";

/** Reference type for `useDispatchCommand("app.inspect")` — preset dispatcher. */
type InspectDispatcher = ReturnType<typeof useDispatchCommand>;

/**
 * Memoize a `<div>` / `<tr>`-grade `onDoubleClick` handler that
 * dispatches `app.inspect` against `moniker` unless the gesture lands
 * on an editable surface.
 *
 * Both `<Inspectable>` and {@link useInspectOnDoubleClick} go through
 * this hook so the handler shape stays identical between the wrapper
 * and the table-row escape hatch — and so callers that already paid
 * for one `useDispatchCommand("app.inspect")` registration can pass
 * that dispatcher through instead of registering a second one.
 *
 * The handler:
 *   - skips editable surfaces (`<input>`, `<textarea>`, `<select>`,
 *     `[contenteditable]`) so caret placement is not stolen,
 *   - calls `e.stopPropagation()` after a successful dispatch so
 *     ancestors do not also see the gesture,
 *   - logs and swallows any dispatch error.
 */
function useInspectDoubleClickHandler(
  dispatch: InspectDispatcher,
  moniker: SegmentMoniker,
): (e: React.MouseEvent) => void {
  return useCallback(
    (e: React.MouseEvent) => {
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;
      e.stopPropagation();
      dispatch({ target: moniker }).catch(console.error);
    },
    [dispatch, moniker],
  );
}

/**
 * Hook that returns a memoized `onDoubleClick` handler dispatching
 * `app.inspect` against `moniker`.
 *
 * Public dispatch site for inspect-on-double-click on hosts that cannot
 * accept the standard `<Inspectable>` wrapper — most notably `<tr>`
 * rows inside a `<tbody>`, where DOM rules forbid a `<div>` between
 * the two. `<Inspectable>` shares the same handler-building hook
 * ({@link useInspectDoubleClickHandler}) so both paths fire identically.
 *
 * The handler:
 *   - skips editable surfaces (`<input>`, `<textarea>`, `<select>`,
 *     `[contenteditable]`) so caret placement is not stolen,
 *   - calls `e.stopPropagation()` after a successful dispatch so
 *     ancestors do not also see the gesture,
 *   - logs and swallows any dispatch error.
 *
 * @param moniker - Entity moniker to dispatch against. Must be one of
 *   `task:`, `tag:`, `column:`, `board:`, `field:`, `attachment:`.
 *   Guard B (`focus-architecture.guards.node.test.ts`) enforces the
 *   prefix at every call site.
 * @returns A reference-stable double-click handler suitable for
 *   `onDoubleClick` on any host element.
 */
export function useInspectOnDoubleClick(
  moniker: SegmentMoniker,
): (e: React.MouseEvent) => void {
  const dispatch = useDispatchCommand("app.inspect");
  return useInspectDoubleClickHandler(dispatch, moniker);
}

/** Props for `<Inspectable>`. */
export interface InspectableProps {
  /**
   * Entity moniker — must resolve to an inspectable entity (`task:`,
   * `tag:`, `column:`, `board:`, `field:`, `attachment:`). The
   * architectural guard (Guard B in
   * `focus-architecture.guards.node.test.ts`) enforces the prefix at
   * the call site.
   */
  moniker: SegmentMoniker;
  /** Children rendered inside the layout-transparent wrapper. */
  children: ReactNode;
}

/**
 * Wrap an entity subtree so a double-click dispatches `app.inspect`
 * against the entity's moniker.
 *
 * The dispatcher is registered exactly once per mounted `<Inspectable>`
 * via the shared {@link useInspectOnDoubleClick} hook — the per-render
 * registry walk that `useDispatchCommand` performs is paid once per
 * inspectable entity, not once per focusable scope/zone in the tree.
 *
 * The `Space` key gesture is NOT wired here (Card G): the plugin-owned
 * global `entity.inspect` command resolves the focused entity from the
 * dispatched scope chain server-side, so a focused descendant of this
 * wrapper inspects via its own focused moniker — the same
 * closest-`<Inspectable>`-wins outcome the retired scope-level
 * `CommandDef` produced. Editable surfaces (`<input>`, `<textarea>`,
 * `<select>`, `[contenteditable]`) are filtered by the global keybinding
 * handler's `isEditableTarget` check before any binding fires.
 *
 * For non-`<div>` hosts (e.g. table rows), use the hook directly
 * instead of this wrapper — see {@link useInspectOnDoubleClick}.
 *
 * @see {@link InspectableProps}
 */
export function Inspectable({ moniker, children }: InspectableProps) {
  const dispatch = useDispatchCommand("app.inspect");

  const onDoubleClick = useInspectDoubleClickHandler(dispatch, moniker);

  // `className="contents"` makes the wrapper layout-transparent: the
  // browser treats the children as if they were direct children of the
  // grandparent. That keeps every existing flex/grid call site working
  // unchanged when the wrapper is dropped between the parent and the
  // primitive that the consumer was already rendering. The wrapper still
  // exists in the DOM (so `onDoubleClick` is reachable), but it does
  // not introduce a new layout box.
  return (
    <div onDoubleClick={onDoubleClick} className="contents">
      {children}
    </div>
  );
}
