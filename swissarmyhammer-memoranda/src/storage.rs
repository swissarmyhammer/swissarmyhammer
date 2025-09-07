//! Storage implementations for memoranda

use crate::error::{MemorandaError, Result};
use crate::types::{Memo, MemoContent, MemoTitle};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::Mutex;
use tracing::warn;

/// Storage abstraction for memos
#[async_trait]
pub trait MemoStorage: Send + Sync {
    /// Create a new memo
    async fn create(&mut self, title: MemoTitle, content: MemoContent) -> Result<Memo>;

    /// Get a memo by title
    async fn get(&self, title: &MemoTitle) -> Result<Option<Memo>>;

    /// Update a memo's content
    async fn update(&mut self, title: &MemoTitle, content: MemoContent) -> Result<Memo>;

    /// Delete a memo
    async fn delete(&mut self, title: &MemoTitle) -> Result<bool>;

    /// List all memos
    async fn list(&self) -> Result<Vec<Memo>>;
}

/// Markdown-based memo storage using title-based filenames
pub struct MarkdownMemoStorage {
    memos_dir: PathBuf,
    creation_lock: Mutex<()>,
}

impl MarkdownMemoStorage {
    /// Create new storage with the given directory
    pub fn new(memos_dir: PathBuf) -> Self {
        Self {
            memos_dir,
            creation_lock: Mutex::new(()),
        }
    }

    /// Create new storage with default directory
    pub async fn new_default() -> Result<Self> {
        let memos_dir = swissarmyhammer_common::utils::paths::get_swissarmyhammer_dir()
            .map_err(|e| {
                MemorandaError::Storage(format!("Failed to get SwissArmyHammer directory: {}", e))
            })?
            .join("memos");

        fs::create_dir_all(&memos_dir).await?;
        Ok(Self::new(memos_dir))
    }

    /// Get the file path for a memo title
    fn get_memo_path(&self, title: &MemoTitle) -> PathBuf {
        self.memos_dir.join(format!("{}.md", title.to_filename()))
    }

    /// Load memo from markdown file
    async fn load_memo_from_file(&self, file_path: &PathBuf, title: &MemoTitle) -> Result<Memo> {
        let content = fs::read_to_string(file_path).await?;
        let metadata = fs::metadata(file_path).await?;

        let created_at = metadata
            .created()
            .map_err(|e| MemorandaError::Storage(format!("Failed to get creation time: {}", e)))?
            .into();

        let updated_at = metadata
            .modified()
            .map_err(|e| {
                MemorandaError::Storage(format!("Failed to get modification time: {}", e))
            })?
            .into();

        Ok(Memo {
            title: title.clone(),
            content: MemoContent::new(content),
            created_at,
            updated_at,
        })
    }
}

#[async_trait]
impl MemoStorage for MarkdownMemoStorage {
    async fn create(&mut self, title: MemoTitle, content: MemoContent) -> Result<Memo> {
        let _lock = self.creation_lock.lock().await;

        fs::create_dir_all(&self.memos_dir).await?;

        let file_path = self.get_memo_path(&title);

        if file_path.exists() {
            return Err(MemorandaError::InvalidOperation(format!(
                "Memo with title '{}' already exists",
                title
            )));
        }

        fs::write(&file_path, content.as_str()).await?;

        self.load_memo_from_file(&file_path, &title).await
    }

    async fn get(&self, title: &MemoTitle) -> Result<Option<Memo>> {
        let file_path = self.get_memo_path(title);

        if !file_path.exists() {
            return Ok(None);
        }

        match self.load_memo_from_file(&file_path, title).await {
            Ok(memo) => Ok(Some(memo)),
            Err(MemorandaError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn update(&mut self, title: &MemoTitle, content: MemoContent) -> Result<Memo> {
        let file_path = self.get_memo_path(title);

        if !file_path.exists() {
            return Err(MemorandaError::MemoNotFound {
                title: title.to_string(),
            });
        }

        fs::write(&file_path, content.as_str()).await?;

        self.load_memo_from_file(&file_path, title).await
    }

    async fn delete(&mut self, title: &MemoTitle) -> Result<bool> {
        let file_path = self.get_memo_path(title);

        if !file_path.exists() {
            return Ok(false);
        }

        fs::remove_file(&file_path).await?;
        Ok(true)
    }

    async fn list(&self) -> Result<Vec<Memo>> {
        fs::create_dir_all(&self.memos_dir).await?;

        let mut memos = Vec::new();
        let mut entries = fs::read_dir(&self.memos_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    match MemoTitle::from_filename(filename) {
                        Ok(title) => {
                            match self.load_memo_from_file(&path, &title).await {
                                Ok(memo) => memos.push(memo),
                                Err(e) => {
                                    // Log error but continue processing other memos
                                    warn!("Failed to load memo from {}: {}", path.display(), e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Invalid memo filename {}: {}", filename, e);
                        }
                    }
                }
            }
        }

        // Sort by creation time (newest first)
        memos.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(memos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_memo_storage_create_and_get() {
        let temp_dir = tempdir().unwrap();
        let mut storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());

        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new("Test content".to_string());

        let created_memo = storage
            .create(title.clone(), content.clone())
            .await
            .unwrap();
        assert_eq!(created_memo.title, title);
        assert_eq!(created_memo.content, content);

        let retrieved_memo = storage.get(&title).await.unwrap().unwrap();
        assert_eq!(retrieved_memo.title, title);
        assert_eq!(retrieved_memo.content, content);
    }

    #[tokio::test]
    async fn test_memo_storage_update() {
        let temp_dir = tempdir().unwrap();
        let mut storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());

        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let original_content = MemoContent::new("Original content".to_string());
        let updated_content = MemoContent::new("Updated content".to_string());

        storage
            .create(title.clone(), original_content)
            .await
            .unwrap();

        let updated_memo = storage
            .update(&title, updated_content.clone())
            .await
            .unwrap();
        assert_eq!(updated_memo.content, updated_content);
    }

    #[tokio::test]
    async fn test_memo_storage_delete() {
        let temp_dir = tempdir().unwrap();
        let mut storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());

        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new("Test content".to_string());

        storage.create(title.clone(), content).await.unwrap();

        let deleted = storage.delete(&title).await.unwrap();
        assert!(deleted);

        let retrieved = storage.get(&title).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_memo_storage_list() {
        let temp_dir = tempdir().unwrap();
        let mut storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());

        let title1 = MemoTitle::new("Memo One".to_string()).unwrap();
        let title2 = MemoTitle::new("Memo Two".to_string()).unwrap();
        let content = MemoContent::new("Test content".to_string());

        storage
            .create(title1.clone(), content.clone())
            .await
            .unwrap();
        storage
            .create(title2.clone(), content.clone())
            .await
            .unwrap();

        let memos = storage.list().await.unwrap();
        assert_eq!(memos.len(), 2);

        // Check that both memos are present
        let titles: Vec<_> = memos.iter().map(|m| &m.title).collect();
        assert!(titles.contains(&&title1));
        assert!(titles.contains(&&title2));
    }
}
