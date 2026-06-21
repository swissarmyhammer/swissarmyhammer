//! Per-task embedding cache backed by a SQLite sidecar in the board dir.
//!
//! kanban is file-based (no SQLite) everywhere else, so this is new infra
//! whose sole purpose is to let the `search tasks` op skip re-embedding
//! unchanged tasks. It is intentionally self-healing: a model or dimension
//! change invalidates every cached vector, and an absent file (fresh clone on
//! another machine) is recreated and repopulated on demand. The cache is
//! gitignored derived data — never committed — so a machine-local,
//! non-portable hash is acceptable for `content_hash`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

/// Compose the canonical text that is BOTH hashed and embedded for a task.
///
/// Hashing and embedding must never drift, so the composition lives in exactly
/// one place. Tags are deliberately excluded: they are a lexical-only search
/// field, so a tag-only edit must not change the hash or force a re-embed.
pub fn task_embedding_text(title: &str, description: &str) -> String {
    format!("{title}\n{description}")
}

/// Stable digest of the embed text, used as the cache key alongside `task_id`.
///
/// The cache is gitignored and rebuilt per machine, so a process-local,
/// non-portable hash (`DefaultHasher`) is sufficient — portability across
/// machines is explicitly not required.
pub fn content_hash(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// SQLite sidecar caching at most one current embedding vector per task.
pub struct EmbeddingCache {
    conn: Connection,
}

impl EmbeddingCache {
    /// Open (creating if absent) the cache at `path` in WAL mode.
    ///
    /// WAL keeps concurrent readers (GUI, MCP server, CLI) from blocking the
    /// writer. The schema is created when missing, then the stored model
    /// name and dimension are compared against the current embedder's; any
    /// mismatch clears every cached vector (self-healing invalidation) so a
    /// model swap can never serve stale, dimension-incompatible vectors.
    pub fn open(path: impl AsRef<Path>, model_name: &str, dim: usize) -> rusqlite::Result<Self> {
        let conn = Connection::open(path.as_ref())?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS embeddings (
                 task_id TEXT NOT NULL,
                 content_hash TEXT NOT NULL,
                 vector BLOB NOT NULL,
                 PRIMARY KEY(task_id, content_hash)
             );
             CREATE TABLE IF NOT EXISTS meta (
                 key TEXT PRIMARY KEY,
                 value TEXT
             );",
        )?;

        let cache = Self { conn };
        cache.invalidate_on_mismatch(model_name, dim)?;
        Ok(cache)
    }

    /// Clear cached vectors and rewrite meta when the model or dim changed.
    ///
    /// Reads the stored `model_name`/`dim`; if either is absent or differs
    /// from the current values, every embedding row is dropped and the meta
    /// rows are upserted to the current values. When both match, embeddings
    /// are left intact so a normal reopen preserves the cache.
    fn invalidate_on_mismatch(&self, model_name: &str, dim: usize) -> rusqlite::Result<()> {
        let stored_model: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'model_name'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let stored_dim: Option<String> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'dim'", [], |row| {
                row.get(0)
            })
            .optional()?;

        let dim_str = dim.to_string();
        let matches =
            stored_model.as_deref() == Some(model_name) && stored_dim.as_deref() == Some(&dim_str);
        if !matches {
            self.conn.execute("DELETE FROM embeddings", [])?;
            self.conn.execute(
                "INSERT INTO meta (key, value) VALUES ('model_name', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![model_name],
            )?;
            self.conn.execute(
                "INSERT INTO meta (key, value) VALUES ('dim', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![dim_str],
            )?;
        }
        Ok(())
    }

    /// Return the cached vector for `(task_id, content_hash)`, or `None`.
    ///
    /// A miss — including any query error — yields `None` so callers treat a
    /// degraded cache as an empty one and simply re-embed.
    pub fn get(&self, task_id: &str, content_hash: &str) -> Option<Vec<f32>> {
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT vector FROM embeddings WHERE task_id = ?1 AND content_hash = ?2",
                params![task_id, content_hash],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .ok()
            .flatten();
        blob.map(|b| swissarmyhammer_search::deserialize_embedding(&b))
    }

    /// Upsert the vector for a task, evicting any prior hash for that task.
    ///
    /// Deleting other rows for the same `task_id` keeps exactly one current
    /// vector per task; without it every edit would leave a dead row and the
    /// cache would grow unbounded.
    pub fn put(&self, task_id: &str, content_hash: &str, vector: &[f32]) -> rusqlite::Result<()> {
        let blob = swissarmyhammer_search::serialize_embedding(vector);
        self.conn.execute(
            "INSERT INTO embeddings (task_id, content_hash, vector) VALUES (?1, ?2, ?3)
             ON CONFLICT(task_id, content_hash) DO UPDATE SET vector = excluded.vector",
            params![task_id, content_hash, blob],
        )?;
        self.conn.execute(
            "DELETE FROM embeddings WHERE task_id = ?1 AND content_hash != ?2",
            params![task_id, content_hash],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn cache_path(temp: &TempDir) -> std::path::PathBuf {
        temp.path().join("search-cache.sqlite3")
    }

    #[test]
    fn put_get_round_trip_preserves_vector() {
        let temp = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(cache_path(&temp), "m1", 4).unwrap();
        let vector = vec![0.1, -0.2, 3.5, 4.0];
        cache.put("task-1", "hashA", &vector).unwrap();
        assert_eq!(cache.get("task-1", "hashA"), Some(vector));
    }

    #[test]
    fn get_miss_returns_none() {
        let temp = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(cache_path(&temp), "m1", 4).unwrap();
        assert_eq!(cache.get("nope", "nope"), None);
    }

    #[test]
    fn put_prunes_prior_rows_for_same_task() {
        let temp = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(cache_path(&temp), "m1", 4).unwrap();
        cache.put("task-1", "hashA", &[1.0, 2.0, 3.0, 4.0]).unwrap();
        cache.put("task-1", "hashB", &[5.0, 6.0, 7.0, 8.0]).unwrap();
        assert_eq!(cache.get("task-1", "hashA"), None);
        assert_eq!(cache.get("task-1", "hashB"), Some(vec![5.0, 6.0, 7.0, 8.0]));
    }

    #[test]
    fn task_embedding_text_excludes_tags_and_hash_is_stable() {
        let text = task_embedding_text("title", "description");
        assert_eq!(text, "title\ndescription");
        assert!(!text.contains("urgent"), "tags must not be in embed text");

        // Stable for equal input, different for different input.
        assert_eq!(content_hash(&text), content_hash("title\ndescription"));
        assert_ne!(content_hash(&text), content_hash("title\nother"));
    }

    #[test]
    fn model_name_mismatch_clears_rows() {
        let temp = TempDir::new().unwrap();
        let path = cache_path(&temp);
        {
            let cache = EmbeddingCache::open(&path, "m1", 4).unwrap();
            cache.put("task-1", "hashA", &[1.0, 2.0, 3.0, 4.0]).unwrap();
        }
        let cache = EmbeddingCache::open(&path, "m2", 4).unwrap();
        assert_eq!(cache.get("task-1", "hashA"), None);
    }

    #[test]
    fn dim_mismatch_clears_rows() {
        let temp = TempDir::new().unwrap();
        let path = cache_path(&temp);
        {
            let cache = EmbeddingCache::open(&path, "m1", 4).unwrap();
            cache.put("task-1", "hashA", &[1.0, 2.0, 3.0, 4.0]).unwrap();
        }
        let cache = EmbeddingCache::open(&path, "m1", 8).unwrap();
        assert_eq!(cache.get("task-1", "hashA"), None);
    }

    #[test]
    fn reopen_same_model_and_dim_preserves_rows() {
        let temp = TempDir::new().unwrap();
        let path = cache_path(&temp);
        {
            let cache = EmbeddingCache::open(&path, "m1", 4).unwrap();
            cache.put("task-1", "hashA", &[1.0, 2.0, 3.0, 4.0]).unwrap();
        }
        let cache = EmbeddingCache::open(&path, "m1", 4).unwrap();
        assert_eq!(cache.get("task-1", "hashA"), Some(vec![1.0, 2.0, 3.0, 4.0]));
    }

    #[test]
    fn cold_start_open_on_nonexistent_path_supports_put_get() {
        let temp = TempDir::new().unwrap();
        // Path does not exist yet — open must create the file and schema.
        let path = temp
            .path()
            .join("subdir-absent")
            .join("..")
            .join("fresh.sqlite3");
        let cache = EmbeddingCache::open(&path, "m1", 4).unwrap();
        cache.put("task-1", "hashA", &[9.0, 8.0, 7.0, 6.0]).unwrap();
        assert_eq!(cache.get("task-1", "hashA"), Some(vec![9.0, 8.0, 7.0, 6.0]));
    }
}
