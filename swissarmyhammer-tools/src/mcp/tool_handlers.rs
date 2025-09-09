//! Tool handlers for MCP operations

use super::memo_types::*;
use super::responses::create_success_response;
use super::shared_utils::{McpErrorHandler, McpFormatter, McpValidation};
use rmcp::model::*;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_common::SwissArmyHammerError;
use swissarmyhammer_memoranda::{MemoContent, MemoStorage, MemoTitle};
use tokio::sync::RwLock;

/// Preview length for memo list operations (characters)
const MEMO_LIST_PREVIEW_LENGTH: usize = 100;

/// Tool handlers for MCP server operations
#[derive(Clone)]
pub struct ToolHandlers {
    memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,
}

impl ToolHandlers {
    /// Create a new tool handlers instance with the given memo storage
    pub fn new(memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>) -> Self {
        Self { memo_storage }
    }

    /// Handle memo operation errors consistently based on error type
    ///
    /// Uses the shared error handler for consistent error mapping across all MCP operations.
    ///
    /// # Arguments
    ///
    /// * `error` - The SwissArmyHammerError to handle
    /// * `operation` - Description of the operation that failed (for logging)
    ///
    /// # Returns
    ///
    /// * `McpError` - Appropriate MCP error response
    fn handle_memo_error(error: SwissArmyHammerError, operation: &str) -> McpError {
        McpErrorHandler::handle_error(error, operation)
    }

    /// Handle the memo_create tool operation.
    ///
    /// Creates a new memo with the given title and content.
    ///
    /// # Arguments
    ///
    /// * `request` - The create memo request containing title and content
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - The tool call result
    pub async fn handle_memo_create(
        &self,
        request: CreateMemoRequest,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!("Creating memo with title: {}", request.title);

        // Note: Both title and content can be empty - storage layer supports this

        let mut memo_storage = self.memo_storage.write().await;
        match memo_storage
            .create(
                match MemoTitle::new(request.title.clone()) {
                    Ok(title) => title,
                    Err(_) => {
                        return Err(McpError::invalid_params(
                            format!("Invalid title format: {}", request.title),
                            None,
                        ))
                    }
                },
                MemoContent::new(request.content),
            )
            .await
        {
            Ok(memo) => {
                tracing::info!("Created memo {}", memo.title);
                Ok(create_success_response(format!(
                    "Successfully created memo '{}' with ID: {}\n\nTitle: {}\nContent: {}",
                    memo.title, memo.title, memo.title, memo.content
                )))
            }
            Err(e) => Err(Self::handle_memo_error(
                SwissArmyHammerError::Other {
                    message: e.to_string(),
                },
                "create memo",
            )),
        }
    }

    /// Handle the memo_get tool operation.
    ///
    /// Retrieves a memo by its ID.
    ///
    /// # Arguments
    ///
    /// * `request` - The get memo request containing the memo ID
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - The tool call result
    pub async fn handle_memo_get(
        &self,
        request: GetMemoRequest,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!("Getting memo with title: {}", request.title);

        let memo_title = match MemoTitle::new(request.title.clone()) {
            Ok(title) => title,
            Err(_) => {
                return Err(McpError::invalid_params(
                    format!("Invalid memo title format: {}", request.title),
                    None,
                ))
            }
        };

        let memo_storage = self.memo_storage.read().await;
        match memo_storage.get(&memo_title).await {
            Ok(Some(memo)) => {
                tracing::info!("Retrieved memo {}", memo.title);
                Ok(create_success_response(format!(
                    "Memo found:\n\nID: {}\nTitle: {}\nCreated: {}\nUpdated: {}\n\nContent:\n{}",
                    memo.title,
                    memo.title,
                    McpFormatter::format_timestamp(memo.created_at),
                    McpFormatter::format_timestamp(memo.updated_at),
                    memo.content
                )))
            }
            Ok(None) => Ok(create_success_response(format!(
                "Memo not found with title: {}",
                memo_title
            ))),
            Err(e) => Err(McpErrorHandler::handle_error(
                SwissArmyHammerError::Other {
                    message: e.to_string(),
                },
                "get memo",
            )),
        }
    }

    /// Handle the memo_update tool operation.
    ///
    /// Updates a memo's content by its ID.
    ///
    /// # Arguments
    ///
    /// * `request` - The update memo request containing memo ID and new content
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - The tool call result
    pub async fn handle_memo_update(
        &self,
        request: UpdateMemoRequest,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!("Updating memo with ID: {}", request.id);

        // Validate memo content using shared validation
        McpValidation::validate_not_empty(&request.content, "memo content")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate memo content"))?;

        let memo_id = match MemoTitle::new(request.id.clone()) {
            Ok(id) => id,
            Err(_) => {
                return Err(McpError::invalid_params(
                    format!("Invalid memo ID format: {}", request.id),
                    None,
                ))
            }
        };

        let mut memo_storage = self.memo_storage.write().await;
        match memo_storage
            .update(&memo_id, MemoContent::new(request.content))
            .await
        {
            Ok(memo) => {
                tracing::info!("Updated memo {}", memo.title);
                Ok(create_success_response(format!(
                    "Successfully updated memo:\n\nID: {}\nTitle: {}\nUpdated: {}\n\nContent:\n{}",
                    memo.title,
                    memo.title,
                    McpFormatter::format_timestamp(memo.updated_at),
                    memo.content
                )))
            }
            Err(e) => Err(McpErrorHandler::handle_error(
                SwissArmyHammerError::Other {
                    message: e.to_string(),
                },
                "update memo",
            )),
        }
    }

    /// Handle the memo_list tool operation.
    ///
    /// Lists all available memos.
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - The tool call result
    pub async fn handle_memo_list(
        &self,
        _request: ListMemosRequest,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!("Listing all memos");

        let memo_storage = self.memo_storage.read().await;
        match memo_storage.list().await {
            Ok(memos) => {
                tracing::info!("Retrieved {} memos", memos.len());
                if memos.is_empty() {
                    Ok(create_success_response("No memos found".to_string()))
                } else {
                    let memo_list = memos
                        .iter()
                        .map(|memo| {
                            McpFormatter::format_memo_preview(memo, MEMO_LIST_PREVIEW_LENGTH)
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    let summary =
                        McpFormatter::format_list_summary("memo", memos.len(), memos.len());
                    Ok(create_success_response(format!(
                        "{summary}:\n\n{memo_list}"
                    )))
                }
            }
            Err(e) => Err(McpErrorHandler::handle_error(
                SwissArmyHammerError::Other {
                    message: e.to_string(),
                },
                "list memos",
            )),
        }
    }

    /// Handle the memo_get_all_context tool operation.
    ///
    /// Gets all memo content formatted for AI context consumption.
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - The tool call result
    pub async fn handle_memo_get_all_context(
        &self,
        _request: GetAllContextRequest,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!("Getting all memo context");

        let memo_storage = self.memo_storage.read().await;
        match memo_storage.list().await {
            Ok(memos) => {
                tracing::info!("Retrieved {} memos for context", memos.len());
                if memos.is_empty() {
                    Ok(create_success_response("No memos available".to_string()))
                } else {
                    // Sort memos by updated_at descending (most recent first)
                    let mut sorted_memos = memos;
                    sorted_memos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

                    let context = sorted_memos
                        .iter()
                        .map(|memo| {
                            format!(
                                "=== {} (ID: {}) ===\nCreated: {}\nUpdated: {}\n\n{}",
                                memo.title,
                                memo.title,
                                McpFormatter::format_timestamp(memo.created_at),
                                McpFormatter::format_timestamp(memo.updated_at),
                                memo.content
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(&format!("\n\n{}\n\n", "=".repeat(80)));

                    let memo_count = sorted_memos.len();
                    let plural_suffix = if memo_count == 1 { "" } else { "s" };
                    Ok(create_success_response(format!(
                        "All memo context ({memo_count} memo{plural_suffix}):\n\n{context}"
                    )))
                }
            }
            Err(e) => Err(McpErrorHandler::handle_error(
                SwissArmyHammerError::Other {
                    message: e.to_string(),
                },
                "get memo context",
            )),
        }
    }
}
