/**
 * <RenderProfiler id="..."> — wraps React.Profiler with aggregated logging
 * to the Tauri OS log stream. Each commit inside the wrapped subtree emits:
 *   [profile] <id> <phase> <ms>ms (m=<mounts> u=<updates> n=<nested> total=<sumMs>ms max=<peakMs>ms)
 *
 * Tail with:
 *   log stream --predicate 'subsystem == "com.swissarmyhammer.kanban" \
 *     AND composedMessage CONTAINS "[profile]"'
 *
 * React.Profiler is dev-only by default; leaving this in the tree for release
 * builds has no runtime cost unless the app is built with profiling enabled.
 */
import {
  Profiler,
  useCallback,
  useRef,
  type ProfilerOnRenderCallback,
  type ReactNode,
} from "react";
import { warn } from "@tauri-apps/plugin-log";

interface RenderProfilerProps {
  /** Tag that appears in the log line — used to identify the subtree. */
  id: string;
  /** Subtree to profile. */
  children: ReactNode;
  /** Suppress log lines for commits faster than this. Default 0 (all commits logged). */
  minDurationMs?: number;
  /** Skip `update`-phase log lines; still log `mount` and `nested-update`. Default false. */
  mountsOnly?: boolean;
}

/**
 * Wrap a React subtree with a `<Profiler>` that aggregates commit counts and
 * emits a single `[profile]` log line per commit. Counters are per-instance,
 * live in a ref so re-renders don't reset them, and reset naturally on unmount
 * (the ref is garbage-collected with the component).
 *
 * The component is a pass-through: children render exactly as if the wrapper
 * were absent. The only observable effect is log output via
 * `@tauri-apps/plugin-log`'s `warn`.
 *
 * @param props.id - Identifier embedded in every log line for this subtree.
 * @param props.children - Subtree to profile.
 * @param props.minDurationMs - Threshold below which commits are counted but not logged.
 * @param props.mountsOnly - When true, `update` commits are counted but not logged.
 */
export function RenderProfiler({
  id,
  children,
  minDurationMs = 0,
  mountsOnly = false,
}: RenderProfilerProps) {
  const counters = useRef({
    mounts: 0,
    updates: 0,
    nested: 0,
    total: 0,
    max: 0,
  });

  const onRender: ProfilerOnRenderCallback = useCallback(
    (_id, phase, actualDuration) => {
      const c = counters.current;
      if (phase === "mount") c.mounts += 1;
      else if (phase === "update") c.updates += 1;
      else c.nested += 1;
      c.total += actualDuration;
      if (actualDuration > c.max) c.max = actualDuration;

      if (actualDuration < minDurationMs) return;
      if (mountsOnly && phase === "update") return;

      warn(
        `[profile] ${id} ${phase} ${actualDuration.toFixed(1)}ms ` +
          `(m=${c.mounts} u=${c.updates} n=${c.nested} ` +
          `total=${c.total.toFixed(0)}ms max=${c.max.toFixed(1)}ms)`,
      );
    },
    [id, minDurationMs, mountsOnly],
  );

  return (
    <Profiler id={id} onRender={onRender}>
      {children}
    </Profiler>
  );
}
