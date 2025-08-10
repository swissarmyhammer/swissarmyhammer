//! Request and response types for memoranda MCP operations

use serde::{Deserialize, Serialize};

/// Request to create a new memo
///
/// # Examples
///
/// Create a memo with title and content:
/// ```ignore
/// CreateMemoRequest {
///     title: "Meeting Notes".to_string(),
///     content: "# Team Meeting\n\nDiscussed project roadmap...".to_string(),
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateMemoRequest {
    /// Title of the memo
    pub title: String,
    /// Markdown content of the memo
    pub content: String,
}

/// Request to get a memo by ID
///
/// # Examples
///
/// Get a memo by its ULID:
/// ```ignore
/// GetMemoRequest {
///     id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetMemoRequest {
    /// ULID identifier of the memo to retrieve
    pub id: String,
}

/// Request to update a memo's content
///
/// # Examples
///
/// Update memo content:
/// ```ignore
/// UpdateMemoRequest {
///     id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
///     content: "# Updated Content\n\nNew information...".to_string(),
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateMemoRequest {
    /// ULID identifier of the memo to update
    pub id: String,
    /// New markdown content for the memo
    pub content: String,
}

/// Request to delete a memo
///
/// # Examples
///
/// Delete a memo by its ULID:
/// ```ignore
/// DeleteMemoRequest {
///     id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DeleteMemoRequest {
    /// ULID identifier of the memo to delete
    pub id: String,
}

/// Request to search memos
///
/// # Examples
///
/// Search for memos containing specific text:
/// ```ignore
/// SearchMemosRequest {
///     query: "meeting notes project".to_string(),
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchMemosRequest {
    /// Search query string to match against memo titles and content
    pub query: String,
}

/// Request to list all memos
///
/// # Examples
///
/// List all available memos:
/// ```ignore
/// ListMemosRequest {
///     // No parameters needed
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListMemosRequest {
    // No parameters needed for listing all memos
}

/// Request to get all memos as context
///
/// # Examples
///
/// Get all memo content for AI context:
/// ```ignore
/// GetAllContextRequest {
///     // No parameters needed
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetAllContextRequest {
    // No parameters needed - returns all memo content formatted for AI consumption
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_memo_request_serialization() {
        let request = CreateMemoRequest {
            title: "Test Title".to_string(),
            content: "Test Content".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: CreateMemoRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.title, deserialized.title);
        assert_eq!(request.content, deserialized.content);
    }

    #[test]
    fn test_get_memo_request_serialization() {
        let request = GetMemoRequest {
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: GetMemoRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.id, deserialized.id);
    }

    #[test]
    fn test_update_memo_request_serialization() {
        let request = UpdateMemoRequest {
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            content: "Updated content".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: UpdateMemoRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.id, deserialized.id);
        assert_eq!(request.content, deserialized.content);
    }

    #[test]
    fn test_delete_memo_request_serialization() {
        let request = DeleteMemoRequest {
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: DeleteMemoRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.id, deserialized.id);
    }

    #[test]
    fn test_search_memos_request_serialization() {
        let request = SearchMemosRequest {
            query: "test search".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: SearchMemosRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.query, deserialized.query);
    }

    #[test]
    fn test_list_memos_request_serialization() {
        let request = ListMemosRequest {};

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ListMemosRequest = serde_json::from_str(&json).unwrap();

        // No fields to compare for empty struct
        assert_eq!(
            std::mem::size_of_val(&request),
            std::mem::size_of_val(&deserialized)
        );
    }

    #[test]
    fn test_get_all_context_request_serialization() {
        let request = GetAllContextRequest {};

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: GetAllContextRequest = serde_json::from_str(&json).unwrap();

        // No fields to compare for empty struct
        assert_eq!(
            std::mem::size_of_val(&request),
            std::mem::size_of_val(&deserialized)
        );
    }
}
