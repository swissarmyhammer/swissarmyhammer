//! Process management for shell command execution
//!
//! This module contains the AsyncProcessGuard for process lifecycle management,
//! output streaming functions, and command spawning utilities.

use crate::mcp::tool_registry::{send_mcp_log, ToolContext};
use rmcp::model::{LoggingLevel, LoggingMessageNotification, LoggingMessageNotificationParam};
use rmcp::{Peer, RoleServer};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use swissarmyhammer_common::Pretty;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use super::infrastructure::{OutputBuffer, OutputLimits, ShellError, ShellExecutionResult};

/// Async process guard for automatic cleanup of tokio Child processes
///
/// This guard automatically terminates and cleans up child processes when dropped,
/// ensuring no orphaned processes remain even if a timeout occurs or the operation is cancelled.
///
/// Unlike the sync ProcessGuard in test_utils.rs, this version works with tokio::process::Child
/// and provides async methods for graceful termination with timeouts.
pub struct AsyncProcessGuard {
    pub(super) child: Option<Child>,
    pub(super) command: String,
}

impl AsyncProcessGuard {
    /// Create a new async process guard from a tokio Child process
    pub fn new(child: Child, command: String) -> Self {
        Self {
            child: Some(child),
            command,
        }
    }

    /// Take the child process out of the guard, transferring ownership.
    /// WARNING: After calling this, the guard's Drop will NOT kill the process.
    /// Only use when you need ownership AND will handle cleanup yourself.
    pub fn take_child(&mut self) -> Option<Child> {
        self.child.take()
    }

    /// Borrow the child process mutably without removing it from the guard.
    /// The guard retains ownership, so its Drop will still kill the process.
    pub fn child_mut(&mut self) -> Option<&mut Child> {
        self.child.as_mut()
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,     // Process is still running
                Ok(Some(_)) => false, // Process has exited
                Err(_) => false,      // Error occurred, assume process is dead
            }
        } else {
            false
        }
    }

    /// Attempt to gracefully terminate the process with a timeout
    pub async fn terminate_gracefully(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref mut child) = self.child {
            tracing::debug!(
                "Attempting graceful termination of process for command: {}",
                self.command
            );

            // Try to terminate the process and wait for it to exit
            let termination_result = tokio::time::timeout(timeout_duration, async {
                // On Unix systems, we can try to send SIGTERM first
                #[cfg(unix)]
                {
                    // Kill the process group to handle child processes
                    if let Some(pid) = child.id() {
                        unsafe {
                            // Send SIGTERM to the process group
                            libc::killpg(pid as i32, libc::SIGTERM);
                        }
                    }
                }

                // On Windows or if Unix signal handling fails, use kill()
                #[cfg(not(unix))]
                {
                    let _ = child.kill().await;
                }

                // Wait for the process to exit
                child.wait().await
            })
            .await;

            match termination_result {
                Ok(wait_result) => {
                    tracing::debug!(
                        "Process terminated gracefully for command: {}",
                        self.command
                    );
                    wait_result?;
                    self.child = None;
                    Ok(())
                }
                Err(_) => {
                    // Timeout occurred, force kill
                    tracing::warn!(
                        "Graceful termination timed out, force killing process for command: {}",
                        self.command
                    );
                    self.force_kill().await
                }
            }
        } else {
            Ok(())
        }
    }

    /// Force kill the process immediately
    pub async fn force_kill(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(mut child) = self.child.take() {
            tracing::debug!("Force killing process for command: {}", self.command);

            #[cfg(unix)]
            {
                // Kill the process group to handle child processes
                if let Some(pid) = child.id() {
                    unsafe {
                        // Send SIGKILL to the process group
                        libc::killpg(pid as i32, libc::SIGKILL);
                    }
                }
            }

            child.kill().await?;
            child.wait().await?;
            tracing::debug!("Process force killed for command: {}", self.command);
        }
        Ok(())
    }
}

impl Drop for AsyncProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Try to clean up the process synchronously
            // This is a best-effort cleanup since Drop cannot be async
            tracing::warn!(
                "AsyncProcessGuard dropping with active process for command: {}",
                self.command
            );

            #[cfg(unix)]
            {
                // Kill the process group on Unix systems
                if let Some(pid) = child.id() {
                    unsafe {
                        libc::killpg(pid as i32, libc::SIGKILL);
                    }
                }
            }

            // Kill the process
            let _ = child.start_kill();

            // IMPORTANT: Wait for the process to prevent zombie processes
            // We must call wait() after kill to reap the process and allow the OS
            // to clean up resources. Without this, killed processes become zombies.
            //
            // We use try_wait() in a loop with a timeout rather than blocking wait()
            // to avoid hanging Drop. The tokio Child doesn't have a blocking wait(),
            // but try_wait() will eventually succeed after start_kill().
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_millis(100);

            while start.elapsed() < timeout {
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        // Process has been reaped successfully
                        tracing::debug!("Process reaped in Drop for command: {}", self.command);
                        return;
                    }
                    Ok(None) => {
                        // Process still running, wait a bit and try again
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Error waiting for process in Drop for command {}: {}",
                            self.command,
                            e
                        );
                        return;
                    }
                }
            }

            // If we get here, the process didn't exit within timeout
            // One final try_wait() to reap if it finished during the last iteration
            match child.try_wait() {
                Ok(Some(_)) => {
                    tracing::debug!(
                        "Process reaped on final attempt in Drop for command: {}",
                        self.command
                    );
                }
                Ok(None) => {
                    tracing::warn!(
                        "Process still running after timeout in Drop for command: {}. \
                         This may result in a zombie process.",
                        self.command
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Final wait failed in Drop for command {}: {}",
                        self.command,
                        e
                    );
                }
            }
        }
    }
}

/// Context for processing output lines
struct OutputLineContext<'a> {
    line_count: &'a mut u32,
    output_buffer: &'a mut OutputBuffer,
    binary_notified: &'a mut bool,
    peer: Option<&'a Arc<Peer<RoleServer>>>,
    batch_size: u32,
}

/// Helper function to process a single output line with common logic
///
/// This eliminates code duplication between stdout and stderr processing,
/// as well as between pre-exit and post-exit output processing.
///
/// # Arguments
///
/// * `line` - The output line to process
/// * `ctx` - Context containing line counter, buffer, and progress tracking
/// * `append_fn` - Function to append data to the appropriate buffer (stdout or stderr)
///
/// # Returns
///
/// Returns the number of bytes written, or 0 if the buffer limit was reached
#[inline]
fn process_output_line(
    line: String,
    ctx: &mut OutputLineContext<'_>,
    append_fn: impl FnOnce(&mut OutputBuffer, &[u8]) -> usize,
) -> usize {
    *ctx.line_count += 1;

    // Send batched progress notifications every batch_size lines via tokio::spawn (sync context)
    if (*ctx.line_count).is_multiple_of(ctx.batch_size) {
        if let Some(peer) = ctx.peer {
            let peer = Arc::clone(peer);
            let msg = format!("Shell output: {} lines processed", ctx.line_count);
            tokio::spawn(async move {
                let param = LoggingMessageNotificationParam {
                    level: LoggingLevel::Info,
                    logger: Some("shell".to_string()),
                    data: serde_json::json!(msg),
                };
                let _ = peer
                    .send_notification(LoggingMessageNotification::new(param).into())
                    .await;
            });
        }
    }

    // Convert line to bytes with newline
    let line_bytes = line.as_bytes();
    let mut line_with_newline = Vec::with_capacity(line_bytes.len() + 1);
    line_with_newline.extend_from_slice(line_bytes);
    line_with_newline.push(b'\n');

    // Append to the appropriate buffer
    let bytes_written = append_fn(ctx.output_buffer, &line_with_newline);

    // Check for binary detection and notify once
    if ctx.output_buffer.has_binary_content() && !*ctx.binary_notified {
        *ctx.binary_notified = true;
        if let Some(peer) = ctx.peer {
            let peer = Arc::clone(peer);
            tokio::spawn(async move {
                let param = LoggingMessageNotificationParam {
                    level: LoggingLevel::Info,
                    logger: Some("shell".to_string()),
                    data: serde_json::json!("Shell output: Binary content detected"),
                };
                let _ = peer
                    .send_notification(LoggingMessageNotification::new(param).into())
                    .await;
            });
        }
    }

    bytes_written
}

/// Helper function to process a single stream line with error handling
///
/// This eliminates duplication in the tokio::select! branches by extracting the common
/// pattern of processing a line result, handling EOF, and checking buffer limits.
///
/// # Returns
///
/// Returns true if processing should continue, false if it should stop (due to error or buffer limit)
#[inline]
fn process_stream_line_result(
    line_result: Result<Option<String>, std::io::Error>,
    ctx: &mut OutputLineContext<'_>,
    append_fn: impl FnOnce(&mut OutputBuffer, &[u8]) -> usize,
    stream_name: &str,
) -> bool {
    match line_result {
        Ok(Some(line)) => {
            let bytes_written = process_output_line(line, ctx, append_fn);

            // If we couldn't write anything, we've hit the limit
            if bytes_written == 0 && ctx.output_buffer.is_at_limit() {
                tracing::debug!("Output buffer limit reached, stopping {stream_name} processing");
                false
            } else {
                true
            }
        }
        Ok(None) => {
            // EOF on stream
            tracing::debug!("{stream_name} EOF reached");
            true
        }
        Err(e) => {
            tracing::warn!("Error reading {stream_name}: {e}");
            false
        }
    }
}

/// Helper function to read remaining output from any stream after process exit
///
/// This generic function works with both stdout and stderr readers by accepting
/// any type that implements AsyncRead + Unpin, eliminating code duplication.
async fn read_remaining_stream_output<R>(
    reader: &mut tokio::io::Lines<BufReader<R>>,
    ctx: &mut OutputLineContext<'_>,
    append_fn: impl Fn(&mut OutputBuffer, &[u8]) -> usize,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    while let Ok(Some(line)) = reader.next_line().await {
        if ctx.output_buffer.is_at_limit() {
            break;
        }
        process_output_line(line, ctx, &append_fn);
    }
}

/// Helper function to read remaining output from a stream with context creation
///
/// This wrapper eliminates duplication when creating OutputLineContext for reading
/// remaining stream output after process exit.
async fn read_remaining_with_context<R>(
    reader: &mut tokio::io::Lines<BufReader<R>>,
    line_count: &mut u32,
    output_buffer: &mut OutputBuffer,
    binary_notified: &mut bool,
    peer: Option<&Arc<Peer<RoleServer>>>,
    append_fn: impl Fn(&mut OutputBuffer, &[u8]) -> usize,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    const BATCH_SIZE: u32 = 10;
    let mut ctx = OutputLineContext {
        line_count,
        output_buffer,
        binary_notified,
        peer,
        batch_size: BATCH_SIZE,
    };
    read_remaining_stream_output(reader, &mut ctx, append_fn).await;
}

/// Setup for output capture from child process
struct OutputCaptureSetup {
    stdout_reader: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    stderr_reader: tokio::io::Lines<BufReader<tokio::process::ChildStderr>>,
    output_buffer: OutputBuffer,
}

/// Initialize output capture for a child process
fn setup_output_capture(
    child: &mut Child,
    output_limits: &OutputLimits,
) -> Result<OutputCaptureSetup, ShellError> {
    let stdout = child.stdout.take().ok_or_else(|| ShellError::SystemError {
        message: "Failed to capture stdout from child process".to_string(),
    })?;

    let stderr = child.stderr.take().ok_or_else(|| ShellError::SystemError {
        message: "Failed to capture stderr from child process".to_string(),
    })?;

    Ok(OutputCaptureSetup {
        stdout_reader: BufReader::new(stdout).lines(),
        stderr_reader: BufReader::new(stderr).lines(),
        output_buffer: OutputBuffer::new(output_limits.max_output_size),
    })
}

/// Collect any remaining output after process exits
async fn collect_remaining_output(
    stdout_reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    stderr_reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStderr>>,
    line_count: &mut u32,
    output_buffer: &mut OutputBuffer,
    binary_notified: &mut bool,
    peer: Option<&Arc<Peer<RoleServer>>>,
) {
    const REMAINING_OUTPUT_TIMEOUT: Duration = Duration::from_millis(500);

    let stdout_future = read_remaining_with_context(
        stdout_reader,
        line_count,
        output_buffer,
        binary_notified,
        peer,
        |buf, data| buf.append_stdout(data),
    );
    let _ = tokio::time::timeout(REMAINING_OUTPUT_TIMEOUT, stdout_future).await;

    let stderr_future = read_remaining_with_context(
        stderr_reader,
        line_count,
        output_buffer,
        binary_notified,
        peer,
        |buf, data| buf.append_stderr(data),
    );
    let _ = tokio::time::timeout(REMAINING_OUTPUT_TIMEOUT, stderr_future).await;
}

/// Stream output until process completes or buffer limit reached
async fn stream_output_until_complete(
    stdout_reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    stderr_reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStderr>>,
    child: &mut Child,
    line_count: &mut u32,
    output_buffer: &mut OutputBuffer,
    binary_notified: &mut bool,
    peer: Option<&Arc<Peer<RoleServer>>>,
) -> Result<std::process::ExitStatus, ShellError> {
    const BATCH_SIZE: u32 = 10;

    loop {
        tokio::select! {
            stdout_line = stdout_reader.next_line() => {
                let mut ctx = OutputLineContext {
                    line_count, output_buffer, binary_notified,
                    peer, batch_size: BATCH_SIZE,
                };
                if !process_stream_line_result(
                    stdout_line, &mut ctx,
                    |buf, data| buf.append_stdout(data), "stdout",
                ) { break; }
            }

            stderr_line = stderr_reader.next_line() => {
                let mut ctx = OutputLineContext {
                    line_count, output_buffer, binary_notified,
                    peer, batch_size: BATCH_SIZE,
                };
                if !process_stream_line_result(
                    stderr_line, &mut ctx,
                    |buf, data| buf.append_stderr(data), "stderr",
                ) { break; }
            }

            exit_status = child.wait() => {
                return exit_status.map_err(|e| ShellError::ExecutionError {
                    command: "child process".to_string(),
                    message: format!("Failed to wait for process: {e}"),
                });
            }
        }

        if output_buffer.is_at_limit() {
            tracing::debug!("Output buffer at limit, stopping all processing");
            break;
        }
    }

    child.wait().await.map_err(|e| ShellError::ExecutionError {
        command: "child process".to_string(),
        message: format!("Failed to wait for process: {e}"),
    })
}

/// Process child output streams with limits using async streaming
///
/// Handles the streaming capture of stdout and stderr from a child process
/// with configurable size limits, binary detection, and intelligent truncation.
pub(super) async fn process_child_output_with_limits(
    child: &mut Child,
    output_limits: &OutputLimits,
    peer: Option<&Arc<Peer<RoleServer>>>,
) -> Result<(std::process::ExitStatus, OutputBuffer, u32), ShellError> {
    let mut setup = setup_output_capture(child, output_limits)?;
    let mut line_count: u32 = 0;
    let mut binary_notified = false;

    let exit_status = stream_output_until_complete(
        &mut setup.stdout_reader,
        &mut setup.stderr_reader,
        child,
        &mut line_count,
        &mut setup.output_buffer,
        &mut binary_notified,
        peer,
    )
    .await?;

    #[derive(serde::Serialize, Debug)]
    struct ProcessExitInfo {
        exit_code: Option<i32>,
        success: bool,
    }
    let exit_info = ProcessExitInfo {
        exit_code: exit_status.code(),
        success: exit_status.success(),
    };
    tracing::debug!("Process exited with status: {}", Pretty(&exit_info));

    collect_remaining_output(
        &mut setup.stdout_reader,
        &mut setup.stderr_reader,
        &mut line_count,
        &mut setup.output_buffer,
        &mut binary_notified,
        peer,
    )
    .await;

    setup.output_buffer.add_truncation_marker();

    Ok((exit_status, setup.output_buffer, line_count))
}

/// Validate and prepare working directory
pub(super) fn prepare_working_directory(
    working_directory: Option<PathBuf>,
) -> Result<PathBuf, ShellError> {
    let work_dir = working_directory
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    if !work_dir.exists() {
        return Err(ShellError::WorkingDirectoryError {
            message: format!("Working directory does not exist: {}", work_dir.display()),
        });
    }

    Ok(work_dir)
}

/// Prepare shell command for execution
pub(super) fn prepare_shell_command(
    command: &str,
    work_dir: &Path,
    environment: Option<&std::collections::HashMap<String, String>>,
) -> Command {
    let (program, args) = if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", command])
    } else {
        ("sh", vec!["-c", command])
    };

    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(work_dir);

    if let Some(env_vars) = environment {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    cmd
}

/// Spawn command process with error handling
pub(super) fn spawn_command_process(
    mut cmd: Command,
    command: &str,
    work_dir: &Path,
) -> Result<Child, ShellError> {
    tracing::info!(
        "Executing shell command: '{}' in directory: {}",
        command,
        work_dir.display()
    );

    cmd.spawn().map_err(|e| {
        tracing::error!("Failed to spawn command '{}': {}", command, e);
        ShellError::CommandSpawnError {
            command: command.to_string(),
            source: e,
        }
    })
}

/// Send completion progress notification
pub(super) async fn send_completion_notification(
    context: &ToolContext,
    line_count: u32,
    exit_code: i32,
    execution_time_ms: u64,
) {
    send_mcp_log(
        context,
        LoggingLevel::Info,
        "shell",
        format!(
            "Command completed: {} lines, exit code {}, {}ms",
            line_count, exit_code, execution_time_ms
        ),
    )
    .await;
}

/// Format execution result from output buffer
pub(super) fn format_execution_result(
    command_id: usize,
    command: String,
    work_dir: PathBuf,
    exit_status: std::process::ExitStatus,
    output_buffer: OutputBuffer,
    execution_time_ms: u64,
    output_limits: &OutputLimits,
) -> ShellExecutionResult {
    let exit_code = exit_status.code().unwrap_or(-1);
    let truncation_info = if output_buffer.is_truncated() {
        format!(
            " (output truncated at {} bytes)",
            output_limits.max_output_size
        )
    } else {
        String::new()
    };
    let binary_info = if output_buffer.has_binary_content() {
        " (binary content detected)"
    } else {
        ""
    };

    tracing::info!(
        "Command '{}' completed with exit code {} in {}ms{}{}",
        command,
        exit_code,
        execution_time_ms,
        truncation_info,
        binary_info
    );

    ShellExecutionResult {
        command_id,
        command,
        exit_code,
        stdout: output_buffer.get_stdout(),
        stderr: output_buffer.get_stderr(),
        execution_time_ms,
        working_directory: work_dir,
        output_truncated: output_buffer.is_truncated(),
        total_output_size: output_buffer.total_bytes_processed(),
        binary_output_detected: output_buffer.has_binary_content(),
    }
}

/// Spawn a shell command and return the guard (with PID available) and working dir.
/// The guard owns the child process — if dropped, it kills the process.
pub(super) fn spawn_shell_command(
    command: &str,
    working_directory: Option<PathBuf>,
    environment: Option<&std::collections::HashMap<String, String>>,
) -> Result<(AsyncProcessGuard, PathBuf), ShellError> {
    let work_dir = prepare_working_directory(working_directory)?;
    let cmd = prepare_shell_command(command, &work_dir, environment);
    let child = spawn_command_process(cmd, command, &work_dir)?;
    let process_guard = AsyncProcessGuard::new(child, command.to_string());
    Ok((process_guard, work_dir))
}

/// Execute using an already-spawned process guard. The guard retains child ownership,
/// so if this future is cancelled (e.g., by timeout), the guard's Drop kills the process.
pub(super) async fn execute_with_guard(
    process_guard: &mut AsyncProcessGuard,
    command_id: usize,
    command: String,
    work_dir: PathBuf,
    context: &ToolContext,
) -> Result<ShellExecutionResult, ShellError> {
    let start_time = Instant::now();
    let output_limits = OutputLimits::with_defaults().map_err(|e| ShellError::SystemError {
        message: format!("Invalid output configuration: {e}"),
    })?;

    let child = process_guard
        .child_mut()
        .ok_or_else(|| ShellError::SystemError {
            message: "Process guard has no child process".to_string(),
        })?;

    let (exit_status, output_buffer, line_count) =
        process_child_output_with_limits(child, &output_limits, context.peer.as_ref()).await?;

    let execution_time_ms = start_time.elapsed().as_millis() as u64;
    let exit_code = exit_status.code().unwrap_or(-1);

    send_completion_notification(context, line_count, exit_code, execution_time_ms).await;

    Ok(format_execution_result(
        command_id,
        command,
        work_dir,
        exit_status,
        output_buffer,
        execution_time_ms,
        &output_limits,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Helper function to spawn a sleep process for testing process guards
    ///
    /// This reduces duplication in process guard tests by providing a common
    /// way to create long-running test processes.
    fn spawn_sleep_process(duration_secs: u64) -> AsyncProcessGuard {
        let mut cmd = Command::new("sleep");
        cmd.arg(duration_secs.to_string());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().expect("Failed to spawn sleep process for test");
        AsyncProcessGuard::new(child, format!("sleep {duration_secs}"))
    }

    #[tokio::test]
    async fn test_async_process_guard_basic_operations() {
        // Test basic creation and operations
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());

        let child = cmd.spawn().expect("Failed to spawn test process");
        let command_str = "echo test".to_string();

        let mut guard = AsyncProcessGuard::new(child, command_str.clone());

        // Initially process should be running or finishing
        // (echo command might finish very quickly)

        // Test that we can take the child process
        let taken_child = guard.take_child();
        assert!(taken_child.is_some());

        // After taking, should not be running
        assert!(!guard.is_running());

        // Taking again should return None
        assert!(guard.take_child().is_none());
    }

    #[tokio::test]
    async fn test_async_process_guard_graceful_termination() {
        // Test graceful termination of a longer-running process
        let mut guard = spawn_sleep_process(10);

        // Process should initially be running
        assert!(guard.is_running());

        // Test graceful termination with a short timeout
        let start = std::time::Instant::now();
        let result = guard.terminate_gracefully(Duration::from_millis(100)).await;
        let elapsed = start.elapsed();

        // Should complete relatively quickly
        assert!(elapsed < Duration::from_secs(2));

        // Termination should succeed
        assert!(
            result.is_ok(),
            "Graceful termination should succeed: {result:?}"
        );

        // Process should no longer be running
        assert!(!guard.is_running());
    }

    #[tokio::test]
    async fn test_async_process_guard_force_kill() {
        // Test force killing a stubborn process
        let mut guard = spawn_sleep_process(30);

        // Process should be running
        assert!(guard.is_running());

        // Force kill should work quickly
        let start = std::time::Instant::now();
        let result = guard.force_kill().await;
        let elapsed = start.elapsed();

        // Should complete very quickly
        assert!(elapsed < Duration::from_secs(1));

        // Kill should succeed
        assert!(result.is_ok(), "Force kill should succeed: {result:?}");

        // Process should no longer be running
        assert!(!guard.is_running());
    }

    #[tokio::test]
    async fn test_async_process_guard_with_completed_process() {
        // Test behavior with a process that completes normally
        let mut cmd = Command::new("echo");
        cmd.arg("quick test");
        cmd.stdout(std::process::Stdio::piped());

        let child = cmd.spawn().expect("Failed to spawn echo process");
        let mut guard = AsyncProcessGuard::new(child, "echo quick test".to_string());

        // Give the echo command time to complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Process may already be finished
        let _was_running = guard.is_running();

        // Graceful termination of already-finished process should succeed
        let result = guard.terminate_gracefully(Duration::from_millis(100)).await;
        assert!(result.is_ok());

        // Process should definitely not be running now
        assert!(!guard.is_running());

        // Force kill on already-finished process should succeed
        let result = guard.force_kill().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_process_guard_drop_behavior() {
        // Test that dropping the guard cleans up properly
        let child = spawn_sleep_process(5).take_child().unwrap();
        let guard = AsyncProcessGuard::new(child, "sleep 5".to_string());

        // Drop the guard (this will trigger the Drop implementation)
        drop(guard);

        // Give time for cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;

        // We can't directly test if the process was killed since we dropped the guard,
        // but the Drop implementation should have attempted cleanup
        // This test mainly ensures Drop doesn't panic
    }

    #[tokio::test]
    async fn test_async_process_guard_prevents_zombie_processes() {
        // This test verifies that the Drop implementation properly reaps processes
        // to prevent zombie processes. We check the SPECIFIC process we create,
        // not all zombies, to avoid flakiness from concurrent tests.

        // Track the PID we create
        #[cfg(unix)]
        let spawned_pid: u32;

        // Create a scope to ensure guard is dropped
        {
            let mut cmd = Command::new("sleep");
            cmd.arg("10"); // Long enough that it won't exit naturally
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            let child = cmd.spawn().expect("Failed to spawn sleep process");
            let pid = child.id().expect("Failed to get process ID");

            #[cfg(unix)]
            {
                spawned_pid = pid;
            }

            let guard = AsyncProcessGuard::new(child, "sleep 10".to_string());

            // Verify process is running
            #[cfg(unix)]
            assert!(is_process_running(pid), "Process should be running");

            // Drop the guard - this should kill and reap the process
            drop(guard);
        } // Guard dropped here

        // Give the Drop implementation time to complete
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check that our specific process is not a zombie
        // We check the specific PID to avoid flakiness from other concurrent processes
        #[cfg(unix)]
        {
            let is_zombie = is_process_zombie(spawned_pid);
            assert!(
                !is_zombie,
                "Process {} should not be a zombie after Drop. \
                 The AsyncProcessGuard should have reaped it.",
                spawned_pid
            );
        }
    }

    #[cfg(unix)]
    fn is_process_zombie(pid: u32) -> bool {
        // Check if a specific process is a zombie
        use std::process::Command as StdCommand;

        // Try to get process status via /proc on Linux or ps on macOS
        #[cfg(target_os = "linux")]
        {
            let stat_path = format!("/proc/{}/stat", pid);
            if let Ok(content) = std::fs::read_to_string(&stat_path) {
                // Format: pid (comm) state ...
                // State is the third field, 'Z' means zombie
                if let Some(state_start) = content.rfind(')') {
                    let after_comm = &content[state_start + 1..];
                    let state = after_comm.trim().chars().next();
                    return state == Some('Z');
                }
            }
            false
        }

        #[cfg(target_os = "macos")]
        {
            let output = StdCommand::new("ps")
                .arg("-p")
                .arg(pid.to_string())
                .arg("-o")
                .arg("stat=")
                .output();

            match output {
                Ok(out) => {
                    let stat = String::from_utf8_lossy(&out.stdout);
                    stat.trim().contains('Z')
                }
                Err(_) => false, // Process doesn't exist, not a zombie
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = pid;
            false // Can't check on other platforms
        }
    }

    #[cfg(unix)]
    fn is_process_running(pid: u32) -> bool {
        // Send signal 0 to check if process exists
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    #[tokio::test]
    async fn test_async_process_guard_empty_guard() {
        // Test behavior with no child process
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());

        let child = cmd.spawn().expect("Failed to spawn test process");
        let mut guard = AsyncProcessGuard::new(child, "echo test".to_string());

        // Take the child, leaving the guard empty
        let _taken = guard.take_child();
        assert!(!guard.is_running());

        // Operations on empty guard should succeed gracefully
        let result = guard.terminate_gracefully(Duration::from_millis(100)).await;
        assert!(result.is_ok());

        let result = guard.force_kill().await;
        assert!(result.is_ok());

        assert!(!guard.is_running());
    }

    #[tokio::test]
    async fn test_async_process_guard_process_status_detection() {
        // Test accurate process status detection
        let mut cmd = Command::new("sleep");
        cmd.arg("1"); // Sleep for 1 second
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().expect("Failed to spawn sleep process");
        let mut guard = AsyncProcessGuard::new(child, "sleep 1".to_string());

        // Should be running initially
        assert!(guard.is_running());

        // Wait for process to complete naturally
        tokio::time::sleep(Duration::from_millis(1200)).await;

        // Should detect that process has completed
        assert!(!guard.is_running());

        // Cleanup should succeed on already-completed process
        let result = guard.terminate_gracefully(Duration::from_millis(100)).await;
        assert!(result.is_ok());
    }
}
