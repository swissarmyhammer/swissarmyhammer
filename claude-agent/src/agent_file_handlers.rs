//! File operation handlers for Agent Client Protocol
//!
//! This module contains the implementation of file read/write operations
//! for the ClaudeAgent, implementing the ACP fs/read_text_file and
//! fs/write_text_file methods.

use crate::agent_file_operations::{ReadTextFileParams, ReadTextFileResponse, WriteTextFileParams};
use crate::constants::sizes;
use agent_client_protocol::{SessionId, WriteTextFileResponse};
use swissarmyhammer_common::Pretty;

impl crate::agent::ClaudeAgent {
    /// Handle fs/read_text_file ACP extension method
    ///
    /// ACP requires integration with client editor state to access unsaved changes.
    /// This method:
    /// 1. Checks if an editor buffer is available for the file
    /// 2. Falls back to disk content if no editor buffer exists
    /// 3. Applies line filtering if requested
    ///
    /// This ensures agents work with current, not stale, file content.
    pub async fn handle_read_text_file(
        &self,
        params: ReadTextFileParams,
    ) -> Result<ReadTextFileResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing fs/read_text_file request: {}", Pretty(&params));

        // Audit logging for file access attempt
        tracing::info!(
            security_event = "file_read_attempt",
            session_id = %params.session_id,
            path = %params.path,
            "File read operation requested"
        );

        // Validate client capabilities for file system read operations
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.fs.read_text_file => {
                    tracing::debug!("File system read capability validated");
                }
                Some(_) => {
                    tracing::error!("fs/read_text_file capability not declared by client");
                    return Err(agent_client_protocol::Error::new(-32602,
                        "File system read capability not declared by client. Set client_capabilities.fs.read_text_file = true during initialization."
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for fs/read_text_file validation"
                    );
                    return Err(agent_client_protocol::Error::new(-32602,
                        "Client capabilities not initialized. Cannot perform file system operations without capability declaration."
                    ));
                }
            }
        }

        // Validate session ID
        self.parse_session_id(&SessionId::new(params.session_id.clone()))
            .map_err(|_| agent_client_protocol::Error::invalid_params())?;

        // Validate path security using PathValidator
        // This checks: absolute path, no traversal, no symlinks, blocked paths
        let validated_path = self
            .path_validator
            .validate_absolute_path(&params.path)
            .map_err(|e| {
                tracing::warn!(
                    security_event = "path_validation_failed",
                    session_id = %params.session_id,
                    path = %params.path,
                    error = %e,
                    "Path validation failed for read operation"
                );
                // Use generic error message to avoid leaking security policy details
                agent_client_protocol::Error::new(-32602, "Invalid file path".to_string())
            })?;

        // Validate line and limit parameters
        if let Some(line) = params.line {
            if line == 0 {
                return Err(agent_client_protocol::Error::invalid_params());
            }
        }

        let path = validated_path.as_path();

        // ACP requires integration with client editor state for unsaved changes
        // Try to get content from editor buffer first
        match self
            .editor_state_manager
            .get_file_content(&params.session_id, path)
            .await
        {
            Ok(Some(editor_buffer)) => {
                tracing::debug!(
                    "Using editor buffer content for: {} (modified: {})",
                    params.path,
                    editor_buffer.modified
                );
                // Editor buffer content needs line filtering applied
                let filtered_content =
                    self.apply_line_filtering(&editor_buffer.content, params.line, params.limit)?;
                Ok(ReadTextFileResponse {
                    content: filtered_content,
                })
            }
            Ok(None) => {
                // No editor buffer available, read from disk (with line filtering)
                tracing::trace!("Reading from disk (no editor buffer): {}", params.path);
                let content = self
                    .read_file_with_options(&params.path, params.line, params.limit)
                    .await?;
                Ok(ReadTextFileResponse { content })
            }
            Err(e) => {
                tracing::warn!(
                    "Editor state query failed for {}: {}, falling back to disk",
                    params.path,
                    e
                );
                let content = self
                    .read_file_with_options(&params.path, params.line, params.limit)
                    .await?;
                Ok(ReadTextFileResponse { content })
            }
        }
    }

    /// Handle fs/write_text_file ACP extension method
    pub async fn handle_write_text_file(
        &self,
        params: WriteTextFileParams,
    ) -> Result<WriteTextFileResponse, agent_client_protocol::Error> {
        tracing::debug!("Processing fs/write_text_file request: {}", Pretty(&params));

        // Audit logging for file write attempt
        tracing::info!(
            security_event = "file_write_attempt",
            session_id = %params.session_id,
            path = %params.path,
            content_size = params.content.len(),
            "File write operation requested"
        );

        // Validate client capabilities for file system write operations
        {
            let client_caps = self.client_capabilities.read().await;
            match &*client_caps {
                Some(caps) if caps.fs.write_text_file => {
                    tracing::debug!("File system write capability validated");
                }
                Some(_) => {
                    tracing::error!("fs/write_text_file capability not declared by client");
                    return Err(agent_client_protocol::Error::new(-32602,
                        "File system write capability not declared by client. Set client_capabilities.fs.write_text_file = true during initialization."
                    ));
                }
                None => {
                    tracing::error!(
                        "No client capabilities available for fs/write_text_file validation"
                    );
                    return Err(agent_client_protocol::Error::new(-32602,
                        "Client capabilities not initialized. Cannot perform file system operations without capability declaration."
                    ));
                }
            }
        }

        // Validate session ID
        self.parse_session_id(&SessionId::new(params.session_id.clone()))
            .map_err(|_| agent_client_protocol::Error::invalid_params())?;

        // Validate path security using PathValidator with non-strict canonicalization
        // For write operations, the file may not exist yet, so we use non-strict mode
        // This still checks: absolute path, no traversal
        // Note: For production use, consider using the same blocked/allowed paths as the main validator
        let write_validator =
            crate::path_validator::PathValidator::new().with_strict_canonicalization(false);

        let validated_path = write_validator
            .validate_absolute_path(&params.path)
            .map_err(|e| {
                tracing::warn!(
                    security_event = "path_validation_failed",
                    session_id = %params.session_id,
                    path = %params.path,
                    error = %e,
                    "Path validation failed for write operation"
                );
                // Use generic error message to avoid leaking security policy details
                agent_client_protocol::Error::new(-32602, "Invalid file path".to_string())
            })?;

        // Validate content size before write to prevent disk exhaustion
        // Using > to reject content strictly larger than the limit (50MB limit is exclusive)
        let content_size = params.content.len();
        if content_size > sizes::content::MAX_RESOURCE_MODERATE {
            tracing::warn!(
                security_event = "content_size_exceeded",
                session_id = %params.session_id,
                path = %params.path,
                size = content_size,
                limit = sizes::content::MAX_RESOURCE_MODERATE,
                "Content size exceeds maximum allowed for write operation"
            );
            // Return error with size information for client debugging
            return Err(agent_client_protocol::Error::new(
                -32602,
                format!(
                    "Content size {} bytes exceeds maximum {} bytes (limit is exclusive)",
                    content_size,
                    sizes::content::MAX_RESOURCE_MODERATE
                ),
            )
            .data(serde_json::json!({
                "error": "content_too_large",
                "size": content_size,
                "max_size": sizes::content::MAX_RESOURCE_MODERATE
            })));
        }

        // Perform atomic write operation with validated path
        self.write_file_atomically(validated_path.to_str().unwrap(), &params.content)
            .await?;

        // Audit logging for successful write
        tracing::info!(
            security_event = "file_write_success",
            session_id = %params.session_id,
            path = %params.path,
            bytes = content_size,
            "File write completed successfully"
        );

        // Return WriteTextFileResponse as per ACP specification
        Ok(WriteTextFileResponse::default())
    }

    /// Read file content with optional line offset and limit
    ///
    /// # Security
    /// This function assumes the caller has already validated the path using PathValidator.
    /// Path validation must include: absolute path check, traversal prevention, and blocked path check.
    pub(crate) async fn read_file_with_options(
        &self,
        path: &str,
        start_line: Option<u32>,
        limit: Option<u32>,
    ) -> Result<String, agent_client_protocol::Error> {
        // Check file size before reading to prevent memory exhaustion
        let metadata = tokio::fs::metadata(path).await.map_err(|e| {
            tracing::error!(
                security_event = "file_metadata_failed",
                path = %path,
                error = %e,
                "Failed to get file metadata"
            );
            match e.kind() {
                std::io::ErrorKind::NotFound => agent_client_protocol::Error::invalid_params(),
                std::io::ErrorKind::PermissionDenied => {
                    agent_client_protocol::Error::invalid_params()
                }
                _ => agent_client_protocol::Error::internal_error(),
            }
        })?;

        let file_size = metadata.len() as usize;

        // Validate file size against configured limits
        // Using > to reject files strictly larger than the limit (50MB limit is exclusive)
        if file_size > sizes::content::MAX_RESOURCE_MODERATE {
            tracing::warn!(
                security_event = "file_size_exceeded",
                path = %path,
                size = file_size,
                limit = sizes::content::MAX_RESOURCE_MODERATE,
                "File size exceeds maximum allowed for read operation"
            );
            return Err(agent_client_protocol::Error::invalid_params());
        }

        // Read the entire file
        let file_content = tokio::fs::read_to_string(path).await.map_err(|e| {
            tracing::error!(
                security_event = "file_read_failed",
                path = %path,
                error = %e,
                "Failed to read file content"
            );
            match e.kind() {
                std::io::ErrorKind::NotFound => agent_client_protocol::Error::invalid_params(),
                std::io::ErrorKind::PermissionDenied => {
                    agent_client_protocol::Error::invalid_params()
                }
                _ => agent_client_protocol::Error::internal_error(),
            }
        })?;

        // Audit logging for successful read
        tracing::info!(
            security_event = "file_read_success",
            path = %path,
            bytes = file_content.len(),
            "File read completed successfully"
        );

        // Apply line filtering if specified
        self.apply_line_filtering(&file_content, start_line, limit)
    }

    /// Apply line offset and limit filtering to file content
    pub(crate) fn apply_line_filtering(
        &self,
        content: &str,
        start_line: Option<u32>,
        limit: Option<u32>,
    ) -> Result<String, agent_client_protocol::Error> {
        let lines: Vec<&str> = content.lines().collect();

        let start_index = match start_line {
            Some(line) => {
                if line == 0 {
                    return Err(agent_client_protocol::Error::invalid_params());
                }
                (line - 1) as usize // Convert to 0-based index
            }
            None => 0,
        };

        // If start index is beyond the end of the file, return empty string
        if start_index >= lines.len() {
            tracing::debug!(
                security_event = "line_out_of_bounds",
                start_line = start_index,
                total_lines = lines.len(),
                "Line offset beyond file end"
            );
            return Ok(String::new());
        }

        let end_index = match limit {
            Some(limit_count) => {
                if limit_count == 0 {
                    return Ok(String::new());
                }
                // Use checked_add to prevent integer overflow
                start_index
                    .checked_add(limit_count as usize)
                    .ok_or_else(agent_client_protocol::Error::invalid_params)?
                    .min(lines.len())
            }
            None => lines.len(),
        };

        let selected_lines = &lines[start_index..end_index];
        Ok(selected_lines.join("\n"))
    }

    /// Write file content atomically with parent directory creation
    pub(crate) async fn write_file_atomically(
        &self,
        path: &str,
        content: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        use std::path::Path;
        use ulid::Ulid;

        let path_buf = Path::new(path);

        // Create parent directories if they don't exist
        if let Some(parent_dir) = path_buf.parent() {
            if !parent_dir.exists() {
                tokio::fs::create_dir_all(parent_dir).await.map_err(|e| {
                    tracing::error!(
                        security_event = "directory_creation_failed",
                        path = %parent_dir.display(),
                        error = %e,
                        "Failed to create parent directory"
                    );
                    agent_client_protocol::Error::internal_error()
                })?;
            }
        }

        // Create temporary file in same directory for atomic write
        // Using ULID ensures uniqueness and prevents predictable temp file names
        let temp_path = if let Some(parent) = path_buf.parent() {
            let file_name = path_buf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            parent.join(format!(".tmp.{}.{}", file_name, Ulid::new()))
        } else {
            std::path::PathBuf::from(format!("{}.tmp.{}", path, Ulid::new()))
        };

        // Ensure temp path is absolute before proceeding
        if !temp_path.is_absolute() {
            tracing::error!(
                security_event = "temp_path_not_absolute",
                path = %temp_path.display(),
                "Temporary file path must be absolute"
            );
            return Err(agent_client_protocol::Error::internal_error());
        }

        // Validate temp file path to prevent symlink manipulation in parent directories
        // This ensures the temp file doesn't escape security boundaries
        let temp_path = if let Some(parent) = temp_path.parent() {
            // Canonicalize the parent directory to resolve symlinks
            match parent.canonicalize() {
                Ok(canonical_parent) => {
                    let resolved = canonical_parent.join(temp_path.file_name().unwrap());
                    // Verify the resolved path is still absolute
                    if !resolved.is_absolute() {
                        tracing::error!(
                            security_event = "temp_path_resolution_failed",
                            resolved_path = %resolved.display(),
                            "Resolved temp path is not absolute"
                        );
                        return Err(agent_client_protocol::Error::internal_error());
                    }

                    // Additional validation: ensure the resolved temp path is within allowed boundaries
                    // Validate the resolved path to ensure it hasn't escaped security boundaries via symlinks
                    // Use non-strict canonicalization since the temp file doesn't exist yet
                    let temp_validator = crate::path_validator::PathValidator::new()
                        .with_strict_canonicalization(false);
                    if let Err(e) = temp_validator.validate_absolute_path(
                        resolved.to_str().ok_or_else(|| {
                            tracing::error!(
                                security_event = "temp_path_utf8_invalid",
                                resolved_path = %resolved.display(),
                                "Resolved temp path contains invalid UTF-8"
                            );
                            agent_client_protocol::Error::internal_error()
                        })?,
                    ) {
                        tracing::error!(
                            security_event = "temp_path_security_validation_failed",
                            resolved_path = %resolved.display(),
                            error = %e,
                            "Resolved temp path failed security validation - possible symlink attack"
                        );
                        return Err(agent_client_protocol::Error::internal_error());
                    }

                    resolved
                }
                Err(e) => {
                    tracing::error!(
                        security_event = "parent_canonicalization_failed",
                        parent = %parent.display(),
                        error = %e,
                        "Failed to canonicalize parent directory for temp file"
                    );
                    return Err(agent_client_protocol::Error::internal_error());
                }
            }
        } else {
            temp_path
        };

        let temp_path_str = temp_path.to_string_lossy();

        // Write content to temporary file
        match tokio::fs::write(&temp_path, content).await {
            Ok(_) => {
                // Set restrictive permissions on Unix systems (owner read/write only)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(e) = tokio::fs::set_permissions(
                        &temp_path,
                        std::fs::Permissions::from_mode(0o600),
                    )
                    .await
                    {
                        tracing::warn!(
                            security_event = "permission_set_failed",
                            path = %temp_path_str,
                            error = %e,
                            "Failed to set restrictive permissions on temp file"
                        );
                        // Continue despite permission setting failure
                    }
                }

                // Atomically rename temporary file to final path
                match tokio::fs::rename(&temp_path, path).await {
                    Ok(_) => {
                        tracing::debug!(
                            security_event = "atomic_write_success",
                            path = %path,
                            "Successfully completed atomic write"
                        );
                        Ok(())
                    }
                    Err(e) => {
                        // Clean up temp file on failure with explicit error handling
                        if let Err(cleanup_err) = tokio::fs::remove_file(&temp_path).await {
                            tracing::error!(
                                security_event = "temp_file_cleanup_failed",
                                temp_path = %temp_path_str,
                                cleanup_error = %cleanup_err,
                                "Failed to clean up temporary file after write failure - manual cleanup may be required"
                            );
                        }
                        tracing::error!(
                            security_event = "atomic_rename_failed",
                            path = %path,
                            temp_path = %temp_path_str,
                            error = %e,
                            "Failed to rename temp file"
                        );
                        Err(agent_client_protocol::Error::internal_error())
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    security_event = "temp_write_failed",
                    path = %temp_path_str,
                    error = %e,
                    "Failed to write temp file"
                );
                match e.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        Err(agent_client_protocol::Error::invalid_params())
                    }
                    _ => Err(agent_client_protocol::Error::internal_error()),
                }
            }
        }
    }
}
