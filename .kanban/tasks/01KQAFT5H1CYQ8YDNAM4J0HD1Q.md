---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff9080
title: 'RecordingAgent: Stop-hook recording never flushed to disk when hook dies mid-flight'
---
## Symptom

The 16:06–16:19 Stop-hook run today logged at 16:06:19:

```
INFO avp_common::context: Wrapping validator agent with RecordingAgent (path=/Users/wballard/github/swissarmyhammer/swissarmyhammer-avp/.avp/recordings/no-session-1777392379807437.json)
```

That recording file **does not exist on disk**. The most recent file in `.avp/recordings/` is from 16:06 local time (Apr 28 11:06 = UTC 16:06 — that's the *PostToolUse* run from the same timestamp; verified by content, contains security-rules verdicts).

The `no-session-1777392379807437.json` path was prepared, the RecordingAgent was wrapped around the validator agent, calls and notifications presumably accumulated in memory for ~10 minutes — and then the parent process (claude-code) timed out and fired SubagentStop without giving the recorder a chance to flush.

## Why this matters

When something goes wrong with the Stop hook (per sibling task `01KQAFFZDX40GSKXQVS0MTNDWV`, the last rule deadlocked silently), the recording is the diagnostic of last resort. It contains the full per-rule prompt, the agent's full text stream, every tool call and its arguments, every notification — far more than what makes it to `.avp/log`.

If the recording only persists when the hook completes successfully, **the recording cannot help diagnose the cases we most want to diagnose**. We need on-disk persistence to be robust to mid-flight termination.

## Where to look

- `agent_client_protocol_extras::recording::RecordingAgent` — find where the JSON is written. If it's only on `Drop` and the writer is buffered via `BufWriter`, an unflushed buffer at process kill time is lost.
- The wrapping site in `avp-common` — search for `RecordingAgent::new` or wherever `Wrapping validator agent with RecordingAgent` is logged. Confirm whether the writer flushes incrementally or only at end.

## Three layers of fix, pick whichever fits the architecture

### Layer 1 (lowest cost): Periodic flush

After every N notifications or every M seconds, flush the JSON to disk. Trades off some I/O for guaranteed bounded data loss. Default to flush-on-every-prompt-call-completion (so a per-rule verdict is always durable even if the next rule deadlocks).

### Layer 2: Append-only JSONL instead of monolithic JSON

The current schema is `{"calls": [...]}` — a single JSON object. To append to it, you have to re-serialize the whole thing on every flush. JSONL (one call per line, written as completed) avoids the rewrite cost. The reader can wrap lines with `{"calls": [...]}` for compatibility.

### Layer 3: Catch SIGTERM / panic and flush

Install a signal handler / panic hook in `RecordingAgent` (or the parent) that flushes pending data on shutdown. Best paired with Layer 1 to bound how much can be lost between the SIGTERM and the handler running.

## Acceptance

- Kill the avp subprocess mid-Stop-hook (e.g. SIGKILL after 30s during a deliberately-slow run). Verify the recording file exists on disk and contains the calls/notifications captured up to ~5 seconds before the kill.
- Specifically test: the 16:06-style scenario (model deadlocks on rule N of 11), recording must contain calls for rules 1..N-1 fully and rule N at least partially.
- The existing recording schema continues to work (`json.load()` returns the same structure); JSONL or whatever format is fine internally as long as the reader API is unchanged.
- Add a unit test in `agent_client_protocol_extras` that creates a `RecordingAgent`, drives a few calls into it, calls something that simulates abnormal termination (`mem::forget` or explicit SIGTERM via subprocess), and asserts the on-disk file is still readable.

## Pairs with

- `01KQ7FWFR4V364AYF29DGGBZ87` — \"always record validator sessions, no env-var gate\". That task established that recording always happens. This task ensures the recording is durable.
- `01KQAFFZDX40GSKXQVS0MTNDWV` — Stop-hook deadlock. Without durable recordings, that task's investigation hits a dead-end every time.

#avp #observability #recording

## Review Findings (2026-04-28 21:15)

### Nits
- [x] `agent-client-protocol-extras/src/recording.rs:281-284` — On `tmp.write_all(bytes)?` or `tmp.sync_all()?` failure, the temp file (`.recording.json.tmp`) is left behind. Subsequent successful flushes overwrite the deterministic temp name, so this is self-healing — but a `let _ = std::fs::remove_file(&tmp_path);` in the error path would be tidier and prevent stale orphans if the process exits before the next flush.
- [x] `agent-client-protocol-extras/src/recording.rs:286` — After `std::fs::rename`, the parent directory is not fsync'd. POSIX-strict durability would open the parent and `sync_all()` it so the directory entry is on stable storage too. For SIGKILL (the actual failure mode in the task) the rename is already atomic at the kernel level, so this is a kernel-crash-only concern. Acceptable to leave as-is given the diagnostic use case.
- [x] `agent-client-protocol-extras/src/recording.rs:198-220` — `save()` and `atomic_write()` both call `create_dir_all` on the parent. The one in `save()` (lines 204-206) is now redundant since `atomic_write` (lines 265-267) does the same check. Minor duplication; remove the one in `save()` for clarity.
- [x] `agent-client-protocol-extras/src/recording.rs:289-305` — `Drop` still does a 2-second `thread::sleep` to settle the capture-thread tail. With per-prompt flush already in place, this final sleep is only ever needed for the *last* prompt's notifications. Worth a comment cross-referencing the per-prompt flush so future readers don't think it's redundant — though the existing comment already does this reasonably (lines 291-294). Borderline; could close as no-op.
- [x] `agent-client-protocol-extras/src/recording.rs:183-196` — Function is named `record_with_notifications` but does NOT capture notifications (it sets `notifications: Vec::new()` and relies on Drop/flush distribution). The doc comment explains this clearly, but the name is misleading. Consider `record_call` with the doc comment unchanged.
