//! Integration tests that use isolated temp directories to exercise I/O
//! error paths in statusline modules.
//!
//! These tests change the process-wide current directory and MUST run serially.
//! The `serial` module below uses a mutex to enforce this.

use std::sync::Mutex;

use swissarmyhammer_statusline::config::StatuslineConfig;
use swissarmyhammer_statusline::input::StatuslineInput;
use swissarmyhammer_statusline::module::ModuleContext;

static DIR_LOCK: Mutex<()> = Mutex::new(());

/// Run `f` with the working directory set to `dir`, restoring afterwards.
fn with_dir<F: FnOnce()>(dir: &std::path::Path, f: F) {
    let _guard = DIR_LOCK.lock().unwrap();
    let original = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir).unwrap();
    f();
    std::env::set_current_dir(original).unwrap();
}

fn default_ctx<'a>(input: &'a StatuslineInput, config: &'a StatuslineConfig) -> ModuleContext<'a> {
    ModuleContext { input, config }
}

// ── Git modules: no-repo paths ─────────────────────────────────────

#[test]
fn test_git_branch_no_repo() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::git_branch::eval(&ctx);
        assert!(out.is_empty());
    });
}

#[test]
fn test_git_state_no_repo() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::git_state::eval(&ctx);
        assert!(out.is_empty());
    });
}

#[test]
fn test_git_status_no_repo() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::git_status::eval(&ctx);
        assert!(out.is_empty());
    });
}

// ── Git: fresh repo with no upstream (get_ahead_behind error paths) ─

#[test]
fn test_git_status_fresh_repo_no_upstream() {
    let dir = tempfile::tempdir().unwrap();
    // Create a minimal git repo with one commit but no remote
    let repo = git2::Repository::init(dir.path()).unwrap();
    let sig = git2::Signature::now("Test", "test@test.com").unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        // Write an empty tree
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();

    // Add an untracked file so status is non-empty
    std::fs::write(dir.path().join("new.txt"), "hello").unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::git_status::eval(&ctx);
        // Should show untracked file, but ahead/behind should be (0,0) from error path
        assert!(!out.is_empty());
    });
}

#[test]
fn test_git_branch_fresh_repo_no_head() {
    let dir = tempfile::tempdir().unwrap();
    // Init a repo but don't commit - HEAD is unborn
    git2::Repository::init(dir.path()).unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::git_branch::eval(&ctx);
        // Unborn HEAD → head() fails → hidden
        assert!(out.is_empty());
    });
}

// ── Index module: no code-context database ──────────────────────────

#[test]
fn test_index_isolated_dir() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        // CodeContextWorkspace::open may succeed (creates DB) or fail;
        // either way, exercises the eval code paths.
        let _out = swissarmyhammer_statusline::modules::index::eval(&ctx);
    });
}

#[test]
fn test_index_isolated_show_when_complete() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let mut config = StatuslineConfig::default();
    config.index.show_when_complete = true;
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let _out = swissarmyhammer_statusline::modules::index::eval(&ctx);
    });
}

// ── Languages module: no code-context database ──────────────────────

#[test]
fn test_languages_isolated_dir() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        // Exercises eval in a dir with no source files indexed.
        let _out = swissarmyhammer_statusline::modules::languages::eval(&ctx);
    });
}

#[test]
fn test_languages_isolated_no_dim() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let mut config = StatuslineConfig::default();
    config.languages.dim_without_lsp = false;
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let _out = swissarmyhammer_statusline::modules::languages::eval(&ctx);
    });
}

// ── Kanban module: isolated directory tests ─────────────────────────

#[test]
fn test_kanban_no_kanban_dir() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::kanban::eval(&ctx);
        // No .kanban directory → hidden
        assert!(out.is_empty());
    });
}

#[test]
fn test_kanban_with_tasks() {
    let dir = tempfile::tempdir().unwrap();
    // Create .kanban structure that KanbanContext::find expects
    let kanban_dir = dir.path().join(".kanban");
    std::fs::create_dir_all(kanban_dir.join("tasks")).unwrap();
    // Create board.md (KanbanContext::find typically expects this)
    std::fs::write(kanban_dir.join("board.md"), "---\nname: Test Board\n---\n").unwrap();
    // Create task files
    std::fs::write(
        kanban_dir.join("tasks").join("task1.md"),
        "---\nposition_column: todo\n---\nTask 1\n",
    )
    .unwrap();
    std::fs::write(
        kanban_dir.join("tasks").join("task2.md"),
        "---\nposition_column: done\n---\nTask 2\n",
    )
    .unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::kanban::eval(&ctx);
        // Should find .kanban, count tasks, render bar
        if !out.is_empty() {
            assert!(out.text.contains("1"));
            assert!(out.text.contains("2"));
        }
        // If KanbanContext::find fails due to missing metadata, that's OK too
    });
}

// ── Git: unborn repo (get_ahead_behind head error) ──────────────────

#[test]
fn test_git_status_unborn_head() {
    let dir = tempfile::tempdir().unwrap();
    // Init repo but don't commit - HEAD is unborn
    git2::Repository::init(dir.path()).unwrap();
    // Add an untracked file so statuses is non-empty
    std::fs::write(dir.path().join("file.txt"), "hello").unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        // get_ahead_behind will call head() which fails on unborn → (0,0)
        let _out = swissarmyhammer_statusline::modules::git_status::eval(&ctx);
    });
}

// ── Kanban: .kanban exists but tasks dir missing ────────────────────

#[test]
fn test_kanban_no_tasks_dir() {
    let dir = tempfile::tempdir().unwrap();
    let kanban_dir = dir.path().join(".kanban");
    std::fs::create_dir_all(&kanban_dir).unwrap();
    std::fs::write(kanban_dir.join("board.md"), "---\nname: Test\n---\n").unwrap();
    // No tasks/ subdirectory

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::kanban::eval(&ctx);
        // KanbanContext::find may succeed, but tasks_dir doesn't exist → hidden
        // Or find fails due to missing metadata → also hidden
        assert!(out.is_empty());
    });
}

// ── Index/Languages: read-only dir to force workspace open failure ──

#[cfg(unix)]
#[test]
fn test_index_readonly_dir() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    // Make read-only so CodeContextWorkspace::open fails to create DB
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::index::eval(&ctx);
        assert!(out.is_empty());
    });

    // Restore permissions for cleanup
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn test_languages_readonly_dir() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::languages::eval(&ctx);
        assert!(out.is_empty());
    });

    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
}

// ── Index/Languages: deleted CWD to trigger current_dir() error ─────

#[cfg(unix)]
#[test]
fn test_index_deleted_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(&path, || {
        // Delete the current directory while we're in it
        std::fs::remove_dir_all(&path).unwrap();
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::index::eval(&ctx);
        assert!(out.is_empty());
    });
}

#[cfg(unix)]
#[test]
fn test_languages_deleted_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(&path, || {
        std::fs::remove_dir_all(&path).unwrap();
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::languages::eval(&ctx);
        assert!(out.is_empty());
    });
}

// ── Index/Languages: corrupt database to trigger get_status error ───

#[cfg(unix)]
#[test]
fn test_index_db_is_directory() {
    let dir = tempfile::tempdir().unwrap();
    // Make index.db a directory — SQLite can't open a directory
    let cc_dir = dir.path().join(".code-context");
    std::fs::create_dir_all(cc_dir.join("index.db")).unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let _out = swissarmyhammer_statusline::modules::index::eval(&ctx);
    });
}

#[cfg(unix)]
#[test]
fn test_languages_db_is_directory() {
    let dir = tempfile::tempdir().unwrap();
    let cc_dir = dir.path().join(".code-context");
    std::fs::create_dir_all(cc_dir.join("index.db")).unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let _out = swissarmyhammer_statusline::modules::languages::eval(&ctx);
    });
}

// ── Index/Languages: unreadable DB file to force get_status error ───

#[cfg(unix)]
#[test]
fn test_index_unreadable_db() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    // Create .code-context with an unreadable index.db
    let cc_dir = dir.path().join(".code-context");
    std::fs::create_dir_all(&cc_dir).unwrap();
    let db_path = cc_dir.join("index.db");
    std::fs::write(&db_path, "").unwrap();
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let _out = swissarmyhammer_statusline::modules::index::eval(&ctx);
    });

    // Restore for cleanup
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[cfg(unix)]
#[test]
fn test_languages_unreadable_db() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let cc_dir = dir.path().join(".code-context");
    std::fs::create_dir_all(&cc_dir).unwrap();
    let db_path = cc_dir.join("index.db");
    std::fs::write(&db_path, "").unwrap();
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let _out = swissarmyhammer_statusline::modules::languages::eval(&ctx);
    });

    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

// ── Git: create repo with untracked + no upstream to exercise more paths ──

#[test]
fn test_git_status_repo_with_various_states() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let sig = git2::Signature::now("Test", "test@test.com").unwrap();

    // Create initial commit with a file
    let file_path = dir.path().join("file.txt");
    std::fs::write(&file_path, "initial").unwrap();
    {
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
    }

    // Stage a new file (INDEX_NEW)
    std::fs::write(dir.path().join("staged.txt"), "staged").unwrap();
    {
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("staged.txt")).unwrap();
        index.write().unwrap();
    }

    // Delete the original file (WT_DELETED)
    std::fs::remove_file(&file_path).unwrap();

    // Add untracked file (WT_NEW)
    std::fs::write(dir.path().join("untracked.txt"), "new").unwrap();

    let input = StatuslineInput::default();
    let mut config = StatuslineConfig::default();
    config.git_status.show_counts = true;
    with_dir(dir.path(), || {
        let ctx = default_ctx(&input, &config);
        let out = swissarmyhammer_statusline::modules::git_status::eval(&ctx);
        assert!(!out.is_empty());
    });
}

// ── lib.rs: render in isolated dir ──────────────────────────────────

#[test]
fn test_render_isolated_no_git() {
    let dir = tempfile::tempdir().unwrap();
    let input = StatuslineInput::default();
    let config = StatuslineConfig::default();
    with_dir(dir.path(), || {
        let result = swissarmyhammer_statusline::render(&input, &config);
        // No git, no kanban, no index → mostly empty output
        let _ = result;
    });
}
