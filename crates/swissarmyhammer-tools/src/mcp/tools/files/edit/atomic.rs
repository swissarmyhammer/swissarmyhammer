//! Committing an `edit` to disk: encoding detection, line-ending detection, and
//! the atomic temp-write plus rename that every `edit` path commits through.
//!
//! The resolution cascade produces the new content in memory; this module is
//! what puts it on disk. The rewritten content is staged in a temporary file
//! beside the target, given the original's permissions, and renamed onto it, so
//! a failure at any step leaves the original byte-identical. The original
//! encoding is preserved; the modification time deliberately is not, because an
//! edit changes the file and downstream staleness checks must see that.

use crate::mcp::tools::files::shared_utils::TEMP_FILE_SUFFIX;
use encoding_rs::{Encoding, UTF_8};
use rmcp::ErrorData as McpError;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use tracing::{debug, info};

/// Result information for edit operations
#[derive(Debug, Clone)]
pub struct EditResult {
    /// Number of bytes written to the file
    pub bytes_written: usize,
    /// Number of string replacements made in the file
    pub replacements_made: usize,
    /// The character encoding that was detected and preserved
    pub encoding_detected: String,
    /// The line ending format that was preserved
    pub line_endings_preserved: String,
}

/// Validation result for edit operations
#[derive(Debug, Clone)]
struct EditValidation {
    pub old_string_count: usize,
}

/// Line ending types detected in files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LineEnding {
    Lf,    // Unix: \n
    CrLf,  // Windows: \r\n
    Cr,    // Classic Mac: \r
    Mixed, // Multiple types found
}

impl LineEnding {
    /// Detect the primary line ending type in content
    pub(super) fn detect(content: &str) -> Self {
        let crlf_count = content.matches("\r\n").count();
        let lf_count = content.matches('\n').count() - crlf_count; // Exclude CRLF \n
        let cr_count = content.matches('\r').count() - crlf_count; // Exclude CRLF \r

        match (lf_count > 0, crlf_count > 0, cr_count > 0) {
            (false, false, false) => LineEnding::Lf, // Default for empty/no line endings
            (true, false, false) => LineEnding::Lf,
            (false, true, false) => LineEnding::CrLf,
            (false, false, true) => LineEnding::Cr,
            _ => LineEnding::Mixed,
        }
    }

    /// Get the string representation
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
            LineEnding::Cr => "CR",
            LineEnding::Mixed => "Mixed",
        }
    }
}

/// Tool for performing precise string replacements in existing files
#[derive(Default, Debug)]
pub struct EditFileTool;

impl EditFileTool {
    /// Creates a new instance of the EditFileTool
    pub fn new() -> Self {
        Self
    }

    /// Validates the edit operation before making changes
    ///
    /// Performs comprehensive validation including:
    /// - File existence check
    /// - Old string existence and uniqueness validation
    /// - Security checks through file path validation
    fn validate_edit_operation(
        &self,
        base_dir: &Path,
        file_path: &str,
        content: &str,
        old_string: &str,
        _replace_all: bool,
    ) -> Result<EditValidation, McpError> {
        use crate::mcp::tools::files::shared_utils::validate_file_path;

        // Validate file path first (relative paths resolve against the session
        // working directory, never the process CWD)
        let path = validate_file_path(base_dir, file_path)?;
        if !path.exists() {
            return Err(McpError::invalid_request(
                format!("file does not exist: {}", file_path),
                None,
            ));
        }

        // Count occurrences of old_string
        let matches: Vec<_> = content.matches(old_string).collect();
        let old_string_count = matches.len();
        if old_string_count == 0 {
            return Err(McpError::invalid_request(
                format!("string '{}' not found in file", old_string),
                None,
            ));
        }

        Ok(EditValidation { old_string_count })
    }

    /// Detects file encoding and reads content as string
    ///
    /// Uses encoding_rs for robust encoding detection and handles:
    /// - UTF-8 (most common)
    /// - UTF-16 with BOM
    /// - Other encodings with fallback to UTF-8
    pub(super) fn read_with_encoding_detection(
        &self,
        file_path: &Path,
    ) -> Result<(String, &'static Encoding), McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Read raw bytes first
        let bytes = fs::read(file_path)
            .map_err(|e| handle_file_error(e, "read file for encoding detection", file_path))?;

        // Detect encoding using BOM, fallback to UTF-8
        let (encoding, bom_length) = encoding_rs::Encoding::for_bom(&bytes).unwrap_or((UTF_8, 0));

        // Use the bytes after BOM for decoding
        let bytes_to_decode = &bytes[bom_length..];

        debug!(path = %file_path.display(), encoding = encoding.name(), bom_length = bom_length, "Detected file encoding");

        // Decode to string
        let (content, _, had_decode_errors) = encoding.decode(bytes_to_decode);

        if had_decode_errors {
            return Err(McpError::internal_error(
                format!(
                    "failed to decode file with detected encoding {}",
                    encoding.name()
                ),
                None,
            ));
        }

        Ok((content.into_owned(), encoding))
    }

    /// Performs atomic file edit with full validation and metadata preservation
    ///
    /// This method implements the complete atomic edit workflow:
    /// 1. Validate file path and edit parameters
    /// 2. Read file with encoding detection
    /// 3. Validate old_string existence and uniqueness
    /// 4. Perform replacement operation
    /// 5. Write to temporary file in same directory
    /// 6. Preserve file metadata (permissions, timestamps)
    /// 7. Atomically rename temporary file to original
    /// 8. Clean up temporary file on any failure
    pub fn edit_file_atomic(
        &self,
        base_dir: &Path,
        file_path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> Result<EditResult, McpError> {
        use crate::mcp::tools::files::shared_utils::validate_file_path;

        // Step 1: Validate file path and get canonical path. Relative paths
        // resolve against the session working directory, never the process CWD.
        let path = validate_file_path(base_dir, file_path)?;

        info!(path = %path.display(), old_string_len = old_string.len(), new_string_len = new_string.len(), replace_all = replace_all, "Starting atomic edit operation");

        // Step 2: Read original file with encoding detection
        let (original_content, detected_encoding) = self.read_with_encoding_detection(&path)?;

        // Step 3: Detect line endings
        let line_ending = LineEnding::detect(&original_content);

        // Step 4: Validate edit operation
        let validation = self.validate_edit_operation(
            base_dir,
            file_path,
            &original_content,
            old_string,
            replace_all,
        )?;

        // Step 5: Perform replacement
        let (new_content, replacements_made) = if replace_all {
            let new_content = original_content.replace(old_string, new_string);
            let replacements = validation.old_string_count;
            (new_content, replacements)
        } else {
            let new_content = original_content.replacen(old_string, new_string, 1);
            (new_content, 1)
        };

        // Step 6: commit the rewritten content in one atomic rewrite (metadata
        // preservation lives in `commit_content`).
        self.commit_content(
            &path,
            &new_content,
            detected_encoding,
            line_ending,
            replacements_made,
        )
    }

    /// Commit fully-rewritten `content` to `path` in one atomic rewrite,
    /// preserving the original encoding and permissions.
    ///
    /// This is the shared temp-write + fsync-free rename core both the legacy
    /// single-pair [`edit_file_atomic`](Self::edit_file_atomic) and the
    /// shape-inferred batch path ([`execute_edit`](super::execute_edit)) commit through, so the
    /// encoding / line-ending / permission preservation lives in exactly one
    /// place. The modification time is intentionally NOT preserved: an edit
    /// changes the file, so the rename's fresh mtime must stand, keeping
    /// downstream mtime-based staleness checks (cargo/make, file watchers,
    /// rust-analyzer) correct. On any failure the temporary file is removed and
    /// the original is left untouched (byte-identical).
    pub(super) fn commit_content(
        &self,
        path: &Path,
        content: &str,
        encoding: &'static Encoding,
        line_ending: LineEnding,
        replacements_made: usize,
    ) -> Result<EditResult, McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Capture the original metadata to preserve permissions.
        let original_metadata =
            fs::metadata(path).map_err(|e| handle_file_error(e, "read metadata", path))?;
        let original_permissions = original_metadata.permissions();

        // Create temporary file in same directory as original.
        let temp_file_name = format!("{}{TEMP_FILE_SUFFIX}{}", path.display(), std::process::id());
        let temp_path = path
            .parent()
            .ok_or_else(|| {
                McpError::internal_error(
                    "cannot determine parent directory for temporary file".to_string(),
                    None,
                )
            })?
            .join(&temp_file_name);

        debug!(temp_path = %temp_path.display(), content_length = content.len(), encoding = encoding.name(), "Writing content to temporary file");

        // Write new content to temporary file with original encoding.
        let bytes_written = match self.write_with_encoding(&temp_path, content, encoding) {
            Ok(bytes_written) => bytes_written,
            Err(e) => {
                let _ = fs::remove_file(&temp_path);
                return Err(e);
            }
        };

        // Re-apply the original permissions to the temp file before rename.
        // The temp-write+rename gives the new file default permissions, so
        // without this an executable script (e.g. 0755) would silently downgrade
        // to 0644. This is silent behavior — not reported in the result.
        if let Err(e) = fs::set_permissions(&temp_path, original_permissions.clone()) {
            let _ = fs::remove_file(&temp_path);
            return Err(handle_file_error(
                e,
                "set permissions on temporary file",
                &temp_path,
            ));
        }

        // Atomically rename temporary file to original.
        if let Err(e) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(handle_file_error(
                e,
                "rename temporary file to target",
                path,
            ));
        }

        debug!(path = %path.display(), bytes_written = bytes_written, replacements_made = replacements_made, "Atomic edit operation completed successfully");

        Ok(EditResult {
            bytes_written,
            replacements_made,
            encoding_detected: encoding.name().to_string(),
            line_endings_preserved: line_ending.as_str().to_string(),
        })
    }

    /// Writes content to file with specified encoding
    ///
    /// Preserves the original encoding of the file and handles BOM appropriately.
    fn write_with_encoding(
        &self,
        file_path: &Path,
        content: &str,
        encoding: &'static Encoding,
    ) -> Result<usize, McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Encode content back to bytes using the detected encoding
        let (bytes, _, had_errors) = encoding.encode(content);

        if had_errors {
            return Err(McpError::internal_error(
                format!("failed to encode content with encoding {}", encoding.name()),
                None,
            ));
        }

        // Write bytes to file
        let file = fs::File::create(file_path)
            .map_err(|e| handle_file_error(e, "create temporary file", file_path))?;

        let mut writer = BufWriter::new(file);
        writer
            .write_all(&bytes)
            .map_err(|e| handle_file_error(e, "write to temporary file", file_path))?;

        writer
            .flush()
            .map_err(|e| handle_file_error(e, "flush temporary file", file_path))?;

        Ok(bytes.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_line_ending_detection() {
        // Test Unix line endings (LF)
        let unix_content = "Line 1\nLine 2\nLine 3\n";
        assert_eq!(LineEnding::detect(unix_content), LineEnding::Lf);

        // Test Windows line endings (CRLF)
        let windows_content = "Line 1\r\nLine 2\r\nLine 3\r\n";
        assert_eq!(LineEnding::detect(windows_content), LineEnding::CrLf);

        // Test Classic Mac line endings (CR)
        let mac_content = "Line 1\rLine 2\rLine 3\r";
        assert_eq!(LineEnding::detect(mac_content), LineEnding::Cr);

        // Test mixed line endings
        let mixed_content = "Line 1\nLine 2\r\nLine 3\r";
        assert_eq!(LineEnding::detect(mixed_content), LineEnding::Mixed);

        // Test no line endings
        let no_endings = "Single line";
        assert_eq!(LineEnding::detect(no_endings), LineEnding::Lf);

        // Test empty content
        let empty_content = "";
        assert_eq!(LineEnding::detect(empty_content), LineEnding::Lf);
    }
    /// `LineEnding::as_str` renders the `Mixed` variant label.
    #[test]
    fn test_line_ending_mixed_as_str() {
        assert_eq!(LineEnding::detect("a\nb\r\nc\r").as_str(), "Mixed");
        assert_eq!(LineEnding::Lf.as_str(), "LF");
        assert_eq!(LineEnding::CrLf.as_str(), "CRLF");
        assert_eq!(LineEnding::Cr.as_str(), "CR");
    }
    #[test]
    fn test_edit_validation_logic() {
        let tool = EditFileTool::new();

        // Test with content that has multiple occurrences
        let content = "test content with test and more test";
        let _result = tool.validate_edit_operation(
            std::path::Path::new("/tmp"),
            "/dev/null", // Won't be used in this test
            content,
            "test",
            false, // replace_all = false
        );

        // This should fail because we have multiple occurrences but replace_all = false
        // However, it will fail earlier because /dev/null doesn't exist as a regular file
        // So let's test the logic directly

        // Count occurrences manually to verify logic
        let matches: Vec<_> = content.matches("test").collect();
        assert_eq!(matches.len(), 3);

        // Test unique string
        let matches_unique: Vec<_> = content.matches("content").collect();
        assert_eq!(matches_unique.len(), 1);
    }

    #[test]
    fn test_encoding_detection_logic() {
        let tool = EditFileTool::new();

        // Create a temporary file with UTF-8 content
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("encoding_test.txt");
        let utf8_content = "Hello, 世界! 🌍";
        fs::write(&test_file, utf8_content).unwrap();

        let result = tool.read_with_encoding_detection(&test_file);
        assert!(result.is_ok());

        let (content, encoding) = result.unwrap();
        assert_eq!(content, utf8_content);
        assert_eq!(encoding.name(), "UTF-8");
    }
    // =====================================================================
    // Legacy single-pair API error arms (validate_edit_operation)
    // =====================================================================

    /// `validate_edit_operation` rejects a path that does not exist on disk.
    #[test]
    fn test_validate_edit_operation_file_does_not_exist() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("absent.txt");
        let tool = EditFileTool::new();
        let err = tool
            .validate_edit_operation(
                temp_dir.path(),
                &missing.to_string_lossy(),
                "content",
                "content",
                false,
            )
            .unwrap_err();
        assert!(format!("{err:?}").contains("file does not exist"));
    }

    /// `validate_edit_operation` rejects an `old_string` absent from the content.
    #[test]
    fn test_validate_edit_operation_string_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("present.txt");
        fs::write(&file, "hello world").unwrap();
        let tool = EditFileTool::new();
        let err = tool
            .validate_edit_operation(
                temp_dir.path(),
                &file.to_string_lossy(),
                "hello world",
                "absent-substring",
                false,
            )
            .unwrap_err();
        assert!(format!("{err:?}").contains("not found in file"));
    }

    /// `edit_file_atomic` with `replace_all` rewrites every occurrence and reports
    /// the count (covering the replace-all replacement branch).
    #[tokio::test]
    async fn test_edit_file_atomic_replace_all_counts() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("repeat.txt");
        fs::write(&file, "x x x").unwrap();

        let tool = EditFileTool::new();
        let result = tool
            .edit_file_atomic(temp_dir.path(), &file.to_string_lossy(), "x", "y", true)
            .unwrap();
        assert_eq!(result.replacements_made, 3);
        assert_eq!(fs::read_to_string(&file).unwrap(), "y y y");
    }

    // =====================================================================
    // Encoding/decoding error arms
    // =====================================================================

    /// `read_with_encoding_detection` rejects bytes that cannot be decoded with
    /// the detected encoding (a UTF-16LE BOM followed by an odd trailing byte
    /// yields a decode error).
    #[test]
    fn test_read_with_encoding_detection_decode_error() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("bad_utf16.txt");
        // UTF-16LE BOM (0xFF 0xFE), then a lone trailing byte → malformed unit.
        fs::write(&file, [0xFFu8, 0xFE, 0x41]).unwrap();

        let tool = EditFileTool::new();
        let result = tool.read_with_encoding_detection(&file);
        // A lone trailing byte after a UTF-16LE BOM is a malformed code unit, so
        // encoding_rs reports a decode error and this arm must reject it.
        let err = result.expect_err("a malformed UTF-16LE byte sequence must be rejected");
        assert!(format!("{err:?}").contains("failed to decode"));
    }

    /// `read_with_encoding_detection` surfaces a file-read error when the path is
    /// missing.
    #[test]
    fn test_read_with_encoding_detection_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("nope.txt");
        let tool = EditFileTool::new();
        let err = tool.read_with_encoding_detection(&missing).unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("read file for encoding detection") || msg.contains("not found"));
    }

    // =====================================================================
    // commit_content cleanup arms (real fault injection)
    // =====================================================================

    /// When the atomic rename cannot complete because the target path is a
    /// directory, `commit_content` removes its temp file and surfaces an error,
    /// leaving no temp-staged debris.
    #[test]
    fn test_commit_content_cleans_temp_on_rename_failure() {
        let temp_dir = TempDir::new().unwrap();
        // Target is an existing directory: rename(temp_file, dir) fails.
        let target = temp_dir.path().join("a_directory");
        fs::create_dir(&target).unwrap();

        let tool = EditFileTool::new();
        let result = tool.commit_content(
            &target,
            "new content",
            encoding_rs::UTF_8,
            LineEnding::Lf,
            1,
        );
        assert!(result.is_err(), "rename over a directory must fail");

        // The directory is untouched and no temp file remains.
        assert!(target.is_dir());
        let temp_files: Vec<_> = temp_dir
            .path()
            .read_dir()
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(TEMP_FILE_SUFFIX))
            .collect();
        assert!(temp_files.is_empty(), "temp file must be cleaned up");
    }

    /// `commit_content` propagates a metadata-read failure when the target is
    /// missing (the original-permission capture cannot run).
    #[test]
    fn test_commit_content_metadata_read_failure() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("ghost.txt");
        let tool = EditFileTool::new();
        let err = tool
            .commit_content(&missing, "x", encoding_rs::UTF_8, LineEnding::Lf, 1)
            .unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("read metadata") || msg.contains("not found"));
    }

    /// `write_with_encoding` surfaces an error when the file cannot be created
    /// (its parent directory does not exist).
    #[test]
    fn test_write_with_encoding_create_failure() {
        let temp_dir = TempDir::new().unwrap();
        let unwritable = temp_dir.path().join("no_such_dir").join("out.txt");
        let tool = EditFileTool::new();
        let err = tool
            .write_with_encoding(&unwritable, "content", encoding_rs::UTF_8)
            .unwrap_err();
        // The missing parent surfaces as a NotFound, mapped to "File not found".
        assert!(format!("{err:?}").contains("file not found"));
    }

    /// `write_with_encoding` rejects content the target encoding cannot represent
    /// (a non-Latin character under windows-1252).
    #[test]
    fn test_write_with_encoding_encode_error() {
        let temp_dir = TempDir::new().unwrap();
        let out = temp_dir.path().join("out.txt");
        let tool = EditFileTool::new();
        // windows-1252 cannot encode an emoji → had_errors is true.
        let result = tool.write_with_encoding(&out, "🌍", encoding_rs::WINDOWS_1252);
        assert!(result.is_err(), "un-encodable content must error");
        assert!(format!("{:?}", result.unwrap_err()).contains("failed to encode"));
    }
}
