//! Startup stale entry cleanup for the code context index.
//!
//! Runs once at startup (leader only) before indexers begin. Walks the
//! filesystem, hashes every non-ignored file, and reconciles the database:
//!
//! - Deletes DB entries for files that no longer exist on disk.
//! - Deletes DB entries for files on disk that the walk skips (for example,
//!   files that the shared ignore filter covers).
//! - Marks files dirty whose `content_hash` changed.
//! - Inserts new files not yet in the DB.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use rusqlite::types::Value;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::CodeContextError;
use crate::ignore_filter::workspace_walk_builder;
use crate::lsp_worker::lsp_capable_extensions;

/// Statistics returned by [`startup_cleanup`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupStats {
    /// Files on disk that were not in the DB and have been inserted.
    pub files_added: usize,
    /// Files in the DB that no longer exist on disk and have been deleted.
    pub files_removed: usize,
    /// Files in the DB that are still on disk, but that the walk skips (for
    /// example, an ignore rule covers them), and have been deleted.
    pub files_ignored: usize,
    /// Files whose content hash changed and have been marked dirty.
    pub files_dirty: usize,
    /// Files whose content hash is unchanged.
    pub files_unchanged: usize,
    /// Walk entries that could not be read (for example, a directory with no
    /// read permission, or a file that is deleted during the walk), and files
    /// that the hash could not read for a reason other than `NotFound` (for
    /// example, a file with no read permission). The cleanup skips each one
    /// and continues, and the rows at or under their paths stay.
    pub entries_skipped: usize,
}

/// Hashed file ready for DB reconciliation.
struct HashedFile {
    /// Path relative to workspace root, stored as the DB key.
    rel_path: String,
    /// First 16 bytes of the SHA-256 digest.
    hash: Vec<u8>,
    /// File size in bytes.
    size: u64,
}

/// Walk the filesystem and reconcile with the DB.
///
/// 1. Walks non-ignored files under `workspace_root` (respects `.gitignore`).
/// 2. Hashes files in parallel with `rayon`.
/// 3. Compares against the DB:
///    - Files in DB but not on disk -> DELETE (CASCADE cleans up).
///    - Files in DB and on disk, but skipped by the walk (ignored) -> DELETE,
///      and log the count.
///    - Files on disk with a different hash -> UPDATE hash, set dirty.
///    - Files on disk not in DB -> INSERT with dirty flags.
/// 4. Returns [`CleanupStats`] summarising what changed.
///
/// A walk entry that cannot be read does not stop the walk, and a file that
/// the hash cannot read does not stop the hash. The cleanup skips it, the rows
/// at or under its path stay, and the count is in
/// [`CleanupStats::entries_skipped`]. A file that the hash finds gone
/// (`NotFound`) is not counted, and its row is deleted.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] if a SQL operation fails.
pub fn startup_cleanup(
    conn: &Connection,
    workspace_root: &Path,
) -> Result<CleanupStats, CodeContextError> {
    // 1. Walk and hash all non-ignored files in parallel
    let walked = walk_and_hash(workspace_root);
    let disk_files = &walked.files;

    // Build a lookup map: rel_path -> (hash, size)
    let disk_map: HashMap<&str, (&[u8], u64)> = disk_files
        .iter()
        .map(|f| (f.rel_path.as_str(), (f.hash.as_slice(), f.size)))
        .collect();

    // 2. Load existing DB entries
    let mut stmt = conn.prepare("SELECT file_path, content_hash FROM indexed_files")?;
    let db_entries: Vec<(String, Vec<u8>)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut stats = CleanupStats {
        entries_skipped: walked.unreadable.entries_skipped(),
        ..CleanupStats::default()
    };
    if stats.entries_skipped > 0 {
        tracing::info!(
            "code-context cleanup: skipped {} walk entries that could not be read, and kept their rows",
            stats.entries_skipped
        );
    }

    // 3a. Reconcile existing DB entries against files on disk.
    reconcile_db_entries(
        conn,
        workspace_root,
        &db_entries,
        &disk_map,
        &walked.unreadable,
        now,
        &mut stats,
    )?;
    if stats.files_ignored > 0 {
        tracing::info!(
            "code-context cleanup: removed {} rows for files that the ignore filter skips",
            stats.files_ignored
        );
    }

    // 3b. Insert files found on disk but not yet in the DB.
    let db_paths: std::collections::HashSet<&str> =
        db_entries.iter().map(|(p, _)| p.as_str()).collect();
    insert_new_disk_files(conn, disk_files, &db_paths, now, &mut stats)?;

    // 3c. Bulk-mark any existing files with non-LSP-capable extensions as lsp_indexed=1.
    // This handles files already in the DB from previous runs that were inserted before
    // this filter existed.
    mark_non_lsp_capable_files(conn)?;

    Ok(stats)
}

/// Reconcile existing DB entries against files currently on disk.
///
/// For each row in `db_entries`:
/// - If the walk did not give the file, and the file is not at or under a
///   path in `unreadable`, delete it (CASCADE cleans up chunks/symbols/edges).
///   See [`delete_unwalked_row`]. A row under an unreadable path stays as it
///   is, because a temporary read error must not delete valid rows.
/// - If the hash changed, update `content_hash`, clear both indexed flags, and touch `last_seen_at`.
/// - Otherwise just touch `last_seen_at`.
///
/// `stats` is updated in place with the counts of removed, ignored, dirty, and unchanged files.
fn reconcile_db_entries(
    conn: &Connection,
    workspace_root: &Path,
    db_entries: &[(String, Vec<u8>)],
    disk_map: &HashMap<&str, (&[u8], u64)>,
    unreadable: &UnreadablePaths,
    now: i64,
    stats: &mut CleanupStats,
) -> Result<(), CodeContextError> {
    for (db_path, db_hash) in db_entries {
        match disk_map.get(db_path.as_str()) {
            None if unreadable.covers(db_path) => {}
            None => delete_unwalked_row(conn, workspace_root, db_path, stats)?,
            Some((disk_hash, _size)) => {
                if *disk_hash != db_hash.as_slice() {
                    // Hash changed -- mark dirty
                    conn.execute(
                        "UPDATE indexed_files
                         SET content_hash = ?1, ts_indexed = 0, lsp_indexed = 0, last_seen_at = ?2
                         WHERE file_path = ?3",
                        rusqlite::params![*disk_hash, now, db_path],
                    )?;
                    stats.files_dirty += 1;
                } else {
                    // Unchanged -- just touch last_seen_at
                    conn.execute(
                        "UPDATE indexed_files SET last_seen_at = ?1 WHERE file_path = ?2",
                        rusqlite::params![now, db_path],
                    )?;
                    stats.files_unchanged += 1;
                }
            }
        }
    }
    Ok(())
}

/// Delete one row whose file the walk did not give, and count it.
///
/// CASCADE removes the chunks, symbols and edges of the row. When the file is
/// still on disk, the walk skipped it (for example, an ignore rule covers it),
/// so the row counts in `stats.files_ignored`. Otherwise the file is gone, and
/// the row counts in `stats.files_removed`.
fn delete_unwalked_row(
    conn: &Connection,
    workspace_root: &Path,
    db_path: &str,
    stats: &mut CleanupStats,
) -> Result<(), CodeContextError> {
    conn.execute("DELETE FROM indexed_files WHERE file_path = ?1", [db_path])?;
    if workspace_root.join(db_path).is_file() {
        stats.files_ignored += 1;
    } else {
        stats.files_removed += 1;
    }
    Ok(())
}

/// Insert files that exist on disk but are not yet tracked in the DB.
///
/// New rows are inserted with `ts_indexed = 0`. `lsp_indexed` is set to `1`
/// up-front when no LSP server handles the file's extension, so the LSP
/// worker never wastes time polling it; otherwise it is `0` so the worker
/// will pick the file up on its next pass.
///
/// `stats.files_added` is incremented for each inserted row.
fn insert_new_disk_files(
    conn: &Connection,
    disk_files: &[HashedFile],
    db_paths: &std::collections::HashSet<&str>,
    now: i64,
    stats: &mut CleanupStats,
) -> Result<(), CodeContextError> {
    for file in disk_files {
        if db_paths.contains(file.rel_path.as_str()) {
            continue;
        }
        let lsp_indexed = if is_lsp_capable(Path::new(&file.rel_path)) {
            0
        } else {
            1
        };
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            rusqlite::params![file.rel_path, file.hash, file.size as i64, now, lsp_indexed],
        )?;
        stats.files_added += 1;
    }
    Ok(())
}

/// File extensions that tree-sitter can parse (kept in sync with LanguageRegistry).
///
/// Only files with these extensions are indexed. This prevents wasting DB space
/// and indexing time on binary files, protobuf, images, etc.
const PARSEABLE_EXTENSIONS: &[&str] = &[
    // Rust
    "rs", // Python
    "py", "pyi", "pyw", // TypeScript / JavaScript
    "ts", "mts", "cts", "tsx", "js", "mjs", "cjs", "jsx", // Go
    "go",  // Java / Kotlin / Scala
    "java", "kt", "kts", "scala", "sc", // C / C++
    "c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "hh", // C#
    "cs", // Ruby
    "rb", "rake", "gemspec", // PHP
    "php", "phtml", // Swift
    "swift", // Dart
    "dart",  // Lua
    "lua",   // Shell
    "sh", "bash", "zsh", // Config (useful for code context)
    "toml", "yaml", "yml",
];

/// Check if a file path has a parseable extension.
fn is_parseable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| PARSEABLE_EXTENSIONS.contains(&ext))
}

/// Check if a file path has an extension that at least one LSP server can handle.
fn is_lsp_capable(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    lsp_capable_extensions().iter().any(|e| e == ext)
}

/// Bulk-mark files as `lsp_indexed = 1` when their extension has no LSP server.
///
/// Builds a SQL UPDATE that excludes all LSP-capable extensions. This ensures
/// the LSP progress percentage reflects only files that could actually be indexed.
fn mark_non_lsp_capable_files(conn: &Connection) -> Result<usize, CodeContextError> {
    let extensions = lsp_capable_extensions();
    if extensions.is_empty() {
        // If no LSP servers are known, mark everything as lsp_indexed=1
        let count = conn.execute(
            "UPDATE indexed_files SET lsp_indexed = 1 WHERE lsp_indexed = 0",
            [],
        )?;
        return Ok(count);
    }

    // Build parameterized: WHERE lsp_indexed = 0 AND NOT (file_path LIKE ?1 OR ?2 ...)
    let like_clauses: Vec<String> = (1..=extensions.len())
        .map(|i| format!("file_path LIKE ?{}", i))
        .collect();
    let filter = like_clauses.join(" OR ");

    let sql = format!(
        "UPDATE indexed_files SET lsp_indexed = 1 WHERE lsp_indexed = 0 AND NOT ({})",
        filter
    );

    let params: Vec<Value> = extensions
        .iter()
        .map(|ext| Value::Text(format!("%.{}", ext)))
        .collect();

    let count = conn.execute(&sql, rusqlite::params_from_iter(params))?;
    Ok(count)
}

/// The paths that the workspace walk could not read.
///
/// The walk gives an error entry for a directory it cannot read (permission
/// denied), for a file that is deleted between the list and the stat, and for
/// a symlink loop. The walk records each such path here and continues. The
/// hash also records here each file that it cannot read for a reason other
/// than `NotFound` (see [`record_read_error`]). The reconcile keeps each row
/// at or under one of these paths, because the cleanup does not know if the
/// file of the row is still there or what it holds.
#[derive(Debug, Default)]
struct UnreadablePaths {
    /// Paths relative to the workspace root. An empty path is the root
    /// itself: it covers each row.
    paths: Vec<PathBuf>,
    /// The count of error entries, also when two entries have one path.
    entries_skipped: usize,
}

impl UnreadablePaths {
    /// Record the path of one error entry of the walk, and count the entry.
    ///
    /// An error with no path, or with a path outside `workspace_root`, records
    /// the root, so that the reconcile deletes no row. Returns `true` when the
    /// path was not recorded before.
    fn record(&mut self, workspace_root: &Path, err: &ignore::Error) -> bool {
        self.entries_skipped += 1;
        let rel = walk_error_path(err)
            .and_then(|path| path.strip_prefix(workspace_root).ok())
            .map(Path::to_path_buf)
            .unwrap_or_default();
        if self.paths.contains(&rel) {
            return false;
        }
        self.paths.push(rel);
        true
    }

    /// Tell if `rel_path` (relative to the workspace root) is at or under a
    /// path that the walk could not read.
    fn covers(&self, rel_path: &str) -> bool {
        let rel_path = Path::new(rel_path);
        self.paths.iter().any(|path| rel_path.starts_with(path))
    }

    /// The count of error entries that the walk skipped.
    fn entries_skipped(&self) -> usize {
        self.entries_skipped
    }
}

/// Find the path that one walk error is about, if the error has one.
fn walk_error_path(err: &ignore::Error) -> Option<&Path> {
    match err {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::Loop { child, .. } => Some(child),
        ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
            walk_error_path(err)
        }
        ignore::Error::Partial(errs) => errs.iter().find_map(walk_error_path),
        _ => None,
    }
}

/// Log one error entry of the walk.
///
/// A permission error goes to `warn` the first time the walk records its path
/// (`is_new`). Each other error goes to `debug`, because files are made and
/// deleted all the time during a build.
fn log_walk_error(err: &ignore::Error, is_new: bool) {
    let denied = err
        .io_error()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
    if denied && is_new {
        tracing::warn!("code-context cleanup: cannot read, skipped: {}", err);
    } else {
        tracing::debug!("code-context cleanup: walk entry skipped: {}", err);
    }
}

/// The result of one workspace walk.
struct WalkedWorkspace {
    /// The files that the walk read and hashed.
    files: Vec<HashedFile>,
    /// The paths that the walk could not read.
    unreadable: UnreadablePaths,
}

/// Walk non-ignored files and hash them in parallel.
///
/// Uses [`workspace_walk_builder`], which obeys the same ignore rules as the
/// file watchers ([`crate::IgnoreFilter`]), and `rayon` for parallel hashing.
/// Only includes files with extensions that tree-sitter can parse. An error
/// entry does not stop the walk, and a file that the hash cannot read does
/// not stop the hash: their paths go into [`WalkedWorkspace::unreadable`].
fn walk_and_hash(workspace_root: &Path) -> WalkedWorkspace {
    let (paths, mut unreadable) = collect_parseable_paths(workspace_root);
    let files = hash_files(workspace_root, &paths, &mut unreadable);
    WalkedWorkspace { files, unreadable }
}

/// Walk the workspace and collect the paths of the parseable files.
///
/// The paths are collected before the hash, because `WalkBuilder` is not
/// `Send`. Each error entry is logged and recorded, and the walk continues.
fn collect_parseable_paths(workspace_root: &Path) -> (Vec<PathBuf>, UnreadablePaths) {
    let mut paths = Vec::new();
    let mut unreadable = UnreadablePaths::default();
    for entry in workspace_walk_builder(workspace_root).build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                let is_new = unreadable.record(workspace_root, &err);
                log_walk_error(&err, is_new);
                continue;
            }
        };
        if entry.file_type().is_some_and(|ft| ft.is_file()) && is_parseable(entry.path()) {
            paths.push(entry.into_path());
        }
    }
    (paths, unreadable)
}

/// Hash `paths` in parallel.
///
/// Returns one [`HashedFile`] with a relative path and a truncated SHA-256
/// hash for each file that can be read. Each file that cannot be read goes to
/// [`record_read_error`], so that a temporary read error does not delete the
/// row of the file.
fn hash_files(
    workspace_root: &Path,
    paths: &[PathBuf],
    unreadable: &mut UnreadablePaths,
) -> Vec<HashedFile> {
    let results: Vec<Result<HashedFile, ignore::Error>> = paths
        .par_iter()
        .map(|path| hash_file(workspace_root, path))
        .collect();

    let mut files = Vec::with_capacity(results.len());
    for result in results {
        match result {
            Ok(file) => files.push(file),
            Err(err) => record_read_error(unreadable, workspace_root, &err),
        }
    }
    files
}

/// Read and hash one file.
///
/// A read error is logged, and it comes back as an [`ignore::Error::WithPath`]
/// for `path`, the same shape as an error entry of the walk.
fn hash_file(workspace_root: &Path, path: &Path) -> Result<HashedFile, ignore::Error> {
    let contents = fs::read(path).map_err(|e| {
        tracing::warn!("Failed to read {}: {}", path.display(), e);
        ignore::Error::WithPath {
            path: path.to_path_buf(),
            err: Box::new(ignore::Error::Io(e)),
        }
    })?;
    let size = contents.len() as u64;

    let mut hasher = Sha256::new();
    hasher.update(&contents);
    let digest = hasher.finalize();
    // Store first 16 bytes of SHA-256
    let hash = digest[..16].to_vec();

    let rel_path = match path.strip_prefix(workspace_root) {
        Ok(rel) => rel.to_string_lossy().to_string(),
        Err(_) => path.to_string_lossy().to_string(),
    };

    Ok(HashedFile {
        rel_path,
        hash,
        size,
    })
}

/// Record one file that the hash could not read.
///
/// A `NotFound` error is a file that is deleted after the walk. It records
/// nothing, so that the reconcile deletes the row. Each other error (for
/// example, permission denied) records the path in `unreadable` and counts
/// it, so that the reconcile keeps the row.
fn record_read_error(unreadable: &mut UnreadablePaths, workspace_root: &Path, err: &ignore::Error) {
    let gone = err
        .io_error()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound);
    if !gone {
        unreadable.record(workspace_root, err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn open_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::configure_connection(&conn).unwrap();
        db::create_schema(&conn).unwrap();
        conn
    }

    /// Hash helper matching the function used in production code.
    fn hash_bytes(data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let digest = hasher.finalize();
        digest[..16].to_vec()
    }

    #[test]
    fn test_startup_cleanup_add_remove_dirty() {
        let dir = tempfile::tempdir().unwrap();

        // Create 3 files
        fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        fs::write(dir.path().join("b.rs"), "fn b() {}").unwrap();
        fs::write(dir.path().join("c.rs"), "fn c() {}").unwrap();

        let conn = open_memory_db();

        // First cleanup: all 3 files should be added
        let stats = startup_cleanup(&conn, dir.path()).unwrap();
        assert_eq!(stats.files_added, 3);
        assert_eq!(stats.files_removed, 0);
        assert_eq!(stats.files_dirty, 0);
        assert_eq!(stats.files_unchanged, 0);

        // Delete b.rs, modify c.rs
        fs::remove_file(dir.path().join("b.rs")).unwrap();
        fs::write(dir.path().join("c.rs"), "fn c_modified() {}").unwrap();

        // Second cleanup
        let stats = startup_cleanup(&conn, dir.path()).unwrap();
        assert_eq!(stats.files_removed, 1, "b.rs should be removed");
        assert_eq!(stats.files_dirty, 1, "c.rs should be dirty");
        assert_eq!(stats.files_unchanged, 1, "a.rs should be unchanged");
        assert_eq!(stats.files_added, 0, "no new files");

        // Verify b.rs is gone from DB
        let b_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE file_path = 'b.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(b_count, 0);

        // Verify c.rs is dirty
        let (ts, lsp): (i64, i64) = conn
            .query_row(
                "SELECT ts_indexed, lsp_indexed FROM indexed_files WHERE file_path = 'c.rs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(ts, 0);
        assert_eq!(lsp, 0);

        // Verify c.rs has the new hash
        let stored_hash: Vec<u8> = conn
            .query_row(
                "SELECT content_hash FROM indexed_files WHERE file_path = 'c.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expected_hash = hash_bytes(b"fn c_modified() {}");
        assert_eq!(stored_hash, expected_hash);
    }

    #[test]
    fn test_cascade_propagation_via_cleanup() {
        let dir = tempfile::tempdir().unwrap();

        // Create a file and run cleanup to populate DB
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let conn = open_memory_db();
        startup_cleanup(&conn, dir.path()).unwrap();

        // Seed chunks/symbols/edges on this file
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES ('main.rs', 0, 100, 1, 10, 'fn main() {}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:main.rs:main', 'main', 12, 'main.rs', 1, 0, 10, 1)",
            [],
        )
        .unwrap();

        // Need a second file/symbol for edge target
        fs::write(dir.path().join("lib.rs"), "fn init() {}").unwrap();
        startup_cleanup(&conn, dir.path()).unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:lib.rs:init', 'init', 12, 'lib.rs', 1, 0, 5, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, from_ranges)
             VALUES ('lsp:main.rs:main', 'lsp:lib.rs:init', 'main.rs', 'lib.rs', '[]')",
            [],
        )
        .unwrap();

        // Delete main.rs from disk
        fs::remove_file(dir.path().join("main.rs")).unwrap();

        // Run cleanup -- should delete main.rs and CASCADE
        let stats = startup_cleanup(&conn, dir.path()).unwrap();
        assert_eq!(stats.files_removed, 1);

        // Chunks should be gone
        let chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(chunks, 0);

        // Symbols for main.rs should be gone
        let syms: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(syms, 0);

        // Edges should be gone
        let edges: i64 = conn
            .query_row("SELECT COUNT(*) FROM lsp_call_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edges, 0);

        // lib.rs symbol still exists
        let lib_syms: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lib_syms, 1);
    }

    #[test]
    fn test_cleanup_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_memory_db();

        let stats = startup_cleanup(&conn, dir.path()).unwrap();
        assert_eq!(stats, CleanupStats::default());
    }

    /// The walk obeys the shared ignore filter: a hard skip directory and a
    /// `.gitignore` entry add no row, also in a workspace with no `.git`.
    #[test]
    fn test_cleanup_skips_hard_skip_dirs_and_gitignored_paths() {
        let dir = tempfile::tempdir().unwrap();
        for rel in ["target/debug/a.rs", "node_modules/pkg/b.js", "gen/c.rs"] {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "fn x() {}").unwrap();
        }
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join(".gitignore"), "gen/\n").unwrap();

        let conn = open_memory_db();
        let stats = startup_cleanup(&conn, dir.path()).unwrap();

        assert_eq!(stats.files_added, 1, "only src/main.rs is indexed");
        let paths: Vec<String> = conn
            .prepare("SELECT file_path FROM indexed_files")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(paths, vec!["src/main.rs".to_string()]);
    }

    /// Make a file and its parent directories under `root`.
    fn write_file(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
    }

    /// Insert one `indexed_files` row, as an earlier run with a weaker
    /// filter left it.
    fn seed_row(conn: &Connection, rel: &str, contents: &str) {
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES (?1, ?2, ?3, 0, 1, 0)",
            rusqlite::params![rel, hash_bytes(contents.as_bytes()), contents.len() as i64],
        )
        .unwrap();
    }

    /// Insert one `indexed_files` row for each `(rel, contents)` pair, and
    /// give the same rows in the shape that [`reconcile_db_entries`] takes.
    fn seed_rows(conn: &Connection, rows: &[(&str, &str)]) -> Vec<(String, Vec<u8>)> {
        rows.iter()
            .map(|(rel, contents)| {
                seed_row(conn, rel, contents);
                (rel.to_string(), hash_bytes(contents.as_bytes()))
            })
            .collect()
    }

    /// Tell if `indexed_files` has a row for `rel`.
    fn has_row(conn: &Connection, rel: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM indexed_files WHERE file_path = ?1",
            [rel],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    }

    /// A row for a file on disk that the ignore filter skips is deleted, and
    /// the count of these rows is in `files_ignored`, not in `files_removed`.
    #[test]
    fn test_cleanup_removes_ignored_row_for_file_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let ignored = ".build/checkouts/X/a.swift";
        let kept = "Sources/b.swift";
        fs::write(dir.path().join(".gitignore"), ".build/\n").unwrap();
        write_file(dir.path(), ignored, "let a = 1");
        write_file(dir.path(), kept, "let b = 2");

        let conn = open_memory_db();
        seed_row(&conn, ignored, "let a = 1");
        seed_row(&conn, kept, "let b = 2");

        let stats = startup_cleanup(&conn, dir.path()).unwrap();

        assert!(!has_row(&conn, ignored), "the ignored row must be deleted");
        assert!(has_row(&conn, kept), "the source row must stay");
        assert_eq!(stats.files_ignored, 1);
        assert_eq!(stats.files_removed, 0);
        assert_eq!(stats.files_unchanged, 1);
    }

    /// A `.gitignore` directory rule deletes the row of each file under that
    /// directory, also when the file is on disk.
    #[test]
    fn test_cleanup_removes_row_under_gitignored_directory_rule() {
        let dir = tempfile::tempdir().unwrap();
        let ignored = "gen-out/nested/x.rs";
        fs::write(dir.path().join(".gitignore"), "gen-out/\n").unwrap();
        write_file(dir.path(), ignored, "fn x() {}");
        write_file(dir.path(), "src/lib.rs", "fn lib() {}");

        let conn = open_memory_db();
        seed_row(&conn, ignored, "fn x() {}");

        let stats = startup_cleanup(&conn, dir.path()).unwrap();

        assert!(!has_row(&conn, ignored), "the ignored row must be deleted");
        assert!(has_row(&conn, "src/lib.rs"), "the source file is added");
        assert_eq!(stats.files_ignored, 1);
        assert_eq!(stats.files_removed, 0);
        assert_eq!(stats.files_added, 1);
    }

    /// The mode that makes a directory or a file unreadable for a user that
    /// is not root.
    #[cfg(unix)]
    const UNREADABLE_DIR_MODE: u32 = 0o000;

    /// The mode that makes a directory or a file readable again, so the temp
    /// dir can be removed.
    #[cfg(unix)]
    const READABLE_DIR_MODE: u32 = 0o755;

    /// Gives the mode [`READABLE_DIR_MODE`] back to a directory or a file when
    /// it drops.
    #[cfg(unix)]
    struct RestoreDirMode {
        /// The directory or file to make readable again.
        path: std::path::PathBuf,
    }

    #[cfg(unix)]
    impl Drop for RestoreDirMode {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::Permissions::from_mode(READABLE_DIR_MODE);
            if let Err(e) = fs::set_permissions(&self.path, mode) {
                eprintln!("cannot restore the mode of {}: {e}", self.path.display());
            }
        }
    }

    /// A directory or a file with the mode [`UNREADABLE_DIR_MODE`], and what
    /// that mode does to the process.
    #[cfg(unix)]
    struct LockedDir {
        /// Gives the readable mode back when the test ends.
        _restore: RestoreDirMode,
        /// `true` when the mode stops the read. It is `false` when the process
        /// runs as root, because root reads a directory or a file with any
        /// mode.
        denied: bool,
    }

    /// Give the directory `rel` under `root` the mode [`UNREADABLE_DIR_MODE`].
    ///
    /// The result tells if the mode stops the read, so that a test asserts
    /// the correct result for a process that is root and for one that is not.
    #[cfg(unix)]
    fn make_unreadable_dir(root: &Path, rel: &str) -> LockedDir {
        lock_path(root, rel, |path| fs::read_dir(path).is_ok())
    }

    /// Give the file `rel` under `root` the mode [`UNREADABLE_DIR_MODE`].
    ///
    /// The result tells if the mode stops the read, so that a test asserts
    /// the correct result for a process that is root and for one that is not.
    #[cfg(unix)]
    fn make_unreadable_file(root: &Path, rel: &str) -> LockedDir {
        lock_path(root, rel, |path| fs::read(path).is_ok())
    }

    /// Give the directory or file `rel` under `root` the mode
    /// [`UNREADABLE_DIR_MODE`], and use `can_read` to tell if the mode stops
    /// the read.
    #[cfg(unix)]
    fn lock_path(root: &Path, rel: &str, can_read: fn(&Path) -> bool) -> LockedDir {
        use std::os::unix::fs::PermissionsExt;
        let path = root.join(rel);
        fs::set_permissions(&path, fs::Permissions::from_mode(UNREADABLE_DIR_MODE)).unwrap();
        let denied = !can_read(&path);
        LockedDir {
            _restore: RestoreDirMode { path },
            denied,
        }
    }

    /// The count of entries that a directory or a file with the mode
    /// [`UNREADABLE_DIR_MODE`] makes the cleanup skip: one when the mode stops
    /// the read, else zero.
    #[cfg(unix)]
    fn expected_skipped_entries(locked: &LockedDir) -> usize {
        usize::from(locked.denied)
    }

    /// An unreadable directory does not stop the walk: the cleanup gives `Ok`
    /// and adds the files in the other directories. When the process runs as
    /// root, the walk reads the directory and also adds its file.
    #[cfg(unix)]
    #[test]
    fn test_cleanup_walk_continues_past_unreadable_dir() {
        let dir = tempfile::tempdir().unwrap();
        let locked_file = "locked/b.rs";
        write_file(dir.path(), "src/a.rs", "fn a() {}");
        write_file(dir.path(), locked_file, "fn b() {}");
        let locked = make_unreadable_dir(dir.path(), "locked");

        let conn = open_memory_db();
        let stats = startup_cleanup(&conn, dir.path())
            .expect("an unreadable directory must not stop the walk");

        assert!(has_row(&conn, "src/a.rs"), "the readable file is added");
        assert_eq!(
            has_row(&conn, locked_file),
            !locked.denied,
            "the file under the directory is added only when the walk can read it"
        );
        assert_eq!(
            stats.files_added,
            1 + usize::from(!locked.denied),
            "each file that the walk can read is added"
        );
        assert_eq!(
            stats.entries_skipped,
            expected_skipped_entries(&locked),
            "the unreadable directory is counted"
        );
    }

    /// Make the walk error that the walker gives for `path` when the read of
    /// `path` fails with `kind`. For example, `NotFound` is a file that is
    /// deleted between the list and the stat, and `PermissionDenied` is a
    /// directory with no read permission.
    fn walk_error(path: PathBuf, kind: std::io::ErrorKind) -> ignore::Error {
        ignore::Error::WithDepth {
            depth: 1,
            err: Box::new(ignore::Error::WithPath {
                path,
                err: Box::new(ignore::Error::Io(std::io::Error::from(kind))),
            }),
        }
    }

    /// Make the walk error that the walker gives for a file that is deleted
    /// between the list and the stat.
    fn vanished_file_error(path: PathBuf) -> ignore::Error {
        walk_error(path, std::io::ErrorKind::NotFound)
    }

    /// The reconcile keeps each row under a directory that the walk could not
    /// read, but it still deletes the row of a file that is gone. The walk
    /// error is made in the test, so the assertions run for each user, also
    /// for root.
    #[test]
    fn test_reconcile_keeps_rows_under_denied_dir_and_deletes_gone_row() {
        let dir = tempfile::tempdir().unwrap();
        let locked = "locked/nested/b.rs";
        let readable = "src/a.rs";
        write_file(dir.path(), locked, "fn b() {}");
        write_file(dir.path(), readable, "fn a() {}");
        let rows = [
            (locked, "fn b() {}"),
            (readable, "fn a() {}"),
            ("gone.rs", "fn gone() {}"),
        ];
        let conn = open_memory_db();
        let db_entries = seed_rows(&conn, &rows);
        let readable_hash = hash_bytes(b"fn a() {}");
        let disk_map = HashMap::from([(readable, (readable_hash.as_slice(), 0))]);
        let mut unreadable = UnreadablePaths::default();
        let denied = walk_error(
            dir.path().join("locked"),
            std::io::ErrorKind::PermissionDenied,
        );
        unreadable.record(dir.path(), &denied);
        let mut stats = CleanupStats::default();

        reconcile_db_entries(
            &conn,
            dir.path(),
            &db_entries,
            &disk_map,
            &unreadable,
            0,
            &mut stats,
        )
        .unwrap();

        assert!(
            has_row(&conn, locked),
            "the row under the unreadable directory must stay"
        );
        assert!(has_row(&conn, readable), "the walked row stays");
        assert!(
            !has_row(&conn, "gone.rs"),
            "the row for the gone file is deleted"
        );
        assert_eq!(stats.files_removed, 1);
        assert_eq!(stats.files_ignored, 0);
        assert_eq!(stats.files_unchanged, 1);
    }

    /// A file that is deleted during the walk is recorded and counted. Only
    /// its own row is kept; a row beside it is not covered.
    #[test]
    fn test_vanished_file_during_walk_covers_only_its_own_row() {
        let root = Path::new("/workspace");
        let mut unreadable = UnreadablePaths::default();

        let is_new = unreadable.record(root, &vanished_file_error(root.join("src/gone.rs")));

        assert!(is_new);
        assert!(unreadable.covers("src/gone.rs"));
        assert!(!unreadable.covers("src/kept.rs"));
        assert_eq!(unreadable.entries_skipped(), 1);
    }

    /// A read error of the hash that is not `NotFound` records the file and
    /// counts it, so that the reconcile keeps its row. A row for a file that
    /// is gone is still deleted. The read error is made in the test, so the
    /// assertions run for each user, also for root.
    #[test]
    fn test_reconcile_keeps_row_of_file_with_denied_read() {
        let dir = tempfile::tempdir().unwrap();
        let locked = "src/locked.rs";
        let rows = [(locked, "fn locked() {}"), ("gone.rs", "fn gone() {}")];
        let conn = open_memory_db();
        let db_entries = seed_rows(&conn, &rows);
        let mut unreadable = UnreadablePaths::default();
        let denied = walk_error(
            dir.path().join(locked),
            std::io::ErrorKind::PermissionDenied,
        );
        record_read_error(&mut unreadable, dir.path(), &denied);
        let mut stats = CleanupStats::default();

        reconcile_db_entries(
            &conn,
            dir.path(),
            &db_entries,
            &HashMap::new(),
            &unreadable,
            0,
            &mut stats,
        )
        .unwrap();

        assert!(
            has_row(&conn, locked),
            "the row of the file that the hash could not read must stay"
        );
        assert!(
            !has_row(&conn, "gone.rs"),
            "the row for the gone file is deleted"
        );
        assert_eq!(unreadable.entries_skipped(), 1);
        assert_eq!(stats.files_removed, 1);
    }

    /// A `NotFound` read error of the hash is a file that is gone. It records
    /// no path and counts no entry, so that the reconcile deletes the row.
    #[test]
    fn test_not_found_read_error_records_nothing() {
        let root = Path::new("/workspace");
        let mut unreadable = UnreadablePaths::default();

        record_read_error(
            &mut unreadable,
            root,
            &vanished_file_error(root.join("src/gone.rs")),
        );

        assert!(!unreadable.covers("src/gone.rs"));
        assert_eq!(unreadable.entries_skipped(), 0);
    }

    /// The same path two times is one recorded path, but two skipped entries.
    #[test]
    fn test_walk_error_for_same_path_is_new_only_once() {
        let root = Path::new("/workspace");
        let mut unreadable = UnreadablePaths::default();

        assert!(unreadable.record(root, &vanished_file_error(root.join("a.rs"))));
        assert!(!unreadable.record(root, &vanished_file_error(root.join("a.rs"))));
        assert_eq!(unreadable.entries_skipped(), 2);
    }

    /// A walk error with no path covers each row, so that the reconcile
    /// deletes no row when it does not know what the walk missed.
    #[test]
    fn test_walk_error_with_no_path_covers_each_row() {
        let root = Path::new("/workspace");
        let mut unreadable = UnreadablePaths::default();
        let err = ignore::Error::Io(std::io::Error::from(std::io::ErrorKind::Other));

        unreadable.record(root, &err);

        assert!(unreadable.covers("src/lib.rs"));
        assert!(unreadable.covers("README.toml"));
    }

    /// A row under a directory with the mode [`UNREADABLE_DIR_MODE`] stays,
    /// but a row for a file that is gone is still deleted. When the mode
    /// stops the read, the reconcile keeps the row because the walk recorded
    /// the directory. When the process runs as root, the walk reads the
    /// directory, and the row stays because its file is unchanged.
    #[cfg(unix)]
    #[test]
    fn test_cleanup_keeps_rows_under_unreadable_dir() {
        let dir = tempfile::tempdir().unwrap();
        let locked_file = "locked/nested/b.rs";
        write_file(dir.path(), "src/a.rs", "fn a() {}");
        write_file(dir.path(), locked_file, "fn b() {}");
        let conn = open_memory_db();
        seed_row(&conn, locked_file, "fn b() {}");
        seed_row(&conn, "gone.rs", "fn gone() {}");
        let locked = make_unreadable_dir(dir.path(), "locked");

        let stats = startup_cleanup(&conn, dir.path()).unwrap();

        assert_locked_row_kept(&conn, &stats, locked_file, &locked);
    }

    /// Assert the result of a cleanup that had a seeded row for `locked_row`,
    /// whose path has the mode [`UNREADABLE_DIR_MODE`], and a seeded row for
    /// the gone file `gone.rs`. The locked row stays, the gone row is
    /// deleted, and the locked row is compared with its file only when the
    /// mode does not stop the read.
    #[cfg(unix)]
    fn assert_locked_row_kept(
        conn: &Connection,
        stats: &CleanupStats,
        locked_row: &str,
        locked: &LockedDir,
    ) {
        assert!(
            has_row(conn, locked_row),
            "the row of the unreadable path must stay"
        );
        assert!(
            !has_row(conn, "gone.rs"),
            "the row for the gone file is deleted"
        );
        assert_eq!(stats.files_removed, 1);
        assert_eq!(stats.files_ignored, 0);
        assert_eq!(
            stats.files_unchanged,
            usize::from(!locked.denied),
            "the row is compared with its file only when the cleanup can read it"
        );
        assert_eq!(stats.entries_skipped, expected_skipped_entries(locked));
    }

    /// The row of a file with the mode [`UNREADABLE_DIR_MODE`] stays, but a
    /// row for a file that is gone is still deleted. When the mode stops the
    /// read, the hash records the file and counts it. When the process runs
    /// as root, the hash reads the file, and the row stays because its file
    /// is unchanged.
    #[cfg(unix)]
    #[test]
    fn test_cleanup_keeps_row_of_unreadable_file() {
        let dir = tempfile::tempdir().unwrap();
        let locked_file = "src/locked.rs";
        write_file(dir.path(), locked_file, "fn locked() {}");
        let conn = open_memory_db();
        seed_row(&conn, locked_file, "fn locked() {}");
        seed_row(&conn, "gone.rs", "fn gone() {}");
        let locked = make_unreadable_file(dir.path(), locked_file);

        let stats = startup_cleanup(&conn, dir.path()).unwrap();

        assert_locked_row_kept(&conn, &stats, locked_file, &locked);
    }

    #[test]
    fn test_cleanup_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();

        let conn = open_memory_db();

        let stats1 = startup_cleanup(&conn, dir.path()).unwrap();
        assert_eq!(stats1.files_added, 1);

        // Run again without changes -- should be all unchanged
        let stats2 = startup_cleanup(&conn, dir.path()).unwrap();
        assert_eq!(stats2.files_unchanged, 1);
        assert_eq!(stats2.files_added, 0);
        assert_eq!(stats2.files_removed, 0);
        assert_eq!(stats2.files_dirty, 0);
    }
}
