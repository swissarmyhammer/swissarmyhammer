//! Shell state management for the virtual shell
//!
//! Maintains command history, output log, and process handles.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, Local};
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};

use swissarmyhammer_directory::{DirectoryConfig, ShellConfig};

/// Number of matches [`ShellState::grep`] returns when the caller names no
/// limit. The reported total match count is never capped.
pub const DEFAULT_GREP_LIMIT: usize = 10;

/// Line number [`ShellState::get_lines`] reads from when the caller names no
/// start. Stored output lines are numbered from 1.
pub const DEFAULT_START_LINE: usize = 1;

/// Number of `:`-separated fields one log entry carries after its session-id
/// prefix. [`ShellState::append_lines`] writes
/// `session_id:cmd_id:line_number:text`, so stripping the session id leaves
/// the command id, the line number, and the text — and the text itself may
/// hold any number of further colons.
const LOG_FIELD_COUNT_AFTER_SESSION_ID: usize = 3;

/// Failure of a [`ShellState`] operation.
///
/// Each variant names one failure a caller can act on, so a caller matches on
/// the cause rather than reading a message. Every variant that wraps an
/// underlying failure keeps it as its `source`, so the chain stays whole.
#[derive(Debug, thiserror::Error)]
pub enum ShellStateError {
    /// The shell directory could not be created.
    #[error("shell directory {path} could not be created: {source}")]
    CreateDir {
        /// The directory the creation targeted.
        path: PathBuf,
        /// The io error the creation returned.
        source: std::io::Error,
    },
    /// A file of the shell directory could not be opened.
    #[error("shell file {path} could not be opened: {source}")]
    OpenFile {
        /// The file path the open targeted.
        path: PathBuf,
        /// The io error the open returned.
        source: std::io::Error,
    },
    /// A file of the shell directory could not be written.
    #[error("shell file {path} could not be written: {source}")]
    WriteFile {
        /// The file path the write targeted.
        path: PathBuf,
        /// The io error the write returned.
        source: std::io::Error,
    },
    /// The output log could not be read.
    #[error("shell log {path} could not be read: {source}")]
    ReadLog {
        /// The log path the read targeted.
        path: PathBuf,
        /// The io error the read returned.
        source: std::io::Error,
    },
    /// No command record carries the id the caller named.
    #[error("unknown command ID {cmd_id}")]
    UnknownCommand {
        /// The command id the caller named.
        cmd_id: usize,
    },
    /// No process is registered for the id the caller named, so there is
    /// nothing to signal.
    #[error("no running process for command ID {cmd_id}")]
    NoRunningProcess {
        /// The command id the caller named.
        cmd_id: usize,
    },
    /// A process was registered for the id, but no command record stands
    /// beside it.
    #[error("command record not found for ID {cmd_id}")]
    MissingRecord {
        /// The command id the caller named.
        cmd_id: usize,
    },
    /// The caller's search pattern is not a valid regular expression.
    #[error("invalid regex pattern: {source}")]
    InvalidPattern {
        /// The error the regex compiler returned.
        source: grep::regex::Error,
    },
}

/// Command execution status
#[derive(Debug, Clone, PartialEq)]
pub enum CommandStatus {
    /// The record exists and has not reached a terminal state. A command whose
    /// spawn failed also stays here, because nothing marks that record.
    Running,
    /// The command reached its end without a kill and without a timeout.
    /// `execute command` also lands here when the run failed inside the shell,
    /// with exit code -1.
    Completed,
    /// `kill process` sent SIGKILL to the command's process group. This state
    /// is transient while `execute command` still owns the child: that task
    /// reaps the signalled child and then writes [`CommandStatus::Completed`]
    /// with exit code -1 over it. The status stays `Killed` only when nothing
    /// is left to reap the child, as after the request is cancelled.
    Killed,
    /// The command's timeout elapsed, so the process guard killed it.
    TimedOut,
}

impl fmt::Display for CommandStatus {
    /// Writes the wire name of the status — `running`, `completed`, `killed`,
    /// or `timed_out`. Responses read these names through this impl, so it is
    /// the single source of truth for the status text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandStatus::Running => write!(f, "running"),
            CommandStatus::Completed => write!(f, "completed"),
            CommandStatus::Killed => write!(f, "killed"),
            CommandStatus::TimedOut => write!(f, "timed_out"),
        }
    }
}

/// Metadata for a single command execution
#[derive(Debug, Clone)]
pub struct CommandRecord {
    /// Id the caller passes back to `get lines`, `grep history`, and
    /// `kill process`. Ids start at 1 and count up within one session.
    pub id: usize,
    /// The command line as the caller wrote it.
    pub command: String,
    /// Where the command stands: running, or how it reached its end.
    pub status: CommandStatus,
    /// Exit code of the child process, or `None` when none was recorded — a
    /// command that still runs, and a command `kill process` just signalled,
    /// both hold `None`. `Some(-1)` means no exit code was reported: a signal
    /// killed the child, the timeout fired, or the run failed inside the
    /// shell.
    pub exit_code: Option<i32>,
    /// Number of output lines stored in the log for this command.
    pub line_count: usize,
    /// Monotonic start instant, used to measure elapsed time.
    pub started_at: Instant,
    /// Wall-clock start time, used to report the time of day to the caller.
    pub started_at_wall: DateTime<Local>,
    /// Monotonic instant the command reached a terminal state, or `None` while
    /// it still runs. Every terminal state sets it, so it pairs with
    /// [`CommandRecord::duration`].
    pub completed_at: Option<Instant>,
    /// Wall-clock time the command reached a terminal state, or `None` while
    /// it still runs.
    pub completed_at_wall: Option<DateTime<Local>>,
}

impl CommandRecord {
    /// Returns the elapsed time from the command's start. A command that
    /// reached a terminal state reports the span between its start and that
    /// end; a command that still runs reports the time since its start.
    pub fn duration(&self) -> std::time::Duration {
        match self.completed_at {
            Some(end) => end.duration_since(self.started_at),
            None => self.started_at.elapsed(),
        }
    }
}

/// The virtual shell state — singleton per server process
#[derive(Debug)]
pub struct ShellState {
    /// Unique id of this shell session. Every log entry carries it, so one log
    /// file can hold the output of more than one session.
    pub session_id: String,
    commands: Vec<CommandRecord>,
    processes: HashMap<usize, u32>, // cmd_id -> PID
    log_path: PathBuf,
}

impl ShellState {
    /// Create a new ShellState, initializing the .shell/ directory and log file.
    ///
    /// Prefers `.shell/` under the current directory so a server launched in a
    /// project keeps its shell history alongside that project. Falls back to a
    /// unique temp directory when the CWD is unavailable *or not writable* —
    /// resolving to an absolute path at creation time so stored paths stay
    /// valid even if the process CWD changes later.
    ///
    /// The not-writable fallback matters for GUI launches: a bundled macOS app
    /// opened from Finder runs with CWD = `/`, which is a read-only system
    /// volume, so `create_dir_all("/.shell")` fails with EROFS. Falling back
    /// here keeps the app running, and
    /// [`ShellExecuteTool::new`](super::ShellExecuteTool::new) reports what is
    /// left over as an error rather than a panic.
    ///
    /// # Errors
    ///
    /// Reports [`ShellStateError::CreateDir`], [`ShellStateError::OpenFile`]
    /// or [`ShellStateError::WriteFile`] when the temp fallback fails as well.
    pub fn new() -> Result<Self, ShellStateError> {
        let preferred = std::env::current_dir()
            .ok()
            .map(|cwd| cwd.join(ShellConfig::DIR_NAME));
        Self::new_with_preferred(preferred)
    }

    /// Build a `ShellState`, preferring `preferred` (e.g. `<cwd>/.shell`) but
    /// falling back to a unique temp directory when it is `None` or cannot be
    /// created (missing, read-only, or otherwise unwritable).
    fn new_with_preferred(preferred: Option<impl AsRef<Path>>) -> Result<Self, ShellStateError> {
        if let Some(dir) = preferred {
            let dir = dir.as_ref();
            match Self::with_dir(dir) {
                Ok(state) => return Ok(state),
                Err(error) => tracing::warn!(
                    %error,
                    dir = %dir.display(),
                    "shell state: preferred .shell directory is not writable; \
                     falling back to a temp directory"
                ),
            }
        }
        Self::with_dir(std::env::temp_dir().join(format!(
            "{}-{}",
            ShellConfig::DIR_NAME,
            ulid::Ulid::new()
        )))
    }

    /// Create a new ShellState with an explicit base directory for the .shell/ data.
    /// This avoids relying on the process-wide CWD, which is important for tests.
    ///
    /// # Errors
    ///
    /// Reports the same failures [`ShellState::with_dir`] reports.
    pub fn new_in_dir(shell_dir: impl AsRef<Path>) -> Result<Self, ShellStateError> {
        Self::with_dir(shell_dir)
    }

    /// Create a new ShellState rooted at the given directory.
    ///
    /// # Errors
    ///
    /// Reports [`ShellStateError::CreateDir`] when the directory cannot be
    /// created, [`ShellStateError::WriteFile`] when its `.gitignore` cannot be
    /// written, and [`ShellStateError::OpenFile`] when the log file cannot be
    /// opened.
    pub fn with_dir(shell_dir: impl AsRef<Path>) -> Result<Self, ShellStateError> {
        let shell_dir = shell_dir.as_ref();
        let session_id = ulid::Ulid::new().to_string();
        fs::create_dir_all(shell_dir).map_err(|source| ShellStateError::CreateDir {
            path: shell_dir.to_path_buf(),
            source,
        })?;

        // Write .gitignore if it doesn't exist yet
        let gitignore_path = shell_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, ShellConfig::GITIGNORE_CONTENT).map_err(|source| {
                ShellStateError::WriteFile {
                    path: gitignore_path.clone(),
                    source,
                }
            })?;
        }

        let log_path = shell_dir.join("log");
        // Touch the log file
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|source| ShellStateError::OpenFile {
                path: log_path.clone(),
                source,
            })?;

        Ok(Self {
            session_id,
            commands: Vec::new(),
            processes: HashMap::new(),
            log_path,
        })
    }

    /// Start tracking a new command. Returns the assigned command ID.
    pub fn start_command(&mut self, command: impl Into<String>) -> usize {
        let id = self.commands.len() + 1;
        let now = Instant::now();
        self.commands.push(CommandRecord {
            id,
            command: command.into(),
            status: CommandStatus::Running,
            exit_code: None,
            line_count: 0,
            started_at: now,
            started_at_wall: Local::now(),
            completed_at: None,
            completed_at_wall: None,
        });
        id
    }

    /// Register a running process PID for a command.
    pub fn register_process(&mut self, cmd_id: usize, pid: u32) {
        self.processes.insert(cmd_id, pid);
    }

    /// Append output lines from a command to the log.
    ///
    /// Note: This performs blocking file I/O (log file append). This is acceptable because
    /// the shell tool is single-user and log writes are small and fast. The outer async mutex
    /// is held during this call, but concurrent shell operations are not expected.
    ///
    /// # Errors
    ///
    /// Reports [`ShellStateError::UnknownCommand`] when no record carries
    /// `cmd_id`, [`ShellStateError::OpenFile`] when the log cannot be opened
    /// for appending, and [`ShellStateError::WriteFile`] when a line cannot be
    /// written.
    pub async fn append_lines(
        &mut self,
        cmd_id: usize,
        lines: &[impl AsRef<str>],
    ) -> Result<(), ShellStateError> {
        let log_path = &self.log_path;
        let session_id = &self.session_id;
        let record = self
            .commands
            .iter_mut()
            .find(|r| r.id == cmd_id)
            .ok_or(ShellStateError::UnknownCommand { cmd_id })?;

        let mut log_file = OpenOptions::new()
            .append(true)
            .open(log_path)
            .map_err(|source| ShellStateError::OpenFile {
                path: log_path.clone(),
                source,
            })?;

        for line in lines {
            record.line_count += 1;
            let log_line = format!(
                "{}:{}:{}:{}\n",
                session_id,
                cmd_id,
                record.line_count,
                line.as_ref()
            );
            log_file.write_all(log_line.as_bytes()).map_err(|source| {
                ShellStateError::WriteFile {
                    path: log_path.clone(),
                    source,
                }
            })?;
        }

        Ok(())
    }

    /// Mark a command as completed with exit code.
    pub async fn complete_command(&mut self, cmd_id: usize, exit_code: Option<i32>) {
        self.finish_command(cmd_id, CommandStatus::Completed, exit_code);
    }

    /// Mark a command as timed out. A timeout reports no exit code of its own,
    /// so the record carries the "no exit code" value [`CommandRecord`]
    /// documents.
    pub async fn timeout_command(&mut self, cmd_id: usize) {
        self.finish_command(cmd_id, CommandStatus::TimedOut, Some(-1));
    }

    /// Move one command record to a terminal state: drop the process
    /// registration, write the status and the exit code, and stamp both
    /// clocks. A `cmd_id` no record carries changes nothing, which is what
    /// every caller of this helper already reported.
    fn finish_command(&mut self, cmd_id: usize, status: CommandStatus, exit_code: Option<i32>) {
        self.processes.remove(&cmd_id);
        if let Some(record) = self.commands.iter_mut().find(|r| r.id == cmd_id) {
            record.status = status;
            record.exit_code = exit_code;
            record.completed_at = Some(Instant::now());
            record.completed_at_wall = Some(Local::now());
        }
    }

    /// Kill a running command by PID. Returns the command record if found.
    ///
    /// # Errors
    ///
    /// Reports [`ShellStateError::NoRunningProcess`] when no PID is registered
    /// for `cmd_id`, and [`ShellStateError::MissingRecord`] when a PID was
    /// registered but no command record stands beside it.
    pub async fn kill_process(&mut self, cmd_id: usize) -> Result<CommandRecord, ShellStateError> {
        let pid = self
            .processes
            .get(&cmd_id)
            .copied()
            .ok_or(ShellStateError::NoRunningProcess { cmd_id })?;

        // Send SIGKILL to the process group
        #[cfg(unix)]
        unsafe {
            libc::killpg(pid as i32, libc::SIGKILL);
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, try to kill by PID via command
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
        }

        self.processes.remove(&cmd_id);

        if let Some(record) = self.commands.iter_mut().find(|r| r.id == cmd_id) {
            record.status = CommandStatus::Killed;
            record.completed_at = Some(Instant::now());
            record.completed_at_wall = Some(Local::now());
            Ok(record.clone())
        } else {
            Err(ShellStateError::MissingRecord { cmd_id })
        }
    }

    /// List all command records.
    pub fn list_commands(&self) -> &[CommandRecord] {
        &self.commands
    }

    /// Get lines from a specific command's output by reading the log file.
    ///
    /// An absent `start` reads from [`DEFAULT_START_LINE`], and an absent `end`
    /// reads to the last stored line.
    ///
    /// Note: This performs blocking file I/O. Acceptable for single-user shell tool
    /// where log reads are fast and infrequent.
    ///
    /// # Errors
    ///
    /// Reports [`ShellStateError::OpenFile`] when the log cannot be opened and
    /// [`ShellStateError::ReadLog`] when a line cannot be read back.
    pub fn get_lines(
        &self,
        cmd_id: usize,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Result<Vec<(usize, String)>, ShellStateError> {
        let start = start.unwrap_or(DEFAULT_START_LINE);
        let end = end.unwrap_or(usize::MAX);
        let session_prefix = format!("{}:", self.session_id);

        let file =
            std::fs::File::open(&self.log_path).map_err(|source| ShellStateError::OpenFile {
                path: self.log_path.clone(),
                source,
            })?;
        let reader = BufReader::new(file);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|source| ShellStateError::ReadLog {
                path: self.log_path.clone(),
                source,
            })?;
            let Some(entry) = parse_log_entry(&line, &session_prefix) else {
                continue;
            };
            if entry.command_id != cmd_id {
                continue;
            }
            if entry.line_number > end {
                break;
            }
            if entry.line_number >= start {
                results.push((entry.line_number, entry.text.to_string()));
            }
        }

        Ok(results)
    }

    /// Grep command output history using regex pattern matching.
    ///
    /// Note: This performs blocking file I/O. Acceptable for single-user shell tool
    /// where grep is fast over local log files.
    /// Returns `(matching_results, total_match_count)`. Results are capped by `limit`
    /// (default [`DEFAULT_GREP_LIMIT`]) but `total_match_count` reflects all
    /// matches found.
    ///
    /// # Errors
    ///
    /// Reports [`ShellStateError::InvalidPattern`] when `pattern` is not a
    /// valid regular expression, and [`ShellStateError::ReadLog`] when the log
    /// cannot be searched.
    pub fn grep(
        &self,
        pattern: &str,
        command_id: Option<usize>,
        limit: Option<usize>,
    ) -> Result<(Vec<GrepResult>, usize), ShellStateError> {
        let limit = limit.unwrap_or(DEFAULT_GREP_LIMIT);
        let matcher = RegexMatcher::new_line_matcher(pattern)
            .map_err(|source| ShellStateError::InvalidPattern { source })?;

        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0))
            .line_number(true)
            .build();

        let mut results = Vec::new();
        let mut total_matches: usize = 0;
        let session_prefix = format!("{}:", self.session_id);

        searcher
            .search_path(
                &matcher,
                &self.log_path,
                UTF8(|_line_num, line| {
                    let Some(entry) = parse_log_entry(line, &session_prefix) else {
                        return Ok(true);
                    };
                    if command_id.is_some_and(|wanted| wanted != entry.command_id) {
                        return Ok(true);
                    }
                    total_matches += 1;
                    if results.len() < limit {
                        results.push(GrepResult {
                            command_id: entry.command_id,
                            line_number: entry.line_number,
                            text: entry.text.trim_end().to_string(),
                        });
                    }
                    Ok(true)
                }),
            )
            .map_err(|source| ShellStateError::ReadLog {
                path: self.log_path.clone(),
                source,
            })?;

        Ok((results, total_matches))
    }
}

/// One log entry, parsed and borrowing the line it came from.
struct LogEntry<'a> {
    /// Id of the command whose output holds the line.
    command_id: usize,
    /// Position of the line within that command's output.
    line_number: usize,
    /// The line text, exactly as it was stored.
    text: &'a str,
}

/// Parse one `session_id:cmd_id:line_number:text` entry
/// [`ShellState::append_lines`] wrote.
///
/// `get lines` and `grep history` read the same log, so both read it through
/// this one parser. Returns `None` when the line belongs to another session,
/// when it carries too few fields, or when either numeric field fails to
/// parse.
fn parse_log_entry<'a>(line: &'a str, session_prefix: &str) -> Option<LogEntry<'a>> {
    let rest = line.strip_prefix(session_prefix)?;
    let parts: Vec<&str> = rest.splitn(LOG_FIELD_COUNT_AFTER_SESSION_ID, ':').collect();
    if parts.len() != LOG_FIELD_COUNT_AFTER_SESSION_ID {
        return None;
    }
    Some(LogEntry {
        command_id: parts[0].parse().ok()?,
        line_number: parts[1].parse().ok()?,
        text: parts[2],
    })
}

/// Result from grep operation
#[derive(Debug, Clone)]
pub struct GrepResult {
    /// Id of the command whose output holds the matching line.
    pub command_id: usize,
    /// Position of the line within that command's output, counting from
    /// [`DEFAULT_START_LINE`].
    pub line_number: usize,
    /// Text of the matching line, with the trailing newline removed.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// Helper to create a `ShellState` inside a temporary directory.
    /// Returns the state and the temp dir (which must be kept alive for the duration
    /// of the test so the directory is not deleted).
    fn create_test_state() -> (ShellState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let shell_dir = tmp.path().join(".shell");
        let state = ShellState::with_dir(shell_dir).expect("ShellState::with_dir");
        (state, tmp)
    }

    #[test]
    fn test_command_status_display() {
        assert_eq!(CommandStatus::Running.to_string(), "running");
        assert_eq!(CommandStatus::Completed.to_string(), "completed");
        assert_eq!(CommandStatus::Killed.to_string(), "killed");
        assert_eq!(CommandStatus::TimedOut.to_string(), "timed_out");
    }

    /// Regression: `new_with_preferred` must fall back to a temp directory when
    /// the preferred `.shell` location cannot be created. A bundled macOS GUI
    /// app launched from Finder runs with CWD = `/` (a read-only system
    /// volume), so `create_dir_all("/.shell")` fails with EROFS. Before this
    /// fallback, that error reached `ShellExecuteTool::new()`, which panicked
    /// on it and aborted the whole app on launch (panic in
    /// `did_finish_launching`). That constructor now reports the error
    /// instead, and this fallback keeps it from arising at all.
    #[cfg(unix)]
    #[test]
    fn falls_back_to_temp_when_preferred_dir_is_read_only() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("temp dir");
        let read_only = tmp.path().join("read-only");
        std::fs::create_dir(&read_only).expect("create read-only dir");
        std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o555))
            .expect("chmod read-only");

        let preferred = read_only.join(".shell");
        let state = ShellState::new_with_preferred(Some(preferred))
            .expect("must not error: should fall back to a writable temp dir");

        // It fell back — the log path is NOT under the read-only directory...
        assert!(
            !state.log_path.starts_with(&read_only),
            "expected fallback away from read-only dir, got {}",
            state.log_path.display()
        );
        // ...and the fallback location is actually usable.
        assert!(state.log_path.exists(), "fallback log file should exist");

        // Restore perms so TempDir cleanup can remove the directory.
        let _ = std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o755));
    }

    // =================================================================
    // ShellState lifecycle: start_command, append_lines, complete_command
    // =================================================================

    #[tokio::test]
    #[serial]
    async fn test_start_command_returns_sequential_ids() {
        let (mut state, _tmp) = create_test_state();
        let id1 = state.start_command("echo hello");
        let id2 = state.start_command("echo world");
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_start_command_creates_running_record() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("ls -la");
        let commands = state.list_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, id);
        assert_eq!(commands[0].command, "ls -la");
        assert_eq!(commands[0].status, CommandStatus::Running);
        assert!(commands[0].exit_code.is_none());
        assert_eq!(commands[0].line_count, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_append_lines_increments_line_count() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo test");
        let lines = vec![
            "line1".to_string(),
            "line2".to_string(),
            "line3".to_string(),
        ];
        state.append_lines(id, &lines).await.expect("append_lines");
        let commands = state.list_commands();
        assert_eq!(commands[0].line_count, 3);
    }

    #[tokio::test]
    #[serial]
    async fn test_append_lines_unknown_command_returns_error() {
        let (mut state, _tmp) = create_test_state();
        let result = state.append_lines(999, &["nope".to_string()]).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("unknown command ID 999"),
            "Error: {err_msg}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_complete_command_sets_status_and_exit_code() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo done");
        state.complete_command(id, Some(0)).await;
        let commands = state.list_commands();
        assert_eq!(commands[0].status, CommandStatus::Completed);
        assert_eq!(commands[0].exit_code, Some(0));
        assert!(commands[0].completed_at.is_some());
        assert!(commands[0].completed_at_wall.is_some());
    }

    #[tokio::test]
    #[serial]
    async fn test_timeout_command_sets_status() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("sleep 999");
        state.timeout_command(id).await;
        let commands = state.list_commands();
        assert_eq!(commands[0].status, CommandStatus::TimedOut);
        assert_eq!(commands[0].exit_code, Some(-1));
        assert!(commands[0].completed_at.is_some());
    }

    #[tokio::test]
    #[serial]
    async fn test_command_record_duration() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("quick");
        // Duration before completion (still running) should be a valid duration
        let running_duration = state.list_commands()[0].duration();
        // Duration is always non-negative by construction; verify it's a reasonable value
        assert!(
            running_duration.as_secs() < 60,
            "running duration unreasonable"
        );

        state.complete_command(id, Some(0)).await;
        let completed_duration = state.list_commands()[0].duration();
        assert!(
            completed_duration.as_secs() < 60,
            "completed duration unreasonable"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_register_process() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("sleep 10");
        state.register_process(id, 12345);
        // Verify registration via internal state
        assert!(state.processes.contains_key(&id));
        assert_eq!(state.processes[&id], 12345);
    }

    // =================================================================
    // get_lines with start/end ranges
    // =================================================================

    #[tokio::test]
    #[serial]
    async fn test_get_lines_all() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo stuff");
        let lines: Vec<String> = (1..=5).map(|i| format!("line{i}")).collect();
        state.append_lines(id, &lines).await.unwrap();

        let result = state.get_lines(id, None, None).unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], (1, "line1".to_string()));
        assert_eq!(result[4], (5, "line5".to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn test_get_lines_with_start_and_end() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("seq");
        let lines: Vec<String> = (1..=10).map(|i| format!("data{i}")).collect();
        state.append_lines(id, &lines).await.unwrap();

        let result = state.get_lines(id, Some(3), Some(7)).unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], (3, "data3".to_string()));
        assert_eq!(result[4], (7, "data7".to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn test_get_lines_start_only() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("cmd");
        let lines: Vec<String> = (1..=5).map(|i| format!("row{i}")).collect();
        state.append_lines(id, &lines).await.unwrap();

        let result = state.get_lines(id, Some(3), None).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, 3);
        assert_eq!(result[2].0, 5);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_lines_end_only() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("cmd");
        let lines: Vec<String> = (1..=5).map(|i| format!("val{i}")).collect();
        state.append_lines(id, &lines).await.unwrap();

        let result = state.get_lines(id, None, Some(2)).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, 1);
        assert_eq!(result[1].0, 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_lines_no_output() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("true");
        // Don't append any lines
        let result = state.get_lines(id, None, None).unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_lines_isolates_commands() {
        let (mut state, _tmp) = create_test_state();
        let id1 = state.start_command("cmd1");
        let id2 = state.start_command("cmd2");

        state
            .append_lines(id1, &["from_cmd1".to_string()])
            .await
            .unwrap();
        state
            .append_lines(id2, &["from_cmd2".to_string()])
            .await
            .unwrap();

        let r1 = state.get_lines(id1, None, None).unwrap();
        let r2 = state.get_lines(id2, None, None).unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].1, "from_cmd1");
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].1, "from_cmd2");
    }

    // =================================================================
    // grep with pattern matching and command_id filtering
    // =================================================================

    #[tokio::test]
    #[serial]
    async fn test_grep_finds_matching_lines() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("build");
        state
            .append_lines(
                id,
                &[
                    "compiling foo...".to_string(),
                    "error: something failed".to_string(),
                    "compiling bar...".to_string(),
                    "error: another failure".to_string(),
                    "done".to_string(),
                ],
            )
            .await
            .unwrap();

        let (results, _total) = state.grep("error:", None, None).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].text.contains("something failed"));
        assert!(results[1].text.contains("another failure"));
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_no_matches_returns_empty() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("ok");
        state
            .append_lines(id, &["all good".to_string()])
            .await
            .unwrap();

        let (results, _total) = state.grep("NONEXISTENT_PATTERN", None, None).unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_filters_by_command_id() {
        let (mut state, _tmp) = create_test_state();
        let id1 = state.start_command("first");
        let id2 = state.start_command("second");

        state
            .append_lines(id1, &["target_word here".to_string()])
            .await
            .unwrap();
        state
            .append_lines(id2, &["target_word there".to_string()])
            .await
            .unwrap();

        // Filter to only id1
        let (results, _total) = state.grep("target_word", Some(id1), None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command_id, id1);
        assert!(results[0].text.contains("here"));

        // Filter to only id2
        let (results, _total) = state.grep("target_word", Some(id2), None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command_id, id2);
        assert!(results[0].text.contains("there"));
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_respects_limit() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("many");
        let lines: Vec<String> = (1..=20).map(|i| format!("match_{i}")).collect();
        state.append_lines(id, &lines).await.unwrap();

        let (results, total) = state.grep("match_", None, Some(5)).unwrap();
        assert_eq!(results.len(), 5);
        assert_eq!(total, 20);
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_regex_pattern() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("log");
        state
            .append_lines(
                id,
                &[
                    "2024-01-01 INFO started".to_string(),
                    "2024-01-01 ERROR crashed".to_string(),
                    "2024-01-02 WARN slow".to_string(),
                ],
            )
            .await
            .unwrap();

        let (results, _total) = state.grep("ERROR|WARN", None, None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_invalid_regex_returns_error() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("x");
        state.append_lines(id, &["text".to_string()]).await.unwrap();

        let result = state.grep("[unclosed", None, None);
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_without_limit_caps_at_default_grep_limit() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("many");
        let extra = 5;
        let lines: Vec<String> = (1..=DEFAULT_GREP_LIMIT + extra)
            .map(|i| format!("match_{i}"))
            .collect();
        state.append_lines(id, &lines).await.unwrap();

        let (results, total) = state.grep("match_", None, None).unwrap();
        assert_eq!(results.len(), DEFAULT_GREP_LIMIT);
        assert_eq!(total, DEFAULT_GREP_LIMIT + extra);
    }

    /// An absent `start` must read exactly as `Some(DEFAULT_START_LINE)` does.
    /// The second assertion pins that the constant is the value in use: any
    /// other default would make the two reads differ.
    #[tokio::test]
    #[serial]
    async fn test_get_lines_without_start_begins_at_default_start_line() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("numbered");
        state
            .append_lines(id, &["first", "second", "third"])
            .await
            .expect("append_lines");

        let defaulted = state.get_lines(id, None, None).expect("get_lines");
        let explicit = state
            .get_lines(id, Some(DEFAULT_START_LINE), None)
            .expect("get_lines");
        assert_eq!(defaulted, explicit);

        let next = state
            .get_lines(id, Some(DEFAULT_START_LINE + 1), None)
            .expect("get_lines");
        assert_ne!(
            defaulted,
            next,
            "a default of {DEFAULT_START_LINE} must not read the same as {}",
            DEFAULT_START_LINE + 1
        );
    }

    /// The path-taking constructors accept any borrowed path, so a caller that
    /// already holds a `&Path` or a `&str` needs no allocation.
    #[tokio::test]
    #[serial]
    async fn test_constructors_accept_borrowed_paths() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");

        let with_dir_path = tmp.path().join("with-dir");
        let state = ShellState::with_dir(with_dir_path.as_path()).expect("with_dir(&Path)");
        assert!(state.log_path.starts_with(&with_dir_path));

        let in_dir_path = tmp.path().join("in-dir");
        let in_dir_str = in_dir_path.to_str().expect("temp path is UTF-8");
        let state = ShellState::new_in_dir(in_dir_str).expect("new_in_dir(&str)");
        assert!(state.log_path.starts_with(&in_dir_path));
    }

    /// `start_command` and `append_lines` accept borrowed strings, so a caller
    /// with `&str` data needs no `to_string()` conversion.
    #[tokio::test]
    #[serial]
    async fn test_command_recording_accepts_borrowed_strings() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo borrowed");
        state
            .append_lines(id, &["one", "two"])
            .await
            .expect("append_lines(&[&str])");

        assert_eq!(state.list_commands()[0].command, "echo borrowed");
        let result = state.get_lines(id, None, None).expect("get_lines");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], (1, "one".to_string()));
        assert_eq!(result[1], (2, "two".to_string()));
    }

    /// A caller must be able to tell one failure from another. That is what a
    /// typed error buys over `anyhow`, and the match arm is the proof.
    #[tokio::test]
    #[serial]
    async fn append_lines_reports_an_unknown_command_by_variant() {
        let (mut state, _tmp) = create_test_state();
        let error = state
            .append_lines(999, &["nope".to_string()])
            .await
            .expect_err("append_lines to an unknown id must fail");

        assert!(
            matches!(error, ShellStateError::UnknownCommand { cmd_id: 999 }),
            "expected UnknownCommand, got {error:?}"
        );
        assert_eq!(error.to_string(), "unknown command ID 999");
    }

    /// The same proof for the kill path: the caller reads the variant, not a
    /// string.
    #[tokio::test]
    #[serial]
    async fn kill_process_reports_a_missing_process_by_variant() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("never registered");
        let error = state
            .kill_process(id)
            .await
            .expect_err("kill_process with no registered PID must fail");

        assert!(
            matches!(error, ShellStateError::NoRunningProcess { cmd_id } if cmd_id == id),
            "expected NoRunningProcess, got {error:?}"
        );
        assert_eq!(
            error.to_string(),
            format!("no running process for command ID {id}")
        );
    }

    /// And for the grep path, whose failure is a caller's pattern rather than
    /// the filesystem.
    #[tokio::test]
    #[serial]
    async fn grep_reports_an_invalid_pattern_by_variant() {
        let (state, _tmp) = create_test_state();
        let error = state
            .grep("[unclosed", None, None)
            .expect_err("an unclosed class must fail");

        assert!(
            matches!(error, ShellStateError::InvalidPattern { .. }),
            "expected InvalidPattern, got {error:?}"
        );
        assert!(
            error.to_string().starts_with("invalid regex pattern: "),
            "message: {error}"
        );
    }

    /// A log entry carries exactly [`LOG_FIELD_COUNT_AFTER_SESSION_ID`] fields
    /// after the session id, and the last field is the whole line text. So a
    /// line whose own text holds colons reads back whole. `get lines` and
    /// `grep history` share one parser, and one line proves both.
    #[tokio::test]
    #[serial]
    async fn a_line_holding_colons_reads_back_whole() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo timestamps");
        let text = "12:34:56 ERROR host:port is down";
        state.append_lines(id, &[text]).await.expect("append_lines");

        let lines = state.get_lines(id, None, None).expect("get_lines");
        assert_eq!(lines, vec![(1, text.to_string())]);

        let (results, total) = state.grep("ERROR", None, None).expect("grep");
        assert_eq!(total, 1);
        assert_eq!(results[0].text, text);
        assert_eq!(results[0].command_id, id);
        assert_eq!(results[0].line_number, 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_result_has_correct_line_numbers() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("test");
        state
            .append_lines(
                id,
                &["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            )
            .await
            .unwrap();

        let (results, _total) = state.grep("beta", None, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line_number, 2);
        assert_eq!(results[0].command_id, id);
    }
}
