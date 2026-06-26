//! InitBoard command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::default_column_entities;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Merge driver registration configuration for a single driver.
struct MergeDriver {
    /// The git merge driver name (e.g. `kanban-jsonl`)
    name: &'static str,
    /// The glob pattern for `.gitattributes` (e.g. `.kanban/**/*.jsonl`)
    pattern: &'static str,
    /// The driver command string (e.g. `kanban merge jsonl %O %A %B`)
    command: &'static str,
}

/// Read-only Unix permission bits (`r--r--r--`). Applied in tests that need a
/// write to fail; also the mode an unwritable file is left in.
#[cfg(all(unix, test))]
const READ_ONLY_PERMISSIONS: u32 = 0o444;

/// Standard non-executable file permission bits (`rw-r--r--`). Used to restore
/// a file's mode after a read-only test so the temp dir can be cleaned up.
#[cfg(all(unix, test))]
const STANDARD_PERMISSIONS: u32 = 0o644;

/// All merge drivers to register when a board is initialized.
const MERGE_DRIVERS: &[MergeDriver] = &[
    MergeDriver {
        name: "kanban-jsonl",
        pattern: "**/*.jsonl",
        command: "kanban merge jsonl %O %A %B",
    },
    MergeDriver {
        name: "kanban-yaml",
        pattern: "**/*.yaml",
        command: "kanban merge yaml %O %A %B",
    },
    MergeDriver {
        name: "kanban-md",
        pattern: "**/*.md",
        command: "kanban merge md %O %A %B",
    },
];

/// Walk up the directory tree from `start` looking for a `.git` entry.
///
/// Returns the path to the directory that *contains* `.git`, or `None` if not
/// inside a git repository. Works with both normal repos (`.git` is a
/// directory) and worktrees (`.git` is a file containing a `gitdir:` pointer).
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

/// Resolve the path to the git config file for the repository rooted at
/// `git_root`.
///
/// In a normal repository `.git` is a directory and the config lives at
/// `.git/config`. In a **worktree** `.git` is a *file* whose first line is
/// `gitdir: <path>` — we follow that pointer and look for `config` inside the
/// resolved directory.
fn resolve_git_config(git_root: &Path) -> Option<std::path::PathBuf> {
    let dot_git = git_root.join(".git");
    if dot_git.is_dir() {
        return resolve_normal_config(&dot_git);
    }
    if dot_git.is_file() {
        return resolve_worktree_config(git_root, &dot_git);
    }
    None
}

/// Resolve the config path for a **normal** repository, where `.git` is a
/// directory and the config always lives at `.git/config`.
fn resolve_normal_config(dot_git: &Path) -> Option<std::path::PathBuf> {
    Some(dot_git.join("config"))
}

/// Resolve the config path for a **worktree**, where `.git` is a *file* whose
/// first line is `gitdir: <path>` pointing at the worktree's git directory
/// (e.g. `.../.git/worktrees/foo`).
///
/// The pointer may be relative (resolved against `git_root`) or absolute. The
/// config can live either directly in the resolved directory or — for the
/// common `worktrees/<name>` layout — in the main `.git` directory two levels
/// up. Returns `None` if the pointer can't be read or no `config` is found.
fn resolve_worktree_config(git_root: &Path, dot_git: &Path) -> Option<std::path::PathBuf> {
    let contents = std::fs::read_to_string(dot_git).ok()?;
    let gitdir = contents.trim().strip_prefix("gitdir:")?.trim();
    let gitdir_path = if std::path::Path::new(gitdir).is_relative() {
        git_root.join(gitdir)
    } else {
        std::path::PathBuf::from(gitdir)
    };

    let config = gitdir_path.join("config");
    if config.exists() {
        return Some(config);
    }

    // Worktree gitdir (e.g. .git/worktrees/foo) — config lives in the main
    // .git directory, two levels up.
    let main_config = gitdir_path.parent()?.parent()?.join("config");
    main_config.exists().then_some(main_config)
}

/// Write one of the `.kanban/`-owned merge-driver files (`.gitconfig` or
/// `.gitattributes`), rewriting only when the desired `content` differs from
/// what is already on disk.
///
/// These files are fully owned by us, so there is no "preserve existing
/// content" concern — the goal is purely idempotence: don't touch the file
/// (and its mtime) when nothing changed.
///
/// # Parameters
/// - `board_root`: the `.kanban/` directory.
/// - `filename`: the file to write within `board_root` (e.g. `.gitconfig`).
/// - `content`: the full desired contents of the file.
fn write_driver_file(board_root: &Path, filename: &str, content: &str) -> std::io::Result<()> {
    let path = board_root.join(filename);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if content != existing {
        std::fs::write(&path, content)?;
    }
    Ok(())
}

/// Register git merge drivers for `.kanban/` files.
///
/// All configuration lives inside the `.kanban/` directory:
/// - `.kanban/.gitconfig` — merge driver definitions (`[merge "kanban-*"]`)
/// - `.kanban/.gitattributes` — file pattern → driver mappings
///
/// The only change to `.git/config` is a single `include.path` directive
/// that pulls in `.kanban/.gitconfig`.
///
/// Silently skips if not inside a git repository. Idempotent.
///
/// # Parameters
/// - `board_root`: the `.kanban/` directory (parent is the project root)
pub fn register_merge_drivers(board_root: &Path) -> Result<(), std::io::Error> {
    let project_root = match board_root.parent() {
        Some(p) => p.to_path_buf(),
        None => return Ok(()),
    };

    let git_root = match find_git_root(&project_root) {
        Some(r) => r,
        None => return Ok(()),
    };

    // ------------------------------------------------------------------
    // Write .kanban/.gitconfig (merge driver definitions) and
    // .kanban/.gitattributes (pattern → driver mappings).
    // ------------------------------------------------------------------
    let mut gitconfig = String::new();
    let mut attrs = String::new();
    for driver in MERGE_DRIVERS {
        gitconfig.push_str(&format!(
            "[merge \"{}\"]\n\tdriver = {}\n",
            driver.name, driver.command
        ));
        attrs.push_str(&format!("{} merge={}\n", driver.pattern, driver.name));
    }

    write_driver_file(board_root, ".gitconfig", &gitconfig)?;
    write_driver_file(board_root, ".gitattributes", &attrs)?;

    // ------------------------------------------------------------------
    // Add include.path to .git/config pointing to .kanban/.gitconfig
    // ------------------------------------------------------------------
    let git_config_path = match resolve_git_config(&git_root) {
        Some(p) => p,
        None => return Ok(()),
    };
    let include_path = "../.kanban/.gitconfig";
    let include_directive = format!("[include]\n\tpath = {}\n", include_path);

    let existing_config = std::fs::read_to_string(&git_config_path).unwrap_or_default();

    if !existing_config.contains(include_path) {
        let mut config = existing_config;
        if !config.ends_with('\n') && !config.is_empty() {
            config.push('\n');
        }
        config.push_str(&include_directive);
        std::fs::write(&git_config_path, &config)?;
    }

    Ok(())
}

/// Remove git merge driver configuration.
///
/// Removes `.kanban/.gitconfig`, `.kanban/.gitattributes`, and the
/// `include.path` directive from `.git/config`. Does NOT remove `.kanban/`
/// or any board data.
///
/// Synchronous — safe to call from both async and sync contexts.
pub fn unregister_merge_drivers(board_root: &Path) -> Result<(), std::io::Error> {
    // Remove .kanban/.gitconfig
    let kanban_gitconfig = board_root.join(".gitconfig");
    if kanban_gitconfig.exists() {
        std::fs::remove_file(&kanban_gitconfig)?;
    }

    // Remove .kanban/.gitattributes
    let kanban_gitattributes = board_root.join(".gitattributes");
    if kanban_gitattributes.exists() {
        std::fs::remove_file(&kanban_gitattributes)?;
    }

    // Remove include.path from .git/config
    let project_root = match board_root.parent() {
        Some(p) => p.to_path_buf(),
        None => return Ok(()),
    };
    let git_root = match find_git_root(&project_root) {
        Some(r) => r,
        None => return Ok(()),
    };
    let git_config_path = match resolve_git_config(&git_root) {
        Some(p) => p,
        None => return Ok(()),
    };
    let include_path = "../.kanban/.gitconfig";

    if let Ok(config) = std::fs::read_to_string(&git_config_path) {
        if config.contains(include_path) {
            let cleaned = remove_config_section(&config, "[include]");
            std::fs::write(&git_config_path, &cleaned)?;
        }
    }

    Ok(())
}

/// Remove a two-line git-config section — its `section` header line and the
/// single body line that follows it — from `config`, preserving every other
/// line.
///
/// Iterating over `lines()` directly (rather than `windows(2)`) preserves the
/// final line of the config, which a windowed approach silently dropped. The
/// returned string keeps a trailing newline when non-empty, matching git's
/// own config formatting. A `section` that does not appear leaves the content
/// unchanged.
///
/// # Parameters
/// - `config`: the full git config text.
/// - `section`: the section header to drop, e.g. `[include]` (matched against
///   the trimmed line).
fn remove_config_section(config: &str, section: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for line in config.lines() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if line.trim() == section {
            skip_next = true;
            continue;
        }
        kept.push(line);
    }
    let cleaned = kept.join("\n");
    if cleaned.is_empty() {
        cleaned
    } else {
        format!("{cleaned}\n")
    }
}

/// Ephemeral gitignore entries (search cache, undo state) that must not be
/// version controlled. Listed explicitly (not as a `search-cache.sqlite3*`
/// glob) so the guarantee is exact and the directory's tracked task files stay
/// un-ignored.
///
/// Declared at module scope (rather than inside [`ensure_gitignore_entries`])
/// so the test suite can assert against the single source of truth instead of
/// re-listing the literals.
pub(crate) const REQUIRED_GITIGNORE_ENTRIES: &[&str] = &[
    "undo_stack.yaml",
    "search-cache.sqlite3",
    "search-cache.sqlite3-wal",
    "search-cache.sqlite3-shm",
];

/// Reconcile the board's `.kanban/.gitignore` so every entry in
/// [`REQUIRED_GITIGNORE_ENTRIES`] is present, appending any that are missing.
///
/// The cache and undo stack are derived data that must never be committed, but
/// `.kanban/` is only partially tracked — task `.md`/`.jsonl` files are
/// committed source — so each ephemeral file is listed explicitly rather than
/// blanket-ignoring the directory. Idempotent and safe on existing boards:
/// reads whatever is already there, appends only the missing required entries
/// without clobbering existing lines, and rewrites the file only when something
/// changed. Returns the underlying [`std::io::Error`] if the rewrite fails.
pub(crate) fn ensure_gitignore_entries(kanban_root: &Path) -> std::io::Result<()> {
    let gitignore_path = kanban_root.join(".gitignore");
    let existing = std::fs::read_to_string(&gitignore_path).unwrap_or_default();

    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
    let mut changed = false;
    for required in REQUIRED_GITIGNORE_ENTRIES {
        if !lines.iter().any(|l| l.trim() == *required) {
            lines.push((*required).to_string());
            changed = true;
        }
    }

    if changed {
        let mut content = lines.join("\n");
        content.push('\n');
        std::fs::write(&gitignore_path, content)?;
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
        let result = async {
            // Check if already initialized
            if ctx.is_initialized() {
                return Err(KanbanError::AlreadyExists {
                    path: ctx.root().to_path_buf(),
                });
            }

            // Create directory structure
            ctx.create_directories().await?;

            // Ensure ephemeral files (undo stack, search cache + SQLite WAL
            // sidecars) are gitignored. Idempotent so existing boards gain
            // any newly-required entries on the next init/open.
            ensure_gitignore_entries(ctx.root()).map_err(KanbanError::Io)?;

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
            register_merge_drivers(ctx.root()).map_err(KanbanError::Io)?;

            // Return board with columns in response (for API compatibility)
            Ok(json!({
                "name": self.name,
                "description": self.description,
                "columns": columns_json,
                "projects": [],
            }))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// The writer must add every required entry, never duplicate on repeat,
    /// and must not blanket-ignore the dir (committed task files stay tracked).
    #[test]
    fn ensure_gitignore_entries_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Run twice — the second pass must be a no-op for content.
        ensure_gitignore_entries(&kanban_dir).unwrap();
        ensure_gitignore_entries(&kanban_dir).unwrap();

        let content = std::fs::read_to_string(kanban_dir.join(".gitignore")).unwrap();

        for required in REQUIRED_GITIGNORE_ENTRIES {
            let count = content.lines().filter(|l| l.trim() == *required).count();
            assert_eq!(
                count, 1,
                "`{required}` must appear exactly once, found {count}"
            );
        }

        // No blanket ignore — task files must remain tracked.
        assert!(
            !content.lines().any(|l| l.trim() == "*"),
            "must not blanket-ignore the directory"
        );
        assert!(
            !content
                .lines()
                .any(|l| l.trim() == "tasks" || l.trim() == "tasks/"),
            "must not ignore the tracked tasks dir"
        );
    }

    /// `remove_config_section` drops the named section's header line and its
    /// single following body line, preserving every other line — including the
    /// final one — and is a no-op when the section is absent.
    #[test]
    fn remove_config_section_drops_only_the_named_block() {
        let config = "[core]\n\trepositoryformatversion = 0\n[include]\n\tpath = ../.kanban/.gitconfig\n[user]\n\tname = Test User\n";

        let cleaned = remove_config_section(config, "[include]");

        assert!(!cleaned.contains("[include]"), "the section header must go");
        assert!(
            !cleaned.contains("../.kanban/.gitconfig"),
            "the section body must go"
        );
        assert!(cleaned.contains("[core]"), "earlier sections survive");
        assert!(
            cleaned.contains("\trepositoryformatversion = 0"),
            "earlier body survives"
        );
        assert!(cleaned.contains("[user]"), "later sections survive");
        assert!(
            cleaned.contains("\tname = Test User"),
            "the final line must not be dropped, got:\n{cleaned}"
        );

        // Absent section → unchanged content.
        let unchanged = remove_config_section("[core]\n\tfoo = bar\n", "[include]");
        assert_eq!(unchanged, "[core]\n\tfoo = bar\n");
    }

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
        assert_eq!(columns.len(), 4); // To Do, Doing, Review, Done
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
    async fn test_init_board_creates_kanban_gitconfig() {
        let (temp, ctx) = setup_git().await;

        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        // Driver definitions live in .kanban/.gitconfig
        let gitconfig =
            std::fs::read_to_string(temp.path().join(".kanban").join(".gitconfig")).unwrap();
        for driver in MERGE_DRIVERS {
            let section = format!("[merge \"{}\"]", driver.name);
            assert!(
                gitconfig.contains(&section),
                ".kanban/.gitconfig should contain {section}"
            );
            assert!(
                gitconfig.contains(driver.command),
                ".kanban/.gitconfig should contain driver command '{}'",
                driver.command
            );
        }

        // .git/config should only have an include.path, not the driver sections directly
        let git_config = std::fs::read_to_string(temp.path().join(".git").join("config")).unwrap();
        assert!(
            git_config.contains("../.kanban/.gitconfig"),
            ".git/config should include .kanban/.gitconfig"
        );
        assert!(
            !git_config.contains("[merge \"kanban-"),
            ".git/config should NOT contain driver sections directly"
        );
    }

    #[tokio::test]
    async fn test_init_board_creates_kanban_gitattributes() {
        let (_temp, ctx) = setup_git().await;

        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        // .gitattributes lives inside .kanban/
        let attrs = std::fs::read_to_string(ctx.root().join(".gitattributes")).unwrap();
        for driver in MERGE_DRIVERS {
            let line = format!("{} merge={}", driver.pattern, driver.name);
            assert!(
                attrs.contains(&line),
                ".kanban/.gitattributes should contain '{line}'"
            );
        }
    }

    #[tokio::test]
    async fn test_init_board_idempotent() {
        let (temp, ctx) = setup_git().await;

        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        let config_1 = std::fs::read_to_string(temp.path().join(".git").join("config")).unwrap();
        let gitconfig_1 = std::fs::read_to_string(ctx.root().join(".gitconfig")).unwrap();
        let attrs_1 = std::fs::read_to_string(ctx.root().join(".gitattributes")).unwrap();

        // Second registration should not change anything
        register_merge_drivers(ctx.root()).unwrap();

        let config_2 = std::fs::read_to_string(temp.path().join(".git").join("config")).unwrap();
        let gitconfig_2 = std::fs::read_to_string(ctx.root().join(".gitconfig")).unwrap();
        let attrs_2 = std::fs::read_to_string(ctx.root().join(".gitattributes")).unwrap();

        assert_eq!(config_1, config_2, ".git/config should not change");
        assert_eq!(
            gitconfig_1, gitconfig_2,
            ".kanban/.gitconfig should not change"
        );
        assert_eq!(attrs_1, attrs_2, ".kanban/.gitattributes should not change");
    }

    #[tokio::test]
    async fn test_init_board_no_git_repo_skips_silently() {
        let (_temp, ctx) = setup().await;

        let cmd = InitBoard::new("Test Board");
        cmd.execute(&ctx).await.into_result().unwrap();

        // No merge driver files should exist when not in a git repo
        assert!(
            !ctx.root().join(".gitconfig").exists(),
            ".kanban/.gitconfig should not exist when not in a git repo"
        );
        assert!(
            !ctx.root().join(".gitattributes").exists(),
            ".kanban/.gitattributes should not exist when not in a git repo"
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

    /// Verify that register_merge_drivers works in a git worktree where
    /// `.git` is a file (not a directory) containing a `gitdir:` pointer.
    #[tokio::test]
    async fn test_register_merge_drivers_worktree() {
        let temp = TempDir::new().unwrap();

        // Create a main .git directory with a config
        let main_git = temp.path().join("main").join(".git");
        std::fs::create_dir_all(main_git.join("worktrees").join("wt")).unwrap();
        std::fs::write(
            main_git.join("config"),
            "[core]\n\trepositoryformatversion = 0\n",
        )
        .unwrap();

        // Create worktree dir with a .git *file* pointing back
        let wt_dir = temp.path().join("wt");
        std::fs::create_dir_all(&wt_dir).unwrap();
        let gitdir_target = main_git.join("worktrees").join("wt");
        std::fs::write(
            wt_dir.join(".git"),
            format!("gitdir: {}", gitdir_target.display()),
        )
        .unwrap();

        let kanban_dir = wt_dir.join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Should not error — previously this would fail with ENOTDIR
        register_merge_drivers(&kanban_dir).unwrap();

        // The include directive should land in the main .git/config
        let config = std::fs::read_to_string(main_git.join("config")).unwrap();
        assert!(
            config.contains("../.kanban/.gitconfig"),
            "main .git/config should contain the include directive"
        );
    }

    /// Verify that a board root with no parent returns Ok(()) silently.
    #[tokio::test]
    async fn test_register_merge_drivers_no_parent() {
        // Path::new("/") has no parent — the function must return Ok(()) immediately.
        let result = register_merge_drivers(std::path::Path::new("/"));
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
        perms.set_mode(READ_ONLY_PERMISSIONS);
        std::fs::set_permissions(&config_path, perms).unwrap();

        let kanban_dir = temp.path().join(".kanban");
        let result = register_merge_drivers(&kanban_dir);

        // Restore permissions so TempDir can clean up
        let mut perms = std::fs::metadata(&config_path).unwrap().permissions();
        perms.set_mode(STANDARD_PERMISSIONS);
        std::fs::set_permissions(&config_path, perms).unwrap();

        assert!(
            result.is_err(),
            "register_merge_drivers should propagate the I/O error from a read-only .git/config"
        );
    }

    /// Verify that a read-only `.kanban/.gitattributes` causes an error.
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

        // Create .kanban/ and a read-only .kanban/.gitattributes
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();
        let attrs_path = kanban_dir.join(".gitattributes");
        std::fs::write(&attrs_path, "# existing content\n").unwrap();
        let mut perms = std::fs::metadata(&attrs_path).unwrap().permissions();
        perms.set_mode(READ_ONLY_PERMISSIONS);
        std::fs::set_permissions(&attrs_path, perms).unwrap();

        let result = register_merge_drivers(&kanban_dir);

        // Restore permissions so TempDir can clean up
        let mut perms = std::fs::metadata(&attrs_path).unwrap().permissions();
        perms.set_mode(STANDARD_PERMISSIONS);
        std::fs::set_permissions(&attrs_path, perms).unwrap();

        assert!(
            result.is_err(),
            "register_merge_drivers should propagate the I/O error from a read-only .kanban/.gitattributes"
        );
    }

    /// .kanban/.gitconfig and .kanban/.gitattributes are fully owned by us,
    /// so there's no "preserve existing content" concern — they're overwritten.
    /// This test verifies unregister cleans up everything.
    #[tokio::test]
    async fn test_unregister_merge_drivers() {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            git_dir.join("config"),
            "[core]\n\trepositoryformatversion = 0\n",
        )
        .unwrap();

        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Register
        register_merge_drivers(&kanban_dir).unwrap();
        assert!(kanban_dir.join(".gitconfig").exists());
        assert!(kanban_dir.join(".gitattributes").exists());
        let config = std::fs::read_to_string(git_dir.join("config")).unwrap();
        assert!(config.contains("../.kanban/.gitconfig"));

        // Unregister
        unregister_merge_drivers(&kanban_dir).unwrap();
        assert!(!kanban_dir.join(".gitconfig").exists());
        assert!(!kanban_dir.join(".gitattributes").exists());
        let config = std::fs::read_to_string(git_dir.join("config")).unwrap();
        assert!(
            !config.contains(".kanban/.gitconfig"),
            ".git/config should not reference .kanban/.gitconfig after unregister"
        );
    }

    /// A register → unregister round-trip must remove ONLY the `[include]`
    /// block and preserve every other line — including the very last line of
    /// the config. The previous `.windows(2)` removal silently dropped the
    /// final line; this guards against that regression.
    #[tokio::test]
    async fn test_unregister_preserves_all_other_lines() {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            git_dir.join("config"),
            "[core]\n\trepositoryformatversion = 0\n",
        )
        .unwrap();

        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Register so the [include] block is appended.
        register_merge_drivers(&kanban_dir).unwrap();

        // Append a trailing section AFTER the include block so the last line
        // of the file is not part of the block we remove.
        let mut config = std::fs::read_to_string(git_dir.join("config")).unwrap();
        config.push_str("[user]\n\tname = Test User\n");
        std::fs::write(git_dir.join("config"), &config).unwrap();

        // Unregister: only the [include] block should disappear.
        unregister_merge_drivers(&kanban_dir).unwrap();

        let cleaned = std::fs::read_to_string(git_dir.join("config")).unwrap();
        assert!(
            !cleaned.contains("[include]"),
            "the [include] section must be removed"
        );
        assert!(
            !cleaned.contains("../.kanban/.gitconfig"),
            "the include path must be removed"
        );
        // Every non-include line must survive, including the final line.
        assert!(cleaned.contains("[core]"));
        assert!(cleaned.contains("\trepositoryformatversion = 0"));
        assert!(cleaned.contains("[user]"));
        assert!(
            cleaned.contains("\tname = Test User"),
            "the final line of the config must not be dropped, got:\n{cleaned}"
        );
    }
}
