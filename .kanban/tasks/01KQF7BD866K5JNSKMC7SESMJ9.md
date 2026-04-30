---
assignees:
- claude-code
position_column: todo
position_ordinal: 9e817f80
project: spatial-nav
title: 'Diagnose & fix: "duplicate FQM registration replaces prior scope" warnings flooding the kernel log'
---
## What

Production logs show repeated `duplicate FQM registration replaces prior scope` warnings from `swissarmyhammer-focus`. The FQM-as-key invariant says paths cannot collide by construction — if they do, two React primitives composed the *same* path, which is a programmer mistake that needs root-cause investigation, not log noise to ignore.

User report:
> "this type of error is all over the log -- let's task this up -- you are the programmer so this is your mistake to figure out"

Sample log line:
```
2026-04-30 07:32:09.655461-0500 Fault kanban-app: [com.swissarmyhammer.kanban:default]
duplicate FQM registration replaces prior scope — a real duplicate FQM is a
programmer mistake (two primitives whose composed paths collide)
fq=/window/ui:perspective/ui:view/board:board/ui:board/column:done/task:01KQ2E7RPBPJ8T8KZX39N2SZ0A/field:task:01KQ2E7RPBPJ8T8KZX39N2SZ0A.project/project:spatial-nav
op="register_scope"
```

Decomposed path:
```
/window
  /ui:perspective
    /ui:view
      /board:board
        /ui:board
          /column:done
            /task:01KQ2E7RPBPJ8T8KZX39N2SZ0A
              /field:task:01KQ2E7RPBPJ8T8KZX39N2SZ0A.project
                /project:spatial-nav        <-- duplicate registration here
```

## Likely shapes (to verify, not assume)

The warning fires from `register_scope` when an FQM key already exists in the registry. Three plausible causes:

1. **React StrictMode double-mount** — in dev, components mount → unmount → remount synchronously. If `useEntityScopeRegistration` (which registers both during render *and* in a cleanup-only `useEffect`) leaks a registration between the two passes, the second mount's register sees the first's still-live entry. The dual register-during-render-+-effect-cleanup pattern was added intentionally for the focus-claim-on-mount path; needs to be re-checked under the FQM-keyed registry.

2. **Field-display children that themselves render FocusScopes** — the path ends in `field:...project / project:spatial-nav`. The field's display component (a project pill / badge / chip) likely wraps in a `<FocusScope moniker={asSegment("project:spatial-nav")}>`. If the field row ALSO renders an inspectable wrapper around the same value (for click-to-inspect), both wrappers would compose the same FQM. Look at `kanban-app/ui/src/components/fields/displays/*` — especially anything project-typed (likely `link-display.tsx` or a project-specific one).

3. **`Inspectable` + `FocusScope` doubling up** — `<Inspectable>` mounts a `<FocusScope>` internally (per `inspectable.tsx`). If a parent component already wrapped the same entity in a `<FocusScope moniker={asSegment("project:spatial-nav")}>` and then renders `<Inspectable moniker={asSegment("project:spatial-nav")}>` inside, both register at the same composed FQM.

## Investigation plan

1. **Confirm the warning fires from a single render**, not from real path collisions across separate components. Add a stack trace to the `register_scope` warn site (or grep for the warning + surrounding context in the log) to see whether both registrations come from the same React tree path.

2. **Reproduce locally** — run `npm run tauri dev`, open a board with project-typed task fields visible, hover/click cards, and watch the live log:
   ```
   log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --info --debug --last 10m | grep "duplicate FQM"
   ```

3. **Inspect field-display rendering** for project values. Walk the call chain from `FieldRow` → field display registry → the project-pill display component. Check whether both the row AND the display register `<FocusScope moniker={asSegment("project:...")}>` against the same parent FQM.

4. **Inspect any other duplicate-FQM patterns in the log** — the user said "this type of error is all over the log," so the project-pill case may be one of several. Other field types (tag pill, attachment, link, mention) may have the same shape. Pull a representative log sample to enumerate which segment values appear most often.

5. **Determine the right fix**:
   - If a wrapper-inside-wrapper is the cause, remove the inner registration (the outer is the correct entity boundary) — or rename one of the segments so they're distinct (`project:spatial-nav` outer, `pill:spatial-nav` inner).
   - If StrictMode double-mount is the cause, the dual register-during-render-+-effect pattern needs an idempotency guard: only re-register on actual `(fq, scope)` change, treat a same-value re-register as a no-op silently.
   - If a real path collision in production code, fix the segment to disambiguate.

## Acceptance criteria

- [ ] Root cause identified and named (which file, which component, which path).
- [ ] Fix lands and the warning no longer fires for the project-pill case during normal use.
- [ ] Repro steps documented in the task description so future regressions can be caught fast.
- [ ] Verified in `npm run tauri dev` against `log show` — zero `duplicate FQM registration` warnings during a 30-second board interaction sample (open inspectors, click pills, scroll, switch perspectives).
- [ ] If StrictMode double-mount turns out to be the cause, the fix MUST keep the dual register-during-render-+-effect contract intact (it exists for focus-claim-on-mount); idempotency guard, not removal.

## Cross-references

- Memory: `feedback_path_monikers.md` — path-as-key invariant.
- Parent surface: `01KQD6064G1C1RAXDFPJVT1F46`, Layer 2 (`01KQD8XM2T0FWHXANCK0KVDJH1`).
- The warning was added in `swissarmyhammer-focus/src/registry.rs` `register_scope` (or thereabouts) as part of the FQM refactor — find via `git log -S "duplicate FQM registration replaces prior scope"`.

## Workflow

- Investigation-first. Don't write code until step 1's stack trace + step 2's repro have named the offender. The architectural answer (path-as-key) is correct; the bug is in *how* React composes the path, not in the kernel.
