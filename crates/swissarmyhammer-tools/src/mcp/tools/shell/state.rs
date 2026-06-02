//! Shell state management for the virtual shell
//!
//! Maintains command history, output log, and process handles.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Instant;

use chrono::{DateTime, Local};
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};

use swissarmyhammer_directory::{DirectoryConfig, ShellConfig};

/// Command execution status
#[derive(Debug, Clone, PartialEq)]
pub enum CommandStatus {
    Running,
    Completed,
    Killed,
    TimedOut,
}

impl fmt::Display for CommandStatus {
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
    pub id: usize,
    pub command: String,
    pub status: CommandStatus,
    pub exit_code: Option<i32>,
    pub line_count: usize,
    pub started_at: Instant,
    pub started_at_wall: DateTime<Local>,
    pub completed_at: Option<Instant>,
    pub completed_at_wall: Option<DateTime<Local>>,
}

impl CommandRecord {
    pub fn duration(&self) -> std::time::Duration {
        match self.completed_at {
            Some(end) => end.duration_since(self.started_at),
            None => self.started_at.elapsed(),
        }
    }
}

/// The virtual shell state — singleton per server process
pub struct ShellState {
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
    /// here keeps that from aborting the whole app via
    /// [`ShellExecuteTool::new`](super::ShellExecuteTool::new)'s `expect`.
    pub fn new() -> anyhow::Result<Self> {
        let preferred = std::env::current_dir().ok().map(|cwd| cwd.join(".shell"));
        Self::new_with_preferred(preferred)
    }

    /// Build a `ShellState`, preferring `preferred` (e.g. `<cwd>/.shell`) but
    /// falling back to a unique temp directory when it is `None` or cannot be
    /// created (missing, read-only, or otherwise unwritable).
    fn new_with_preferred(preferred: Option<PathBuf>) -> anyhow::Result<Self> {
        if let Some(dir) = preferred {
            match Self::with_dir(dir.clone()) {
                Ok(state) => return Ok(state),
                Err(error) => tracing::warn!(
                    %error,
                    dir = %dir.display(),
                    "shell state: preferred .shell directory is not writable; \
                     falling back to a temp directory"
                ),
            }
        }
        Self::with_dir(std::env::temp_dir().join(format!(".shell-{}", ulid::Ulid::new())))
    }

    /// Create a new ShellState with an explicit base directory for the .shell/ data.
    /// This avoids relying on the process-wide CWD, which is important for tests.
    pub fn new_in_dir(shell_dir: PathBuf) -> anyhow::Result<Self> {
        Self::with_dir(shell_dir)
    }

    /// Create a new ShellState rooted at the given directory.
    pub fn with_dir(shell_dir: PathBuf) -> anyhow::Result<Self> {
        let session_id = ulid::Ulid::new().to_string();
        fs::create_dir_all(&shell_dir)?;

        // Write .gitignore if it doesn't exist yet
        let gitignore_path = shell_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, ShellConfig::GITIGNORE_CONTENT)?;
        }

        let log_path = shell_dir.join("log");
        // Touch the log file
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        Ok(Self {
            session_id,
            commands: Vec::new(),
            processes: HashMap::new(),
            log_path,
        })
    }

    /// Start tracking a new command. Returns the assigned command ID.
    pub fn start_command(&mut self, command: String) -> usize {
        let id = self.commands.len() + 1;
        let now = Instant::now();
        self.commands.push(CommandRecord {
            id,
            command,
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
    pub async fn append_lines(&mut self, cmd_id: usize, lines: &[String]) -> anyhow::Result<()> {
        let record = self
            .commands
            .iter_mut()
            .find(|r| r.id == cmd_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown command ID {}", cmd_id))?;

        let mut log_file = OpenOptions::new().append(true).open(&self.log_path)?;

        for line in lines {
            record.line_count += 1;
            let log_line = format!(
                "{}:{}:{}:{}\n",
                self.session_id, cmd_id, record.line_count, line
            );
            log_file.write_all(log_line.as_bytes())?;
        }

        Ok(())
    }

    /// Mark a command as completed with exit code.
    pub async fn complete_command(&mut self, cmd_id: usize, exit_code: Option<i32>) {
        self.processes.remove(&cmd_id);
        if let Some(record) = self.commands.iter_mut().find(|r| r.id == cmd_id) {
            record.status = CommandStatus::Completed;
            record.exit_code = exit_code;
            record.completed_at = Some(Instant::now());
            record.completed_at_wall = Some(Local::now());
        }
    }

    /// Mark a command as timed out.
    pub async fn timeout_command(&mut self, cmd_id: usize) {
        self.processes.remove(&cmd_id);
        if let Some(record) = self.commands.iter_mut().find(|r| r.id == cmd_id) {
            record.status = CommandStatus::TimedOut;
            record.exit_code = Some(-1);
            record.completed_at = Some(Instant::now());
            record.completed_at_wall = Some(Local::now());
        }
    }

    /// Kill a running command by PID. Returns the command record if found.
    pub async fn kill_process(&mut self, cmd_id: usize) -> anyhow::Result<CommandRecord> {
        let pid = self
            .processes
            .get(&cmd_id)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("No running process for command ID {}", cmd_id))?;

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
            anyhow::bail!("Command record not found for ID {}", cmd_id)
        }
    }

    /// List all command records.
    pub fn list_commands(&self) -> &[CommandRecord] {
        &self.commands
    }

    /// Get lines from a specific command's output by reading the log file.
    ///
    /// Note: This performs blocking file I/O. Acceptable for single-user shell tool
    /// where log reads are fast and infrequent.
    pub fn get_lines(
        &self,
        cmd_id: usize,
        start: Option<usize>,
        end: Option<usize>,
    ) -> anyhow::Result<Vec<(usize, String)>> {
        let start = start.unwrap_or(1);
        let end = end.unwrap_or(usize::MAX);
        let prefix = format!("{}:{}:", self.session_id, cmd_id);

        let file = std::fs::File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Some(rest) = line.strip_prefix(&prefix) {
                // Parse line_number:text
                if let Some((line_num_str, text)) = rest.split_once(':') {
                    if let Ok(line_num) = line_num_str.parse::<usize>() {
                        if line_num >= start && line_num <= end {
                            results.push((line_num, text.to_string()));
                        }
                        if line_num > end {
                            break;
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Grep command output history using regex pattern matching.
    ///
    /// Note: This performs blocking file I/O. Acceptable for single-user shell tool
    /// where grep is fast over local log files.
    /// Returns `(matching_results, total_match_count)`. Results are capped by `limit`
    /// (default 10) but `total_match_count` reflects all matches found.
    pub fn grep(
        &self,
        pattern: &str,
        command_id: Option<usize>,
        limit: Option<usize>,
    ) -> anyhow::Result<(Vec<GrepResult>, usize)> {
        let limit = limit.unwrap_or(10);
        let matcher = RegexMatcher::new_line_matcher(pattern)
            .map_err(|e| anyhow::anyhow!("Invalid regex pattern: {}", e))?;

        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0))
            .line_number(true)
            .build();

        let mut results = Vec::new();
        let mut total_matches: usize = 0;
        let session_prefix = format!("{}:", self.session_id);

        searcher.search_path(
            &matcher,
            &self.log_path,
            UTF8(|_line_num, line| {
                if let Some(entry) = parse_grep_log_line(line, &session_prefix, command_id) {
                    total_matches += 1;
                    if results.len() < limit {
                        results.push(entry);
                    }
                }
                Ok(true)
            }),
        )?;

        Ok((results, total_matches))
    }
}

/// Parse one `session_id:cmd_id:line_num:text` log entry into a [`GrepResult`].
///
/// Returns `None` when the line does not belong to this session, when the
/// command_id filter rejects it, or when any required field fails to parse.
fn parse_grep_log_line(
    line: &str,
    session_prefix: &str,
    cmd_id_filter: Option<usize>,
) -> Option<GrepResult> {
    let rest = line.strip_prefix(session_prefix)?;
    let parts: Vec<&str> = rest.splitn(3, ':').collect();
    if parts.len() != 3 {
        return None;
    }
    let command_id = parts[0].parse::<usize>().ok()?;
    if cmd_id_filter.is_some() && cmd_id_filter != Some(command_id) {
        return None;
    }
    let line_number = parts[1].parse::<usize>().ok()?;
    Some(GrepResult {
        command_id,
        line_number,
        text: parts[2].trim_end().to_string(),
    })
}

/// Result from grep operation
#[derive(Debug, Clone)]
pub struct GrepResult {
    pub command_id: usize,
    pub line_number: usize,
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
    /// fallback, that error propagated through `ShellExecuteTool::new()`'s
    /// `expect("Failed to initialize shell state")` and aborted the whole app
    /// on launch (panic in `did_finish_launching`).
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
        let id1 = state.start_command("echo hello".into());
        let id2 = state.start_command("echo world".into());
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_start_command_creates_running_record() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("ls -la".into());
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
        let id = state.start_command("echo test".into());
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
            err_msg.contains("Unknown command ID 999"),
            "Error: {err_msg}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_complete_command_sets_status_and_exit_code() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo done".into());
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
        let id = state.start_command("sleep 999".into());
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
        let id = state.start_command("quick".into());
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
        let id = state.start_command("sleep 10".into());
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
        let id = state.start_command("echo stuff".into());
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
        let id = state.start_command("seq".into());
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
        let id = state.start_command("cmd".into());
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
        let id = state.start_command("cmd".into());
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
        let id = state.start_command("true".into());
        // Don't append any lines
        let result = state.get_lines(id, None, None).unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_lines_isolates_commands() {
        let (mut state, _tmp) = create_test_state();
        let id1 = state.start_command("cmd1".into());
        let id2 = state.start_command("cmd2".into());

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
        let id = state.start_command("build".into());
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
        let id = state.start_command("ok".into());
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
        let id1 = state.start_command("first".into());
        let id2 = state.start_command("second".into());

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
        let id = state.start_command("many".into());
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
        let id = state.start_command("log".into());
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
        let id = state.start_command("x".into());
        state.append_lines(id, &["text".to_string()]).await.unwrap();

        let result = state.grep("[unclosed", None, None);
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_grep_result_has_correct_line_numbers() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("test".into());
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
