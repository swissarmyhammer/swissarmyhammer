use hf_hub::api::tokio::ApiBuilder;
use llama_loader::{get_folder_files, ModelSource};

#[tokio::test]
async fn test_folder_download_detection() -> Result<(), Box<dyn std::error::Error>> {
    let repo = "unsloth/Qwen3-Coder-480B-A35B-Instruct-1M-GGUF";
    let folder = "UD-Q4_K_XL";

    // Create HuggingFace API client
    let api = ApiBuilder::new()
        .with_chunk_size(Some(100 * 1024 * 1024))
        .with_max_files(2)
        .build()?;

    let repo_api = api.model(repo.to_string());

    // Test folder file detection
    match get_folder_files(&repo_api, folder).await {
        Ok(files) => {
            println!(
                "Found {} files in folder '{}': {:?}",
                files.len(),
                folder,
                files
            );
            assert!(!files.is_empty(), "Should find files in the folder");

            // Verify all files are GGUF files in the correct folder
            for file in &files {
                assert!(
                    file.starts_with(&format!("{}/", folder)),
                    "File should be in folder: {}",
                    file
                );
                assert!(
                    file.ends_with(".gguf"),
                    "File should be a GGUF file: {}",
                    file
                );
            }
        }
        Err(e) => {
            println!("Error getting folder files: {}", e);
            // Don't fail the test if we can't access HuggingFace (network issues, etc.)
            return Ok(());
        }
    }

    Ok(())
}

#[test]
fn test_model_source_with_folder() {
    let source = ModelSource::HuggingFace {
        repo: "unsloth/Qwen3-Coder-480B-A35B-Instruct-1M-GGUF".to_string(),
        filename: None,
        folder: Some("UD-Q4_K_XL".to_string()),
    };

    match source {
        ModelSource::HuggingFace {
            repo,
            filename,
            folder,
        } => {
            assert_eq!(repo, "unsloth/Qwen3-Coder-480B-A35B-Instruct-1M-GGUF");
            assert_eq!(filename, None);
            assert_eq!(folder, Some("UD-Q4_K_XL".to_string()));
        }
        _ => panic!("Expected HuggingFace source"),
    }
}
