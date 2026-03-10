//! Startup stale entry cleanup for the code context index.
//!
//! Runs once at startup (leader only) before indexers begin. Walks the
//! filesystem, hashes every non-ignored file, and reconciles the database:
//!
//! - Deletes DB entries for files that no longer exist on disk.
//! - Marks files dirty whose `content_hash` changed.
//! - Inserts new files not yet in the DB.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ignore::WalkBuilder;
use rayon::prelude::*;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::CodeContextError;

/// Statistics returned by [`startup_cleanup`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupStats {
    /// Files on disk that were not in the DB and have been inserted.
    pub files_added: usize,
    /// Files in the DB that no longer exist on disk and have been deleted.
    pub files_removed: usize,
    /// Files whose content hash changed and have been marked dirty.
    pub files_dirty: usize,
    /// Files whose content hash is unchanged.
    pub files_unchanged: usize,
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
///    - Files on disk with a different hash -> UPDATE hash, set dirty.
///    - Files on disk not in DB -> INSERT with dirty flags.
/// 4. Returns [`CleanupStats`] summarising what changed.
///
/// # Errors
///
/// Returns [`CodeContextError::Io`] if filesystem walking fails, or
/// [`CodeContextError::Database`] if a SQL operation fails.
pub fn startup_cleanup(
    conn: &Connection,
    workspace_root: &Path,
) -> Result<CleanupStats, CodeContextError> {
    // 1. Walk and hash all non-ignored files in parallel
    let disk_files = walk_and_hash(workspace_root)?;

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

    let mut stats = CleanupStats::default();

    // 3a. Check DB entries against disk
    for (db_path, db_hash) in &db_entries {
        match disk_map.get(db_path.as_str()) {
            None => {
                // File no longer on disk -- delete (CASCADE)
                conn.execute("DELETE FROM indexed_files WHERE file_path = ?1", [db_path])?;
                stats.files_removed += 1;
            }
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

    // 3b. Insert new files not yet in the DB
    let db_paths: std::collections::HashSet<&str> =
        db_entries.iter().map(|(p, _)| p.as_str()).collect();

    for file in &disk_files {
        if !db_paths.contains(file.rel_path.as_str()) {
            conn.execute(
                "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
                 VALUES (?1, ?2, ?3, ?4, 0, 0)",
                rusqlite::params![file.rel_path, file.hash, file.size as i64, now],
            )?;
            stats.files_added += 1;
        }
    }

    Ok(stats)
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

/// Walk non-ignored files and hash them in parallel.
///
/// Uses the `ignore` crate (respects `.gitignore`) and `rayon` for parallel hashing.
/// Only includes files with extensions that tree-sitter can parse.
/// Returns a `Vec<HashedFile>` with relative paths and truncated SHA-256 hashes.
fn walk_and_hash(workspace_root: &Path) -> Result<Vec<HashedFile>, CodeContextError> {
    // Collect file paths first (WalkBuilder is not Send)
    let mut paths = Vec::new();
    for entry in WalkBuilder::new(workspace_root)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build()
    {
        let entry = entry.map_err(std::io::Error::other)?;
        if entry.file_type().is_some_and(|ft| ft.is_file()) && is_parseable(entry.path()) {
            paths.push(entry.into_path());
        }
    }

    // Hash in parallel
    let workspace_root_owned = workspace_root.to_path_buf();
    let results: Vec<Option<HashedFile>> = paths
        .par_iter()
        .map(|path| {
            let contents = match fs::read(path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Failed to read {}: {}", path.display(), e);
                    return None;
                }
            };
            let size = contents.len() as u64;

            let mut hasher = Sha256::new();
            hasher.update(&contents);
            let digest = hasher.finalize();
            // Store first 16 bytes of SHA-256
            let hash = digest[..16].to_vec();

            let rel_path = match path.strip_prefix(&workspace_root_owned) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => path.to_string_lossy().to_string(),
            };

            Some(HashedFile {
                rel_path,
                hash,
                size,
            })
        })
        .collect();

    Ok(results.into_iter().flatten().collect())
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
