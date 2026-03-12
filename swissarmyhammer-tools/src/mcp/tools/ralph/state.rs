//! Ralph state management
//!
//! Stores per-session instructions as markdown files with YAML frontmatter
//! in `.sah/ralph/<session_id>.md`.

use std::fs;
use std::path::{Path, PathBuf};

/// Per-session ralph instruction state
///
/// Represents the parsed content of a `.sah/ralph/<session_id>.md` file,
/// including frontmatter fields and the instruction body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RalphState {
    /// The instruction text (stored in frontmatter)
    pub instruction: String,
    /// Current iteration count
    pub iteration: u32,
    /// Maximum iterations before auto-stop
    pub max_iterations: u32,
    /// Optional notes/context (stored as markdown body after frontmatter)
    pub body: String,
}

/// Validate a session ID for use as a filename
///
/// Rejects empty strings, path separators, `..` sequences, and null bytes.
/// Only allows alphanumeric characters, hyphens, and underscores.
pub fn validate_session_id(session_id: &str) -> anyhow::Result<()> {
    if session_id.is_empty() {
        anyhow::bail!("session_id must not be empty");
    }
    if session_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        anyhow::bail!(
            "session_id contains invalid characters (only alphanumeric, hyphens, underscores allowed): {session_id}"
        );
    }
}

/// Escape a string for safe inclusion as a YAML value
///
/// Wraps the value in double quotes if it contains characters that could
/// corrupt the frontmatter (newlines, colons, quotes). Escapes internal
/// double quotes and backslashes.
fn escape_yaml_value(s: &str) -> String {
    if s.contains('\n') || s.contains(':') || s.contains('"') || s.contains('\\') {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

/// Unescape a YAML double-quoted value
///
/// Strips surrounding double quotes and unescapes `\\n`, `\\\"`, and `\\\\`.
fn unescape_yaml_value(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        let mut result = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('"') => result.push('"'),
                    Some('\\') => result.push('\\'),
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(c);
            }
        }
        result
    } else {
        s.to_string()
    }
}

/// Get the ralph directory path
///
/// Returns the `.sah/ralph/` directory under the given base directory.
fn ralph_dir(base_dir: &Path) -> PathBuf {
    base_dir.join(".sah").join("ralph")
}

/// Get the file path for a session's ralph instruction
///
/// Returns the path to `<session_id>.md` in the ralph directory.
fn ralph_file(base_dir: &Path, session_id: &str) -> PathBuf {
    ralph_dir(base_dir).join(format!("{session_id}.md"))
}

/// Ensure the ralph directory exists
///
/// Creates `.sah/ralph/` and all parent directories if they don't exist.
/// This operation is idempotent — calling it multiple times has no effect.
pub fn ensure_ralph_dir(base_dir: &Path) -> anyhow::Result<()> {
    let dir = ralph_dir(base_dir);
    fs::create_dir_all(&dir)?;
    Ok(())
}

/// Read a session's ralph state from disk
///
/// Returns `Ok(None)` if the file does not exist. Returns `Ok(Some(RalphState))`
/// with parsed frontmatter if the file exists. Returns `Err(...)` for I/O errors
/// or invalid session IDs — callers should propagate these rather than treating
/// them as "no state".
pub fn read_ralph(base_dir: &Path, session_id: &str) -> anyhow::Result<Option<RalphState>> {
    validate_session_id(session_id)?;
    let path = ralph_file(base_dir, session_id);
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    Ok(parse_ralph_file(&content))
}

/// Write a session's ralph state to disk
///
/// Creates `.sah/ralph/<session_id>.md` with YAML frontmatter containing
/// instruction, iteration, and max_iterations. The `state.body` field is
/// written as the markdown body after the frontmatter.
pub fn write_ralph(base_dir: &Path, session_id: &str, state: &RalphState) -> anyhow::Result<()> {
    validate_session_id(session_id)?;
    ensure_ralph_dir(base_dir)?;

    // Quote the instruction to prevent YAML injection via newlines or special chars
    let escaped_instruction = escape_yaml_value(&state.instruction);
    let content = format!(
        "---\ninstruction: {escaped_instruction}\niteration: {}\nmax_iterations: {}\n---\n\n{}\n",
        state.iteration, state.max_iterations, state.body
    );

    fs::write(ralph_file(base_dir, session_id), content)?;
    Ok(())
}

/// Delete a session's ralph state file
///
/// Removes `.sah/ralph/<session_id>.md` if it exists. No-ops if the file
/// doesn't exist.
pub fn delete_ralph(base_dir: &Path, session_id: &str) -> anyhow::Result<()> {
    validate_session_id(session_id)?;
    let path = ralph_file(base_dir, session_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Parse the frontmatter and body from a ralph markdown file
///
/// Expects the format:
/// ```text
/// ---
/// instruction: ...
/// iteration: N
/// max_iterations: N
/// ---
///
/// Body text here
/// ```
fn parse_ralph_file(content: &str) -> Option<RalphState> {
    // Split on frontmatter delimiters
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 2 {
        return None;
    }

    let frontmatter = parts[1];
    let mut instruction = String::new();
    let mut iteration: u32 = 0;
    let mut max_iterations: u32 = 50;

    for line in frontmatter.lines() {
        if let Some(val) = line.strip_prefix("instruction:") {
            instruction = unescape_yaml_value(val.trim());
        } else if let Some(val) = line.strip_prefix("iteration:") {
            if let Ok(n) = val.trim().parse::<u32>() {
                iteration = n;
            }
        } else if let Some(val) = line.strip_prefix("max_iterations:") {
            if let Ok(n) = val.trim().parse::<u32>() {
                max_iterations = n;
            }
        }
    }

    // Reject malformed files with no instruction
    if instruction.is_empty() {
        return None;
    }

    let body = if parts.len() >= 3 {
        parts[2].trim_start_matches('\n').to_string()
    } else {
        String::new()
    };

    Some(RalphState {
        instruction,
        iteration,
        max_iterations,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    #[test]
    fn test_ensure_ralph_dir_creates_directory() {
        let tmp = setup();
        ensure_ralph_dir(tmp.path()).unwrap();
        assert!(tmp.path().join(".sah").join("ralph").is_dir());
    }

    #[test]
    fn test_ensure_ralph_dir_is_idempotent() {
        let tmp = setup();
        ensure_ralph_dir(tmp.path()).unwrap();
        ensure_ralph_dir(tmp.path()).unwrap();
        assert!(tmp.path().join(".sah").join("ralph").is_dir());
    }

    #[test]
    fn test_read_nonexistent_returns_none() {
        let tmp = setup();
        let result = read_ralph(tmp.path(), "no-such-session").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_write_and_read_round_trip() {
        let tmp = setup();
        let state = RalphState {
            instruction: "Implement all kanban cards".to_string(),
            iteration: 3,
            max_iterations: 50,
            body: "Agent notes go here.".to_string(),
        };

        write_ralph(tmp.path(), "session-123", &state).unwrap();
        let read_state = read_ralph(tmp.path(), "session-123").unwrap().unwrap();

        assert_eq!(read_state.instruction, "Implement all kanban cards");
        assert_eq!(read_state.iteration, 3);
        assert_eq!(read_state.max_iterations, 50);
        assert!(read_state.body.contains("Agent notes go here."));
    }

    #[test]
    fn test_write_creates_correct_file_path() {
        let tmp = setup();
        let state = RalphState {
            instruction: "test".to_string(),
            iteration: 0,
            max_iterations: 10,
            body: String::new(),
        };

        write_ralph(tmp.path(), "test-session", &state).unwrap();

        let expected_path = tmp.path().join(".sah").join("ralph").join("test-session.md");
        assert!(expected_path.exists());
    }

    #[test]
    fn test_delete_ralph_removes_file() {
        let tmp = setup();
        let state = RalphState {
            instruction: "test".to_string(),
            iteration: 0,
            max_iterations: 10,
            body: String::new(),
        };

        write_ralph(tmp.path(), "session-123", &state).unwrap();
        assert!(read_ralph(tmp.path(), "session-123").unwrap().is_some());

        delete_ralph(tmp.path(), "session-123").unwrap();
        assert!(read_ralph(tmp.path(), "session-123").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_is_ok() {
        let tmp = setup();
        delete_ralph(tmp.path(), "no-such-session").unwrap();
    }

    #[test]
    fn test_two_sessions_are_isolated() {
        let tmp = setup();
        let state_a = RalphState {
            instruction: "instruction A".to_string(),
            iteration: 1,
            max_iterations: 10,
            body: "body A".to_string(),
        };
        let state_b = RalphState {
            instruction: "instruction B".to_string(),
            iteration: 2,
            max_iterations: 20,
            body: "body B".to_string(),
        };

        write_ralph(tmp.path(), "session-a", &state_a).unwrap();
        write_ralph(tmp.path(), "session-b", &state_b).unwrap();

        let a = read_ralph(tmp.path(), "session-a").unwrap().unwrap();
        let b = read_ralph(tmp.path(), "session-b").unwrap().unwrap();

        assert_eq!(a.instruction, "instruction A");
        assert_eq!(b.instruction, "instruction B");
        assert_ne!(a, b);
    }

    #[test]
    fn test_write_overwrites_existing() {
        let tmp = setup();
        let state_first = RalphState {
            instruction: "first".to_string(),
            iteration: 1,
            max_iterations: 10,
            body: String::new(),
        };
        let state_second = RalphState {
            instruction: "second".to_string(),
            iteration: 2,
            max_iterations: 20,
            body: String::new(),
        };

        write_ralph(tmp.path(), "session-123", &state_first).unwrap();
        write_ralph(tmp.path(), "session-123", &state_second).unwrap();

        let read = read_ralph(tmp.path(), "session-123").unwrap().unwrap();
        assert_eq!(read.instruction, "second");
        assert_eq!(read.iteration, 2);
    }

    #[test]
    fn test_frontmatter_parsing() {
        let content = "---\ninstruction: Implement all kanban cards\niteration: 3\nmax_iterations: 50\n---\n\nAgent notes go here.\n";
        let state = parse_ralph_file(content).unwrap();
        assert_eq!(state.instruction, "Implement all kanban cards");
        assert_eq!(state.iteration, 3);
        assert_eq!(state.max_iterations, 50);
    }

    // --- Session ID validation tests ---

    #[test]
    fn test_validate_session_id_accepts_valid_ids() {
        assert!(validate_session_id("abc123").is_ok());
        assert!(validate_session_id("my-session").is_ok());
        assert!(validate_session_id("my_session").is_ok());
        assert!(validate_session_id("01KKHB8ZD1P1D59NNJ06SYHDKN").is_ok());
    }

    #[test]
    fn test_validate_session_id_rejects_path_traversal() {
        assert!(validate_session_id("../../etc/passwd").is_err());
        assert!(validate_session_id("..").is_err());
        assert!(validate_session_id("foo/bar").is_err());
        assert!(validate_session_id("foo\\bar").is_err());
    }

    #[test]
    fn test_validate_session_id_rejects_empty() {
        assert!(validate_session_id("").is_err());
    }

    #[test]
    fn test_validate_session_id_rejects_null_bytes() {
        assert!(validate_session_id("abc\0def").is_err());
    }

    #[test]
    fn test_write_ralph_rejects_bad_session_id() {
        let tmp = setup();
        let state = RalphState {
            instruction: "test".to_string(),
            iteration: 0,
            max_iterations: 10,
            body: String::new(),
        };
        assert!(write_ralph(tmp.path(), "../escape", &state).is_err());
    }

    #[test]
    fn test_read_ralph_rejects_bad_session_id() {
        let tmp = setup();
        assert!(read_ralph(tmp.path(), "../escape").is_err());
    }

    // --- Frontmatter injection tests ---

    #[test]
    fn test_instruction_with_newline_does_not_corrupt_frontmatter() {
        let tmp = setup();
        let state = RalphState {
            instruction: "line one\nmax_iterations: 999".to_string(),
            iteration: 5,
            max_iterations: 10,
            body: String::new(),
        };
        write_ralph(tmp.path(), "inject-test", &state).unwrap();
        let read = read_ralph(tmp.path(), "inject-test").unwrap().unwrap();
        // max_iterations should NOT be overwritten by the injected value
        assert_eq!(read.max_iterations, 10);
        assert_eq!(read.iteration, 5);
        // Instruction should round-trip faithfully
        assert!(read.instruction.contains("line one"));
    }

    #[test]
    fn test_instruction_with_colon_round_trips() {
        let tmp = setup();
        let state = RalphState {
            instruction: "key: value pair in instruction".to_string(),
            iteration: 0,
            max_iterations: 50,
            body: String::new(),
        };
        write_ralph(tmp.path(), "colon-test", &state).unwrap();
        let read = read_ralph(tmp.path(), "colon-test").unwrap().unwrap();
        assert_eq!(read.instruction, "key: value pair in instruction");
    }

    #[test]
    fn test_frontmatter_defaults_for_missing_fields() {
        let content = "---\ninstruction: Only instruction\n---\n\nBody.\n";
        let state = parse_ralph_file(content).unwrap();
        assert_eq!(state.instruction, "Only instruction");
        assert_eq!(state.iteration, 0);
        assert_eq!(state.max_iterations, 50);
    }

    #[test]
    fn test_malformed_frontmatter_with_empty_instruction_returns_none() {
        // Has frontmatter delimiters but no instruction key
        let content = "---\niteration: 5\nmax_iterations: 10\n---\n\nSome body.\n";
        assert!(parse_ralph_file(content).is_none());
    }

    #[test]
    fn test_completely_empty_frontmatter_returns_none() {
        let content = "---\n---\n\nBody.\n";
        assert!(parse_ralph_file(content).is_none());
    }
}
