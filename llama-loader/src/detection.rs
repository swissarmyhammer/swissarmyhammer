use crate::error::ModelError;
use crate::multipart::detect_multi_part_base;
use tracing::info;

/// Auto-detects the best model file from a HuggingFace repository, with support for folder-based models
pub async fn auto_detect_hf_model_file(
    repo_api: &hf_hub::api::tokio::ApiRepo,
) -> Result<String, ModelError> {
    auto_detect_hf_model_file_with_folder(repo_api, None).await
}

/// Auto-detects the best model file from a HuggingFace repository, optionally within a specific folder
pub async fn auto_detect_hf_model_file_with_folder(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    folder: Option<&str>,
) -> Result<String, ModelError> {
    // List files in the repository
    match repo_api.info().await {
        Ok(repo_info) => {
            let mut gguf_files = Vec::new();
            let mut bf16_files = Vec::new();

            // Look for GGUF files in the repository (optionally within a folder)
            for sibling in repo_info.siblings {
                if sibling.rfilename.ends_with(".gguf") {
                    // If folder is specified, only consider files in that folder
                    if let Some(folder_name) = folder {
                        if !sibling.rfilename.starts_with(&format!("{}/", folder_name)) {
                            continue;
                        }
                    }

                    let filename = sibling.rfilename.to_lowercase();
                    if filename.contains("bf16") {
                        bf16_files.push(sibling.rfilename);
                    } else {
                        gguf_files.push(sibling.rfilename);
                    }
                }
            }

            // Prioritize BF16 files - check for multi-part files first
            if !bf16_files.is_empty() {
                // Sort to ensure consistent ordering
                bf16_files.sort();

                // Check if this is a multi-part file
                if let Some(base_filename) = detect_multi_part_base(&bf16_files[0]) {
                    info!("Found multi-part BF16 model file: {}", base_filename);
                    return Ok(base_filename);
                } else {
                    info!("Found BF16 model file: {}", bf16_files[0]);
                    return Ok(bf16_files[0].clone());
                }
            }

            // Fallback to first GGUF file
            if !gguf_files.is_empty() {
                gguf_files.sort();
                if let Some(base_filename) = detect_multi_part_base(&gguf_files[0]) {
                    info!("Found multi-part GGUF model file: {}", base_filename);
                    return Ok(base_filename);
                } else {
                    info!("Found GGUF model file: {}", gguf_files[0]);
                    return Ok(gguf_files[0].clone());
                }
            }

            Err(ModelError::NotFound(format!(
                "No .gguf model files found in HuggingFace repository{}",
                if folder.is_some() {
                    format!(" in folder {}", folder.unwrap())
                } else {
                    String::new()
                }
            )))
        }
        Err(e) => Err(ModelError::LoadingFailed(format!(
            "Failed to get repository info: {}",
            e
        ))),
    }
}

/// Gets all GGUF files from a specific folder in a HuggingFace repository
pub async fn get_folder_files(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    folder: &str,
) -> Result<Vec<String>, ModelError> {
    match repo_api.info().await {
        Ok(repo_info) => {
            let mut folder_files = Vec::new();
            let folder_prefix = format!("{}/", folder);

            // Collect all GGUF files in the specified folder
            for sibling in repo_info.siblings {
                if sibling.rfilename.starts_with(&folder_prefix)
                    && sibling.rfilename.ends_with(".gguf")
                {
                    folder_files.push(sibling.rfilename);
                }
            }

            if folder_files.is_empty() {
                Err(ModelError::NotFound(format!(
                    "No .gguf files found in folder '{}' in HuggingFace repository",
                    folder
                )))
            } else {
                // Sort files to ensure consistent ordering
                folder_files.sort();
                info!(
                    "Found {} GGUF files in folder '{}'",
                    folder_files.len(),
                    folder
                );
                Ok(folder_files)
            }
        }
        Err(e) => Err(ModelError::LoadingFailed(format!(
            "Failed to get repository info: {}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {

    // Note: These would be integration tests that require actual HuggingFace API access
    // For unit testing, we'd need to mock the ApiRepo and repo_info structures

    #[test]
    fn test_module_exists() {
        // Basic test to ensure the module compiles correctly
        // If this test runs, the module definition is valid
    }
}
