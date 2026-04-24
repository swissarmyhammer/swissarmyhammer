---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9380
title: Add RenderProfiler diagnostic + strategic placements to establish before/after telemetry for performance work
---
## What

Establish render-telemetry infrastructure that every downstream performance task will use as its before/after measurement. Without this, each task's "did it actually help?" acceptance criterion reduces to subjective feel — with this in place, every task can show a concrete delta in commits/ms against a named subtree.

**New component** — `kanban-app/ui/src/lib/render-profiler.tsx`:

```tsx
/**
 * <RenderProfiler id="..."> — wraps React.Profiler with aggregated logging
 * to the Tauri OS log stream. Each commit inside the wrapped subtree emits:
 *   [profile] <id> <phase> <ms> (m=<mounts> u=<updates> n=<nested> total=<sumMs> max=<peakMs>)
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
  id: string;
  children: ReactNode;
  /** Suppress commits faster than this. Default 0 (all commits logged). */
  minDurationMs?: number;
  /** Skip updates, log mounts and nested-updates only. Default false. */
  mountsOnly?: boolean;
}

export function RenderProfiler({ id, children, minDurationMs = 0, mountsOnly = false }: RenderProfilerProps) {
  const c = useRef({ mounts: 0, updates: 0, nested: 0, total: 0, max: 0 });

  const onRender: ProfilerOnRenderCallback = useCallback(
    (_id, phase, actualDuration) => {
      const s = c.current;
      if (phase === "mount") s.mounts += 1;
      else if (phase === "update") s.updates += 1;
      else s.nested += 1;
      s.total += actualDuration;
      if (actualDuration > s.max) s.max = actualDuration;
      if (actualDuration < minDurationMs) return;
      if (mountsOnly && phase === "update") return;
      warn(
        `[profile] ${id} ${phase} ${actualDuration.toFixed(1)}ms ` +
          `(m=${s.mounts} u=${s.updates} n=${s.nested} total=${s.total.toFixed(0)}ms max=${s.max.toFixed(1)}ms)`,
      );
    },
    [id, minDurationMs, mountsOnly],
  );

  return <Profiler id={id} onRender={onRender}>{children}</Profiler>;
}
```

**Strategic placements** in `kanban-app/ui/src/App.tsx`:

```tsx
<CommandBusyProvider>
  <RenderProfiler id="rust-engine">
    <RustEngineContainer>
      <RenderProfiler id="window">
        <WindowContainer>
          <RenderProfiler id="app-mode">
            <AppModeContainer>
              <BoardContainer>
                <div className="h-screen ...">
                  <NavBar />
                  <ViewsContainer>
                    <PerspectivesContainer>
                      <PerspectiveContainer>
                        <div className="flex-1 ...">
                          <RenderProfiler id="view-body">
                            <ViewContainer />
                          </RenderProfiler>
                        </div>
                      </PerspectiveContainer>
                    </PerspectivesContainer>
                  </ViewsContainer>
                  <ModeIndicator />
                </div>
                <InspectorsContainer />
              </BoardContainer>
            </AppModeContainer>
          </WindowContainer>
        </RustEngineContainer>
      </RenderProfiler>
    </RustEngineContainer>
  </RenderProfiler>
</CommandBusyProvider>
```

Four tags give a clear signal map for every downstream task:
- `rust-engine` updates → entity store churned (event or refresh).
- `window` updates → WindowContainer state changed (board switch, open_boards, loading).
- `app-mode` updates → UIState consumers re-rendered (the target of 01KPZREKCQXN5AX0SMEE2X0ZWR).
- `view-body` updates → grid/board/perspective rendered (the target of 01KPZQDAC6P0AHTQ5F08A170H4 and 01KPZQ6QA62FRBSMB1VK0ATYSY).

Each downstream performance task can capture a before/after snapshot:
```
# Before fix — hold ↓ for 3s
log show --last 5s --predicate '... AND composedMessage CONTAINS "[profile]"'
# [profile] app-mode update 0.4ms (m=1 u=28 n=0 total=12ms max=0.9ms)
# [profile] view-body update 3.1ms (m=1 u=28 n=0 total=89ms max=4.2ms)

# After fix
# [profile] app-mode update 0.4ms (m=1 u=0 n=0 total=0ms max=0.0ms)
# [profile] view-body update 3.1ms (m=1 u=2 n=0 total=6ms max=3.4ms)
```

### Files
- `kanban-app/ui/src/lib/render-profiler.tsx` — new.
- `kanban-app/ui/src/App.tsx` — four wrapper placements at the Rust-engine / window / app-mode / view-body cut points.
- `kanban-app/ui/src/lib/render-profiler.test.tsx` — unit test.

### Subtasks
- [ ] Create `render-profiler.tsx` with the `RenderProfiler` component and the counters ref pattern.
- [ ] Wrap the four strategic points in `App.tsx` without perturbing the existing provider order or JSX structure.
- [ ] Add unit test: mount `RenderProfiler id="t">{…}</RenderProfiler>`; assert the `Profiler` renders children; force a re-render; assert the mocked `@tauri-apps/plugin-log` `warn` was called with a string starting with `[profile] t ` and increments from `m=1 u=0` on mount to `m=1 u=1` on update.
- [ ] Verify the component is a no-op vs. children semantics: assert children output is unchanged whether wrapped or not (use snapshot or DOM query).
- [ ] Manual smoke: start `npm run dev`, open the 2000-row board, `log stream --predicate '... AND composedMessage CONTAINS "[profile]"'`, confirm four named subtrees emit `mount` lines on load and `update` lines on arrow-key nav.

## Acceptance Criteria
- [ ] `RenderProfiler` is a thin wrapper over `React.Profiler`: zero behavioral impact on children, just logging/counting.
- [ ] Each commit within a wrapped subtree emits exactly one `[profile] <id> <phase> ...` log line via `@tauri-apps/plugin-log`'s `warn`.
- [ ] Cumulative counters (`m=`, `u=`, `n=`, `total=`, `max=`) are maintained per-instance and survive re-renders, reset on unmount.
- [ ] `minDurationMs` suppression works: commits shorter than the threshold are counted but not logged.
- [ ] `mountsOnly` flag works: `update`-phase commits are counted but not logged when enabled.
- [ ] With the four strategic placements live, running the app and tailing the log shows distinct `rust-engine` / `window` / `app-mode` / `view-body` lines on load and on nav — confirmed by `log stream --predicate '... AND composedMessage CONTAINS "[profile]"'`.
- [ ] No change to any existing test's behavior or any user-visible UI.

## Tests
- [ ] `kanban-app/ui/src/lib/render-profiler.test.tsx` — mount the wrapper with a mocked `@tauri-apps/plugin-log`; rerender; assert `warn` was called with `[profile] <id> mount` and `[profile] <id> update` in order, and that the logged counters advanced correctly.
- [ ] Same file — `minDurationMs` threshold test: force a render whose `actualDuration` is below the threshold; assert `warn` NOT called (may require feeding a mocked onRender since actualDuration is React-provided).
- [ ] Same file — children-are-unchanged test: mount with and without the wrapper, assert identical DOM output.
- [ ] Run: `cd kanban-app/ui && npm test -- render-profiler`. Expected: green.
- [ ] Full UI suite: `cd kanban-app/ui && npm test`. Expected: green (no regressions from the App.tsx placements).
- [ ] Manual smoke described above.

## Workflow
- Use `/tdd` — write the unit tests first, then the component, then the App.tsx placements.
- This task is the dependency root for the performance chain. Land it first so every subsequent task can measure its claim against named subtree telemetry. #performance #frontend