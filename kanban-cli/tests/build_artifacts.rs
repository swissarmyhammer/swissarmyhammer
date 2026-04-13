//! Integration tests asserting that `build.rs` generates the expected
//! documentation, man page, and shell completion files.
//!
//! The build script runs before tests compile, so by the time this test
//! executes, every artifact should already be on disk. If any file is
//! missing, the build script silently failed to write it (or the output
//! paths in `build.rs` drifted from what consumers expect), and this test
//! should fail loudly.
//!
//! Paths are resolved relative to `CARGO_MANIFEST_DIR` (the `kanban-cli/`
//! crate root) with `..` to reach the repository root, matching the
//! `Path::new("..")` base used in `build.rs`.

use std::path::{Path, PathBuf};

/// Repository root, derived from the crate manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("kanban-cli must live inside the workspace")
        .to_path_buf()
}

#[test]
fn generated_markdown_reference_exists() {
    let path = repo_root().join("doc/src/reference/kanban-cli.md");
    assert!(
        path.exists(),
        "build.rs should generate {} via doc_gen::generate_markdown_with_brew",
        path.display()
    );
    let content = std::fs::read_to_string(&path).expect("reference markdown readable");
    // Sanity-check that the lifecycle subcommands appear in the overview.
    for subcmd in ["serve", "init", "deinit", "doctor"] {
        assert!(
            content.contains(&format!("kanban {subcmd}")),
            "generated markdown missing subcommand: {subcmd}\n---\n{content}"
        );
    }
    // The brew install section must be present too.
    assert!(
        content.contains("brew install swissarmyhammer/tap/kanban-cli"),
        "generated markdown missing brew install section\n---\n{content}"
    );
}

#[test]
fn generated_manpage_exists() {
    let path = repo_root().join("docs/kanban.1");
    assert!(
        path.exists(),
        "build.rs should generate {} via doc_gen::generate_manpage",
        path.display()
    );
    let size = std::fs::metadata(&path)
        .expect("man page metadata readable")
        .len();
    assert!(size > 0, "man page {} is empty", path.display());
}

#[test]
fn generated_shell_completions_exist() {
    let completions = repo_root().join("completions");
    for filename in ["kanban.bash", "kanban.fish", "_kanban"] {
        let path = completions.join(filename);
        assert!(
            path.exists(),
            "build.rs should generate {} via doc_gen::generate_completions",
            path.display()
        );
        let size = std::fs::metadata(&path)
            .expect("completion file metadata readable")
            .len();
        assert!(size > 0, "completion file {} is empty", path.display());
    }
}
