use crate::error::ModelError;
use crate::retry::download_with_retry;
use crate::types::RetryConfig;
use std::path::PathBuf;
use tracing::info;

/// Downloads all parts of a multi-part model
pub async fn download_multi_part_model(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    parts: &[String],
    repo: &str,
    retry_config: &RetryConfig,
) -> Result<PathBuf, ModelError> {
    info!(
        "Starting download of {} parts for multi-part model",
        parts.len()
    );

    let mut downloaded_paths = Vec::new();

    // Download each part
    for (index, part) in parts.iter().enumerate() {
        info!(
            "Downloading part {} of {}: {}",
            index + 1,
            parts.len(),
            part
        );

        let path = download_with_retry(repo_api, part, repo, retry_config).await?;

        downloaded_paths.push(path);
    }

    info!("Successfully downloaded all {} parts", parts.len());

    // Return the path to the first part (which llama.cpp uses to load multi-part files)
    Ok(downloaded_paths[0].clone())
}

/// Downloads all files from a folder (for folder-based chunked models)
pub async fn download_folder_model(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    files: &[String],
    repo: &str,
    retry_config: &RetryConfig,
) -> Result<PathBuf, ModelError> {
    info!(
        "Starting download of {} files from folder-based model",
        files.len()
    );

    let mut downloaded_paths = Vec::new();

    // Download each file
    for (index, file) in files.iter().enumerate() {
        info!(
            "Downloading file {} of {}: {}",
            index + 1,
            files.len(),
            file
        );

        let path = download_with_retry(repo_api, file, repo, retry_config).await?;

        downloaded_paths.push(path);
    }

    info!(
        "Successfully downloaded all {} files from folder",
        files.len()
    );

    // Return the path to the first file (llama.cpp loads from first part and finds others)
    Ok(downloaded_paths[0].clone())
}

/// Detects if a filename is part of a multi-part GGUF file and returns the base filename (first part)
pub fn detect_multi_part_base(filename: &str) -> Option<String> {
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
