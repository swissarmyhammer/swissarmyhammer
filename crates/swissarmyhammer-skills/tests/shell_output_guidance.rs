//! Enforces that the builtin `shell` skill tells the agent five facts about
//! `execute command`:
//!
//! 1. The op blocks until the command exits (or the timeout kills it).
//! 2. Do not pipe to `tail`, `head`, or `grep` — read the stored output later
//!    with `get lines` or `grep history`.
//! 3. A command the timeout kills stores no output at all.
//! 4. This shell does not search files. The file search tools do, and `rg` is
//!    the fallback.
//! 5. This shell does not edit files. The file editing tools do.
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
//! Fact 4 is the gap facts 2 and 3 left open. The no-pipe rule is about
//! discarding captured output, so it says nothing about which tool searches
//! files. An agent kept the letter of it, ran
//! `grep -rn ... --include=* . | grep -v ./target/`, and held a core for 22
//! minutes: `grep -r` ignores `.gitignore` and read a 64 GB `target/`, while
//! the output-side filter could not drop a line until the scan had already
//! paid to read it.
//!
//! Fact 5 is the same gap on the write side. Facts 2 through 4 are all about
//! reading, so they leave the agent free to reach for `sed -i`, `>`, or a
//! heredoc to change a file. Agents did exactly that for single-file edits.
//!
//! Failing this test means the skill body drifted back to guidance that does
//! not state all five facts.

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

/// Sends file search off the shell. The no-pipe rule above is about discarding
/// captured output, so it does not say which tool searches files. An agent
/// obeyed the letter of it and still ran `grep -rn ... --include=* .`, which
/// held a core for 22 minutes because `grep -r` reads `.gitignore`d build
/// directories.
const NO_GREP_SEARCH_MARKER: &str = "Do not use grep to search files";

/// Names the shell fallback. `rg` honors `.gitignore`; `grep -r` does not.
const RG_MARKER: &str = "use `rg`";

/// Sends file edits off the shell. The rules above are all about reading, so
/// none of them stop an agent from reaching for `sed -i` or `>` to change a
/// file. Agents did that for single-file edits.
const NO_SHELL_EDIT_MARKER: &str = "Do not use shell to edit files";

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
    assert!(
        body.contains(NO_GREP_SEARCH_MARKER),
        "builtin skill 'shell' must send file search off the shell \
         ('{NO_GREP_SEARCH_MARKER}'); the no-pipe rule alone does not say \
         which tool searches files"
    );
    assert!(
        body.contains(RG_MARKER),
        "builtin skill 'shell' must state the shell fallback \
         ('{RG_MARKER}'); `grep -r` scans `.gitignore`d build directories and \
         does not finish"
    );
    assert!(
        body.contains(NO_SHELL_EDIT_MARKER),
        "builtin skill 'shell' must send file edits off the shell \
         ('{NO_SHELL_EDIT_MARKER}'); the read-side rules above say nothing \
         about which tool changes a file"
    );
}
