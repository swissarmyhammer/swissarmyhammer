//! High-level operations for memo management

use crate::error::Result;
use crate::storage::{MarkdownMemoStorage, MemoStorage};
use crate::types::{
    CreateMemoRequest, DeleteMemoRequest, GetMemoRequest, ListMemosResponse, Memo, MemoContent,
    MemoTitle, UpdateMemoRequest,
};

/// High-level service for memo operations
pub struct MemoService {
    storage: Box<dyn MemoStorage + Send + Sync>,
}

impl MemoService {
    /// Create a new memo service with default markdown storage
    pub async fn new_default() -> Result<Self> {
        let storage = MarkdownMemoStorage::new_default().await?;
        Ok(Self {
            storage: Box::new(storage),
        })
    }

    /// Create a new memo service with custom storage
    pub fn new(storage: Box<dyn MemoStorage + Send + Sync>) -> Self {
        Self { storage }
    }

    /// Create a new memo
    pub async fn create_memo(&mut self, request: CreateMemoRequest) -> Result<Memo> {
        let title = MemoTitle::new(request.title)?;
        let content = MemoContent::new(request.content);

        self.storage.create(title, content).await
    }

    /// Get a memo by title
    pub async fn get_memo(&self, request: GetMemoRequest) -> Result<Option<Memo>> {
        let title = MemoTitle::new(request.title)?;
        self.storage.get(&title).await
    }

    /// Update a memo's content
    pub async fn update_memo(&mut self, request: UpdateMemoRequest) -> Result<Memo> {
        let title = MemoTitle::new(request.title)?;
        let content = MemoContent::new(request.content);

        self.storage.update(&title, content).await
    }

    /// Delete a memo
    pub async fn delete_memo(&mut self, request: DeleteMemoRequest) -> Result<bool> {
        let title = MemoTitle::new(request.title)?;
        self.storage.delete(&title).await
    }

    /// List all memos
    pub async fn list_memos(&self) -> Result<ListMemosResponse> {
        let memos = self.storage.list().await?;
        Ok(ListMemosResponse {
            total_count: memos.len(),
            memos,
        })
    }

    /// Get all memo content formatted for AI context consumption
    pub async fn get_all_context(&self) -> Result<MemoContent> {
        let memos = self.storage.list().await?;

        if memos.is_empty() {
            return Ok(MemoContent::new(String::new()));
        }

        let mut context_parts = Vec::new();

        for memo in memos {
            // Format: Title followed by content
            context_parts.push(format!("# {}\n\n{}", memo.title, memo.content));
        }

        let combined_context = context_parts.join("\n\n---\n\n");
        Ok(MemoContent::new(combined_context))
    }

    /// Search memos by title and content (simple string matching)
    pub async fn search_memos(&self, query: &str) -> Result<Vec<Memo>> {
        let all_memos = self.storage.list().await?;
        let query_lower = query.to_lowercase();

        let matching_memos: Vec<Memo> = all_memos
            .into_iter()
            .filter(|memo| {
                memo.title.as_str().to_lowercase().contains(&query_lower)
                    || memo.content.as_str().to_lowercase().contains(&query_lower)
            })
            .collect();

        Ok(matching_memos)
    }
}

/// Convenience functions for common operations
impl MemoService {
    /// Create a memo with title and content strings
    pub async fn create_memo_simple(&mut self, title: String, content: String) -> Result<Memo> {
        self.create_memo(CreateMemoRequest { title, content }).await
    }

    /// Get a memo by title string
    pub async fn get_memo_simple(&self, title: String) -> Result<Option<Memo>> {
        self.get_memo(GetMemoRequest { title }).await
    }

    /// Update a memo by title and content strings
    pub async fn update_memo_simple(&mut self, title: String, content: String) -> Result<Memo> {
        self.update_memo(UpdateMemoRequest { title, content }).await
    }

    /// Delete a memo by title string
    pub async fn delete_memo_simple(&mut self, title: String) -> Result<bool> {
        self.delete_memo(DeleteMemoRequest { title }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MarkdownMemoStorage;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_memo_service_create_and_get() {
        let temp_dir = tempdir().unwrap();
        let storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());
        let mut service = MemoService::new(Box::new(storage));

        let memo = service
            .create_memo_simple("Test Memo".to_string(), "Test content".to_string())
            .await
            .unwrap();

        assert_eq!(memo.title.as_str(), "Test Memo");
        assert_eq!(memo.content.as_str(), "Test content");

        let retrieved = service
            .get_memo_simple("Test Memo".to_string())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.title, memo.title);
        assert_eq!(retrieved.content, memo.content);
    }

    #[tokio::test]
    async fn test_memo_service_list() {
        let temp_dir = tempdir().unwrap();
        let storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());
        let mut service = MemoService::new(Box::new(storage));

        service
            .create_memo_simple("Memo 1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        service
            .create_memo_simple("Memo 2".to_string(), "Content 2".to_string())
            .await
            .unwrap();

        let response = service.list_memos().await.unwrap();
        assert_eq!(response.total_count, 2);
        assert_eq!(response.memos.len(), 2);
    }

    #[tokio::test]
    async fn test_memo_service_update() {
        let temp_dir = tempdir().unwrap();
        let storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());
        let mut service = MemoService::new(Box::new(storage));

        service
            .create_memo_simple("Test Memo".to_string(), "Original".to_string())
            .await
            .unwrap();

        let updated = service
            .update_memo_simple("Test Memo".to_string(), "Updated content".to_string())
            .await
            .unwrap();

        assert_eq!(updated.content.as_str(), "Updated content");
    }

    #[tokio::test]
    async fn test_memo_service_delete() {
        let temp_dir = tempdir().unwrap();
        let storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());
        let mut service = MemoService::new(Box::new(storage));

        service
            .create_memo_simple("Test Memo".to_string(), "Content".to_string())
            .await
            .unwrap();

        let deleted = service
            .delete_memo_simple("Test Memo".to_string())
            .await
            .unwrap();
        assert!(deleted);

        let retrieved = service
            .get_memo_simple("Test Memo".to_string())
            .await
            .unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_memo_service_get_all_context() {
        let temp_dir = tempdir().unwrap();
        let storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());
        let mut service = MemoService::new(Box::new(storage));

        service
            .create_memo_simple("Memo 1".to_string(), "Content for memo 1".to_string())
            .await
            .unwrap();
        service
            .create_memo_simple("Memo 2".to_string(), "Content for memo 2".to_string())
            .await
            .unwrap();

        let context = service.get_all_context().await.unwrap();
        let context_str = context.as_str();

        assert!(context_str.contains("# Memo 1"));
        assert!(context_str.contains("Content for memo 1"));
        assert!(context_str.contains("# Memo 2"));
        assert!(context_str.contains("Content for memo 2"));
        assert!(context_str.contains("---")); // Delimiter between memos
    }

    #[tokio::test]
    async fn test_memo_service_search() {
        let temp_dir = tempdir().unwrap();
        let storage = MarkdownMemoStorage::new(temp_dir.path().to_path_buf());
        let mut service = MemoService::new(Box::new(storage));

        service
            .create_memo_simple(
                "Project Notes".to_string(),
                "Working on the new feature".to_string(),
            )
            .await
            .unwrap();
        service
            .create_memo_simple(
                "Meeting Minutes".to_string(),
                "Discussed project timeline".to_string(),
            )
            .await
            .unwrap();
        service
            .create_memo_simple(
                "Shopping List".to_string(),
                "Buy groceries and supplies".to_string(),
            )
            .await
            .unwrap();

        let project_results = service.search_memos("project").await.unwrap();
        assert_eq!(project_results.len(), 2); // "Project Notes" and "Meeting Minutes"

        let feature_results = service.search_memos("feature").await.unwrap();
        assert_eq!(feature_results.len(), 1); // Only "Project Notes"

        let nonexistent_results = service.search_memos("nonexistent").await.unwrap();
        assert_eq!(nonexistent_results.len(), 0);
    }
}
