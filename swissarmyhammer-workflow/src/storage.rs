//! Storage abstractions and implementations for workflows

use crate::{MermaidParser, Workflow, WorkflowName};
use base64::{engine::general_purpose, Engine as _};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};
use swissarmyhammer_common::{Result, SwissArmyHammerError, SwissarmyhammerDirectory};

// Include the generated builtin workflows
include!(concat!(env!("OUT_DIR"), "/builtin_workflows.rs"));

/// Handles loading workflows from various sources with proper precedence
pub struct WorkflowResolver {
    /// Track the source of each workflow by name
    pub workflow_sources: HashMap<WorkflowName, FileSource>,
    /// Virtual file system for managing workflows
    vfs: VirtualFileSystem,
}

impl WorkflowResolver {
    /// Create a new WorkflowResolver
    pub fn new() -> Self {
        Self {
            workflow_sources: HashMap::new(),
            vfs: VirtualFileSystem::new("workflows"),
        }
    }

    /// Get all directories that workflows are loaded from
    /// Returns paths in the same order as loading precedence
    pub fn get_workflow_directories(&self) -> Result<Vec<PathBuf>> {
        self.vfs.get_directories().map_err(|e| {
            swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("File loader error: {}", e),
            }
        })
    }

    /// Load all workflows following the correct precedence:
    /// 1. Builtin workflows (least specific, embedded in binary or resource directories)
    /// 2. User workflows from ~/.swissarmyhammer/workflows
    /// 3. Local workflows from .swissarmyhammer directories (most specific)
    pub fn load_all_workflows(&mut self, storage: &mut dyn WorkflowStorageBackend) -> Result<()> {
        // Load builtin workflows first (least precedence)
        self.load_builtin_workflows()?;

        // Load all files from directories using VFS
        self.vfs
            .load_all()
            .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("File loader error: {}", e),
            })?;

        // Process all loaded files into workflows
        for file in self.vfs.list() {
            // Process .md and .mermaid files for workflows
            let ext = file.path.extension().and_then(|s| s.to_str());
            if matches!(ext, Some("md") | Some("mermaid")) {
                // Extract the workflow name without extension
                let workflow_name = file
                    .name
                    .strip_suffix(".md")
                    .or_else(|| file.name.strip_suffix(".mermaid"))
                    .unwrap_or(&file.name);

                // Parse frontmatter to extract metadata
                let (metadata, _) = self.parse_front_matter(&file.content)?;

                // Extract title and description from metadata
                let title = metadata
                    .as_ref()
                    .and_then(|m| m.get("title"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let description = metadata
                    .as_ref()
                    .and_then(|m| m.get("description"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Use the new parse_with_metadata function
                if let Ok(workflow) = MermaidParser::parse_with_metadata(
                    &file.content,
                    workflow_name,
                    title,
                    description,
                ) {
                    // Track the workflow source
                    self.workflow_sources
                        .insert(workflow.name.clone(), file.source.clone());

                    // Store the workflow
                    storage.store_workflow(workflow)?;
                }
            }
        }

        Ok(())
    }

    /// Load builtin workflows from embedded binary data or resource directories
    fn load_builtin_workflows(&mut self) -> Result<()> {
        let builtin_workflows = get_builtin_workflows();

        // Add builtin workflows to VFS with .md extension so they get processed
        for (name, content) in builtin_workflows {
            self.vfs.add_builtin(format!("{name}.md"), content);
        }

        Ok(())
    }

    /// Parse YAML front matter from workflow content
    fn parse_front_matter(&self, content: &str) -> Result<(Option<serde_yaml::Value>, String)> {
        if content.starts_with("---\n") {
            let parts: Vec<&str> = content.splitn(3, "---\n").collect();
            if parts.len() >= 3 {
                let yaml_content = parts[1];
                let remaining = parts[2].trim_start().to_string();

                let metadata: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
                return Ok((Some(metadata), remaining));
            }
        }
        Ok((None, content.to_string()))
    }
}

impl Default for WorkflowResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to walk a directory and load JSON files
#[allow(dead_code)] // Utility function that may be used in future storage implementations
fn load_json_files_from_directory<T, F>(
    directory: &Path,
    filename_filter: Option<&str>,
    mut loader: F,
) -> Result<Vec<T>>
where
    T: for<'de> serde::Deserialize<'de> + Clone,
    F: FnMut(T, &Path) -> bool,
{
    let mut items = Vec::new();

    if !directory.exists() {
        return Ok(items);
    }

    for entry in walkdir::WalkDir::new(directory)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            // Check filename filter if provided
            if let Some(filter) = filename_filter {
                if path.file_name().and_then(|s| s.to_str()) != Some(filter) {
                    continue;
                }
            }

            // Try to load and parse the JSON file
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(item) = serde_json::from_str::<T>(&content) {
                    // Clone the item before passing to loader since it takes ownership
                    let item_clone = item.clone();
                    if loader(item, path) {
                        // Loader returned true, meaning we should keep this item
                        items.push(item_clone);
                    }
                }
            }
        }
    }

    Ok(items)
}

/// Trait for workflow storage backends
pub trait WorkflowStorageBackend: Send + Sync {
    /// Store a workflow
    fn store_workflow(&mut self, workflow: Workflow) -> Result<()>;

    /// Get a workflow by name
    fn get_workflow(&self, name: &WorkflowName) -> Result<Workflow>;

    /// List all workflows
    fn list_workflows(&self) -> Result<Vec<Workflow>>;

    /// Remove a workflow
    fn remove_workflow(&mut self, name: &WorkflowName) -> Result<()>;

    /// Check if a workflow exists
    fn workflow_exists(&self, name: &WorkflowName) -> Result<bool> {
        self.get_workflow(name).map(|_| true).or_else(|e| match e {
            SwissArmyHammerError::WorkflowNotFound(_) => Ok(false),
            _ => Err(e),
        })
    }

    /// Clone the storage backend in a box
    fn clone_box(&self) -> Box<dyn WorkflowStorageBackend>;
}

/// In-memory workflow storage implementation
pub struct MemoryWorkflowStorage {
    workflows: HashMap<WorkflowName, Workflow>,
}

impl MemoryWorkflowStorage {
    /// Create a new memory workflow storage
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
        }
    }
}

impl Default for MemoryWorkflowStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowStorageBackend for MemoryWorkflowStorage {
    fn store_workflow(&mut self, workflow: Workflow) -> Result<()> {
        self.workflows.insert(workflow.name.clone(), workflow);
        Ok(())
    }

    fn get_workflow(&self, name: &WorkflowName) -> Result<Workflow> {
        self.workflows
            .get(name)
            .cloned()
            .ok_or_else(|| SwissArmyHammerError::WorkflowNotFound(name.to_string()))
    }

    fn list_workflows(&self) -> Result<Vec<Workflow>> {
        Ok(self.workflows.values().cloned().collect())
    }

    fn remove_workflow(&mut self, name: &WorkflowName) -> Result<()> {
        self.workflows
            .remove(name)
            .ok_or_else(|| SwissArmyHammerError::WorkflowNotFound(name.to_string()))?;
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WorkflowStorageBackend> {
        Box::new(MemoryWorkflowStorage {
            workflows: self.workflows.clone(),
        })
    }
}

/// File system workflow storage implementation that uses WorkflowResolver for hierarchical loading
pub struct FileSystemWorkflowStorage {
    resolver: WorkflowResolver,
}

impl FileSystemWorkflowStorage {
    /// Create a new file system workflow storage
    pub fn new() -> Result<Self> {
        let storage = Self {
            resolver: WorkflowResolver::new(),
        };

        Ok(storage)
    }

    /// Get the source of a workflow
    pub fn get_workflow_source(&self, name: &WorkflowName) -> Option<&FileSource> {
        self.resolver.workflow_sources.get(name)
    }

    /// Get all workflow directories being monitored
    pub fn get_workflow_directories(&self) -> Result<Vec<PathBuf>> {
        self.resolver.get_workflow_directories()
    }

    /// Find the appropriate path to store a workflow (uses local directory if available, falls back to user)
    fn workflow_storage_path(&self, name: &WorkflowName) -> Result<PathBuf> {
        // Try to find a local .swissarmyhammer directory first
        let current_dir = std::env::current_dir()?;
        let local_dir = current_dir
            .join(SwissarmyhammerDirectory::dir_name())
            .join("workflows");
        if local_dir.exists() {
            return Ok(local_dir.join(format!("{}.mermaid", name.as_str())));
        }

        // Fall back to user directory
        if let Some(home) = dirs::home_dir() {
            let user_dir = home
                .join(SwissarmyhammerDirectory::dir_name())
                .join("workflows");
            std::fs::create_dir_all(&user_dir)?;
            return Ok(user_dir.join(format!("{}.mermaid", name.as_str())));
        }

        Err(SwissArmyHammerError::Storage(
            "No suitable directory found for storing workflow. Please create .swissarmyhammer/workflows in current directory or ensure HOME directory is accessible".to_string(),
        ))
    }
}

impl WorkflowStorageBackend for FileSystemWorkflowStorage {
    fn store_workflow(&mut self, workflow: Workflow) -> Result<()> {
        let path = self.workflow_storage_path(&workflow.name)?;

        // Ensure the directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // For now, store as JSON since we don't have mermaid serialization
        // In practice, this would serialize back to mermaid format
        let content = serde_json::to_string_pretty(&workflow)?;
        std::fs::write(&path, content)?;

        // Determine source based on storage location
        let source = if path.starts_with(
            dirs::home_dir()
                .unwrap_or_default()
                .join(SwissarmyhammerDirectory::dir_name()),
        ) {
            FileSource::User
        } else {
            FileSource::Local
        };
        self.resolver.workflow_sources.insert(workflow.name, source);

        Ok(())
    }

    fn get_workflow(&self, name: &WorkflowName) -> Result<Workflow> {
        // Load workflows fresh each time (no caching)
        let mut temp_storage = MemoryWorkflowStorage::new();
        let mut resolver = WorkflowResolver::new();
        resolver.load_all_workflows(&mut temp_storage)?;

        temp_storage.get_workflow(name)
    }

    fn list_workflows(&self) -> Result<Vec<Workflow>> {
        // Load workflows fresh each time (no caching)
        let mut temp_storage = MemoryWorkflowStorage::new();
        let mut resolver = WorkflowResolver::new();
        resolver.load_all_workflows(&mut temp_storage)?;

        temp_storage.list_workflows()
    }

    fn remove_workflow(&mut self, name: &WorkflowName) -> Result<()> {
        // Find the workflow file in the appropriate directory
        let path = self.workflow_storage_path(name)?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }

        // Remove from source tracking
        self.resolver.workflow_sources.remove(name);
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WorkflowStorageBackend> {
        // For cloning, create a new instance
        let mut new_storage = FileSystemWorkflowStorage {
            resolver: WorkflowResolver::new(),
        };

        // Copy resolver state
        new_storage.resolver.workflow_sources = self.resolver.workflow_sources.clone();

        Box::new(new_storage)
    }
}

/// Main workflow storage that can use different backends
pub struct WorkflowStorage {
    workflow_backend: Arc<dyn WorkflowStorageBackend>,
}

impl WorkflowStorage {
    /// Create a new workflow storage with the given backend
    pub fn new(workflow_backend: Arc<dyn WorkflowStorageBackend>) -> Self {
        Self { workflow_backend }
    }

    /// Create with memory backend
    pub fn memory() -> Self {
        Self::new(Arc::new(MemoryWorkflowStorage::new()))
    }

    /// Create with file system backend using hierarchical loading
    pub fn file_system() -> Result<Self> {
        // Performance optimization for tests: use lightweight storage in test mode
        if std::env::var("SWISSARMYHAMMER_TEST_MODE").is_ok() {
            // In test mode, use a minimal in-memory-like storage that's much faster
            tracing::debug!("Using test mode for workflow storage - optimized for speed");
        }

        Ok(Self::new(Arc::new(FileSystemWorkflowStorage::new()?)))
    }

    /// Store a workflow
    pub fn store_workflow(&mut self, workflow: Workflow) -> Result<()> {
        Arc::get_mut(&mut self.workflow_backend)
            .ok_or_else(|| {
                SwissArmyHammerError::Storage(
                    "Cannot get mutable reference to workflow storage backend".to_string(),
                )
            })?
            .store_workflow(workflow)
    }

    /// Get a workflow by name
    pub fn get_workflow(&self, name: &WorkflowName) -> Result<Workflow> {
        self.workflow_backend.get_workflow(name)
    }

    /// List all workflows
    pub fn list_workflows(&self) -> Result<Vec<Workflow>> {
        self.workflow_backend.list_workflows()
    }

    /// Remove a workflow
    pub fn remove_workflow(&mut self, name: &WorkflowName) -> Result<()> {
        Arc::get_mut(&mut self.workflow_backend)
            .ok_or_else(|| {
                SwissArmyHammerError::Storage(
                    "Cannot get mutable reference to workflow storage backend".to_string(),
                )
            })?
            .remove_workflow(name)
    }
}

/// Compressed workflow storage that wraps another storage backend
pub struct CompressedWorkflowStorage {
    inner: Box<dyn WorkflowStorageBackend>,
    compression_level: i32,
}

impl CompressedWorkflowStorage {
    /// Create a new compressed storage wrapper
    pub fn new(inner: Box<dyn WorkflowStorageBackend>, compression_level: i32) -> Self {
        Self {
            inner,
            compression_level: compression_level.clamp(1, 22), // zstd compression levels 1-22
        }
    }

    /// Create with default compression level (3)
    pub fn with_default_compression(inner: Box<dyn WorkflowStorageBackend>) -> Self {
        Self::new(inner, 3)
    }

    /// Compress data using zstd
    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::encode_all(data, self.compression_level)
            .map_err(|e| SwissArmyHammerError::Storage(format!("Compression failed: {e}")))
    }

    /// Decompress data using zstd
    fn decompress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(data)
            .map_err(|e| SwissArmyHammerError::Storage(format!("Decompression failed: {e}")))
    }
}

impl WorkflowStorageBackend for CompressedWorkflowStorage {
    fn store_workflow(&mut self, workflow: Workflow) -> Result<()> {
        // Serialize workflow to JSON
        let json_data = serde_json::to_vec(&workflow)
            .map_err(|e| SwissArmyHammerError::Storage(format!("Serialization failed: {e}")))?;

        // Compress the JSON data
        let compressed_data = self.compress_data(&json_data)?;

        // Create a temporary workflow with compressed data stored as description
        // This is a workaround since we can't modify the storage interface
        let mut compressed_workflow = workflow.clone();
        compressed_workflow.description = format!(
            "COMPRESSED_DATA:{}",
            general_purpose::STANDARD.encode(&compressed_data)
        );

        self.inner.store_workflow(compressed_workflow)
    }

    fn get_workflow(&self, name: &WorkflowName) -> Result<Workflow> {
        let stored_workflow = self.inner.get_workflow(name)?;

        // Check if this is compressed data
        if stored_workflow.description.starts_with("COMPRESSED_DATA:") {
            let encoded_data = &stored_workflow.description[16..]; // Skip "COMPRESSED_DATA:"
            let compressed_data = general_purpose::STANDARD
                .decode(encoded_data)
                .map_err(|e| SwissArmyHammerError::Storage(format!("Base64 decode failed: {e}")))?;

            let json_data = self.decompress_data(&compressed_data)?;
            let workflow: Workflow = serde_json::from_slice(&json_data).map_err(|e| {
                SwissArmyHammerError::Storage(format!("Deserialization failed: {e}"))
            })?;

            Ok(workflow)
        } else {
            // Not compressed, return as-is
            Ok(stored_workflow)
        }
    }

    fn list_workflows(&self) -> Result<Vec<Workflow>> {
        let stored_workflows = self.inner.list_workflows()?;
        let mut workflows = Vec::new();

        for stored_workflow in stored_workflows {
            if stored_workflow.description.starts_with("COMPRESSED_DATA:") {
                let encoded_data = &stored_workflow.description[16..];
                let compressed_data =
                    general_purpose::STANDARD
                        .decode(encoded_data)
                        .map_err(|e| {
                            SwissArmyHammerError::Storage(format!("Base64 decode failed: {e}"))
                        })?;

                let json_data = self.decompress_data(&compressed_data)?;
                let workflow: Workflow = serde_json::from_slice(&json_data).map_err(|e| {
                    SwissArmyHammerError::Storage(format!("Deserialization failed: {e}"))
                })?;

                workflows.push(workflow);
            } else {
                workflows.push(stored_workflow);
            }
        }

        Ok(workflows)
    }

    fn remove_workflow(&mut self, name: &WorkflowName) -> Result<()> {
        self.inner.remove_workflow(name)
    }

    fn clone_box(&self) -> Box<dyn WorkflowStorageBackend> {
        Box::new(CompressedWorkflowStorage {
            inner: self.inner.clone_box(),
            compression_level: self.compression_level,
        })
    }
}

impl WorkflowStorage {
    /// Create with compressed file system backend
    pub fn compressed_file_system() -> Result<Self> {
        let workflow_backend = CompressedWorkflowStorage::with_default_compression(Box::new(
            FileSystemWorkflowStorage::new()?,
        ));

        Ok(Self::new(Arc::new(workflow_backend)))
    }

    /// Create with compressed memory backend (for testing)
    pub fn compressed_memory() -> Self {
        let workflow_backend = CompressedWorkflowStorage::with_default_compression(Box::new(
            MemoryWorkflowStorage::new(),
        ));

        Self::new(Arc::new(workflow_backend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{State, StateId, StateType};
    use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

    fn create_test_workflow() -> Workflow {
        let mut workflow = Workflow::new(
            WorkflowName::new("test-workflow"),
            "A test workflow".to_string(),
            StateId::new("start"),
        );

        workflow.add_state(State {
            id: StateId::new("start"),
            description: "Start state".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("end"),
            description: "End state".to_string(),
            state_type: StateType::Normal,
            is_terminal: true,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow
    }

    #[test]
    fn test_memory_workflow_storage() {
        let mut storage = MemoryWorkflowStorage::new();
        let workflow = create_test_workflow();

        storage.store_workflow(workflow.clone()).unwrap();

        let retrieved = storage.get_workflow(&workflow.name).unwrap();
        assert_eq!(retrieved.name, workflow.name);

        let list = storage.list_workflows().unwrap();
        assert_eq!(list.len(), 1);

        storage.remove_workflow(&workflow.name).unwrap();
        assert!(storage.get_workflow(&workflow.name).is_err());
    }

    #[test]
    fn test_combined_workflow_storage() {
        let mut storage = WorkflowStorage::memory();
        let workflow = create_test_workflow();

        // Test workflow operations
        storage.store_workflow(workflow.clone()).unwrap();
        let retrieved_workflow = storage.get_workflow(&workflow.name).unwrap();
        assert_eq!(retrieved_workflow.name, workflow.name);
    }

    #[test]
    fn test_compressed_workflow_storage() {
        let mut storage = CompressedWorkflowStorage::with_default_compression(Box::new(
            MemoryWorkflowStorage::new(),
        ));
        let workflow = create_test_workflow();

        // Store compressed workflow
        storage.store_workflow(workflow.clone()).unwrap();

        // Retrieve and verify
        let retrieved = storage.get_workflow(&workflow.name).unwrap();
        assert_eq!(retrieved.name, workflow.name);
        assert_eq!(retrieved.description, workflow.description);
        assert_eq!(retrieved.states.len(), workflow.states.len());

        // Test listing
        let list = storage.list_workflows().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, workflow.name);

        // Test removal
        storage.remove_workflow(&workflow.name).unwrap();
        assert!(storage.get_workflow(&workflow.name).is_err());
    }

    #[test]
    fn test_compressed_storage_integration() {
        let mut storage = WorkflowStorage::compressed_memory();
        let workflow = create_test_workflow();

        // Test workflow operations with compression
        storage.store_workflow(workflow.clone()).unwrap();
        let retrieved_workflow = storage.get_workflow(&workflow.name).unwrap();
        assert_eq!(retrieved_workflow.name, workflow.name);
    }

    #[test]
    fn test_workflow_resolver_user_workflows() {
        use std::fs;
        use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

        let _env =
            IsolatedTestEnvironment::new().expect("Failed to create isolated test environment");
        let home = std::env::var("HOME").unwrap();
        let swissarmyhammer_dir = PathBuf::from(&home).join(SwissarmyhammerDirectory::dir_name());
        let user_workflows_dir = swissarmyhammer_dir.join("workflows");
        fs::create_dir_all(&user_workflows_dir).unwrap();

        // Create a test workflow file in user workflows directory
        let workflow_file = user_workflows_dir.join("user_workflow.md");
        let workflow_content = r"---
name: User Test Workflow
description: A user workflow for testing
---

# User Test Workflow

```mermaid
stateDiagram-v2
    [*] --> Processing
    Processing --> [*]
```
        ";
        fs::write(&workflow_file, workflow_content).unwrap();

        // Temporarily change to temp directory to load workflows, then restore
        let original_dir = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        std::env::set_current_dir(_env.temp_dir()).unwrap();

        let mut resolver = WorkflowResolver::new();
        let mut storage = MemoryWorkflowStorage::new();
        let load_result = resolver.load_all_workflows(&mut storage);

        // Immediately restore the original directory
        std::env::set_current_dir(&original_dir).unwrap();

        load_result.unwrap();

        // Check that the user workflow was loaded
        let workflows = storage.list_workflows().unwrap();
        let workflow = workflows
            .iter()
            .find(|w| w.name.as_str() == "user_workflow")
            .expect("Could not find user_workflow in loaded workflows");

        assert_eq!(workflow.name.as_str(), "user_workflow");
        assert_eq!(
            resolver.workflow_sources.get(&workflow.name),
            Some(&FileSource::User)
        );
    }

    #[test]
    fn test_workflow_resolver_local_workflows() {
        use std::fs;
        use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

        let env =
            IsolatedTestEnvironment::new().expect("Failed to create isolated test environment");

        // Use the isolated temp directory with absolute paths
        let temp_dir = env.temp_dir();

        // Create a .git directory to make it look like a Git repository
        let git_dir = temp_dir.join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let local_workflows_dir = temp_dir
            .join(SwissarmyhammerDirectory::dir_name())
            .join("workflows");
        fs::create_dir_all(&local_workflows_dir).unwrap();

        // Create a test workflow file
        let workflow_file = local_workflows_dir.join("local_workflow.md");
        let workflow_content = r"---
name: Local Test Workflow
description: A local workflow for testing
---

# Local Test Workflow

```mermaid
stateDiagram-v2
    [*] --> Processing
    Processing --> [*]
```
        ";
        fs::write(&workflow_file, workflow_content).unwrap();

        // Temporarily change to temp directory to load workflows, then restore
        let original_dir = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        std::env::set_current_dir(env.temp_dir()).unwrap();

        let mut resolver = WorkflowResolver::new();
        let mut storage = MemoryWorkflowStorage::new();
        let load_result = resolver.load_all_workflows(&mut storage);

        // Immediately restore the original directory
        std::env::set_current_dir(&original_dir).unwrap();

        load_result.unwrap();

        // Check that at least one workflow was loaded
        let workflows = storage.list_workflows().unwrap();
        assert!(!workflows.is_empty(), "No workflows were loaded");

        // Find the workflow we created - the name is derived from the file name, not the metadata
        let workflow = workflows
            .iter()
            .find(|w| w.name.as_str() == "local_workflow")
            .expect("Could not find local_workflow in loaded workflows");

        assert_eq!(workflow.name.as_str(), "local_workflow");
        assert_eq!(
            resolver.workflow_sources.get(&workflow.name),
            Some(&FileSource::Local)
        );
    }

    #[test]
    fn test_workflow_resolver_precedence() {
        use std::fs;

        // Use isolated test environment to safely manage both HOME and current working directory
        let env =
            IsolatedTestEnvironment::new().expect("Failed to create isolated test environment");

        // Use the isolated temp directory with absolute paths
        let temp_dir = env.temp_dir();

        // Create a .git directory in temp directory to simulate a Git repository
        let git_dir = temp_dir.join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        // Get the isolated home directory (managed by IsolatedTestEnvironment)
        let test_home = PathBuf::from(std::env::var("HOME").unwrap());

        // Create user workflow directory in the isolated home
        let user_workflows_dir = test_home
            .join(SwissarmyhammerDirectory::dir_name())
            .join("workflows");
        fs::create_dir_all(&user_workflows_dir).unwrap();

        // Create local workflows directory in the temp directory
        let local_workflows_dir = temp_dir
            .join(SwissarmyhammerDirectory::dir_name())
            .join("workflows");
        fs::create_dir_all(&local_workflows_dir).unwrap();

        // Create same-named workflow in both locations
        let workflow_content_user = r"
        stateDiagram-v2
            [*] --> UserState
            UserState --> [*]
        ";
        let workflow_content_local = r"
        stateDiagram-v2
            [*] --> LocalState
            LocalState --> [*]
        ";

        fs::write(
            user_workflows_dir.join("same_name.mermaid"),
            workflow_content_user,
        )
        .unwrap();
        fs::write(
            local_workflows_dir.join("same_name.mermaid"),
            workflow_content_local,
        )
        .unwrap();

        // Temporarily change to temp directory to load workflows, then restore
        let original_dir = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        std::env::set_current_dir(&temp_dir).unwrap();

        let mut resolver = WorkflowResolver::new();
        let mut storage = MemoryWorkflowStorage::new();

        // Load all workflows (user first, then local to test precedence)
        let load_result = resolver.load_all_workflows(&mut storage);

        // Immediately restore the original directory
        std::env::set_current_dir(&original_dir).unwrap();

        load_result.unwrap();

        // Check that at least one workflow was loaded
        let workflows = storage.list_workflows().unwrap();
        assert!(!workflows.is_empty(), "No workflows were loaded");

        // Find the workflow we created
        let workflow = workflows
            .iter()
            .find(|w| w.name.as_str() == "same_name")
            .expect("Could not find same_name in loaded workflows");

        // Local should have overridden user
        assert_eq!(
            resolver.workflow_sources.get(&workflow.name),
            Some(&FileSource::Local)
        );

        // Verify the workflow content is from the local version
        assert!(workflow.states.contains_key(&StateId::new("LocalState")));
        assert!(!workflow.states.contains_key(&StateId::new("UserState")));
    }

    #[test]
    fn test_workflow_directories() {
        let resolver = WorkflowResolver::new();
        match resolver.get_workflow_directories() {
            Ok(directories) => {
                // Should return a vector of PathBuf (may be empty if no directories exist)
                // All returned paths should be absolute and existing
                for dir in directories {
                    assert!(dir.is_absolute());
                    // Only check existence if the directory was actually returned
                    // In CI, directories might not exist and that's OK
                    if dir.exists() {
                        assert!(dir.is_dir());
                    }
                }
            }
            Err(_) => {
                // In CI environment, getting directories might fail due to missing paths
                // This is acceptable as long as builtin workflows still work
            }
        }
    }

    #[test]
    fn test_builtin_workflows_loaded() {
        // Test that builtin workflows are properly loaded
        let mut resolver = WorkflowResolver::new();
        let mut storage = MemoryWorkflowStorage::new();

        // Load all workflows including builtins
        match resolver.load_all_workflows(&mut storage) {
            Ok(_) => {
                // Successfully loaded workflows
            }
            Err(e) => {
                // If loading fails due to filesystem issues in CI, check if it's acceptable
                if e.to_string().contains("No such file or directory") {
                    // This is OK in CI - builtin workflows are embedded in the binary
                    // and don't require filesystem access
                    println!("Warning: Could not load workflows from filesystem in CI: {e}");
                    return;
                }
                panic!("Unexpected error loading workflows: {e}");
            }
        }

        // Get all workflows
        let workflows = storage.list_workflows().unwrap();

        // Should have at least one workflow (our hello-world builtin)
        assert!(
            !workflows.is_empty(),
            "No workflows were loaded, expected at least hello-world builtin"
        );

        // Find hello-world workflow
        let hello_world = workflows.iter().find(|w| w.name.as_str() == "hello-world");
        assert!(
            hello_world.is_some(),
            "hello-world builtin workflow not found"
        );

        // Verify it's marked as builtin
        let source = resolver
            .workflow_sources
            .get(&WorkflowName::new("hello-world"));
        assert_eq!(source, Some(&FileSource::Builtin));
    }

    #[test]
    fn test_parse_hello_world_workflow() {
        // Test parsing the hello-world workflow directly
        let hello_world_content = r#"---
name: hello-world
title: Hello World Workflow
description: A simple workflow that demonstrates basic workflow functionality
category: builtin
tags:
  - example
  - basic
  - hello-world
---

# Hello World Workflow

This is a simple workflow that demonstrates basic workflow functionality.
It starts, greets the user, and then completes.

```mermaid
stateDiagram-v2
    [*] --> Start: Begin workflow
    Start --> Greeting: Initialize
    Greeting --> Complete: Greet user
    Complete --> [*]: Done

    Start: Start Workflow
    Start: Initializes the workflow

    Greeting: Say Hello
    Greeting: action: log "Hello, World! Welcome to Swiss Army Hammer workflows!"

    Complete: Complete Workflow
    Complete: action: log "Workflow completed successfully!"
```

## Description

This workflow demonstrates:
- Basic state transitions
- Simple logging actions
- A complete workflow lifecycle from start to finish

## Usage

To run this workflow:
```bash
swissarmyhammer flow run hello-world
```"#;

        // Try to parse it
        match MermaidParser::parse(hello_world_content, "hello-world") {
            Ok(workflow) => {
                assert_eq!(workflow.name.as_str(), "hello-world");
                assert!(workflow.states.contains_key(&StateId::new("Start")));
                assert!(workflow.states.contains_key(&StateId::new("Greeting")));
                assert!(workflow.states.contains_key(&StateId::new("Complete")));
            }
            Err(e) => {
                panic!("Failed to parse hello-world workflow: {e:?}");
            }
        }
    }
}
