/**
 * Strict `useDispatchCommand` mock factory for dispatch-capturing tests.
 *
 * Component tests shadow `@/lib/command-scope`'s `useDispatchCommand` to
 * capture what a component dispatches. The historical hand-rolled pattern
 * fell back to a SILENT no-op (`return vi.fn(() => Promise.resolve())`) for
 * any id the test didn't branch on — so a production dispatch site reverting
 * to a retired command id (e.g. `ui.inspector.close` after the `ui.*` →
 * `app.*` rename, card 01KTEBZSVGAZ881RAZZWWZXGPE) passed every node test,
 * while the real-dispatch browser tests sit in the known-fail
 * `SERIALIZE_TO_IPC_FN` set and could not catch it either.
 *
 * This factory replaces that fallback with a loud failure: every command id
 * requested anywhere in the rendered tree must appear in the test's `known`
 * map — ids the test asserts on map to their capture spies, ids the tree
 * legitimately dispatches but the test ignores map to explicit no-ops. Any
 * other id (retired, renamed, typo'd) throws, failing the test at render
 * time.
 *
 * Usage inside a `vi.mock` factory (the dynamic import keeps the hoisted
 * factory self-contained):
 *
 * ```ts
 * vi.mock("@/lib/command-scope", async (importOriginal) => {
 *   const actual = await importOriginal<typeof import("@/lib/command-scope")>();
 *   const { strictUseDispatchCommand } =
 *     await import("@/test/strict-dispatch-mock");
 *   return {
 *     ...actual,
 *     useDispatchCommand: strictUseDispatchCommand({
 *       "app.inspector.close": mockDispatchClose,
 *       "nav.focus": () => Promise.resolve(),
 *     }),
 *   };
 * });
 * ```
 */

/**
 * Loose dispatch-callable shape covering both production overload returns
 * (pre-bound `(opts?) => Promise` and ad-hoc `(cmd, opts?) => Promise`) so
 * capture spies of either arity slot in without casts.
 */
export type MockDispatchFn = (...args: unknown[]) => Promise<unknown>;

/**
 * Build the thrown error for an id outside the test's `known` map, naming
 * both the offending id and the ids the test does recognize.
 */
function unknownIdError(cmd: string, known: Record<string, unknown>): Error {
  return new Error(
    `useDispatchCommand mock: unknown command id "${cmd}". ` +
      `Known ids: ${Object.keys(known).sort().join(", ")}. ` +
      `If this is a real registered command the tree legitimately ` +
      `dispatches, add it to this test's known map; a retired or typo'd ` +
      `id at a dispatch site must fail loudly, never silently no-op.`,
  );
}

/**
 * Create a strict `useDispatchCommand` replacement from a map of known
 * command ids to their dispatch implementations.
 *
 * Mirrors both production overloads:
 * - Pre-bound (`useDispatchCommand("app.dismiss")`) — validates the id at
 *   hook time and returns the mapped dispatch.
 * - Ad-hoc (`useDispatchCommand()`) — returns a `(cmd, opts)` dispatcher
 *   that validates each id at call time.
 *
 * @param known - Every command id the rendered tree may request, mapped to
 *   its dispatch implementation (a capture spy or an explicit no-op).
 * @returns The mock hook; it throws {@link unknownIdError} for any id not
 *   present in `known`.
 */
export function strictUseDispatchCommand(
  known: Record<string, MockDispatchFn>,
): (cmd?: string) => MockDispatchFn {
  return (cmd?: string) => {
    if (cmd === undefined) {
      // Ad-hoc overload: the id arrives at call time.
      return (adHocCmd?: unknown, opts?: unknown) => {
        const id = String(adHocCmd);
        const dispatch = known[id];
        if (!dispatch) throw unknownIdError(id, known);
        return dispatch(opts);
      };
    }
    const dispatch = known[cmd];
    if (!dispatch) throw unknownIdError(cmd, known);
    return dispatch;
  };
}
