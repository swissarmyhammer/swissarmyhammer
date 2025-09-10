//! Storage backend trait and implementations for prompt libraries
//!
//! This module defines the storage abstraction used by prompt libraries
//! to persist and retrieve prompts from various storage backends.

use crate::prompts::Prompt;
use crate::Result;
use std::collections::HashMap;
use std::path::Path;
use swissarmyhammer_common::SwissArmyHammerError;

/// Trait for storage backends that can persist and retrieve prompts
pub trait StorageBackend: Send + Sync {
    /// Store a prompt with the given key
    fn store(&mut self, key: &str, prompt: &Prompt) -> Result<()>;

    /// Retrieve a prompt by key
    fn get(&self, key: &str) -> Result<Option<Prompt>>;

    /// List all stored prompt keys
    fn list_keys(&self) -> Result<Vec<String>>;

    /// Remove a prompt by key
    fn remove(&mut self, key: &str) -> Result<bool>;

    /// Clear all stored prompts
    fn clear(&mut self) -> Result<()>;

    /// Check if a prompt exists
    fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.get(key)?.is_some())
    }

    /// Get the total number of stored prompts
    fn count(&self) -> Result<usize> {
        Ok(self.list_keys()?.len())
    }

    /// List all stored prompts
    fn list(&self) -> Result<Vec<Prompt>> {
        let keys = self.list_keys()?;
        let mut prompts = Vec::new();
        for key in keys {
            if let Some(prompt) = self.get(&key)? {
                prompts.push(prompt);
            }
        }
        Ok(prompts)
    }

    /// Search prompts by query string
    fn search(&self, query: &str) -> Result<Vec<Prompt>> {
        let prompts = self.list()?;
        let query_lower = query.to_lowercase();

        Ok(prompts
            .into_iter()
            .filter(|prompt| {
                prompt.name.to_lowercase().contains(&query_lower)
                    || prompt
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                    || prompt.template.to_lowercase().contains(&query_lower)
                    || prompt
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .collect())
    }
}

/// In-memory storage backend for prompts
///
/// This is the default storage backend that keeps all prompts in memory.
/// Prompts are lost when the application exits.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    prompts: HashMap<String, Prompt>,
}

impl MemoryStorage {
    /// Create a new memory storage backend
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all stored prompts
    pub fn get_all(&self) -> &HashMap<String, Prompt> {
        &self.prompts
    }

    /// Insert a prompt directly (for testing)
    pub fn insert(&mut self, key: String, prompt: Prompt) {
        self.prompts.insert(key, prompt);
    }
}

impl StorageBackend for MemoryStorage {
    fn store(&mut self, key: &str, prompt: &Prompt) -> Result<()> {
        self.prompts.insert(key.to_string(), prompt.clone());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Prompt>> {
        Ok(self.prompts.get(key).cloned())
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        Ok(self.prompts.keys().cloned().collect())
    }

    fn remove(&mut self, key: &str) -> Result<bool> {
        Ok(self.prompts.remove(key).is_some())
    }

    fn clear(&mut self) -> Result<()> {
        self.prompts.clear();
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.prompts.contains_key(key))
    }

    fn count(&self) -> Result<usize> {
        Ok(self.prompts.len())
    }
}

/// File-based storage backend for prompts
///
/// This storage backend persists prompts to individual files on disk.
/// Each prompt is stored as a separate file with YAML front matter.
#[derive(Debug)]
pub struct FileStorage {
    base_path: std::path::PathBuf,
}

impl FileStorage {
    /// Create a new file storage backend with the given base directory
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Get the file path for a given prompt key
    fn get_file_path(&self, key: &str) -> std::path::PathBuf {
        self.base_path.join(format!("{}.md", key))
    }
}

impl StorageBackend for FileStorage {
    fn store(&mut self, key: &str, prompt: &Prompt) -> Result<()> {
        let file_path = self.get_file_path(key);

        // Ensure the directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to create directory {}: {}", parent.display(), e),
            })?;
        }

        // Serialize the prompt to YAML front matter + content
        let yaml_front_matter = serde_yaml::to_string(&serde_json::json!({
            "name": prompt.name,
            "description": prompt.description,
            "category": prompt.category,
            "tags": prompt.tags,
            "parameters": prompt.parameters
        }))
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to serialize prompt metadata: {}", e),
        })?;

        let content = format!("---\n{}---\n{}", yaml_front_matter, prompt.template);

        std::fs::write(&file_path, content).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to write prompt file {}: {}", file_path.display(), e),
        })?;

        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Prompt>> {
        let file_path = self.get_file_path(key);

        if !file_path.exists() {
            return Ok(None);
        }

        let content =
            std::fs::read_to_string(&file_path).map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to read prompt file {}: {}", file_path.display(), e),
            })?;

        // Parse the file using the frontmatter parser
        // For now, return a simple prompt until frontmatter module is ready
        Ok(Some(Prompt::new(key, content)))
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        if !self.base_path.exists() {
            return Ok(Vec::new());
        }

        let entries =
            std::fs::read_dir(&self.base_path).map_err(|e| SwissArmyHammerError::Other {
                message: format!(
                    "Failed to read directory {}: {}",
                    self.base_path.display(),
                    e
                ),
            })?;

        let mut keys = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to read directory entry: {}", e),
            })?;

            let path = entry.path();
            if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("md")) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    keys.push(stem.to_string());
                }
            }
        }

        Ok(keys)
    }

    fn remove(&mut self, key: &str) -> Result<bool> {
        let file_path = self.get_file_path(key);

        if !file_path.exists() {
            return Ok(false);
        }

        std::fs::remove_file(&file_path).map_err(|e| SwissArmyHammerError::Other {
            message: format!(
                "Failed to remove prompt file {}: {}",
                file_path.display(),
                e
            ),
        })?;

        Ok(true)
    }

    fn clear(&mut self) -> Result<()> {
        let keys = self.list_keys()?;
        for key in keys {
            self.remove(&key)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_storage() {
        let mut storage = MemoryStorage::new();
        let prompt = Prompt::new("test", "Hello {{name}}!");

        // Test store and get
        storage.store("test", &prompt).unwrap();
        let retrieved = storage.get("test").unwrap().unwrap();
        assert_eq!(retrieved.name, "test");
        assert_eq!(retrieved.template, "Hello {{name}}!");

        // Test exists and count
        assert!(storage.exists("test").unwrap());
        assert_eq!(storage.count().unwrap(), 1);

        // Test list_keys
        let keys = storage.list_keys().unwrap();
        assert_eq!(keys, vec!["test"]);

        // Test remove
        assert!(storage.remove("test").unwrap());
        assert!(!storage.exists("test").unwrap());
        assert_eq!(storage.count().unwrap(), 0);
    }

    #[test]
    fn test_memory_storage_clear() {
        let mut storage = MemoryStorage::new();
        let prompt1 = Prompt::new("test1", "Template 1");
        let prompt2 = Prompt::new("test2", "Template 2");

        storage.store("test1", &prompt1).unwrap();
        storage.store("test2", &prompt2).unwrap();
        assert_eq!(storage.count().unwrap(), 2);

        storage.clear().unwrap();
        assert_eq!(storage.count().unwrap(), 0);
    }
}
