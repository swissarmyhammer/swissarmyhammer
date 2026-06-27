//! The `file` surface adapter — the deterministic, no-agent filesystem path.
//!
//! Per `ideas/expect.md` §"Surface adapters" (the file row) and §"Provisioning
//! and Isolation": the adapter **provisions** a scratch directory, **drives** it
//! by writing files (and directories) through `std::fs`, **observes** the captured
//! files/dirs/content, and **tears it down** by dropping the scratch dir.
//!
//! The drive dialect is `write <path> <content>` or `mkdir <path>`; every path is
//! resolved through the shared [`safe_join`](crate::surface::safe_join) traversal
//! guard so a spec can never read or write outside the scratch root. The locator
//! dialect for the observed state — a `path + content`, plus a json-path
//! **sub-locator** into a structured file — lives in the
//! [assertion compiler](crate::assertion); this module only produces the
//! [`FileState`] those locators resolve against.

use std::collections::BTreeMap;
use std::path::Path;

use tempfile::TempDir;
use walkdir::WalkDir;

use crate::error::ExpectError;
use crate::spec::Setup;
use crate::surface::{safe_join, setup_commands, SurfaceAdapter};
use crate::types::{FileState, SurfaceState};

/// The drive-dialect verb that writes a file: `write <path> <content>`.
const WRITE_VERB: &str = "write";

/// The drive-dialect verb that creates a directory: `mkdir <path>`.
const MKDIR_VERB: &str = "mkdir";

/// The `file` surface adapter: provisions a scratch directory, writes files/dirs
/// into it, and captures its files/dirs/content.
///
/// The adapter is deterministic and mechanical — a file step is always a concrete
/// write — so it resolves every step itself and never reaches the agent fallback
/// (the trait's default
/// [`resolves_mechanically`](SurfaceAdapter::resolves_mechanically) of `true`).
#[derive(Debug, Clone, Default)]
pub struct FileAdapter;

impl FileAdapter {
    /// Create a file adapter. Each provision gets its own scratch directory.
    pub fn new() -> Self {
        Self
    }
}

/// The provisioned file system under test: a scratch directory that drives write
/// into and observe captures, deleted on teardown.
#[derive(Debug)]
pub struct FileSut {
    /// The scratch root; dropping it deletes the directory tree (teardown).
    scratch: TempDir,
}

impl FileSut {
    /// The scratch root every path is safe-joined under.
    fn root(&self) -> &Path {
        self.scratch.path()
    }
}

impl SurfaceAdapter for FileAdapter {
    type ProvisionedSut = FileSut;

    fn provision(&self, setup: Option<&Setup>, _repo_root: &Path) -> Result<FileSut, ExpectError> {
        let sut = FileSut {
            scratch: TempDir::new()?,
        };
        // Load the `setup:` fixture: each command is a file action arranging the
        // scratch dir before it is driven.
        if let Some(setup) = setup {
            for command in setup_commands(setup) {
                apply_action(sut.root(), command)?;
            }
        }
        Ok(sut)
    }

    fn drive(&self, sut: &mut FileSut, when_step: &str) -> Result<(), ExpectError> {
        apply_action(sut.root(), when_step)
    }

    fn observe(&self, sut: &FileSut) -> Result<SurfaceState, ExpectError> {
        let (files, dirs) = capture_tree(sut.root())?;
        Ok(SurfaceState::File(FileState { files, dirs }))
    }

    fn teardown(&self, sut: FileSut) -> Result<(), ExpectError> {
        // Dropping the scratch `TempDir` deletes the directory tree — "teardown
        // cleans up".
        drop(sut);
        Ok(())
    }
}

/// Apply one file action against `root` in the dialect `write <path> <content>` or
/// `mkdir <path>`.
///
/// An empty action is a no-op (mirrors the cli/http empty step). Every path is
/// resolved through [`safe_join`], so a `..` component or an absolute path is
/// rejected before any filesystem write happens.
///
/// # Errors
///
/// Returns [`ExpectError::Surface`] for an unknown verb, a missing path, or a path
/// that escapes the scratch root, and [`ExpectError::Io`] when the write fails.
fn apply_action(root: &Path, action: &str) -> Result<(), ExpectError> {
    let trimmed = action.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let (verb, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    match verb {
        WRITE_VERB => write_file(root, rest.trim_start()),
        MKDIR_VERB => make_dir(root, rest.trim()),
        other => Err(ExpectError::Surface(format!(
            "unknown file action `{other}` (expected `{WRITE_VERB}` or `{MKDIR_VERB}`)"
        ))),
    }
}

/// Write `args` as `<path> <content>` under `root`, creating parent dirs as needed.
fn write_file(root: &Path, args: &str) -> Result<(), ExpectError> {
    let (path, content) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
    if path.is_empty() {
        return Err(ExpectError::Surface(
            "`write` needs a path: `write <path> <content>`".to_string(),
        ));
    }
    let target = safe_join(root, path)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(target, content)?;
    Ok(())
}

/// Create the directory `path` (and any parents) under `root`.
fn make_dir(root: &Path, path: &str) -> Result<(), ExpectError> {
    if path.is_empty() {
        return Err(ExpectError::Surface(
            "`mkdir` needs a path: `mkdir <path>`".to_string(),
        ));
    }
    let target = safe_join(root, path)?;
    std::fs::create_dir_all(target)?;
    Ok(())
}

/// Walk the scratch tree under `root`, capturing file contents (keyed by relative
/// path) and the set of directory paths.
///
/// Paths are made relative to `root` and joined with `/` so identities are stable
/// across platforms (matching how [`crate::spec`] derives spec identities).
///
/// # Errors
///
/// Returns [`ExpectError::Surface`] when the tree cannot be walked and
/// [`ExpectError::Io`] when a captured file cannot be read.
fn capture_tree(root: &Path) -> Result<(BTreeMap<String, String>, Vec<String>), ExpectError> {
    let mut files = BTreeMap::new();
    let mut dirs = Vec::new();
    for entry in WalkDir::new(root).min_depth(1).sort_by_file_name() {
        let entry =
            entry.map_err(|err| ExpectError::Surface(format!("file walk failed: {err}")))?;
        let relative = relative_path(root, entry.path())?;
        if entry.file_type().is_dir() {
            dirs.push(relative);
        } else if entry.file_type().is_file() {
            files.insert(relative, std::fs::read_to_string(entry.path())?);
        }
    }
    dirs.sort();
    Ok((files, dirs))
}

/// The `/`-joined path of `path` relative to `root`.
fn relative_path(root: &Path, path: &Path) -> Result<String, ExpectError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ExpectError::Surface("captured entry escaped the scratch root".to_string()))?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provisioned() -> FileSut {
        let repo = TempDir::new().unwrap();
        FileAdapter::new()
            .provision(None, repo.path())
            .expect("provision")
    }

    #[test]
    fn write_creates_nested_files_and_observe_captures_them() {
        let adapter = FileAdapter::new();
        let mut sut = provisioned();
        adapter
            .drive(&mut sut, "write a/b/c.txt hello there")
            .expect("drive write");

        let SurfaceState::File(file) = adapter.observe(&sut).expect("observe") else {
            panic!("expected file state");
        };
        // Content is the verbatim rest of the line (internal spaces preserved).
        assert_eq!(
            file.files.get("a/b/c.txt").map(String::as_str),
            Some("hello there")
        );
        // The intermediate directories were captured.
        assert!(file.dirs.iter().any(|dir| dir == "a"));
        assert!(file.dirs.iter().any(|dir| dir == "a/b"));
    }

    #[test]
    fn mkdir_creates_an_empty_directory() {
        let adapter = FileAdapter::new();
        let mut sut = provisioned();
        adapter.drive(&mut sut, "mkdir empty/dir").expect("mkdir");

        let SurfaceState::File(file) = adapter.observe(&sut).expect("observe") else {
            panic!("expected file state");
        };
        assert!(file.dirs.iter().any(|dir| dir == "empty/dir"));
        assert!(file.files.is_empty(), "no files were written");
    }

    #[test]
    fn an_empty_action_is_a_no_op() {
        let adapter = FileAdapter::new();
        let mut sut = provisioned();
        adapter.drive(&mut sut, "   ").expect("empty action");
        let SurfaceState::File(file) = adapter.observe(&sut).expect("observe") else {
            panic!("expected file state");
        };
        assert!(file.files.is_empty());
        assert!(file.dirs.is_empty());
    }

    #[test]
    fn an_unknown_verb_is_a_surface_error() {
        let adapter = FileAdapter::new();
        let mut sut = provisioned();
        let err = adapter
            .drive(&mut sut, "delete a.txt")
            .expect_err("unknown verb must error");
        assert!(matches!(err, ExpectError::Surface(_)), "got {err:?}");
    }

    #[test]
    fn a_write_without_a_path_is_a_surface_error() {
        let adapter = FileAdapter::new();
        let mut sut = provisioned();
        let err = adapter
            .drive(&mut sut, "write")
            .expect_err("missing path must error");
        assert!(matches!(err, ExpectError::Surface(_)), "got {err:?}");
    }

    #[test]
    fn write_rejects_traversal_and_absolute_paths() {
        let adapter = FileAdapter::new();
        let mut sut = provisioned();
        assert!(matches!(
            adapter.drive(&mut sut, "write ../escape.txt x"),
            Err(ExpectError::Surface(_))
        ));
        assert!(matches!(
            adapter.drive(&mut sut, "write /etc/escape.txt x"),
            Err(ExpectError::Surface(_))
        ));
        // mkdir is guarded too.
        assert!(matches!(
            adapter.drive(&mut sut, "mkdir ../escape"),
            Err(ExpectError::Surface(_))
        ));
    }
}
