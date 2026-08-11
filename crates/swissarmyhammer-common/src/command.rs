//! Running a command string through a shell, and describing one that failed.
//!
//! Two decisions live here once. Which interpreter runs a command string and
//! how the child's streams are wired is [`shell_command`]; how a nonzero exit
//! reads back to a person is [`command_failure_detail`]. Every caller that
//! spawns a shell — the review engine's tool-rule runner, the `shell` MCP
//! tool, the doctor's `--version` probes — goes through them, so two surfaces
//! can never run the same command two ways or describe the same failure two
//! ways.

use std::process::{Command, Output, Stdio};

/// The interpreter a command string runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    /// The platform's own interpreter — `cmd /C` on Windows, `sh -c`
    /// everywhere else.
    ///
    /// The choice for a command a user or a caller supplied: it is written in
    /// whatever the machine it runs on speaks.
    Platform,
    /// `bash -c`, on every platform.
    ///
    /// The choice for a script this repository ships: it is written once, in
    /// bash, and has to behave the same wherever it runs.
    Bash,
}

impl Shell {
    /// The program to spawn, and the flag that introduces the script.
    fn program_and_flag(self) -> (&'static str, &'static str) {
        match self {
            Shell::Platform if cfg!(target_os = "windows") => ("cmd", "/C"),
            Shell::Platform => ("sh", "-c"),
            Shell::Bash => ("bash", "-c"),
        }
    }
}

/// Build a [`Command`] that runs `script` through `shell`.
///
/// Stdin is closed and both output streams are captured, so the caller can
/// read the output back and no script can block waiting on a terminal that is
/// not there.
///
/// The caller adds what is its own: the working directory, the environment,
/// and any positional parameters the script reads as `"$@"`.
pub fn shell_command(shell: Shell, script: &str) -> Command {
    let (program, flag) = shell.program_and_flag();
    let mut command = Command::new(program);
    command
        .arg(flag)
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Summarize a command that failed: its stderr when it wrote any, its exit
/// status otherwise.
///
/// A tool that fails says why on stderr, and that sentence is what a reader
/// needs. A tool that fails silently leaves only its status, so the status is
/// the answer rather than an empty string.
pub fn command_failure_detail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("exited with {}", output.status)
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read a command's program and arguments back as plain strings.
    fn spelling(command: &Command) -> (String, Vec<String>) {
        (
            command.get_program().to_string_lossy().to_string(),
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect(),
        )
    }

    #[test]
    fn bash_runs_the_script_through_bash_on_every_platform() {
        let command = shell_command(Shell::Bash, "echo hello");

        assert_eq!(
            spelling(&command),
            (
                "bash".to_string(),
                vec!["-c".to_string(), "echo hello".to_string()]
            )
        );
    }

    #[test]
    fn the_platform_shell_is_the_one_this_platform_speaks() {
        let command = shell_command(Shell::Platform, "echo hello");

        let expected = if cfg!(target_os = "windows") {
            (
                "cmd".to_string(),
                vec!["/C".to_string(), "echo hello".to_string()],
            )
        } else {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "echo hello".to_string()],
            )
        };
        assert_eq!(spelling(&command), expected);
    }

    #[test]
    fn the_child_cannot_read_a_terminal_and_its_output_is_captured() {
        let output = shell_command(Shell::Bash, "cat; echo out; echo err >&2")
            .output()
            .expect("bash runs");

        assert!(
            output.status.success(),
            "a closed stdin ends `cat`, it does not hang it"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "out\n");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "err\n");
    }

    #[test]
    fn a_failure_reads_back_as_its_stderr() {
        let output = shell_command(Shell::Bash, "echo 'no such option' >&2; exit 2")
            .output()
            .expect("bash runs");

        assert_eq!(command_failure_detail(&output), "no such option");
    }

    #[test]
    fn a_silent_failure_reads_back_as_its_status() {
        let output = shell_command(Shell::Bash, "exit 3")
            .output()
            .expect("bash runs");

        assert_eq!(
            command_failure_detail(&output),
            format!("exited with {}", output.status)
        );
    }
}
