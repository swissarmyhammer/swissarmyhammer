use crate::detection::{auto_detect_hf_model_file_with_folder, get_folder_files};
use crate::error::ModelError;
use crate::multipart::{download_folder_model, download_multi_part_model};
use crate::retry::download_with_retry;
use crate::types::RetryConfig;
use hf_hub::api::tokio::ApiBuilder;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Download a single file from a HuggingFace repo without model extension validation.
///
/// Use this for companion files like `tokenizer.json` that live alongside model files.
/// Returns the cached file path.
pub async fn download_hf_file(
    repo: &str,
    filename: &str,
    retry_config: &RetryConfig,
) -> Result<PathBuf, ModelError> {
    let api = ApiBuilder::new()
        .build()
        .map_err(|e| ModelError::Network(format!("Failed to create HF API: {e}")))?;
    let repo_api = api.model(repo.to_string());
    download_with_retry(&repo_api, filename, repo, retry_config).await
}

/// Loads a model from HuggingFace and returns path info for caching
pub async fn load_huggingface_model_with_path(
    repo: &str,
    filename: Option<&str>,
    retry_config: &RetryConfig,
) -> Result<(PathBuf, String), ModelError> {
    load_huggingface_model_with_path_and_folder(repo, filename, None, retry_config).await
}

/// Loads a model from HuggingFace with folder support and returns path info for caching
pub async fn load_huggingface_model_with_path_and_folder(
    repo: &str,
    filename: Option<&str>,
    folder: Option<&str>,
    retry_config: &RetryConfig,
) -> Result<(PathBuf, String), ModelError> {
    info!("Loading HuggingFace model: {}", repo);

    // Create HuggingFace API client
    let api = match ApiBuilder::new()
        .with_chunk_size(Some(100 * 1024 * 1024))
        .with_max_files(2)
        .build()
    {
        Ok(api) => api,
        Err(e) => {
            return Err(ModelError::Network(format!(
                "Failed to create HuggingFace API client for {}: {}. Use ModelSource::Local to load from local path instead.",
                repo, e
            )));
        }
    };

    let repo_api = api.model(repo.to_string());

    // Handle folder-based downloads (download all files in folder)
    if let Some(folder_name) = folder {
        info!("Downloading all files from folder: {}", folder_name);
        let folder_files = get_folder_files(&repo_api, folder_name).await?;
        let model_path =
            download_folder_model(&repo_api, &folder_files, repo, retry_config).await?;
        info!("Folder-based model downloaded to: {}", model_path.display());

        // HF cache uses symlinks (snapshot → blobs). Some frameworks like CoreML
        // don't follow symlinks when compiling .mlpackage directories.
        // Replace symlinks with hardlinks to the same blobs (no extra storage).
        deref_folder_symlinks(&model_path, folder_name)?;

        // Extract just the filename from the first file (remove folder prefix)
        let filename = folder_files[0]
            .split('/')
            .next_back()
            .unwrap_or(&folder_files[0])
            .to_string();
        return Ok((model_path, filename));
    }

    // Determine which file to download
    let target_filename = if let Some(filename) = filename {
        filename.to_string()
    } else {
        // Auto-detect the model file by listing repository files
        match auto_detect_hf_model_file_with_folder(&repo_api, folder).await {
            Ok(detected_filename) => detected_filename,
            Err(e) => {
                warn!("Failed to auto-detect model file: {}", e);
                return Err(ModelError::NotFound(format!(
                    "Could not auto-detect model file in repository: {}. Please specify --filename{}",
                    repo,
                    if let Some(f) = folder { format!(" or check folder {}", f) } else { String::new() }
                )));
            }
        }
    };

    info!("Downloading model file: {}", target_filename);

    // Download the model file(s) with retry logic
    let model_path = if let Some(parts) = get_all_parts(&target_filename) {
        info!("Downloading multi-part model with {} parts", parts.len());
        download_multi_part_model(&repo_api, &parts, repo, retry_config).await?
    } else {
        download_with_retry(&repo_api, &target_filename, repo, retry_config).await?
    };

    info!("Model downloaded to: {}", model_path.display());

    Ok((model_path, target_filename))
}

/// Gets all parts of a multi-part GGUF file
pub fn get_all_parts(base_filename: &str) -> Option<Vec<String>> {
    use regex::Regex;
    let re = Regex::new(r"^(.+)-00001-of-(\d{5})\.gguf$").ok()?;

    if let Some(captures) = re.captures(base_filename) {
        let base_name = captures.get(1)?.as_str();
        let total_parts_str = captures.get(2)?.as_str();
        let total_parts: u32 = total_parts_str.parse().ok()?;

        let mut parts = Vec::new();
        for part_num in 1..=total_parts {
            parts.push(format!(
                "{}-{:05}-of-{}.gguf",
                base_name, part_num, total_parts_str
            ));
        }

        Some(parts)
    } else {
        None
    }
}

/// Replace symlinks with hardlinks inside a downloaded folder.
///
/// HF cache stores blobs separately and creates symlinks in the snapshot tree.
/// Some frameworks (notably CoreML's `compileModelAtURL`) don't follow symlinks
/// when copying to temp dirs. Hardlinks reference the same inode — no extra disk
/// space — but look like regular files to all tools.
fn deref_folder_symlinks(any_file_in_folder: &Path, folder_name: &str) -> Result<(), ModelError> {
    // Walk up from the downloaded file path to find the folder root
    let mut folder_root = any_file_in_folder.to_path_buf();
    loop {
        if folder_root
            .file_name()
            .map(|n| n.to_string_lossy() == folder_name)
            .unwrap_or(false)
        {
            break;
        }
        if !folder_root.pop() {
            // Couldn't find the folder root — skip silently
            return Ok(());
        }
    }

    deref_dir_recursive(&folder_root)
}

fn deref_dir_recursive(dir: &Path) -> Result<(), ModelError> {
    let entries = std::fs::read_dir(dir).map_err(|e| ModelError::LoadingFailed(format!("{e}")))?;

    for entry in entries {
        let entry = entry.map_err(|e| ModelError::LoadingFailed(format!("{e}")))?;
        let path = entry.path();
        let ft = entry
            .file_type()
            .map_err(|e| ModelError::LoadingFailed(format!("{e}")))?;

        if ft.is_symlink() {
            // Resolve the symlink target, remove the symlink, hardlink to the blob
            let target = std::fs::canonicalize(&path)
                .map_err(|e| ModelError::LoadingFailed(format!("symlink resolve: {e}")))?;
            std::fs::remove_file(&path)
                .map_err(|e| ModelError::LoadingFailed(format!("remove symlink: {e}")))?;
            std::fs::hard_link(&target, &path).map_err(|e| {
                ModelError::LoadingFailed(format!(
                    "hardlink {} → {}: {e}",
                    path.display(),
                    target.display()
                ))
            })?;
            info!(
                "Dereferenced symlink: {} → {}",
                path.display(),
                target.display()
            );
        } else if ft.is_dir() {
            deref_dir_recursive(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_parts_valid() {
        let parts = get_all_parts("model-00001-of-00003.gguf").unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "model-00001-of-00003.gguf");
        assert_eq!(parts[1], "model-00002-of-00003.gguf");
        assert_eq!(parts[2], "model-00003-of-00003.gguf");
    }

    #[test]
    fn test_get_all_parts_single_file() {
        let parts = get_all_parts("model.gguf");
        assert!(parts.is_none());
    }

    #[test]
    fn test_get_all_parts_invalid_format() {
        let parts = get_all_parts("model-part1-of-3.gguf");
        assert!(parts.is_none());
    }
}
