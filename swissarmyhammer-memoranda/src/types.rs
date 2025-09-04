//! Core types for memoranda system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{MemorandaError, Result};

/// A memo title that serves as both display name and file identifier
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemoTitle(String);

impl MemoTitle {
    /// Create a new memo title with validation
    pub fn new(title: String) -> Result<Self> {
        let title = title.trim().to_string();
        
        if title.is_empty() {
            return Err(MemorandaError::InvalidTitle("Title cannot be empty".to_string()));
        }

        if title.len() > 255 {
            return Err(MemorandaError::InvalidTitle(format!(
                "Title too long: {} characters (max 255)", title.len()
            )));
        }

        // Check for invalid filesystem characters
        let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
        for ch in invalid_chars {
            if title.contains(ch) {
                return Err(MemorandaError::InvalidTitle(format!(
                    "Title contains invalid character '{}': {}", ch, title
                )));
            }
        }

        Ok(Self(title))
    }

    /// Get the title as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to a safe filename (replaces spaces with underscores)
    pub fn to_filename(&self) -> String {
        self.0.replace(' ', "_")
    }

    /// Create from a filename (reverses the to_filename transformation)
    pub fn from_filename(filename: &str) -> Result<Self> {
        let title = filename.replace('_', " ");
        Self::new(title)
    }
}

impl fmt::Display for MemoTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for MemoTitle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Typed memo content
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoContent(String);

impl MemoContent {
    /// Create new memo content
    pub fn new(content: String) -> Self {
        Self(content)
    }

    /// Get the content as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if content is empty
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }

    /// Get content length
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Display for MemoContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MemoContent {
    fn from(content: String) -> Self {
        Self::new(content)
    }
}

impl AsRef<str> for MemoContent {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A memo with title-based identification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memo {
    /// The title of the memo (also serves as identifier)
    pub title: MemoTitle,
    /// The markdown content of the memo
    pub content: MemoContent,
    /// When this memo was created
    pub created_at: DateTime<Utc>,
    /// When this memo was last updated
    pub updated_at: DateTime<Utc>,
}

impl Memo {
    /// Create a new memo
    pub fn new(title: MemoTitle, content: MemoContent) -> Self {
        let now = Utc::now();
        Self {
            title,
            content,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the content and timestamp
    pub fn update_content(&mut self, content: MemoContent) {
        self.content = content;
        self.updated_at = Utc::now();
    }
}

/// Request to create a new memo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMemoRequest {
    pub title: String,
    pub content: String,
}

/// Request to update a memo's content
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateMemoRequest {
    pub title: String,
    pub content: String,
}

/// Request to get a memo by title
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetMemoRequest {
    pub title: String,
}

/// Request to delete a memo by title
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteMemoRequest {
    pub title: String,
}

/// Response with list of memos
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListMemosResponse {
    pub memos: Vec<Memo>,
    pub total_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memo_title_creation() {
        let title = MemoTitle::new("Valid Title".to_string()).unwrap();
        assert_eq!(title.as_str(), "Valid Title");
        assert_eq!(title.to_filename(), "Valid_Title");
    }

    #[test]
    fn test_memo_title_validation() {
        assert!(MemoTitle::new("".to_string()).is_err());
        assert!(MemoTitle::new("  ".to_string()).is_err());
        assert!(MemoTitle::new("a".repeat(256)).is_err());
        assert!(MemoTitle::new("invalid/title".to_string()).is_err());
    }

    #[test]
    fn test_memo_title_filename_conversion() {
        let title = MemoTitle::new("Meeting Notes Today".to_string()).unwrap();
        let filename = title.to_filename();
        assert_eq!(filename, "Meeting_Notes_Today");
        
        let restored = MemoTitle::from_filename(&filename).unwrap();
        assert_eq!(restored.as_str(), "Meeting Notes Today");
    }

    #[test]
    fn test_memo_content() {
        let test_string = "# Hello World\nThis is content";
        let content = MemoContent::new(test_string.to_string());
        assert!(!content.is_empty());
        assert_eq!(content.len(), test_string.len());
        assert_eq!(content.as_str(), test_string);
    }

    #[test]
    fn test_memo_creation() {
        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new("Test content".to_string());
        let memo = Memo::new(title.clone(), content.clone());

        assert_eq!(memo.title, title);
        assert_eq!(memo.content, content);
        assert_eq!(memo.created_at, memo.updated_at);
    }

    #[test]
    fn test_memo_update() {
        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new("Original content".to_string());
        let mut memo = Memo::new(title, content);
        
        let original_created = memo.created_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        
        let new_content = MemoContent::new("Updated content".to_string());
        memo.update_content(new_content.clone());
        
        assert_eq!(memo.content, new_content);
        assert_eq!(memo.created_at, original_created);
        assert!(memo.updated_at > original_created);
    }
}