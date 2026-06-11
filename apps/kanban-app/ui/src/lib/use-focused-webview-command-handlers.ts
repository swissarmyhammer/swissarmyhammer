/**
 * `useFocusedWebviewCommandHandlers` — register webview command-bus handlers
 * for a component WHILE SPATIAL FOCUS IS WITHIN ITS SCOPE'S SUBTREE.
 *
 * # Why focus-gated registration
 *
 * The webview command bus (`webview-command-bus.ts`) holds ONE handler per
 * command id. That suits singleton surfaces — the grid view registers its
 * `grid.*` handlers on mount because at most one grid body is live — but
 * fields and pressables are MANY-INSTANCE surfaces: dozens of `<Field>` zones
 * and `<Pressable>` leaves mount at once, each owning its own live closure
 * (`onEdit`, `onPress`). Registering every instance on mount would leave the
 * last-mounted closure in the slot, so dispatching `pressable.activate`
 * could press a button the user never focused.
 *
 * The contract that makes one-slot-per-id correct anyway: these commands are
 * only ever dispatched AT the focused instance — their keys are scope-gated
 * to a marker moniker the owning component mounts (`ui:field` /
 * `ui:pressable`), and the keymap layer's chain walk (`extractChainBindings`)
 * binds them whenever that marker appears ANYWHERE in the focused scope
 * chain. The bus registration must match that granularity exactly: the
 * handler stays live while focus is anywhere WITHIN the instance's spatial
 * subtree (the instance's zone itself, or a descendant such as a tag pill
 * inside a field), via `useOptionalIsFocusWithin`. Gating on strict direct
 * focus instead left a dead key: with a pill focused, the keymap still
 * resolved Enter to `field.edit`, the slot was empty, and the dispatch died
 * on the plugin's inert host execute. For a true spatial leaf (a
 * `<Pressable>` — a registered `<FocusScope>` cannot contain another, per
 * the kernel's scope-is-leaf invariant) subtree containment degenerates to
 * direct focus, so the same gate serves both surfaces. Distinct instances'
 * subtrees are disjoint, so the slot always holds exactly one closure —
 * the focused instance's — and is empty when no instance contains focus
 * (a dispatch then falls through to the plugin's inert host execute, a
 * harmless success).
 *
 * On a focus handoff between two instances the bus's ownership-guarded
 * cleanup keeps the transition safe regardless of effect ordering: the newly
 * focused instance's registration overwrites the slot, and the previously
 * focused instance's cleanup deletes the slot only if it still owns it.
 *
 * # Handler invariant — presentation only
 *
 * Handlers registered through this hook inherit the bus's invariant: pure
 * presentation (local state, DOM focus, re-dispatch via `useDispatchCommand`)
 * — never a direct MCP-transport call. `webview-command-bus.guard.node.test
 * .ts` enforces this mechanically for every registration site, including
 * this one.
 *
 * @example
 * ```tsx
 * const handlers = useMemo(
 *   () => ({ "pressable.activate": guarded, "pressable.activateSpace": guarded }),
 *   [],
 * );
 * useFocusedWebviewCommandHandlers(moniker, handlers);
 * ```
 */

import { useEffect, useMemo, useRef } from "react";
import { useOptionalIsFocusWithin } from "@/lib/entity-focus-context";
import {
  registerWebviewCommandHandler,
  type WebviewCommandHandler,
} from "@/lib/webview-command-bus";
import { useOptionalFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import { composeFq, type SegmentMoniker } from "@/types/spatial";

/**
 * Register `handlers` (a `commandId → handler` record) on the webview command
 * bus whenever spatial focus is within the caller's scope subtree (the
 * caller's zone or any spatial descendant of it), and release them when focus
 * moves outside (or on unmount).
 *
 * The caller's scope FQM is composed exactly the way its `<FocusScope>`
 * composes it — parent FQM from context plus `segment` — so the focus
 * subscription tracks the same kernel slot the scope registers. Outside the
 * spatial provider stack (lightweight unit-test harnesses) the parent FQM is
 * absent and the hook degrades to never registering, matching the keymap
 * layer's behavior of never binding the commands there.
 *
 * The id set of `handlers` must be fixed for the lifetime of the caller —
 * the record's VALUES are read fresh through a ref on every invocation (so
 * closures over the latest props are unnecessary at the call site), but the
 * KEYS are snapshotted when focus arrives.
 *
 * @param segment - The caller's relative scope moniker (the same `moniker`
 *   its `<FocusScope>` mounts).
 * @param handlers - Map of plugin command id to the live behavior to run
 *   while focus is within this instance's subtree.
 */
export function useFocusedWebviewCommandHandlers(
  segment: SegmentMoniker,
  handlers: Readonly<Record<string, WebviewCommandHandler>>,
): void {
  const parentFq = useOptionalFullyQualifiedMoniker();
  const fq = useMemo(
    () => (parentFq === null ? null : composeFq(parentFq, segment)),
    [parentFq, segment],
  );
  // `""` is the degenerate no-spatial-stack key — `useOptionalIsFocusWithin`
  // treats it as never-contained, so a missing spatial provider stack
  // (fq === null) means the handlers never register.
  const isFocusWithin = useOptionalIsFocusWithin(fq ?? "");

  // Latest-value ref so the registered closures always see the current
  // handler record without re-registering per render.
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    if (!isFocusWithin) return;
    const cleanups = Object.keys(handlersRef.current).map((id) =>
      registerWebviewCommandHandler(id, (opts) =>
        handlersRef.current[id]?.(opts),
      ),
    );
    return () => {
      for (const cleanup of cleanups) cleanup();
    };
  }, [isFocusWithin]);
}
