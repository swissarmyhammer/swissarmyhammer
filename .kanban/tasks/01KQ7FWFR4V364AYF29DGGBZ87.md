---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff8180
title: Always record validator agent sessions — drop AVP_RECORD_VALIDATORS env-var gate
---
**Drop the `AVP_RECORD_VALIDATORS` env-var gate.** Recording is always on, period. Validators always produce a transcript on disk under `.avp/recordings/`. No flag, no opt-in, no off switch.

The recording-agent infrastructure landed in `01KQ369KBDK6Y5DRN53WB7FDXQ` but was gated behind two env vars: `AVP_RECORD_VALIDATORS` (boolean enable) and `AVP_RECORD_DIR` (override location). For the same reason the SAH_HTTP_PORT pathway is being collapsed (`01KQ35MHFJQPMEKQ08PZKBKFY0`) — this is config theater for a single right answer. We always want to record. So always record.

## Why always-on is right

- **The transcripts are how we debug.** This morning's qwen failure was diagnosed entirely from `.avp/log` plus reasoning about what should have been in the prompt. Recordings are stronger evidence — verbatim request/response/notifications per validator session, machine-replayable. Asking the user to remember to set `AVP_RECORD_VALIDATORS=1` *before* hitting a bug is asking them to predict the future.
- **The transcripts are how we test.** Per the original task, recordings double as `PlaybackAgent` fixtures. A recording that doesn't exist isn't a fixture.
- **Validator runs are bounded and infrequent.** A recording is one JSON file per `AvpContext` lifetime — small, append-only during the run. The on-disk cost is negligible compared to the value.
- **Privacy isn't a real concern at this layer.** The recordings are local-only, gitignored, and only contain what the validators saw — which is the user's own diff and the rule prompts, all of which the user can see in `.avp/log` anyway.

## What to change

### 1. Remove the env-var gate in `avp-common/src/context.rs`

Today, `maybe_wrap_with_recording` (per the `01KQ369KBDK6Y5DRN53WB7FDXQ` implementation) only wraps when `AVP_RECORD_VALIDATORS` is set. Drop the env-var check. Wrapping is unconditional. Rename the function `wrap_with_recording` since "maybe" is misleading.

The `recording_path` function similarly drops its `AVP_RECORD_DIR` override. Recordings always go to `${project_root}/.avp/recordings/{session_id}-{unix_micros}.json`. If a user wants a different location for some reason they can move the directory after the fact; it's not worth the env-var.

### 2. Remove `AVP_RECORD_VALIDATORS` and `AVP_RECORD_DIR` references throughout

`grep` confirms they should be unused after this change. Any unit tests that flip the env vars on/off should be rewritten to test recording behavior directly (e.g., create a context, drop it, assert a recording file exists).

### 3. Keep the `AVP_SESSION_ID` env-var fallback (just in case) but don't make it the primary path

The reviewer-resolution work on `01KQ369KBDK6Y5DRN53WB7FDXQ` already established that the preferred path is `ctx.set_session_id(...)` and `AVP_SESSION_ID` is the env-var fallback. That stays as-is. This task only kills the *recording-enable* gate, not the session-id-resolution mechanism.

### 4. Update the module-level doc comment

`avp-common/src/context.rs` opens with a description of the recording mechanism's env-var configuration. Replace with: "Validator agent sessions are always recorded under `.avp/recordings/`; transcripts double as audit trails and as `PlaybackAgent` fixtures for the integration tests."

### 5. Update the `.avp/recordings/README.md` (if one exists) and any other docs

Drop references to `AVP_RECORD_VALIDATORS=1` from setup/regen instructions. The recordings just appear when avp runs.

## Acceptance

- A Stop-hook validation run produces a JSON file at `${project_root}/.avp/recordings/{session_id}-{unix_micros}.json` with no env vars set.
- `grep -r 'AVP_RECORD_VALIDATORS\|AVP_RECORD_DIR' avp-common avp-cli` returns zero matches.
- Existing recording-replay tests continue to pass (they should — they exercise the recording behavior, not the env-var gate).
- A new unit test asserts that constructing and dropping an `AvpContext` produces at least one recording file under the project's `.avp/recordings/` directory, with no env-var manipulation.
- `cargo test -p avp-common` and `cargo clippy -p avp-common --all-targets -- -D warnings` are clean.

## Pairs with

- `01KQ35MHFJQPMEKQ08PZKBKFY0` (always-on validator MCP server). Same pattern: drop the conditional, always do it. After both land, avp's startup is single-path: `init` → start MCP server, wrap agent in recording → done.

## Out of scope

- Trimming what's recorded. The full request/response/notifications stream stays. Don't optimize storage until the tests start choking on file size, which they won't.
- Compressing recordings. Same reason. JSON is fine.
- Rotating recordings on disk. They're per-session-id and the user can `rm -rf .avp/recordings/` whenever they want. Auto-cleanup is a future concern. #avp

## Review Findings (2026-04-27 09:05)

### Nits
- [x] `.avp/.gitignore:17` — Stale comment: `# Validator agent recordings (opt-in via AVP_RECORD_VALIDATORS=1; large)`. The on-disk file in the working tree contradicts the source-of-truth generator at `swissarmyhammer-directory/src/config.rs:126`, which was correctly updated to `# Validator agent recordings (one JSON file per AvpContext lifetime)`. The task's step 5 ("Drop references to `AVP_RECORD_VALIDATORS=1` from setup/regen instructions") covers this — bring the on-disk gitignore in line with the generator (either delete `.avp/.gitignore` and let it regenerate, or hand-edit the comment to match `swissarmyhammer-directory/src/config.rs:126`).