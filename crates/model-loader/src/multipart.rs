use crate::error::ModelError;
use crate::observer::DownloadObserver;
use crate::retry::download_with_retry;
use crate::types::RetryConfig;
use std::path::PathBuf;
use tracing::info;

/// Downloads every item in `items`, returning the path of the first.
///
/// Shared download-and-collect loop behind [`download_multi_part_model`] and
/// [`download_folder_model`]; `item_label` and `collection_label` only vary
/// the log messages. The first item's path is returned because llama.cpp
/// loads multi-file models from the first file and locates the rest beside
/// it.
async fn download_all(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    items: &[String],
    item_label: &str,
    collection_label: &str,
    repo: &str,
    retry_config: &RetryConfig,
    observer: Option<&DownloadObserver>,
) -> Result<PathBuf, ModelError> {
    info!(
        "Starting download of {} {}s for {}",
        items.len(),
        item_label,
        collection_label
    );

    let mut downloaded_paths = Vec::new();
    for (index, item) in items.iter().enumerate() {
        info!(
            "Downloading {} {} of {}: {}",
            item_label,
            index + 1,
            items.len(),
            item
        );

        let path = download_with_retry(repo_api, item, repo, retry_config, observer).await?;
        downloaded_paths.push(path);
    }

    info!(
        "Successfully downloaded all {} {}s",
        items.len(),
        item_label
    );

    Ok(downloaded_paths[0].clone())
}

/// Downloads all parts of a multi-part model
///
/// # Arguments
///
/// * `repo_api` - hf-hub repo handle to download through
/// * `parts` - filenames of every part, in order
/// * `repo` - repository identifier (e.g. `org/repo`)
/// * `retry_config` - retry/backoff behavior
/// * `observer` - optional progress callback, invoked for each part; `None`
///   is byte-identical to the pre-observer behavior
///
/// # Errors
///
/// Propagates the first [`download_with_retry`] failure — typically
/// [`ModelError::LoadingFailed`] once retries for a part are exhausted
/// (wrapping the underlying not-found or network cause).
pub async fn download_multi_part_model(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    parts: &[String],
    repo: &str,
    retry_config: &RetryConfig,
    observer: Option<&DownloadObserver>,
) -> Result<PathBuf, ModelError> {
    download_all(
        repo_api,
        parts,
        "part",
        "multi-part model",
        repo,
        retry_config,
        observer,
    )
    .await
}

/// Downloads all files from a folder (for folder-based chunked models)
///
/// # Arguments
///
/// * `repo_api` - hf-hub repo handle to download through
/// * `files` - repository-relative paths of every file in the folder
/// * `repo` - repository identifier (e.g. `org/repo`)
/// * `retry_config` - retry/backoff behavior
/// * `observer` - optional progress callback, invoked for each file; `None`
///   is byte-identical to the pre-observer behavior
///
/// # Errors
///
/// Propagates the first [`download_with_retry`] failure — typically
/// [`ModelError::LoadingFailed`] once retries for a file are exhausted
/// (wrapping the underlying not-found or network cause).
pub async fn download_folder_model(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    files: &[String],
    repo: &str,
    retry_config: &RetryConfig,
    observer: Option<&DownloadObserver>,
) -> Result<PathBuf, ModelError> {
    download_all(
        repo_api,
        files,
        "file",
        "folder-based model",
        repo,
        retry_config,
        observer,
    )
    .await
}

/// Detects if a filename is part of a multi-part GGUF file and returns the base filename (first part)
///
/// Crate-internal: used by auto-detection (`crate::detection`) to map any
/// part of a multi-part model back to its first part, which is what gets
/// downloaded.
pub(crate) fn detect_multi_part_base(filename: &str) -> Option<String> {
    // Check for pattern like "model-00001-of-00002.gguf"
    use regex::Regex;
    let re = Regex::new(r"^(.+)-(\d{5})-of-(\d{5})\.gguf$").ok()?;

    if let Some(captures) = re.captures(filename) {
        let base_name = captures.get(1)?.as_str();
        let current_part = captures.get(2)?.as_str();
        let total_parts = captures.get(3)?.as_str();

        info!(
            "Detected multi-part GGUF file: {} (part {} of {})",
            base_name, current_part, total_parts
        );

        // Return the first part filename pattern
        Some(format!("{}-00001-of-{}.gguf", base_name, total_parts))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_multi_part_base_valid() {
        let result = detect_multi_part_base("model-00002-of-00003.gguf").unwrap();
        assert_eq!(result, "model-00001-of-00003.gguf");
    }

    #[test]
    fn test_detect_multi_part_base_first_part() {
        let result = detect_multi_part_base("model-00001-of-00005.gguf").unwrap();
        assert_eq!(result, "model-00001-of-00005.gguf");
    }

    #[test]
    fn test_detect_multi_part_base_complex_name() {
        let result =
            detect_multi_part_base("my-complex-model-name-bf16-00003-of-00010.gguf").unwrap();
        assert_eq!(result, "my-complex-model-name-bf16-00001-of-00010.gguf");
    }

    #[test]
    fn test_detect_multi_part_base_single_file() {
        let result = detect_multi_part_base("model.gguf");
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_multi_part_base_wrong_pattern() {
        let result = detect_multi_part_base("model-part1-of-3.gguf");
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_multi_part_base_wrong_extension() {
        let result = detect_multi_part_base("model-00001-of-00003.bin");
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_multi_part_base_invalid_part_numbers() {
        let result = detect_multi_part_base("model-001-of-003.gguf");
        assert!(result.is_none());
    }
}
