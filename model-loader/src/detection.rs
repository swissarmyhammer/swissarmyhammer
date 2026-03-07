use crate::error::ModelError;
use crate::multipart::detect_multi_part_base;
use crate::types::MODEL_EXTENSIONS;
use tracing::info;

/// Check if a filename has a supported model extension
fn has_model_extension(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    MODEL_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{}", ext)))
}

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
    match repo_api.info().await {
        Ok(repo_info) => {
            let mut model_files = Vec::new();
            let mut bf16_files = Vec::new();

            for sibling in repo_info.siblings {
                if !has_model_extension(&sibling.rfilename) {
                    continue;
                }

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
                    model_files.push(sibling.rfilename);
                }
            }

            // Prioritize BF16 files - check for multi-part files first
            if !bf16_files.is_empty() {
                bf16_files.sort();

                if let Some(base_filename) = detect_multi_part_base(&bf16_files[0]) {
                    info!("Found multi-part BF16 model file: {}", base_filename);
                    return Ok(base_filename);
                } else {
                    info!("Found BF16 model file: {}", bf16_files[0]);
                    return Ok(bf16_files[0].clone());
                }
            }

            // Fallback to first model file
            if !model_files.is_empty() {
                model_files.sort();
                if let Some(base_filename) = detect_multi_part_base(&model_files[0]) {
                    info!("Found multi-part model file: {}", base_filename);
                    return Ok(base_filename);
                } else {
                    info!("Found model file: {}", model_files[0]);
                    return Ok(model_files[0].clone());
                }
            }

            Err(ModelError::NotFound(format!(
                "No model files found in HuggingFace repository{}",
                if let Some(f) = folder {
                    format!(" in folder {}", f)
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

/// Gets all files from a specific folder in a HuggingFace repository.
///
/// Downloads ALL files in the folder, not just model files. This is important
/// for directory-based formats like `.mlpackage` that contain metadata files
/// (e.g. `Manifest.json`) alongside model weights.
pub async fn get_folder_files(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    folder: &str,
) -> Result<Vec<String>, ModelError> {
    match repo_api.info().await {
        Ok(repo_info) => {
            let mut folder_files = Vec::new();
            let folder_prefix = format!("{}/", folder);

            for sibling in repo_info.siblings {
                if sibling.rfilename.starts_with(&folder_prefix) {
                    folder_files.push(sibling.rfilename);
                }
            }

            if folder_files.is_empty() {
                Err(ModelError::NotFound(format!(
                    "No files found in folder '{}' in HuggingFace repository",
                    folder
                )))
            } else {
                folder_files.sort();
                info!("Found {} files in folder '{}'", folder_files.len(), folder);
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
    use super::*;

    #[test]
    fn test_has_model_extension_gguf() {
        assert!(has_model_extension("model.gguf"));
        assert!(has_model_extension("model.GGUF"));
        assert!(has_model_extension("path/to/model.gguf"));
    }

    #[test]
    fn test_has_model_extension_onnx() {
        assert!(has_model_extension("model.onnx"));
        assert!(has_model_extension("path/to/model.onnx"));
    }

    #[test]
    fn test_has_model_extension_safetensors() {
        assert!(has_model_extension("model.safetensors"));
    }

    #[test]
    fn test_has_model_extension_mlmodel() {
        assert!(has_model_extension("model.mlmodel"));
    }

    #[test]
    fn test_has_model_extension_bin() {
        assert!(has_model_extension("model.bin"));
    }

    #[test]
    fn test_has_model_extension_rejects_unsupported() {
        assert!(!has_model_extension("model.txt"));
        assert!(!has_model_extension("model.json"));
        assert!(!has_model_extension("README.md"));
        assert!(!has_model_extension("tokenizer.json"));
    }
}
