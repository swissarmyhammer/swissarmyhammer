# Task Dates

## Two kinds of date fields

**User-set dates** — the user picks a value via the date editor.

```
Due          hard deadline, when the task must be done
Scheduled    earliest you can start ("ready after" date)
Wait         hide the task until this date (declutters default views)
```

**System-set dates** — written by the system as side effects of mutations. Never user-edited. Displayed as read-only.

```
Created      set once on create(), never changes
Updated      set on every set() primitive, always reflects last mutation
Started      first time Status transitions to "In Progress", never overwritten
Completed    last time Status transitions to "Done", overwritten on re-completion
```

## Editor types

User-set dates use `editor: date` (date picker). System-set dates use `editor: auto` — the system writes them, the UI displays them, nobody edits them. Both are `kind: date` in the field registry. Both are stored as regular EAV triples. The only difference is who writes the value.

## System date behavior

**Created** — set once during `create()`. Immutable after that. Every entity gets one.

**Updated** — set to `now` on every `set()` that touches this entity. Always reflects the most recent mutation. Useful for "recently changed" sorts and staleness detection.

**Started** — set the *first* time Status transitions to "In Progress." Once set, never overwritten, even if the task leaves and re-enters In Progress. This is the "work began" timestamp.

**Completed** — set the *last* time Status transitions to "Done." If a task is reopened and completed again, Completed updates to the new timestamp. This is the "work finished" timestamp.

```
Todo → In Progress         Started = now (first time only)
In Progress → Review       (no change to Started or Completed)
Review → In Progress       (no change — Started already set)
In Progress → Done         Completed = now
Done → In Progress         Completed clears, Started unchanged
In Progress → Done         Completed = now (re-set)
```

## Cycle time

```
Cycle time = Completed - Started
```

This captures the full lifecycle including bounces back and forth. Started is anchored to when work first began. Completed is when it last finished. The number you actually want for flow metrics.

## Status changelog

The `set(task, Status, new_value)` primitive already logs `previous` for undo. If that log entry also carries a timestamp, the undo log for the Status field doubles as a status changelog:

```yaml
- timestamp: 2025-03-01T09:00:00
  value: "In Progress"
  previous: "Todo"
- timestamp: 2025-03-03T14:30:00
  value: "Review"
  previous: "In Progress"
- timestamp: 2025-03-04T10:00:00
  value: "In Progress"
  previous: "Review"
- timestamp: 2025-03-05T16:00:00
  value: "Done"
  previous: "In Progress"
```

Started and Completed are reads against this log:

```
Started   = timestamp of first entry where value = "In Progress"
Completed = timestamp of last entry where value = "Done"
```

The changelog also answers: how long was this task in each status? How many times did it bounce? When did it first enter review? One structure, multiple uses.

## Wait date behavior

A task with a Wait date in the future is hidden from default views. When the Wait date passes, the task reappears automatically. No status change needed — the perspective filter handles it:

```javascript
// Default perspective filter excludes waiting tasks
filter: (entity) => !entity.Wait || new Date(entity.Wait) <= new Date()
```

Wait is a user-set date with filter semantics. The field registry doesn't need to know about the hiding behavior — the perspective filter function does.

## Scheduled date behavior

A task with a Scheduled date is "not ready" until that date passes. It's visible but marked as not actionable yet. "Ready" means: not blocked, not waiting, and past the scheduled date (or no scheduled date).

```javascript
// "Ready" filter
filter: (entity) => {
  if (entity.Wait && new Date(entity.Wait) > new Date()) return false
  if (entity.Scheduled && new Date(entity.Scheduled) > new Date()) return false
  if (entity.Status === "Done") return false
  return true
}
```

## Field definitions

### User-set dates

```yaml
- id: 01JMTASK0000000000DUEDAT0
  name: Due
  type: { kind: date }
  editor: date
  display: date
  width: 120
  sort: datetime

- id: 01JMTASK000000000SCHEDUL0
  name: Scheduled
  type: { kind: date }
  editor: date
  display: date
  width: 120
  sort: datetime

- id: 01JMTASK00000000000WAIT00
  name: Wait
  type: { kind: date }
  editor: date
  display: date
  width: 120
  sort: datetime
```

### System-set dates

```yaml
- id: 01JMTASK000000000CREATED0
  name: Created
  type: { kind: date }
  editor: auto
  display: date
  width: 120
  sort: datetime

- id: 01JMTASK000000000UPDATED0
  name: Updated
  type: { kind: date }
  editor: auto
  display: date
  width: 120
  sort: datetime

- id: 01JMTASK000000000STARTED0
  name: Started
  type: { kind: date }
  editor: auto
  display: date
  width: 120
  sort: datetime

- id: 01JMTASK0000000000COMPL00
  name: Completed
  type: { kind: date }
  editor: auto
  display: date
  width: 120
  sort: datetime
```

## Design decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Started semantics | First time Status → In Progress | Anchors to when work actually began, not when it resumed after a bounce |
| Completed semantics | Last time Status → Done | You care about when it was *finally* done, not the first false finish |
| Status changelog | Undo log entries with timestamps | One structure for undo and flow metrics. No separate history table. |
| Wait behavior | Perspective filter, not a status | No new status values. The field is a date. The filter is a function. |
| Scheduled behavior | Perspective filter for "ready" | Same — no new status, just a date that filters use. |
| editor: auto | System-written, user-visible, not editable | Distinct from `none` (computed at read time) and `date` (user picks). |
| Cycle time | Completed - Started | Full lifecycle including bounces. The useful metric. |
