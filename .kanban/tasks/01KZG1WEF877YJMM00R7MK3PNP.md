---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9b80
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