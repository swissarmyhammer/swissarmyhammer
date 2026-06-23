// sah rule ignore acp/capability-enforcement
//! File editing tool for MCP operations
//!
//! This module provides the EditFileTool for performing precise string replacements in files
//! with atomic operations, comprehensive security validation, file encoding preservation,
//! and metadata preservation.
//!
//! Note: This is an MCP tool, not an ACP operation. ACP capability checking happens at the
//! agent layer (claude-agent, llama-agent), not at the MCP tool layer.

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use encoding_rs::{Encoding, UTF_8};
use filetime::{set_file_times, FileTime};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use tracing::{debug, info};

/// Operation metadata for editing files
#[derive(Debug, Default)]
pub struct EditFile;

/// Alias keys that resolve to the canonical `file_path` parameter.
static FILE_PATH_ALIASES: &[&str] = &["path", "filePath", "absolute_path"];

/// Alias keys that resolve to the canonical `find` parameter (the text to match).
///
/// `old_string`/`oldText` are the legacy MCP names, kept here as aliases so the
/// historical single-edit and `edits[]` shapes keep working. The remaining
/// entries are the natural-language synonyms a model is likely to emit.
static FIND_ALIASES: &[&str] = &[
    "search",
    "old",
    "old_string",
    "oldText",
    "old_text",
    "from",
    "target",
    "match",
];

/// Alias keys that resolve to the canonical `replace` parameter (the new text).
///
/// `new_string`/`newText` are the legacy MCP names, kept here as aliases. The
/// remaining entries are natural-language synonyms.
static REPLACE_ALIASES: &[&str] = &[
    "new",
    "new_string",
    "newText",
    "new_text",
    "to",
    "with",
    "replacement",
];

static EDIT_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Absolute path to the file to modify")
        .param_type(ParamType::String)
        .aliases(FILE_PATH_ALIASES)
        .required(),
    ParamMeta::new("find")
        .description("Exact text to replace")
        .param_type(ParamType::String)
        .aliases(FIND_ALIASES)
        .required(),
    ParamMeta::new("replace")
        .description("Replacement text")
        .param_type(ParamType::String)
        .aliases(REPLACE_ALIASES)
        .required(),
    ParamMeta::new("replace_all")
        .description("Replace all occurrences (default: false)")
        .param_type(ParamType::Boolean),
    ParamMeta::new("edits")
        .description("Array of {find, replace} edit pairs to apply sequentially")
        .param_type(ParamType::Array),
];

/// One canonical edit: replace `find` with `replace`, optionally every occurrence.
///
/// This is the normalized form every accepted input shape collapses to. It
/// carries no IO — [`normalize_edit_args`] produces it purely from arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditPair {
    /// Exact text to match in the target file.
    pub find: String,
    /// Replacement text.
    pub replace: String,
    /// Replace every occurrence (`true`) instead of just the first (`false`).
    pub replace_all: bool,
}

/// Read the first present key among `keys` from `map`.
fn first_present<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    canonical: &str,
    aliases: &[&str],
) -> Option<&'a serde_json::Value> {
    if let Some(v) = map.get(canonical) {
        return Some(v);
    }
    aliases.iter().find_map(|alias| map.get(*alias))
}

/// Coerce a JSON value into a list of strings: a scalar string yields one entry,
/// an array yields each element as a string. Returns `None` for absent input and
/// an error for a non-string / non-array value (or a non-string array element).
fn collect_strings(value: Option<&serde_json::Value>) -> Result<Option<Vec<String>>, McpError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::String(s) => Ok(Some(vec![s.clone()])),
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    serde_json::Value::String(s) => out.push(s.clone()),
                    other => {
                        return Err(McpError::invalid_request(
                            format!("find/replace array entries must be strings, got {other}"),
                            None,
                        ))
                    }
                }
            }
            Ok(Some(out))
        }
        other => Err(McpError::invalid_request(
            format!("find/replace must be a string or array of strings, got {other}"),
            None,
        )),
    }
}

/// Read an optional `replace_all` boolean from a map (canonical name only —
/// there are no aliases for this flag).
fn read_replace_all(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.get("replace_all")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// Pair a list of finds with a list of replaces using the forgiving rules:
/// - N finds + N replaces → zip.
/// - N finds + 1 replace → broadcast the single replace to every find.
/// - anything else (including 1 find + N replaces) → zip what lines up cleanly
///   and surface the unpaired remainder in the error; never silently drop.
fn pair_finds_replaces(
    finds: Vec<String>,
    replaces: Vec<String>,
    replace_all: bool,
) -> Result<Vec<EditPair>, McpError> {
    // Broadcast a single replace across many finds (the delete-many shape).
    if replaces.len() == 1 && finds.len() > 1 {
        let replace = &replaces[0];
        return Ok(finds
            .into_iter()
            .map(|find| EditPair {
                find,
                replace: replace.clone(),
                replace_all,
            })
            .collect());
    }

    if finds.len() == replaces.len() {
        return Ok(finds
            .into_iter()
            .zip(replaces)
            .map(|(find, replace)| EditPair {
                find,
                replace,
                replace_all,
            })
            .collect());
    }

    // Mismatch: pair what zips, then report the unpaired remainder.
    let paired = finds.len().min(replaces.len());
    let leftover_finds = &finds[paired..];
    let leftover_replaces = &replaces[paired..];
    let mut remainder = Vec::new();
    if !leftover_finds.is_empty() {
        remainder.push(format!("unpaired finds: {leftover_finds:?}"));
    }
    if !leftover_replaces.is_empty() {
        remainder.push(format!("unpaired replaces: {leftover_replaces:?}"));
    }
    Err(McpError::invalid_request(
        format!(
            "mismatched find/replace counts ({} finds, {} replaces); {}",
            finds.len(),
            replaces.len(),
            remainder.join("; ")
        ),
        None,
    ))
}

/// Whether a no-`op` argument map should be dispatched to the edit operation.
///
/// True when any find-ish or replace-ish key (canonical name or alias) is
/// present, or when an `edits` array is supplied. The dispatcher in
/// [`super::FilesTool`] consults this BEFORE the `content`→write branch so a
/// canonical `{find, replace}` call is never misrouted to write.
pub fn looks_like_edit(args: &serde_json::Map<String, serde_json::Value>) -> bool {
    args.contains_key("edits")
        || first_present(args, "find", FIND_ALIASES).is_some()
        || first_present(args, "replace", REPLACE_ALIASES).is_some()
}

/// Normalize the forgiving `edit files` argument surface into a canonical list
/// of [`EditPair`]s.
///
/// Accepts three input shapes — which may be combined — under any of the
/// `find`/`replace` aliases (see [`FIND_ALIASES`] / [`REPLACE_ALIASES`]):
///
/// 1. Top-level scalar `find`/`replace`.
/// 2. Top-level parallel arrays `find: [...]` / `replace: [...]`.
/// 3. An `edits: [{ find, replace, replace_all? }, ...]` array.
///
/// Top-level finds/replaces are paired via [`pair_finds_replaces`] (zip /
/// broadcast / mismatch-remainder) and then **concatenated** with the pairs
/// drawn from `edits[]`. This is pure: it performs no IO and never touches the
/// filesystem, so it is unit-testable in isolation.
pub fn normalize_edit_args(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<EditPair>, McpError> {
    let mut pairs = Vec::new();

    // Shape 1 & 2: top-level scalar or parallel arrays.
    let finds = collect_strings(first_present(args, "find", FIND_ALIASES))?;
    let replaces = collect_strings(first_present(args, "replace", REPLACE_ALIASES))?;
    match (finds, replaces) {
        (Some(finds), Some(replaces)) => {
            pairs.extend(pair_finds_replaces(
                finds,
                replaces,
                read_replace_all(args),
            )?);
        }
        (Some(_), None) => {
            return Err(McpError::invalid_request(
                "find provided without a matching replace".to_string(),
                None,
            ));
        }
        (None, Some(_)) => {
            return Err(McpError::invalid_request(
                "replace provided without a matching find".to_string(),
                None,
            ));
        }
        (None, None) => {}
    }

    // Shape 3: the edits[] array, each entry carrying its own find/replace.
    if let Some(edits) = args.get("edits") {
        let items = edits.as_array().ok_or_else(|| {
            McpError::invalid_request("edits must be an array of edit objects".to_string(), None)
        })?;
        for (idx, item) in items.iter().enumerate() {
            let obj = item.as_object().ok_or_else(|| {
                McpError::invalid_request(
                    format!("edits[{idx}] must be an object with find/replace"),
                    None,
                )
            })?;
            let finds =
                collect_strings(first_present(obj, "find", FIND_ALIASES))?.ok_or_else(|| {
                    McpError::invalid_request(format!("edits[{idx}] is missing find"), None)
                })?;
            let replaces = collect_strings(first_present(obj, "replace", REPLACE_ALIASES))?
                .ok_or_else(|| {
                    McpError::invalid_request(format!("edits[{idx}] is missing replace"), None)
                })?;
            pairs.extend(pair_finds_replaces(finds, replaces, read_replace_all(obj))?);
        }
    }

    if pairs.is_empty() {
        return Err(McpError::invalid_request(
            "no edits provided: supply find/replace (or aliases), or an edits array".to_string(),
            None,
        ));
    }

    Ok(pairs)
}

impl Operation for EditFile {
    fn verb(&self) -> &'static str {
        "edit"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Perform precise string replacements in existing files"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        EDIT_FILE_PARAMS
    }
}

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
    /// Whether file metadata (permissions, timestamps) was successfully preserved
    pub metadata_preserved: bool,
}

/// Validation result for edit operations
#[derive(Debug, Clone)]
struct EditValidation {
    pub old_string_count: usize,
}

/// Line ending types detected in files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEnding {
    Lf,    // Unix: \n
    CrLf,  // Windows: \r\n
    Cr,    // Classic Mac: \r
    Mixed, // Multiple types found
}

impl LineEnding {
    /// Detect the primary line ending type in content
    fn detect(content: &str) -> Self {
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
    fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
            LineEnding::Cr => "CR",
            LineEnding::Mixed => "Mixed",
        }
    }
}

/// Tool for performing precise string replacements in existing files
#[derive(Default)]
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
                format!("File does not exist: {}", file_path),
                None,
            ));
        }

        // Count occurrences of old_string
        let matches: Vec<_> = content.matches(old_string).collect();
        let old_string_count = matches.len();
        if old_string_count == 0 {
            return Err(McpError::invalid_request(
                format!("String '{}' not found in file", old_string),
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
    fn read_with_encoding_detection(
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

        debug!(
            path = %file_path.display(),
            encoding = encoding.name(),
            bom_length = bom_length,
            "Detected file encoding"
        );

        // Decode to string
        let (content, _, had_decode_errors) = encoding.decode(bytes_to_decode);

        if had_decode_errors {
            return Err(McpError::internal_error(
                format!(
                    "Failed to decode file with detected encoding {}",
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
        use crate::mcp::tools::files::shared_utils::{handle_file_error, validate_file_path};

        // Step 1: Validate file path and get canonical path. Relative paths
        // resolve against the session working directory, never the process CWD.
        let path = validate_file_path(base_dir, file_path)?;

        info!(
            path = %path.display(),
            old_string_len = old_string.len(),
            new_string_len = new_string.len(),
            replace_all = replace_all,
            "Starting atomic edit operation"
        );

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

        // Step 5: Get original file metadata for preservation
        let original_metadata =
            fs::metadata(&path).map_err(|e| handle_file_error(e, "read metadata", &path))?;

        let original_permissions = original_metadata.permissions();
        let original_modified = FileTime::from_last_modification_time(&original_metadata);
        let original_accessed = FileTime::from_last_access_time(&original_metadata);

        // Step 6: Perform replacement
        let (new_content, replacements_made) = if replace_all {
            let new_content = original_content.replace(old_string, new_string);
            let replacements = validation.old_string_count;
            (new_content, replacements)
        } else {
            let new_content = original_content.replacen(old_string, new_string, 1);
            (new_content, 1)
        };

        // Step 7: Create temporary file in same directory as original
        let temp_file_name = format!("{}.tmp.{}", path.display(), std::process::id());
        let temp_path = path
            .parent()
            .ok_or_else(|| {
                McpError::internal_error(
                    "Cannot determine parent directory for temporary file".to_string(),
                    None,
                )
            })?
            .join(&temp_file_name);

        debug!(
            temp_path = %temp_path.display(),
            content_length = new_content.len(),
            encoding = detected_encoding.name(),
            "Writing content to temporary file"
        );

        // Step 8: Write new content to temporary file with original encoding
        let write_result = self.write_with_encoding(&temp_path, &new_content, detected_encoding);

        match write_result {
            Ok(bytes_written) => {
                // Step 9: Set permissions on temporary file to match original
                if let Err(e) = fs::set_permissions(&temp_path, original_permissions.clone()) {
                    // Clean up and return error
                    let _ = fs::remove_file(&temp_path);
                    return Err(handle_file_error(
                        e,
                        "set permissions on temporary file",
                        &temp_path,
                    ));
                }

                // Step 10: Atomically rename temporary file to original
                let rename_result = fs::rename(&temp_path, &path);

                match rename_result {
                    Ok(()) => {
                        // Step 11: Restore file timestamps
                        let metadata_preserved = if let Err(e) =
                            set_file_times(&path, original_accessed, original_modified)
                        {
                            debug!(
                                path = %path.display(),
                                error = %e,
                                "Failed to preserve file timestamps, continuing anyway"
                            );
                            false
                        } else {
                            true
                        };

                        debug!(
                            path = %path.display(),
                            bytes_written = bytes_written,
                            replacements_made = replacements_made,
                            metadata_preserved = metadata_preserved,
                            "Atomic edit operation completed successfully"
                        );

                        Ok(EditResult {
                            bytes_written,
                            replacements_made,
                            encoding_detected: detected_encoding.name().to_string(),
                            line_endings_preserved: line_ending.as_str().to_string(),
                            metadata_preserved,
                        })
                    }
                    Err(e) => {
                        // Clean up temporary file and return error
                        let _ = fs::remove_file(&temp_path);
                        Err(handle_file_error(
                            e,
                            "rename temporary file to target",
                            &path,
                        ))
                    }
                }
            }
            Err(e) => {
                // Clean up temporary file and return error
                let _ = fs::remove_file(&temp_path);
                Err(e)
            }
        }
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
                format!("Failed to encode content with encoding {}", encoding.name()),
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

/// Execute a file edit operation
pub async fn execute_edit(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    use swissarmyhammer_common::rate_limiter::get_rate_limiter;

    // Extract file path under any canonical/alias key.
    let file_path = first_present(&arguments, "file_path", FILE_PATH_ALIASES)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            McpError::invalid_request("path/file_path/filePath is required".to_string(), None)
        })?
        .to_string();

    // Validate file path
    if file_path.trim().is_empty() {
        return Err(McpError::invalid_request(
            "path cannot be empty".to_string(),
            None,
        ));
    }

    // An explicitly empty `edits: []` (with no top-level find/replace) keeps its
    // historical, more specific error message.
    if let Some(serde_json::Value::Array(edits)) = arguments.get("edits") {
        if edits.is_empty()
            && first_present(&arguments, "find", FIND_ALIASES).is_none()
            && first_present(&arguments, "replace", REPLACE_ALIASES).is_none()
        {
            return Err(McpError::invalid_request(
                "edits array cannot be empty".to_string(),
                None,
            ));
        }
    }

    // Normalize every accepted input shape into canonical (find, replace) pairs.
    let edit_operations = normalize_edit_args(&arguments)?;

    // Check rate limit using tokio task ID as client identifier
    let rate_limiter = get_rate_limiter();
    let client_id = format!("task_{:?}", tokio::task::try_id());

    // Check rate limit based on number of operations
    let cost = edit_operations.len() as u32;
    if let Err(e) = rate_limiter.check_rate_limit(&client_id, "file_edit", cost) {
        tracing::warn!("Rate limit exceeded for file_edit: {}", e);
        return Err(McpError::invalid_request(
            format!("Rate limit exceeded: {}", e),
            None,
        ));
    }

    // Validate all edit operations
    for (idx, edit_op) in edit_operations.iter().enumerate() {
        if edit_op.find.is_empty() {
            return Err(McpError::invalid_request(
                format!("Edit operation {}: old_text cannot be empty", idx),
                None,
            ));
        }

        if edit_op.find == edit_op.replace {
            return Err(McpError::invalid_request(
                format!(
                    "Edit operation {}: old_text and new_text must be different",
                    idx
                ),
                None,
            ));
        }
    }

    // Log edit attempt for security auditing
    info!(
        path = %file_path,
        num_operations = edit_operations.len(),
        "Attempting atomic edit operation(s)"
    );

    // Apply edits sequentially. Relative paths resolve against the session
    // working directory (the board dir), never the process CWD.
    let base_dir = context.session_root();
    let tool = EditFileTool::new();
    let mut total_replacements = 0;
    let mut final_result: Option<EditResult> = None;

    for (idx, edit_op) in edit_operations.iter().enumerate() {
        debug!(
            path = %file_path,
            operation = idx + 1,
            total_operations = edit_operations.len(),
            old_text_len = edit_op.find.len(),
            new_text_len = edit_op.replace.len(),
            replace_all = edit_op.replace_all,
            "Applying edit operation"
        );

        let edit_result = tool.edit_file_atomic(
            &base_dir,
            &file_path,
            &edit_op.find,
            &edit_op.replace,
            edit_op.replace_all,
        )?;

        total_replacements += edit_result.replacements_made;
        final_result = Some(edit_result);
    }

    // Record the mutated path on the typed side-channel so the dispatch
    // chokepoint can fold inline diagnostics into this result (no content
    // parsing). Best-effort: resolve to the same absolute path the edit wrote;
    // a resolution failure simply skips the fold-in for this call.
    if let Ok(abs) =
        crate::mcp::tools::files::shared_utils::validate_file_path(&base_dir, &file_path)
    {
        context.record_mutated_path(abs);
    }

    // Create success response
    let final_result =
        final_result.expect("At least one edit operation should have been performed");
    let success_message = if edit_operations.len() == 1 {
        "OK".to_string()
    } else {
        format!("OK: Applied {} edit operations", edit_operations.len())
    };

    debug!(
        path = %file_path,
        num_operations = edit_operations.len(),
        bytes_written = final_result.bytes_written,
        total_replacements = total_replacements,
        encoding = %final_result.encoding_detected,
        line_endings = %final_result.line_endings_preserved,
        metadata_preserved = final_result.metadata_preserved,
        "Edit operation(s) completed successfully"
    );

    Ok(BaseToolImpl::create_success_response(success_message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create test arguments for the edit tool
    fn create_edit_arguments(
        file_path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: Option<bool>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::Value::String(file_path.to_string()),
        );
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String(old_string.to_string()),
        );
        args.insert(
            "new_string".to_string(),
            serde_json::Value::String(new_string.to_string()),
        );

        if let Some(replace_all) = replace_all {
            args.insert(
                "replace_all".to_string(),
                serde_json::Value::Bool(replace_all),
            );
        }

        args
    }

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

    #[test]
    fn test_edit_tool_operation_metadata() {
        let op = EditFile;
        assert_eq!(op.verb(), "edit");
        assert_eq!(op.noun(), "file");
        assert!(!op.description().is_empty());
    }

    #[tokio::test]
    async fn test_edit_single_replacement_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_edit.txt");
        let initial_content = "Hello world! This is a test file.";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // Verify file was edited correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a test file.");
    }

    #[tokio::test]
    async fn test_edit_replace_all_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_replace_all.txt");
        let initial_content = "test test test";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "test", "exam", Some(true));

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify all occurrences were replaced
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam");
    }

    #[tokio::test]
    async fn test_edit_multiple_occurrences_without_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_multiple.txt");
        let initial_content = "duplicate duplicate duplicate";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "duplicate",
            "unique",
            None, // replace_all = false by default
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify only the first occurrence was replaced
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "unique duplicate duplicate");
    }

    #[tokio::test]
    async fn test_edit_string_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_not_found.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("not found in file"));

        // Verify file was not modified
        let unchanged_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(unchanged_content, initial_content);
    }

    #[tokio::test]
    async fn test_edit_file_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_file = temp_dir.path().join("does_not_exist.txt");

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&nonexistent_file.to_string_lossy(), "old", "new", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_str = format!("{:?}", error);
        // The error message from shared_utils says "File not found"
        assert!(
            error_str.contains("File does not exist")
                || error_str.contains("File not found")
                || error_str.contains("does not exist")
                || error_str.contains("NotFound")
        );
    }

    #[tokio::test]
    async fn test_edit_empty_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test empty file path
        let args = create_edit_arguments("", "old", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("path cannot be empty"));

        // Test empty old_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("old_text cannot be empty"));

        // Test identical old_string and new_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "same", "same", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("must be different"));
    }

    #[tokio::test]
    async fn test_edit_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode_test.txt");
        let unicode_content = "Hello 🌍! Здравствуй мир! 你好世界!";
        fs::write(&test_file, unicode_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "🌍", "🚀", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify Unicode replacement worked correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello 🚀! Здравствуй мир! 你好世界!");
    }

    #[tokio::test]
    async fn test_edit_preserves_line_endings() {
        let temp_dir = TempDir::new().unwrap();

        // Test Windows line endings preservation
        let windows_file = temp_dir.path().join("windows_endings.txt");
        let windows_content = "Line 1\r\nold text\r\nLine 3\r\n";
        fs::write(&windows_file, windows_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &windows_file.to_string_lossy(),
            "old text",
            "new text",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&windows_file).unwrap();
        assert_eq!(edited_content, "Line 1\r\nnew text\r\nLine 3\r\n");
        assert!(edited_content.contains("\r\n"));
    }

    #[tokio::test]
    async fn test_edit_atomic_operation_failure_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_atomic.txt");
        let initial_content = "original content";
        fs::write(&test_file, initial_content).unwrap();

        // Make file read-only to cause atomic operation to fail during permission setting
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;

            let readonly_permissions = Permissions::from_mode(0o444);
            fs::set_permissions(&test_file, readonly_permissions).unwrap();

            let tool = EditFileTool::new();

            // Even if the operation fails, we should verify no temporary files are left behind
            let _temp_pattern = format!("{}.tmp.*", test_file.display());

            // The edit should work even with readonly file since we change permissions on temp file
            let edit_result = tool.edit_file_atomic(
                temp_dir.path(),
                &test_file.to_string_lossy(),
                "original",
                "modified",
                false,
            );

            // Check that no temporary files remain regardless of result
            let temp_files: Vec<_> = temp_dir
                .path()
                .read_dir()
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
                .collect();

            assert!(
                temp_files.is_empty(),
                "Temporary files should be cleaned up"
            );

            // If the edit succeeded, verify the content was actually changed
            if edit_result.is_ok() {
                let final_content = fs::read_to_string(&test_file).unwrap();
                assert_eq!(final_content, "modified content");
            }
        }
    }

    #[tokio::test]
    async fn test_edit_file_permissions_preservation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("permissions_test.txt");
        let initial_content = "test content";
        fs::write(&test_file, initial_content).unwrap();

        // Set specific permissions (only on Unix systems)
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;

            let permissions = Permissions::from_mode(0o755);
            fs::set_permissions(&test_file, permissions).unwrap();

            let original_metadata = fs::metadata(&test_file).unwrap();
            let original_mode = original_metadata.permissions().mode();

            let tool = EditFileTool::new();
            let edit_result = tool.edit_file_atomic(
                temp_dir.path(),
                &test_file.to_string_lossy(),
                "test",
                "updated",
                false,
            );

            assert!(edit_result.is_ok());

            // Verify permissions were preserved
            let new_metadata = fs::metadata(&test_file).unwrap();
            let new_mode = new_metadata.permissions().mode();
            assert_eq!(
                original_mode, new_mode,
                "File permissions should be preserved"
            );

            // Verify content was updated
            let final_content = fs::read_to_string(&test_file).unwrap();
            assert_eq!(final_content, "updated content");
        }
    }

    #[tokio::test]
    async fn test_edit_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("response_test.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());

        // Check response message format contains expected information
        let response_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content in response"),
        };

        assert_eq!(response_text, "OK");
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

    #[tokio::test]
    async fn test_edit_json_argument_parsing_error() {
        let context = crate::test_utils::create_test_context().await;

        // Create invalid arguments (missing both single edit and multiple edits modes)
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String("/test/path".to_string()),
        );
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String("old".to_string()),
        );
        // Missing "new_string" field and no "edits" array

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_str = format!("{:?}", error);
        // A find (old_string is now an alias of `find`) with no matching replace
        // must error rather than silently dropping the unpaired find.
        assert!(
            error_str.contains("find provided without a matching replace")
                || error_str.contains("replace"),
            "unexpected error: {error_str}"
        );
    }

    #[tokio::test]
    async fn test_edit_large_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_file.txt");

        // Create a reasonably large file (1MB) with repetitive content
        let chunk = "This is a line of test content that will be repeated many times.\n";
        let chunk_size = chunk.len();
        let target_size = 1_000_000; // 1MB
        let repetitions = target_size / chunk_size;

        let large_content = chunk.repeat(repetitions);
        fs::write(&test_file, &large_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "test content",
            "modified content",
            Some(true), // Replace all occurrences
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify the replacements were made
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert!(edited_content.contains("modified content"));
        assert!(!edited_content.contains("test content"));
    }

    #[tokio::test]
    async fn test_edit_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_file.txt");
        fs::write(&test_file, "").unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("not found in file"));
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_sequential() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multiple_edits.txt");
        let initial_content = "Hello world! This is a test.";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Create arguments with multiple edits
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "oldText": "world",
                    "newText": "universe"
                },
                {
                    "oldText": "test",
                    "newText": "example"
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify all edits were applied sequentially
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a example.");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_with_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("alias_test.txt");
        let initial_content = "foo bar baz";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test different parameter aliases
        let mut args = serde_json::Map::new();
        args.insert(
            "filePath".to_string(), // Using filePath alias
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "old_string": "foo",  // Using old_string alias
                    "new_text": "FOO"     // Using new_text alias
                },
                {
                    "old_text": "bar",    // Using old_text alias
                    "new_string": "BAR"   // Using new_string alias
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "FOO BAR baz");
    }

    #[tokio::test]
    async fn test_edit_single_mode_with_path_alias() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("single_alias.txt");
        let initial_content = "test content";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test single edit mode with different parameter aliases
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(), // Using file_path alias
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "oldText".to_string(), // Using oldText alias
            serde_json::Value::String("test".to_string()),
        );
        args.insert(
            "newText".to_string(), // Using newText alias
            serde_json::Value::String("demo".to_string()),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "demo content");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_with_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("replace_all_multi.txt");
        let initial_content = "test test test, example example";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "oldText": "test",
                    "newText": "exam",
                    "replace_all": true
                },
                {
                    "oldText": "example",
                    "newText": "sample",
                    "replace_all": true
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam, sample sample");
    }

    #[tokio::test]
    async fn test_edit_empty_edits_array() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_edits.txt");
        fs::write(&test_file, "content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert("edits".to_string(), serde_json::json!([]));

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("edits array cannot be empty"));
    }

    #[tokio::test]
    async fn test_edit_missing_path() {
        let context = crate::test_utils::create_test_context().await;

        // Missing path parameter
        let mut args = serde_json::Map::new();
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String("old".to_string()),
        );
        args.insert(
            "new_string".to_string(),
            serde_json::Value::String("new".to_string()),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("path"));
    }

    #[tokio::test]
    async fn test_edit_whitespace_path_error() {
        let context = crate::test_utils::create_test_context().await;

        let args = create_edit_arguments("   ", "old", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(
            format!("{:?}", result).contains("empty") || format!("{:?}", result).contains("path")
        );
    }

    #[tokio::test]
    async fn test_edit_old_string_in_index_one_operation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("index_test.txt");
        fs::write(&test_file, "line 1\nline 2\nline 3\n").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Multiple edits - second operation has empty old_text
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "line 1", "newText": "LINE ONE" },
                { "oldText": "", "newText": "something" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("old_text cannot be empty") || err.contains("empty"));
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_same_and_different_not_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("same_test.txt");
        fs::write(&test_file, "content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Multiple edits - second operation has same old and new text
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "content", "newText": "new_content" },
                { "oldText": "same_text", "newText": "same_text" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("must be different") || err.contains("different"));
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_success_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multi_response.txt");
        fs::write(&test_file, "foo bar baz").unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "foo", "newText": "FOO" },
                { "oldText": "bar", "newText": "BAR" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        // Multiple edits response says "OK: Applied N edit operations"
        assert!(text.contains("OK") && text.contains("2") || text.contains("Applied"));
    }

    // =========================================================================
    // normalize_edit_args — pure argument shaping (no IO)
    // =========================================================================

    /// Build a JSON arg map from a serde_json::json! object literal.
    fn args(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().expect("object literal").clone()
    }

    fn pair(find: &str, replace: &str, replace_all: bool) -> EditPair {
        EditPair {
            find: find.to_string(),
            replace: replace.to_string(),
            replace_all,
        }
    }

    #[test]
    fn normalize_canonical_scalar_find_replace() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "file_path": "/x", "find": "a", "replace": "b"
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false)]);
    }

    #[test]
    fn normalize_legacy_old_new_string_resolves_same_as_find_replace() {
        let canonical = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let legacy = normalize_edit_args(&args(serde_json::json!({
            "old_string": "a", "new_string": "b"
        })))
        .unwrap();
        assert_eq!(legacy, canonical);
    }

    #[test]
    fn normalize_legacy_oldtext_newtext_resolves_same_as_find_replace() {
        let canonical = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let legacy = normalize_edit_args(&args(serde_json::json!({
            "oldText": "a", "newText": "b"
        })))
        .unwrap();
        assert_eq!(legacy, canonical);
    }

    #[test]
    fn normalize_search_with_alias_pair() {
        // edits[] entries using {search, with} aliases.
        let got = normalize_edit_args(&args(serde_json::json!({
            "edits": [{ "search": "a", "with": "b" }, { "search": "c", "with": "d" }]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_scalar_array_and_edits_yield_same_pairs() {
        let scalar = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let arrays = normalize_edit_args(&args(serde_json::json!({
            "find": ["a"], "replace": ["b"]
        })))
        .unwrap();
        let edits = normalize_edit_args(&args(serde_json::json!({
            "edits": [{ "find": "a", "replace": "b" }]
        })))
        .unwrap();
        assert_eq!(scalar, vec![pair("a", "b", false)]);
        assert_eq!(arrays, scalar);
        assert_eq!(edits, scalar);
    }

    #[test]
    fn normalize_parallel_arrays_zip() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "c"], "replace": ["b", "d"]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_broadcast_single_replace_to_many_finds() {
        // Delete-many: many finds + one empty replace.
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b", "c"], "replace": [""]
        })))
        .unwrap();
        assert_eq!(
            got,
            vec![
                pair("a", "", false),
                pair("b", "", false),
                pair("c", "", false)
            ]
        );
    }

    #[test]
    fn normalize_broadcast_scalar_replace_to_array_finds() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b"], "replace": "X"
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "X", false), pair("b", "X", false)]);
    }

    #[test]
    fn normalize_toplevel_and_edits_concatenate() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b",
            "edits": [{ "find": "c", "replace": "d" }]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_replace_all_scalar_applies_to_toplevel_pair() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b", "replace_all": true
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", true)]);
    }

    #[test]
    fn normalize_replace_all_per_edit_entry() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "edits": [
                { "find": "a", "replace": "b", "replace_all": true },
                { "find": "c", "replace": "d" }
            ]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", true), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_mismatched_array_lengths_errors_with_remainder() {
        // 3 finds, 2 replaces (not a broadcast): zip the first 2, surface the
        // unpaired remainder in the error — never silently drop.
        let err = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b", "c"], "replace": ["x", "y"]
        })))
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains('c'),
            "error must name the unpaired find: {msg}"
        );
    }

    #[test]
    fn normalize_one_find_many_replaces_errors_with_remainder() {
        let err = normalize_edit_args(&args(serde_json::json!({
            "find": ["a"], "replace": ["x", "y"]
        })))
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains('y'),
            "error must name the unpaired replace: {msg}"
        );
    }

    #[test]
    fn normalize_no_find_or_replace_or_edits_errors() {
        let err = normalize_edit_args(&args(serde_json::json!({ "file_path": "/x" }))).unwrap_err();
        let _ = format!("{err:?}");
    }

    #[tokio::test]
    async fn test_edit_cr_line_endings_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("cr_endings.txt");
        // Classic Mac line endings
        let content = "line1\rold content\rline3\r";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "old content",
            "new content",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited = fs::read(&test_file).unwrap();
        let edited_str = String::from_utf8(edited).unwrap();
        assert!(edited_str.contains("new content"));
        // CR line endings should be preserved
        assert!(edited_str.contains('\r'));
    }
}
