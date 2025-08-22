//! Storage abstractions and implementations

use crate::fs_utils::FileSystemUtils;
use crate::{Prompt, Result, SwissArmyHammerError};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Trait for prompt storage backends
pub trait StorageBackend: Send + Sync {
    /// Store a prompt
    fn store(&mut self, prompt: Prompt) -> Result<()>;

    /// Get a prompt by name
    fn get(&self, name: &str) -> Result<Prompt>;

    /// List all prompts
    fn list(&self) -> Result<Vec<Prompt>>;

    /// Remove a prompt
    fn remove(&mut self, name: &str) -> Result<()>;

    /// Search prompts by query
    fn search(&self, query: &str) -> Result<Vec<Prompt>>;

    /// Check if a prompt exists
    fn exists(&self, name: &str) -> Result<bool> {
        self.get(name).map(|_| true).or_else(|e| match e {
            SwissArmyHammerError::PromptNotFound(_) => Ok(false),
            _ => Err(e),
        })
    }

    /// Get total count of prompts
    fn count(&self) -> Result<usize> {
        self.list().map(|prompts| prompts.len())
    }

    /// Clone the storage backend in a box
    fn clone_box(&self) -> Box<dyn StorageBackend>;
}

/// In-memory storage implementation
pub struct MemoryStorage {
    prompts: HashMap<String, Prompt>,
}

impl MemoryStorage {
    /// Create a new memory storage
    pub fn new() -> Self {
        Self {
            prompts: HashMap::new(),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageBackend for MemoryStorage {
    fn store(&mut self, prompt: Prompt) -> Result<()> {
        self.prompts.insert(prompt.name.clone(), prompt);
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Prompt> {
        self.prompts
            .get(name)
            .cloned()
            .ok_or_else(|| SwissArmyHammerError::PromptNotFound(name.to_string()))
    }

    fn list(&self) -> Result<Vec<Prompt>> {
        Ok(self.prompts.values().cloned().collect())
    }

    fn remove(&mut self, name: &str) -> Result<()> {
        self.prompts
            .remove(name)
            .ok_or_else(|| SwissArmyHammerError::PromptNotFound(name.to_string()))?;
        Ok(())
    }

    fn search(&self, query: &str) -> Result<Vec<Prompt>> {
        let query_lower = query.to_lowercase();
        Ok(self
            .prompts
            .values()
            .filter(|prompt| {
                prompt.name.to_lowercase().contains(&query_lower)
                    || prompt
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                    || prompt
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
                    || prompt
                        .category
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
            })
            .cloned()
            .collect())
    }

    fn clone_box(&self) -> Box<dyn StorageBackend> {
        Box::new(MemoryStorage {
            prompts: self.prompts.clone(),
        })
    }
}

/// File system storage implementation
pub struct FileSystemStorage {
    base_path: std::path::PathBuf,
    cache: dashmap::DashMap<String, Prompt>,
    fs_utils: FileSystemUtils,
}

impl FileSystemStorage {
    /// Create a new file system storage
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let fs_utils = FileSystemUtils::new();

        if !fs_utils.fs().exists(&base_path) {
            fs_utils.fs().create_dir_all(&base_path)?;
        }

        let storage = Self {
            base_path,
            cache: dashmap::DashMap::new(),
            fs_utils,
        };

        // Load existing prompts into cache
        storage.reload_cache()?;

        Ok(storage)
    }

    /// Create a new file system storage with custom filesystem utils (for testing)
    #[cfg(test)]
    pub fn new_with_fs_utils(
        base_path: impl AsRef<Path>,
        fs_utils: FileSystemUtils,
    ) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        if !fs_utils.fs().exists(&base_path) {
            fs_utils.fs().create_dir_all(&base_path)?;
        }

        let storage = Self {
            base_path,
            cache: dashmap::DashMap::new(),
            fs_utils,
        };

        // Load existing prompts into cache
        storage.reload_cache()?;

        Ok(storage)
    }

    /// Reload the cache from disk
    pub fn reload_cache(&self) -> Result<()> {
        self.cache.clear();

        for entry in walkdir::WalkDir::new(&self.base_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if self.fs_utils.fs().is_file(path)
                && path.extension().and_then(|s| s.to_str()) == Some("yaml")
            {
                if let Ok(prompt) = self.fs_utils.read_yaml::<Prompt>(path) {
                    self.cache.insert(prompt.name.clone(), prompt);
                }
            }
        }

        Ok(())
    }

    fn prompt_path(&self, name: &str) -> std::path::PathBuf {
        self.base_path.join(format!("{name}.yaml"))
    }
}

impl StorageBackend for FileSystemStorage {
    fn store(&mut self, prompt: Prompt) -> Result<()> {
        let path = self.prompt_path(&prompt.name);
        self.fs_utils.write_yaml(&path, &prompt)?;
        self.cache.insert(prompt.name.clone(), prompt);
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Prompt> {
        if let Some(prompt) = self.cache.get(name) {
            return Ok(prompt.clone());
        }

        let path = self.prompt_path(name);
        if !self.fs_utils.fs().exists(&path) {
            return Err(SwissArmyHammerError::PromptNotFound(name.to_string()));
        }

        let prompt: Prompt = self.fs_utils.read_yaml(&path)?;
        self.cache.insert(name.to_string(), prompt.clone());

        Ok(prompt)
    }

    fn list(&self) -> Result<Vec<Prompt>> {
        Ok(self
            .cache
            .iter()
            .map(|entry| entry.value().clone())
            .collect())
    }

    fn remove(&mut self, name: &str) -> Result<()> {
        let path = self.prompt_path(name);
        if !self.fs_utils.fs().exists(&path) {
            return Err(SwissArmyHammerError::PromptNotFound(name.to_string()));
        }

        self.fs_utils.fs().remove_file(&path)?;
        self.cache.remove(name);
        Ok(())
    }

    fn search(&self, query: &str) -> Result<Vec<Prompt>> {
        let query_lower = query.to_lowercase();
        Ok(self
            .cache
            .iter()
            .filter(|entry| {
                let prompt = entry.value();
                prompt.name.to_lowercase().contains(&query_lower)
                    || prompt
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                    || prompt
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
                    || prompt
                        .category
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
            })
            .map(|entry| entry.value().clone())
            .collect())
    }

    fn clone_box(&self) -> Box<dyn StorageBackend> {
        Box::new(FileSystemStorage {
            base_path: self.base_path.clone(),
            cache: self.cache.clone(),
            fs_utils: FileSystemUtils::new(),
        })
    }
}

/// Main prompt storage that can use different backends
pub struct PromptStorage {
    backend: Arc<dyn StorageBackend>,
}

impl PromptStorage {
    /// Create a new prompt storage with the given backend
    pub fn new(backend: Arc<dyn StorageBackend>) -> Self {
        Self { backend }
    }

    /// Create with memory backend
    pub fn memory() -> Self {
        Self::new(Arc::new(MemoryStorage::new()))
    }

    /// Create with file system backend
    pub fn file_system(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::new(Arc::new(FileSystemStorage::new(path)?)))
    }

    /// Store a prompt
    pub fn store(&mut self, prompt: Prompt) -> Result<()> {
        Arc::get_mut(&mut self.backend)
            .ok_or_else(|| {
                SwissArmyHammerError::Storage(
                    "Cannot get mutable reference to storage backend".to_string(),
                )
            })?
            .store(prompt)
    }

    /// Get a prompt by name
    pub fn get(&self, name: &str) -> Result<Prompt> {
        self.backend.get(name)
    }

    /// List all prompts
    pub fn list(&self) -> Result<Vec<Prompt>> {
        self.backend.list()
    }

    /// Remove a prompt
    pub fn remove(&mut self, name: &str) -> Result<()> {
        Arc::get_mut(&mut self.backend)
            .ok_or_else(|| {
                SwissArmyHammerError::Storage(
                    "Cannot get mutable reference to storage backend".to_string(),
                )
            })?
            .remove(name)
    }

    /// Search prompts
    pub fn search(&self, query: &str) -> Result<Vec<Prompt>> {
        self.backend.search(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_prompt(name: &str, template: &str) -> Prompt {
        Prompt::new(name, template)
            .with_description(format!("Description for {name}"))
            .with_category("test")
            .with_tags(vec!["test".to_string(), name.to_string()])
    }

    #[test]
    fn test_memory_storage() {
        let mut storage = MemoryStorage::new();

        let prompt = Prompt::new("test", "Hello {{ name }}!");
        storage.store(prompt.clone()).unwrap();

        let retrieved = storage.get("test").unwrap();
        assert_eq!(retrieved.name, "test");
        assert_eq!(retrieved.template, "Hello {{ name }}!");

        let list = storage.list().unwrap();
        assert_eq!(list.len(), 1);

        storage.remove("test").unwrap();
        assert!(storage.get("test").is_err());
    }

    #[test]
    fn test_memory_storage_default() {
        let storage = MemoryStorage::default();
        let list = storage.list().unwrap();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_memory_storage_exists() {
        let mut storage = MemoryStorage::new();
        let prompt = create_test_prompt("exists-test", "Template");

        assert!(!storage.exists("exists-test").unwrap());
        storage.store(prompt).unwrap();
        assert!(storage.exists("exists-test").unwrap());
    }

    #[test]
    fn test_memory_storage_count() {
        let mut storage = MemoryStorage::new();
        assert_eq!(storage.count().unwrap(), 0);

        storage
            .store(create_test_prompt("prompt1", "Template 1"))
            .unwrap();
        assert_eq!(storage.count().unwrap(), 1);

        storage
            .store(create_test_prompt("prompt2", "Template 2"))
            .unwrap();
        assert_eq!(storage.count().unwrap(), 2);

        storage.remove("prompt1").unwrap();
        assert_eq!(storage.count().unwrap(), 1);
    }

    #[test]
    fn test_memory_storage_clone_box() {
        let mut storage = MemoryStorage::new();
        let prompt = create_test_prompt("clone-test", "Template");
        storage.store(prompt.clone()).unwrap();

        let cloned = storage.clone_box();
        let retrieved = cloned.get("clone-test").unwrap();
        assert_eq!(retrieved.name, prompt.name);
        assert_eq!(retrieved.template, prompt.template);
    }

    #[test]
    fn test_memory_storage_remove_nonexistent() {
        let mut storage = MemoryStorage::new();
        let result = storage.remove("nonexistent");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SwissArmyHammerError::PromptNotFound(_)
        ));
    }

    #[test]
    fn test_memory_storage_get_nonexistent() {
        let storage = MemoryStorage::new();
        let result = storage.get("nonexistent");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SwissArmyHammerError::PromptNotFound(_)
        ));
    }

    #[test]
    fn test_search() {
        let mut storage = MemoryStorage::new();

        let prompt1 = Prompt::new("code-review", "Review this code")
            .with_description("A prompt for code review")
            .with_tags(vec!["code".to_string(), "review".to_string()]);

        let prompt2 = Prompt::new("bug-fix", "Fix this bug")
            .with_description("A prompt for fixing bugs")
            .with_tags(vec!["bug".to_string(), "fix".to_string()]);

        storage.store(prompt1).unwrap();
        storage.store(prompt2).unwrap();

        let results = storage.search("code").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");

        let results = storage.search("bug").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "bug-fix");
    }

    #[test]
    fn test_search_by_description() {
        let mut storage = MemoryStorage::new();
        let prompt =
            Prompt::new("test", "Template").with_description("This is a unique description");
        storage.store(prompt).unwrap();

        let results = storage.search("unique").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test");
    }

    #[test]
    fn test_search_by_category() {
        let mut storage = MemoryStorage::new();
        let prompt = Prompt::new("test", "Template").with_category("special-category");
        storage.store(prompt).unwrap();

        let results = storage.search("special").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test");
    }

    #[test]
    fn test_search_by_tags() {
        let mut storage = MemoryStorage::new();
        let prompt = Prompt::new("test", "Template").with_tags(vec!["unique-tag".to_string()]);
        storage.store(prompt).unwrap();

        let results = storage.search("unique-tag").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test");
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut storage = MemoryStorage::new();
        let prompt = Prompt::new("TEST-NAME", "Template").with_description("UPPER DESCRIPTION");
        storage.store(prompt).unwrap();

        let results = storage.search("test").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "TEST-NAME");

        let results = storage.search("upper").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "TEST-NAME");
    }

    #[test]
    fn test_search_no_matches() {
        let mut storage = MemoryStorage::new();
        storage
            .store(create_test_prompt("test", "Template"))
            .unwrap();

        let results = storage.search("nonexistent").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_empty_query() {
        let mut storage = MemoryStorage::new();
        storage
            .store(create_test_prompt("test", "Template"))
            .unwrap();

        let results = storage.search("").unwrap();
        assert_eq!(results.len(), 1); // Empty string matches everything
    }

    #[test]
    fn test_filesystem_storage_creation() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let storage = FileSystemStorage::new(temp_dir.path()).unwrap();
        assert!(temp_dir.path().exists());
        assert_eq!(storage.list().unwrap().len(), 0);
    }

    #[test]
    fn test_filesystem_storage_nonexistent_directory() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let nonexistent_path = temp_dir.path().join("nonexistent");

        let _storage = FileSystemStorage::new(&nonexistent_path).unwrap();
        assert!(nonexistent_path.exists());
    }

    #[test]
    fn test_filesystem_storage_store_and_get() {
        use crate::fs_utils::tests::MockFileSystem;
        use crate::fs_utils::FileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let mut storage = FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap();

        let prompt = create_test_prompt("fs-test", "Filesystem test template");
        storage.store(prompt.clone()).unwrap();

        let retrieved = storage.get("fs-test").unwrap();
        assert_eq!(retrieved.name, prompt.name);
        assert_eq!(retrieved.template, prompt.template);
        assert_eq!(retrieved.description, prompt.description);

        // Check that file was created in mock filesystem
        let prompt_file = std::path::Path::new("/test/fs-test.yaml");
        assert!(mock_fs.is_file(prompt_file));
    }

    #[test]
    fn test_filesystem_storage_get_nonexistent() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let storage = FileSystemStorage::new(temp_dir.path()).unwrap();

        let result = storage.get("nonexistent");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SwissArmyHammerError::PromptNotFound(_)
        ));
    }

    #[test]
    fn test_filesystem_storage_remove() {
        use crate::fs_utils::tests::MockFileSystem;
        use crate::fs_utils::FileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let mut storage = FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap();

        let prompt = create_test_prompt("remove-test", "Template");
        storage.store(prompt).unwrap();

        assert!(storage.get("remove-test").is_ok());
        storage.remove("remove-test").unwrap();
        assert!(storage.get("remove-test").is_err());

        // Check that file was removed from mock filesystem
        let prompt_file = std::path::Path::new("/test/remove-test.yaml");
        assert!(!mock_fs.is_file(prompt_file));
    }

    #[test]
    fn test_filesystem_storage_remove_nonexistent() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let mut storage = FileSystemStorage::new(temp_dir.path()).unwrap();

        let result = storage.remove("nonexistent");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SwissArmyHammerError::PromptNotFound(_)
        ));
    }

    #[test]
    fn test_filesystem_storage_list() {
        use crate::fs_utils::tests::MockFileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let mut storage = FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap();

        assert_eq!(storage.list().unwrap().len(), 0);

        storage
            .store(create_test_prompt("prompt1", "Template 1"))
            .unwrap();
        storage
            .store(create_test_prompt("prompt2", "Template 2"))
            .unwrap();

        let prompts = storage.list().unwrap();
        assert_eq!(prompts.len(), 2);

        let names: Vec<String> = prompts.iter().map(|p| p.name.clone()).collect();
        assert!(names.contains(&"prompt1".to_string()));
        assert!(names.contains(&"prompt2".to_string()));
    }

    #[test]
    fn test_filesystem_storage_search() {
        use crate::fs_utils::tests::MockFileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let mut storage = FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap();

        let prompt1 =
            Prompt::new("search-test-1", "Template").with_description("Contains keyword UNIQUE");
        let prompt2 =
            Prompt::new("search-test-2", "Template").with_description("Different description");

        storage.store(prompt1).unwrap();
        storage.store(prompt2).unwrap();

        let results = storage.search("unique").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "search-test-1");
    }

    #[test]
    fn test_filesystem_storage_reload_cache() {
        use crate::fs_utils::tests::MockFileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let mut storage = FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap();

        let prompt = create_test_prompt("reload-test", "Template");
        storage.store(prompt.clone()).unwrap();

        // Clear cache and reload
        storage.cache.clear();
        assert_eq!(storage.list().unwrap().len(), 0);

        storage.reload_cache().unwrap();
        let retrieved = storage.get("reload-test").unwrap();
        assert_eq!(retrieved.name, prompt.name);
    }

    #[test]
    fn test_filesystem_storage_clone_box() {
        use crate::fs_utils::tests::MockFileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let mut storage = FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap();

        let prompt = create_test_prompt("clone-fs-test", "Template");
        storage.store(prompt.clone()).unwrap();

        let cloned = storage.clone_box();
        let retrieved = cloned.get("clone-fs-test").unwrap();
        assert_eq!(retrieved.name, prompt.name);
    }

    #[test]
    fn test_filesystem_storage_exists_and_count() {
        use crate::fs_utils::tests::MockFileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let mut storage = FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap();

        assert_eq!(storage.count().unwrap(), 0);
        assert!(!storage.exists("test").unwrap());

        storage
            .store(create_test_prompt("test", "Template"))
            .unwrap();
        assert_eq!(storage.count().unwrap(), 1);
        assert!(storage.exists("test").unwrap());
    }

    #[test]
    fn test_prompt_storage_memory() {
        let mut storage = PromptStorage::memory();
        let prompt = create_test_prompt("memory-test", "Template");

        storage.store(prompt.clone()).unwrap();

        let retrieved = storage.get("memory-test").unwrap();
        assert_eq!(retrieved.name, prompt.name);

        let prompts = storage.list().unwrap();
        assert_eq!(prompts.len(), 1);

        let results = storage.search("memory").unwrap();
        assert_eq!(results.len(), 1);

        storage.remove("memory-test").unwrap();
        assert!(storage.get("memory-test").is_err());
    }

    #[test]
    fn test_prompt_storage_file_system() {
        use crate::fs_utils::tests::MockFileSystem;
        use std::sync::Arc;

        let mock_fs = Arc::new(MockFileSystem::new());
        let fs_utils = FileSystemUtils::with_fs(mock_fs.clone());
        let filesystem_storage =
            Arc::new(FileSystemStorage::new_with_fs_utils("/test", fs_utils).unwrap());
        let mut storage = PromptStorage::new(filesystem_storage);
        let prompt = create_test_prompt("fs-storage-test", "Template");

        storage.store(prompt.clone()).unwrap();

        let retrieved = storage.get("fs-storage-test").unwrap();
        assert_eq!(retrieved.name, prompt.name);

        let prompts = storage.list().unwrap();
        assert_eq!(prompts.len(), 1);

        let results = storage.search("fs-storage").unwrap();
        assert_eq!(results.len(), 1);

        storage.remove("fs-storage-test").unwrap();
        assert!(storage.get("fs-storage-test").is_err());
    }

    #[test]
    fn test_prompt_storage_new_with_backend() {
        let backend = Arc::new(MemoryStorage::new());
        let storage = PromptStorage::new(backend);

        let prompts = storage.list().unwrap();
        assert_eq!(prompts.len(), 0);
    }

    #[test]
    fn test_storage_backend_exists_error_handling() {
        let storage = MemoryStorage::new();

        // Test exists with non-PromptNotFound error would be complex to set up
        // but we can at least test the happy paths
        assert!(!storage.exists("nonexistent").unwrap());
    }

    #[test]
    fn test_filesystem_storage_invalid_yaml_file() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();

        // Create an invalid YAML file manually
        let invalid_file = temp_dir.path().join("invalid.yaml");
        std::fs::write(&invalid_file, "invalid: yaml: content: [").unwrap();

        // Storage should handle invalid files gracefully during cache reload
        let storage = FileSystemStorage::new(temp_dir.path()).unwrap();
        assert_eq!(storage.list().unwrap().len(), 0);
    }

    #[test]
    fn test_filesystem_storage_non_yaml_files() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();

        // Create a non-YAML file
        let text_file = temp_dir.path().join("readme.txt");
        std::fs::write(&text_file, "This is not a YAML file").unwrap();

        // Storage should ignore non-YAML files
        let storage = FileSystemStorage::new(temp_dir.path()).unwrap();
        assert_eq!(storage.list().unwrap().len(), 0);
    }

    #[test]
    fn test_prompt_path_generation() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let storage = FileSystemStorage::new(temp_dir.path()).unwrap();

        let path = storage.prompt_path("test-prompt");
        assert_eq!(path, temp_dir.path().join("test-prompt.yaml"));
    }
}
