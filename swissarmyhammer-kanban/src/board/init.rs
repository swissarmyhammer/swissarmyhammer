//! InitBoard command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::default_column_entities;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Merge driver registration configuration for a single driver.
struct MergeDriver {
    /// The git merge driver name (e.g. `kanban-jsonl`)
    name: &'static str,
    /// The glob pattern for `.gitattributes` (e.g. `.kanban/**/*.jsonl`)
    pattern: &'static str,
    /// The driver command string (e.g. `kanban merge jsonl %O %A %B`)
    command: &'static str,
}

/// All merge drivers to register when a board is initialized.
const MERGE_DRIVERS: &[MergeDriver] = &[
    MergeDriver {
        name: "kanban-jsonl",
        pattern: ".kanban/**/*.jsonl",
        command: "kanban merge jsonl %O %A %B",
    },
    MergeDriver {
        name: "kanban-yaml",
        pattern: ".kanban/**/*.yaml",
        command: "kanban merge yaml %O %A %B",
    },
    MergeDriver {
        name: "kanban-md",
        pattern: ".kanban/**/*.md",
        command: "kanban merge md %O %A %B",
    },
];

/// Walk up the directory tree from `start` looking for a `.git` directory.
///
/// Returns the path to the directory that *contains* `.git`, or `None` if not
/// inside a git repository.
fn find_git_root(start: &Path) -> Option<std::path::PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

/// Register git merge drivers in `.git/config` and `.gitattributes`.
///
/// Silently skips if the board root is not inside a git repository. The
/// operation is idempotent — running it multiple times will not duplicate
/// any configuration entries.
///
/// # Parameters
/// - `board_root`: the `.kanban/` directory (parent is the project root)
async fn register_merge_drivers(board_root: &Path) -> Result<(), std::io::Error> {
    // The project root is the parent of .kanban/
    let project_root = match board_root.parent() {
        Some(p) => p.to_path_buf(),
        None => return Ok(()), // No parent — can't determine project root
    };

    // Walk up to find the git repo root
    let git_root = match find_git_root(&project_root) {
        Some(r) => r,
        None => return Ok(()), // Not in a git repository — skip silently
    };

    let git_config_path = git_root.join(".git").join("config");

    // ------------------------------------------------------------------
    // Update .git/config — register each merge driver if not present
    // ------------------------------------------------------------------
    let existing_config = tokio::fs::read_to_string(&git_config_path)
        .await
        .unwrap_or_default();

    let mut config = existing_config.clone();

    for driver in MERGE_DRIVERS {
        let section_header = format!("[merge \"{}\"]", driver.name);
        if !config.contains(&section_header) {
            // Append driver section
            if !config.ends_with('\n') && !config.is_empty() {
                config.push('\n');
            }
            config.push_str(&format!("{}\n", section_header));
            config.push_str(&format!("\tdriver = {}\n", driver.command));
        }
    }

    if config != existing_config {
        tokio::fs::write(&git_config_path, &config).await?;
    }

    // ------------------------------------------------------------------
    // Update .gitattributes — add patterns if not already present
    // ------------------------------------------------------------------
    let gitattributes_path = git_root.join(".gitattributes");

    let existing_attrs = tokio::fs::read_to_string(&gitattributes_path)
        .await
        .unwrap_or_default();

    let mut attrs = existing_attrs.clone();

    for driver in MERGE_DRIVERS {
        let line = format!("{} merge={}", driver.pattern, driver.name);
        if !attrs.contains(&line) {
            if !attrs.ends_with('\n') && !attrs.is_empty() {
                attrs.push('\n');
            }
            attrs.push_str(&line);
            attrs.push('\n');
        }
    }

    if attrs != existing_attrs {
        tokio::fs::write(&gitattributes_path, &attrs).await?;
    }

    Ok(())
}

/// Initialize a new kanban board
#[operation(
    verb = "init",
    noun = "board",
    description = "Initialize a new kanban board"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct InitBoard {
    /// The board name
    pub name: String,
    /// Optional board description
    pub description: Option<String>,
}

impl InitBoard {
    /// Create a new InitBoard command
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for InitBoard {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            // Check if already initialized
            if ctx.is_initialized() {
                return Err(KanbanError::AlreadyExists {
                    path: ctx.root().to_path_buf(),
                });
            }

            // Create directory structure
            ctx.create_directories().await?;

            // Build board entity
            let ectx = ctx.entity_context().await?;
            let mut board_entity = Entity::new("board", "board");
            board_entity.set("name", json!(self.name));
            if let Some(desc) = &self.description {
                board_entity.set("description", json!(desc));
            }
            ectx.write(&board_entity).await?;

            // Write default columns as entities
            let default_cols = default_column_entities();
            let mut columns_json: Vec<Value> = Vec::new();
            for entity in &default_cols {
                ectx.write(entity).await?;
                columns_json.push(json!({
                    "id": entity.id,
                    "name": entity.get_str("name").unwrap_or(""),
                    "order": entity.get_i64("order").unwrap_or(0),
                }));
            }

            // Register git merge drivers (.git/config + .gitattributes).
            // Silently ignored if not inside a git repository.
            register_merge_drivers(ctx.root())
                .await
                .map_err(KanbanError::Io)?;

            // Return board with columns in response (for API compatibility)
            Ok(json!({
                "name": self.name,
                "description": self.description,
                "columns": columns_json,
                "swimlanes": [],
            }))
        }
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(value) => ExecutionResult::Logged {
                value: value.clone(),
                log_entry: LogEntry::new(self.op_string(), input, value, None, duration_ms),
            },
            Err(error) => {
                let error_msg = error.to_string();
                ExecutionResult::Failed {
                    error,
                    log_entry: Some(LogEntry::new(
                        self.op_string(),
                        input,
                        serde_json::json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a temp dir WITHOUT a git repo
    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        (temp, ctx)
    }

    /// Create a temp dir WITH a git repo (bare `.git/config` only)
    async fn setup_git() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        // Create minimal git structure
        let git_dir = temp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        // Write minimal .git/config
        std::fs::write(
            git_dir.join("config"),
            "[core]\n\trepositoryformatversion = 0\n",
        )
        .unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_init_board() {
        let (_temp, ctx) = setup().await;

        let cmd = InitBoard::new("Test Board").with_description("A test board");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["name"], "Test Board");
        assert_eq!(result["description"], "A test board");
        assert!(result["columns"].is_array());
        let columns = result["columns"].as_array().unwrap();
        assert_eq!(columns.len(), 3);
        // Verify column IDs are present
        for col in columns {
            assert!(col["id"].is_string(), "Column should have id field");
        }
    }

    #[tokio::test]
    async fn test_init_board_already_exists() {
        let (_temp, ctx) = setup().await;

        // First init should succeed
        let cmd = InitBoard::new("Test");
        cmd.execute(&ctx).await.into_result().unwrap();

        // Second init should fail
        let result = cmd.execute(&ctx).await.into_result();
        assert!(matches!(result, Err(KanbanError::AlreadyExists { .. })));
    }

    #[test]
    fn test_operation_metadata() {
        use swissarmyhammer_operations::Operation;

        // Create an instance to test Operation trait methods
        let op = InitBoard::new("test");

        // Verify the Operation trait is correctly implemented via macro
        assert_eq!(op.verb(), "init");
        assert_eq!(op.noun(), "board");
        assert_eq!(op.description(), "Initialize a new kanban board");

        // Verify parameters
        let params = op.parameters();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "name");
        assert!(params[0].required);
        assert_eq!(params[1].name, "description");
        assert!(!params[1].required);
    }

    #[tokio::test]
    async fn test_init_board_registers_git_config() {
        let (temp, ctx) = setup_git().await;

        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        // Verify .git/config contains all three merge driver sections
        let config = std::fs::read_to_string(temp.path().join(".git").join("config")).unwrap();
        for driver in MERGE_DRIVERS {
            let section = format!("[merge \"{}\"]", driver.name);
            assert!(
                config.contains(&section),
                ".git/config should contain {section}"
            );
            assert!(
                config.contains(driver.command),
                ".git/config should contain driver command '{}'",
                driver.command
            );
        }
    }

    #[tokio::test]
    async fn test_init_board_creates_gitattributes() {
        let (temp, ctx) = setup_git().await;

        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        // Verify .gitattributes contains all three patterns
        let attrs = std::fs::read_to_string(temp.path().join(".gitattributes")).unwrap();
        for driver in MERGE_DRIVERS {
            let line = format!("{} merge={}", driver.pattern, driver.name);
            assert!(
                attrs.contains(&line),
                ".gitattributes should contain '{line}'"
            );
        }
    }

    #[tokio::test]
    async fn test_init_board_idempotent_git_config() {
        let (temp, ctx) = setup_git().await;

        // Init once — populate
        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        let config_after_first =
            std::fs::read_to_string(temp.path().join(".git").join("config")).unwrap();

        // Re-running register_merge_drivers directly a second time should not duplicate
        let kanban_dir = ctx.root().to_path_buf();
        register_merge_drivers(&kanban_dir).await.unwrap();

        let config_after_second =
            std::fs::read_to_string(temp.path().join(".git").join("config")).unwrap();

        assert_eq!(
            config_after_first, config_after_second,
            ".git/config should not change on second registration"
        );
    }

    #[tokio::test]
    async fn test_init_board_idempotent_gitattributes() {
        let (temp, ctx) = setup_git().await;

        // Init once — populate
        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        let attrs_after_first =
            std::fs::read_to_string(temp.path().join(".gitattributes")).unwrap();

        // Re-running register_merge_drivers directly a second time should not duplicate
        let kanban_dir = ctx.root().to_path_buf();
        register_merge_drivers(&kanban_dir).await.unwrap();

        let attrs_after_second =
            std::fs::read_to_string(temp.path().join(".gitattributes")).unwrap();

        assert_eq!(
            attrs_after_first, attrs_after_second,
            ".gitattributes should not change on second registration"
        );
    }

    #[tokio::test]
    async fn test_init_board_no_git_repo_skips_silently() {
        // Temp dir with NO .git directory
        let (temp, ctx) = setup().await;

        // Should succeed without error even though there's no git repo
        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        // No .git/config or .gitattributes should have been created
        assert!(
            !temp.path().join(".gitattributes").exists(),
            ".gitattributes should not exist when not in a git repo"
        );
    }

    #[test]
    fn test_find_git_root_walks_up() {
        let temp = TempDir::new().unwrap();
        // Create .git in temp root
        std::fs::create_dir_all(temp.path().join(".git")).unwrap();
        // Start searching from a subdirectory
        let subdir = temp.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&subdir).unwrap();

        let found = find_git_root(&subdir);
        assert_eq!(found, Some(temp.path().to_path_buf()));
    }

    #[test]
    fn test_find_git_root_not_found() {
        // Use a temp dir with no .git anywhere above it
        // We can't guarantee no .git above /tmp, so create one inside and check
        // that a sibling dir returns None when there's no .git
        let found = find_git_root(std::path::Path::new("/"));
        assert_eq!(found, None);
    }

    /// Verify that a board root with no parent returns Ok(()) silently.
    #[tokio::test]
    async fn test_register_merge_drivers_no_parent() {
        // Path::new("/") has no parent — the function must return Ok(()) immediately.
        let result = register_merge_drivers(std::path::Path::new("/")).await;
        assert!(
            result.is_ok(),
            "register_merge_drivers with no parent should return Ok(())"
        );
    }

    /// Verify that a read-only `.git/config` causes register_merge_drivers to
    /// return an Err(io::Error) rather than panicking or silently succeeding.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_register_merge_drivers_readonly_git_config() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        let config_path = git_dir.join("config");
        std::fs::write(&config_path, "[core]\n\trepositoryformatversion = 0\n").unwrap();

        // Make .git/config read-only so the write will fail
        let mut perms = std::fs::metadata(&config_path).unwrap().permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&config_path, perms).unwrap();

        let kanban_dir = temp.path().join(".kanban");
        let result = register_merge_drivers(&kanban_dir).await;

        // Restore permissions so TempDir can clean up
        let mut perms = std::fs::metadata(&config_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&config_path, perms).unwrap();

        assert!(
            result.is_err(),
            "register_merge_drivers should propagate the I/O error from a read-only .git/config"
        );
    }

    /// Verify that a read-only `.gitattributes` causes register_merge_drivers to
    /// return an Err(io::Error).
    #[cfg(unix)]
    #[tokio::test]
    async fn test_register_merge_drivers_readonly_gitattributes() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            git_dir.join("config"),
            "[core]\n\trepositoryformatversion = 0\n",
        )
        .unwrap();

        // Create a read-only .gitattributes so the write to it will fail.
        // The .git/config must be writable so the first write succeeds and we
        // reach the .gitattributes write path.
        let attrs_path = temp.path().join(".gitattributes");
        std::fs::write(&attrs_path, "# existing content\n").unwrap();
        let mut perms = std::fs::metadata(&attrs_path).unwrap().permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&attrs_path, perms).unwrap();

        let kanban_dir = temp.path().join(".kanban");
        let result = register_merge_drivers(&kanban_dir).await;

        // Restore permissions so TempDir can clean up
        let mut perms = std::fs::metadata(&attrs_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&attrs_path, perms).unwrap();

        assert!(
            result.is_err(),
            "register_merge_drivers should propagate the I/O error from a read-only .gitattributes"
        );
    }

    /// Verify that pre-existing content in `.gitattributes` is preserved and
    /// that merge driver lines are appended without disturbing existing entries.
    #[tokio::test]
    async fn test_register_merge_drivers_preserves_existing_gitattributes() {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            git_dir.join("config"),
            "[core]\n\trepositoryformatversion = 0\n",
        )
        .unwrap();

        // Pre-populate .gitattributes with unrelated content
        let pre_existing = "*.png binary\n*.jpg binary\n";
        let attrs_path = temp.path().join(".gitattributes");
        std::fs::write(&attrs_path, pre_existing).unwrap();

        let kanban_dir = temp.path().join(".kanban");
        register_merge_drivers(&kanban_dir).await.unwrap();

        let attrs = std::fs::read_to_string(&attrs_path).unwrap();

        // Pre-existing content must still be present
        assert!(
            attrs.contains("*.png binary"),
            ".gitattributes must retain pre-existing '*.png binary' line"
        );
        assert!(
            attrs.contains("*.jpg binary"),
            ".gitattributes must retain pre-existing '*.jpg binary' line"
        );

        // All merge driver patterns must also be present
        for driver in MERGE_DRIVERS {
            let line = format!("{} merge={}", driver.pattern, driver.name);
            assert!(
                attrs.contains(&line),
                ".gitattributes must contain '{line}' after registration"
            );
        }
    }
}
