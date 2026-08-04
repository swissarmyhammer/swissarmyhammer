//! Enforces that the builtin `shell` skill tells the agent three facts about
//! `execute command`:
//!
//! 1. The op blocks until the command exits (or the timeout kills it).
//! 2. Do not pipe to `tail`, `head`, or `grep` — read the stored output later
//!    with `get lines` or `grep history`.
//! 3. A command the timeout kills stores no output at all.
//!
//! The shell tool keeps every command's full output. An agent that writes
//! `cmd 2>&1 | tail -60` throws away output the tool already holds. The old
//! skill text carried only a weak bullet ("skip `| tail` / `| grep`
//! pipelines"), which did not stop that habit.
//!
//! Fact 3 is the limit of fact 2. `finalize_timed_out` only marks the command;
//! `store_command_output` runs solely in `finalize_completed`. Guidance that
//! promises stored output without naming that limit sends the agent to
//! `get lines` for output that was never written.
//!
//! Failing this test means the skill body drifted back to guidance that does
//! not state all three facts.

mod common;
use common::rendered_builtin_instructions;

/// States that `execute command` runs to completion before it answers.
const BLOCKING_MARKER: &str = "blocks until the command exits";

/// States the no-pipe rule.
const NO_PIPE_MARKER: &str = "Do not pipe to `tail`";

/// States that a timed-out command keeps nothing. `finalize_timed_out` marks
/// the command and returns; only `finalize_completed` calls
/// `store_command_output`. Text that promises stored output without this
/// qualifier sends the agent to `get lines` for output that does not exist.
const TIMEOUT_MARKER: &str = "no output is stored";

/// The weak bullet this guidance replaced.
const OLD_WEAK_BULLET: &str = "skip `| tail`";

#[test]
fn shell_output_guidance_states_blocking_and_no_tail() {
    let body = rendered_builtin_instructions("shell");
    assert!(
        body.contains(BLOCKING_MARKER),
        "builtin skill 'shell' must state that execute command \
         {BLOCKING_MARKER}"
    );
    assert!(
        body.contains(NO_PIPE_MARKER),
        "builtin skill 'shell' must state the no-pipe rule ('{NO_PIPE_MARKER}')"
    );
    assert!(
        body.contains("get lines"),
        "builtin skill 'shell' must name `get lines` as the way to read output"
    );
    assert!(
        body.contains("grep history"),
        "builtin skill 'shell' must name `grep history` as the way to read output"
    );
    assert!(
        body.contains(TIMEOUT_MARKER),
        "builtin skill 'shell' must state that a timed-out command keeps \
         nothing ('{TIMEOUT_MARKER}'), or the agent reads `get lines` for \
         output that does not exist"
    );
    assert!(
        !body.contains(OLD_WEAK_BULLET),
        "builtin skill 'shell' must drop the weak bullet '{OLD_WEAK_BULLET}'"
    );
}
