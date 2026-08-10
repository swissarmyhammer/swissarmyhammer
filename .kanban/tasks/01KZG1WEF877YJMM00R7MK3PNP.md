---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzp820vensbg35stv1vk412g
  text: |-
    Card picked up. Red output captured — the race is real and it reproduces.

    ## How the failure was made to happen

    The card records two shapes that did not reproduce it. This one did:

    - 48 endless CPU loops on an 18-core machine (about 2.7 loops for each core).
    - `cargo nextest run -p claude-agent --lib a_lagged_collector_is_an_error_not_a_reply_with_holes`, 200 times in sequence.

    Result: **195 passed, 5 failed**. Failing iterations were spread through the run; iteration 169 is one of them.

    The earlier attempts used about 200 hogs. That is so much load that the whole process, sender included, is starved equally. Roughly 2.7 hogs for each core starves the two worker threads unequally, which is what lets the collector thread run between two sends.

    ## The red output, word for word

    ```
    thread 'collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes' (64272919) panicked at crates/claude-agent/src/lib.rs:844:14:
    a lagged collector cannot prove the reply is whole: "one two three four "
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
    ```

    The `Ok` value is the whole reply — `"one two three four "`, all four chunks, no hole. That proves the diagnosis on the card: the collector was scheduled between the sends, drained the ring, never lagged, and `collect_response_content` correctly returned `Ok`. The test, not the code, is wrong.

    ## A second defect the reproduction showed

    The assertion `error.to_string().contains("dropped")` does not name the `Lagged` path alone. The `Backstop` message also holds the word "dropped" — "({skipped} notifications were dropped by lag)". So the test can pass through the wrong failure path. The word `broadcast dropped` belongs to `DrainFailure::Lagged` only.
  timestamp: 2026-08-10T16:28:25.838053+00:00
- actor: claude-code
  id: 01kzp91j43j7y9fcde3zqzv9pb
  text: |-
    Fix landed. One file: `crates/claude-agent/src/lib.rs`. No production code changed.

    ## Which option was taken, and why

    **Option 1 — hold the collector off until every send is done.** The test now subscribes the receiver BEFORE the first send and calls `spawn_notification_collector` only AFTER the last one.

    The receiver joins the broadcast at the first notification and then never moves, because no task is reading it. Five notifications go into a ring of two slots, so the ring overwrites the receiver's place. The collector's first `recv()` can answer nothing but `Lagged`. The scheduler cannot change that answer, whatever order it runs the threads in.

    The other two options were weaker:

    - **Drive the broadcast directly.** Asserting `RecvError::Lagged` on a bare receiver tests tokio, not this crate. It leaves `collect_response_content` and the collector task out of the test, and those are the code the card says the test must exercise.
    - **`tokio::time::pause()`.** The race is between two threads, not between a task and a timer. No send here waits on the clock, so a paused clock orders nothing. It would leave the same race in place.

    No retry was added. The card forbids one, and the test does not need one: the drop is now made by construction.

    ## The `Lagged` path is still exercised — proven, not claimed

    `if skipped > 0` in `collect_response_content` was temporarily changed to `if false && skipped > 0` and the test run:

    ```
    thread 'collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes' panicked at crates/claude-agent/src/lib.rs:855:14:
    a lagged collector cannot prove the reply is whole: "four "
    ```

    Two facts come out of that one line:

    1. The test reaches the `Lagged` branch. Take the branch away and the test fails.
    2. The reply the drain would return is `"four "` — three chunks gone. That is the reply with holes the test is named for, now made every run instead of by accident.

    The production line was put back immediately.

    ## The assertion was tightened

    `contains("dropped")` also matches the `Backstop` message, so it could pass through the wrong failure path. It now reads `contains("the notification broadcast dropped")`, which only `DrainFailure::Lagged` writes.

    ## Repeat runs

    | Shape | Runs | Passed | Failed |
    |---|---|---|---|
    | Before the fix, 48 CPU hogs | 200 | 195 | 5 |
    | After the fix, 48 CPU hogs | 300 | 300 | 0 |
    | After the fix, no load | 200 | 200 | 0 |

    500 runs after the fix, 100 percent pass, under the same load that made the old test fail.

    ## Full checks

    - `cargo fmt --all -- --check` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo nextest run --workspace` — 14027 tests, 14023 passed, 4 failed. The 4 are the known failures on ^bh5ncd0: three `review_e2e` tests and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`. `the_swift_package_root_restores_the_directory_before_it_removes_it` passed in this run.
  timestamp: 2026-08-10T16:45:39.331724+00:00
- actor: claude-code
  id: 01kzp91ssc3dmw06aj8k0x18qr
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/claude-agent/src/lib.rs. Red output captured: `a lagged collector cannot prove the reply is whole: "one two three four "` at lib.rs:844, 5 failures in 200 runs under 48 CPU hogs. After the fix: 300/300 under the same load and 200/200 with no load, 500 runs at 100 percent. Commands: `cargo nextest run -p claude-agent --lib a_lagged_collector_is_an_error_not_a_reply_with_holes` (x500), `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` (14027 tests, 14023 passed, 4 known failures on ^bh5ncd0).
    - next: /review
  timestamp: 2026-08-10T16:45:47.180806+00:00
position_column: doing
position_ordinal: '8480'
title: claude-agent lagged-collector test races the scheduler under full-workspace parallelism
---
`claude-agent collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes` failed once in a `cargo nextest run --workspace` run of 13783 tests. It passes alone, and it passed 10 of 10 full `-p claude-agent --lib` runs afterwards.

The test lives at `crates/claude-agent/src/lib.rs`, function `a_lagged_collector_is_an_error_not_a_reply_with_holes`. It is pre-existing. The mirdan change on ^qh5fnpd does not touch the `claude-agent` crate.

## What the test does

It builds a `NotificationSender` with `LAGGING_NOTIFICATION_RING = 2`, spawns the collector on its own task, then awaits five sends (four chunks plus the end-of-turn marker). It then asserts `collect_response_content` returns an error whose text contains `dropped`.

## Why it can fail

The test needs the broadcast ring to overflow. Overflow happens only when the sender gets ahead of the collector task. Nothing forces that order:

- The test runs on `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`, so the collector has its own worker thread.
- Every `send_chunk` is awaited, which yields.
- If the collector task is scheduled between two sends, it consumes the message, the ring never fills, no `Lagged` is raised, and `expect_err` panics.

So the test asserts on a scheduling accident, not on a guaranteed condition. The name promises "a lagged collector"; the body only makes lag likely.

## What was tried and did NOT reproduce it

- `cargo nextest run -p claude-agent --lib -j 24`, ten runs: 764 passed each time.
- The single test under about 200 spinning CPU hogs, load average about 24, five runs: all passed.

Neither shape reproduced it, so the exact trigger is still unproven. The original failure text was not captured, because the failing run only kept the summary line.

## Fix direction

Make the lag deterministic rather than probable. Options to weigh:

- Hold the collector off until every send is done — subscribe, send, and only then let the collector task start reading, so the drop is forced by construction.
- Drive the broadcast directly and assert the `RecvError::Lagged` handling in `collect_response_content` without a live task race.
- Use `tokio::time::pause()` so the test owns the clock instead of the scheduler.

Do not answer with a retry. A retry hides the race the test exists to describe.

## First step

Capture the red output. Run the full `--workspace` suite with the failure output retained, or run the test in a loop with an artificially slowed collector, and record the assertion text word for word before changing the test.
#tool-validators