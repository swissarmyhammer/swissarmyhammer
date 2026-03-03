//! Shell state management for the virtual shell
//!
//! Maintains command history, output log, process handles, and embedding storage
//! for semantic search across command output.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Local};
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use rusqlite::Connection;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use llama_embedding::{EmbeddingConfig, EmbeddingModel};

const CHUNK_SIZE: usize = 15; // lines per embedding chunk
const BYTES_PER_F32: usize = 4;
/// Bounded channel capacity — provides backpressure when the embedding worker falls behind.
const CHUNK_CHANNEL_CAPACITY: usize = 256;

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

/// A chunk of output text to be inserted into DB and embedded in the background.
struct ChunkJob {
    session_id: String,
    command_id: usize,
    chunk_index: usize,
    start_line: usize,
    end_line: usize,
    text: String,
}

/// The virtual shell state — singleton per server process
pub struct ShellState {
    pub session_id: String,
    commands: Vec<CommandRecord>,
    processes: HashMap<usize, u32>, // cmd_id -> PID
    log_path: PathBuf,
    db: Arc<Mutex<Connection>>,
    chunk_tx: mpsc::Sender<ChunkJob>,
    worker_handle: Option<JoinHandle<()>>,
    line_buffer: HashMap<usize, Vec<String>>,
    chunk_counts: HashMap<usize, usize>,
}

impl ShellState {
    /// Create a new ShellState, initializing the .shell/ directory, log file, SQLite DB,
    /// and background embedding worker.
    pub fn new() -> anyhow::Result<Self> {
        let session_id = ulid::Ulid::new().to_string();
        let shell_dir = PathBuf::from(".shell");
        fs::create_dir_all(&shell_dir)?;

        let log_path = shell_dir.join("log");
        // Touch the log file
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let db_path = shell_dir.join("embeddings.db");
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunks (
                session_id  TEXT    NOT NULL,
                command_id  INTEGER NOT NULL,
                chunk_index INTEGER NOT NULL,
                start_line  INTEGER NOT NULL,
                end_line    INTEGER NOT NULL,
                text        TEXT    NOT NULL,
                embedding   BLOB,
                PRIMARY KEY (session_id, command_id, chunk_index)
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_session ON chunks(session_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_command ON chunks(session_id, command_id);",
        )?;

        let db = Arc::new(Mutex::new(conn));
        let (chunk_tx, chunk_rx) = mpsc::channel::<ChunkJob>(CHUNK_CHANNEL_CAPACITY);

        // Spawn background embedding worker (handles both INSERT and embedding)
        let db_clone = Arc::clone(&db);
        let worker_handle = tokio::spawn(async move {
            embedding_worker(chunk_rx, db_clone).await;
        });

        Ok(Self {
            session_id,
            commands: Vec::new(),
            processes: HashMap::new(),
            log_path,
            db,
            chunk_tx,
            worker_handle: Some(worker_handle),
            line_buffer: HashMap::new(),
            chunk_counts: HashMap::new(),
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
        self.line_buffer.insert(id, Vec::new());
        self.chunk_counts.insert(id, 0);
        id
    }

    /// Register a running process PID for a command.
    pub fn register_process(&mut self, cmd_id: usize, pid: u32) {
        self.processes.insert(cmd_id, pid);
    }

    /// Append output lines from a command to the log and buffer chunks for embedding.
    ///
    /// Note: This performs blocking file I/O (log file append). This is acceptable because
    /// the shell tool is single-user and log writes are small and fast. The outer async mutex
    /// is held during this call, but concurrent shell operations are not expected.
    pub fn append_lines(&mut self, cmd_id: usize, lines: &[String]) -> anyhow::Result<()> {
        let record = self
            .commands
            .iter_mut()
            .find(|r| r.id == cmd_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown command ID {}", cmd_id))?;

        let mut log_file = OpenOptions::new().append(true).open(&self.log_path)?;

        // Collect chunks to send after releasing the mutable borrow on commands
        let mut pending_chunks: Vec<(usize, usize, usize, String)> = Vec::new();

        for line in lines {
            record.line_count += 1;
            let log_line = format!(
                "{}:{}:{}:{}\n",
                self.session_id, cmd_id, record.line_count, line
            );
            log_file.write_all(log_line.as_bytes())?;

            // Buffer for chunking
            if let Some(buf) = self.line_buffer.get_mut(&cmd_id) {
                buf.push(line.clone());
                if buf.len() >= CHUNK_SIZE {
                    let chunk_text: String = std::mem::take(buf).join("\n");
                    let chunk_index = self.chunk_counts.get(&cmd_id).copied().unwrap_or(0);
                    self.chunk_counts.insert(cmd_id, chunk_index + 1);

                    let start_line = record.line_count - CHUNK_SIZE + 1;
                    let end_line = record.line_count;

                    pending_chunks.push((chunk_index, start_line, end_line, chunk_text));
                }
            }
        }

        // Now send chunks (no longer borrowing self.commands mutably)
        for (chunk_index, start_line, end_line, chunk_text) in pending_chunks {
            self.send_chunk(cmd_id, chunk_index, start_line, end_line, chunk_text);
        }

        Ok(())
    }

    /// Flush remaining buffered lines as a final chunk when a command completes.
    fn flush_line_buffer(&mut self, cmd_id: usize) {
        if let Some(buf) = self.line_buffer.remove(&cmd_id) {
            if !buf.is_empty() {
                let record = self.commands.iter().find(|r| r.id == cmd_id);
                if let Some(record) = record {
                    let chunk_text = buf.join("\n");
                    let chunk_index = self.chunk_counts.get(&cmd_id).copied().unwrap_or(0);
                    self.chunk_counts.insert(cmd_id, chunk_index + 1);

                    let end_line = record.line_count;
                    let start_line = end_line.saturating_sub(buf.len()) + 1;

                    self.send_chunk(cmd_id, chunk_index, start_line, end_line, chunk_text);
                }
            }
        }
    }

    /// Send a chunk to the background worker for DB insert and embedding.
    /// Uses try_send to avoid blocking — if the channel is full, the chunk is dropped
    /// with a warning (backpressure shedding).
    fn send_chunk(
        &self,
        cmd_id: usize,
        chunk_index: usize,
        start_line: usize,
        end_line: usize,
        text: String,
    ) {
        if let Err(e) = self.chunk_tx.try_send(ChunkJob {
            session_id: self.session_id.clone(),
            command_id: cmd_id,
            chunk_index,
            start_line,
            end_line,
            text,
        }) {
            tracing::warn!(
                "Chunk channel full or closed (cmd {}, chunk {}) — chunk dropped: {}",
                cmd_id,
                chunk_index,
                e
            );
        }
    }

    /// Mark a command as completed with exit code.
    pub fn complete_command(&mut self, cmd_id: usize, exit_code: Option<i32>) {
        self.flush_line_buffer(cmd_id);
        self.processes.remove(&cmd_id);
        if let Some(record) = self.commands.iter_mut().find(|r| r.id == cmd_id) {
            record.status = CommandStatus::Completed;
            record.exit_code = exit_code;
            record.completed_at = Some(Instant::now());
            record.completed_at_wall = Some(Local::now());
        }
    }

    /// Mark a command as timed out.
    pub fn timeout_command(&mut self, cmd_id: usize) {
        self.flush_line_buffer(cmd_id);
        self.processes.remove(&cmd_id);
        if let Some(record) = self.commands.iter_mut().find(|r| r.id == cmd_id) {
            record.status = CommandStatus::TimedOut;
            record.exit_code = Some(-1);
            record.completed_at = Some(Instant::now());
            record.completed_at_wall = Some(Local::now());
        }
    }

    /// Kill a running command by PID. Returns the command record if found.
    pub fn kill_process(&mut self, cmd_id: usize) -> anyhow::Result<CommandRecord> {
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

        self.flush_line_buffer(cmd_id);
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
    pub fn grep(
        &self,
        pattern: &str,
        command_id: Option<usize>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<GrepResult>> {
        let limit = limit.unwrap_or(50);
        let matcher = RegexMatcher::new_line_matcher(pattern)
            .map_err(|e| anyhow::anyhow!("Invalid regex pattern: {}", e))?;

        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0))
            .line_number(true)
            .build();

        let mut results = Vec::new();
        let session_prefix = format!("{}:", self.session_id);

        searcher.search_path(
            &matcher,
            &self.log_path,
            UTF8(|_line_num, line| {
                if results.len() >= limit {
                    return Ok(false); // stop searching
                }
                // Only match lines from this session
                if let Some(rest) = line.strip_prefix(&session_prefix) {
                    // Parse cmd_id:line_number:text
                    let parts: Vec<&str> = rest.splitn(3, ':').collect();
                    if parts.len() == 3 {
                        if let Ok(cmd_id_parsed) = parts[0].parse::<usize>() {
                            // Filter by command_id if specified
                            if command_id.is_some() && command_id != Some(cmd_id_parsed) {
                                return Ok(true); // skip, continue searching
                            }
                            if let Ok(line_num) = parts[1].parse::<usize>() {
                                results.push(GrepResult {
                                    command_id: cmd_id_parsed,
                                    line_number: line_num,
                                    text: parts[2].trim_end().to_string(),
                                });
                            }
                        }
                    }
                }
                Ok(true)
            }),
        )?;

        Ok(results)
    }

    /// Get the data needed for a search operation without holding self borrowed.
    /// Returns (session_id, db_handle) so the caller can release the outer lock
    /// before performing the expensive async search.
    pub fn search_handle(&self) -> (String, Arc<Mutex<Connection>>) {
        (self.session_id.clone(), Arc::clone(&self.db))
    }
}

impl Drop for ShellState {
    fn drop(&mut self) {
        // Drop chunk_tx first to close the channel, signaling the worker to exit
        // (sender is dropped automatically as part of struct drop order, but we
        // want to abort the worker explicitly for immediate cleanup)
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }
    }
}

/// Semantic search across command output using embeddings.
/// This is a standalone function (not a &self method) so the outer SHELL_STATE
/// lock can be released before calling it.
pub async fn search(
    session_id: &str,
    db: &Arc<Mutex<Connection>>,
    query: &str,
    command_id: Option<usize>,
    limit: Option<usize>,
) -> anyhow::Result<Vec<SearchResult>> {
    let limit = limit.unwrap_or(10);

    // Create a temporary embedding model for the query
    let config = EmbeddingConfig::default();
    let mut model = EmbeddingModel::new(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create embedding model: {}", e))?;
    model
        .load_model()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load embedding model: {}", e))?;

    let query_result = model
        .embed_text(query)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to embed query: {}", e))?;
    let query_embedding = &query_result.embedding;

    // Load stored chunks with embeddings
    let conn = db.lock().await;
    let mut stmt = conn.prepare(
        "SELECT command_id, chunk_index, start_line, end_line, text, embedding FROM chunks WHERE session_id = ?1 AND embedding IS NOT NULL"
    )?;

    let cmd_id_filter = command_id;
    let mut scored: Vec<SearchResult> = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let cmd_id: usize = row.get(0)?;
            let _chunk_index: usize = row.get(1)?;
            let start_line: usize = row.get(2)?;
            let end_line: usize = row.get(3)?;
            let text: String = row.get(4)?;
            let embedding_blob: Vec<u8> = row.get(5)?;
            Ok((cmd_id, start_line, end_line, text, embedding_blob))
        })?
        .filter_map(|r| r.ok())
        .filter(|(cmd_id, _, _, _, _)| cmd_id_filter.is_none() || cmd_id_filter == Some(*cmd_id))
        .map(|(cmd_id, start_line, end_line, text, blob)| {
            let chunk_embedding = decode_embedding(&blob);
            let similarity = cosine_similarity(query_embedding, &chunk_embedding);
            SearchResult {
                command_id: cmd_id,
                start_line,
                end_line,
                text,
                similarity,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);
    Ok(scored)
}

/// Result from grep operation
#[derive(Debug, Clone)]
pub struct GrepResult {
    pub command_id: usize,
    pub line_number: usize,
    pub text: String,
}

/// Result from semantic search operation
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub command_id: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub similarity: f32,
}

/// Background embedding worker — receives chunks via channel, inserts into DB,
/// then computes and stores embeddings.
async fn embedding_worker(mut rx: mpsc::Receiver<ChunkJob>, db: Arc<Mutex<Connection>>) {
    // Lazy-init the model on first chunk
    let mut model: Option<EmbeddingModel> = None;

    while let Some(job) = rx.recv().await {
        // Step 1: INSERT the chunk text into DB (even if embedding fails, text is stored)
        {
            let conn = db.lock().await;
            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO chunks (session_id, command_id, chunk_index, start_line, end_line, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![job.session_id, job.command_id, job.chunk_index, job.start_line, job.end_line, job.text],
            ) {
                tracing::warn!("Failed to insert chunk into DB (cmd {}, chunk {}): {}", job.command_id, job.chunk_index, e);
            }
        }

        // Step 2: Initialize embedding model on first use
        if model.is_none() {
            let config = EmbeddingConfig::default();
            match EmbeddingModel::new(config).await {
                Ok(mut m) => {
                    if let Err(e) = m.load_model().await {
                        tracing::warn!(
                            "Failed to load embedding model: {}. Embeddings disabled.",
                            e
                        );
                        // Continue draining jobs (INSERT only, no embedding)
                        drain_inserts_only(&mut rx, &db).await;
                        return;
                    }
                    model = Some(m);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create embedding model: {}. Embeddings disabled.",
                        e
                    );
                    drain_inserts_only(&mut rx, &db).await;
                    return;
                }
            }
        }

        // Step 3: Compute embedding and UPDATE the row
        if let Some(ref mut m) = model {
            match m.embed_text(&job.text).await {
                Ok(mut result) => {
                    result.normalize();
                    let embedding_bytes = encode_embedding(&result.embedding);
                    let conn = db.lock().await;
                    if let Err(e) = conn.execute(
                        "UPDATE chunks SET embedding = ?1 WHERE session_id = ?2 AND command_id = ?3 AND chunk_index = ?4",
                        rusqlite::params![embedding_bytes, job.session_id, job.command_id, job.chunk_index],
                    ) {
                        tracing::warn!("Failed to store embedding in DB (cmd {}, chunk {}): {}", job.command_id, job.chunk_index, e);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to embed chunk: {}", e);
                }
            }
        }
    }
}

/// Drain remaining jobs from the channel, performing INSERT-only (no embedding).
/// Called when the embedding model fails to load.
async fn drain_inserts_only(rx: &mut mpsc::Receiver<ChunkJob>, db: &Arc<Mutex<Connection>>) {
    while let Some(job) = rx.recv().await {
        let conn = db.lock().await;
        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO chunks (session_id, command_id, chunk_index, start_line, end_line, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![job.session_id, job.command_id, job.chunk_index, job.start_line, job.end_line, job.text],
        ) {
            tracing::warn!("Failed to insert chunk into DB (cmd {}, chunk {}): {}", job.command_id, job.chunk_index, e);
        }
    }
}

/// Encode f32 embedding vector as little-endian bytes for SQLite BLOB storage.
fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * BYTES_PER_F32);
    for val in embedding {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Decode little-endian bytes from SQLite BLOB back to f32 embedding vector.
fn decode_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(BYTES_PER_F32)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a `ShellState` inside a temporary directory.
    /// Returns the state and the temp dir (which must be kept alive for the duration
    /// of the test so the directory is not deleted).
    fn create_test_state() -> (ShellState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let original = std::env::current_dir().expect("get cwd");
        std::env::set_current_dir(tmp.path()).expect("cd to tmp");
        let state = ShellState::new().expect("ShellState::new");
        std::env::set_current_dir(original).expect("restore cwd");
        (state, tmp)
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let embedding = vec![1.0f32, -2.5, 3.14, 0.0, -0.001];
        let encoded = encode_embedding(&embedding);
        let decoded = decode_embedding(&encoded);
        assert_eq!(embedding, decoded);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_command_status_display() {
        assert_eq!(CommandStatus::Running.to_string(), "running");
        assert_eq!(CommandStatus::Completed.to_string(), "completed");
        assert_eq!(CommandStatus::Killed.to_string(), "killed");
        assert_eq!(CommandStatus::TimedOut.to_string(), "timed_out");
    }

    // =================================================================
    // ShellState lifecycle: start_command, append_lines, complete_command
    // =================================================================

    #[tokio::test]
    async fn test_start_command_returns_sequential_ids() {
        let (mut state, _tmp) = create_test_state();
        let id1 = state.start_command("echo hello".into());
        let id2 = state.start_command("echo world".into());
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[tokio::test]
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
    async fn test_append_lines_increments_line_count() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo test".into());
        let lines = vec![
            "line1".to_string(),
            "line2".to_string(),
            "line3".to_string(),
        ];
        state.append_lines(id, &lines).expect("append_lines");
        let commands = state.list_commands();
        assert_eq!(commands[0].line_count, 3);
    }

    #[tokio::test]
    async fn test_append_lines_unknown_command_returns_error() {
        let (mut state, _tmp) = create_test_state();
        let result = state.append_lines(999, &["nope".to_string()]);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Unknown command ID 999"),
            "Error: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_complete_command_sets_status_and_exit_code() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo done".into());
        state.complete_command(id, Some(0));
        let commands = state.list_commands();
        assert_eq!(commands[0].status, CommandStatus::Completed);
        assert_eq!(commands[0].exit_code, Some(0));
        assert!(commands[0].completed_at.is_some());
        assert!(commands[0].completed_at_wall.is_some());
    }

    #[tokio::test]
    async fn test_timeout_command_sets_status() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("sleep 999".into());
        state.timeout_command(id);
        let commands = state.list_commands();
        assert_eq!(commands[0].status, CommandStatus::TimedOut);
        assert_eq!(commands[0].exit_code, Some(-1));
        assert!(commands[0].completed_at.is_some());
    }

    #[tokio::test]
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

        state.complete_command(id, Some(0));
        let completed_duration = state.list_commands()[0].duration();
        assert!(
            completed_duration.as_secs() < 60,
            "completed duration unreasonable"
        );
    }

    #[tokio::test]
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
    async fn test_get_lines_all() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("echo stuff".into());
        let lines: Vec<String> = (1..=5).map(|i| format!("line{i}")).collect();
        state.append_lines(id, &lines).unwrap();

        let result = state.get_lines(id, None, None).unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], (1, "line1".to_string()));
        assert_eq!(result[4], (5, "line5".to_string()));
    }

    #[tokio::test]
    async fn test_get_lines_with_start_and_end() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("seq".into());
        let lines: Vec<String> = (1..=10).map(|i| format!("data{i}")).collect();
        state.append_lines(id, &lines).unwrap();

        let result = state.get_lines(id, Some(3), Some(7)).unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], (3, "data3".to_string()));
        assert_eq!(result[4], (7, "data7".to_string()));
    }

    #[tokio::test]
    async fn test_get_lines_start_only() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("cmd".into());
        let lines: Vec<String> = (1..=5).map(|i| format!("row{i}")).collect();
        state.append_lines(id, &lines).unwrap();

        let result = state.get_lines(id, Some(3), None).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, 3);
        assert_eq!(result[2].0, 5);
    }

    #[tokio::test]
    async fn test_get_lines_end_only() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("cmd".into());
        let lines: Vec<String> = (1..=5).map(|i| format!("val{i}")).collect();
        state.append_lines(id, &lines).unwrap();

        let result = state.get_lines(id, None, Some(2)).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, 1);
        assert_eq!(result[1].0, 2);
    }

    #[tokio::test]
    async fn test_get_lines_no_output() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("true".into());
        // Don't append any lines
        let result = state.get_lines(id, None, None).unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_lines_isolates_commands() {
        let (mut state, _tmp) = create_test_state();
        let id1 = state.start_command("cmd1".into());
        let id2 = state.start_command("cmd2".into());

        state.append_lines(id1, &["from_cmd1".to_string()]).unwrap();
        state.append_lines(id2, &["from_cmd2".to_string()]).unwrap();

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
            .unwrap();

        let results = state.grep("error:", None, None).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].text.contains("something failed"));
        assert!(results[1].text.contains("another failure"));
    }

    #[tokio::test]
    async fn test_grep_no_matches_returns_empty() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("ok".into());
        state.append_lines(id, &["all good".to_string()]).unwrap();

        let results = state.grep("NONEXISTENT_PATTERN", None, None).unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_grep_filters_by_command_id() {
        let (mut state, _tmp) = create_test_state();
        let id1 = state.start_command("first".into());
        let id2 = state.start_command("second".into());

        state
            .append_lines(id1, &["target_word here".to_string()])
            .unwrap();
        state
            .append_lines(id2, &["target_word there".to_string()])
            .unwrap();

        // Filter to only id1
        let results = state.grep("target_word", Some(id1), None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command_id, id1);
        assert!(results[0].text.contains("here"));

        // Filter to only id2
        let results = state.grep("target_word", Some(id2), None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command_id, id2);
        assert!(results[0].text.contains("there"));
    }

    #[tokio::test]
    async fn test_grep_respects_limit() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("many".into());
        let lines: Vec<String> = (1..=20).map(|i| format!("match_{i}")).collect();
        state.append_lines(id, &lines).unwrap();

        let results = state.grep("match_", None, Some(5)).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
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
            .unwrap();

        let results = state.grep("ERROR|WARN", None, None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_grep_invalid_regex_returns_error() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("x".into());
        state.append_lines(id, &["text".to_string()]).unwrap();

        let result = state.grep("[unclosed", None, None);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_grep_result_has_correct_line_numbers() {
        let (mut state, _tmp) = create_test_state();
        let id = state.start_command("test".into());
        state
            .append_lines(
                id,
                &["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            )
            .unwrap();

        let results = state.grep("beta", None, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line_number, 2);
        assert_eq!(results[0].command_id, id);
    }
}
