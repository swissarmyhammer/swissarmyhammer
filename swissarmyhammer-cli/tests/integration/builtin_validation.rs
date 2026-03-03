use anyhow::Result;
use std::fs;
use std::path::Path;
use swissarmyhammer::validation::ValidationResult;
use swissarmyhammer::PromptLoader;
use walkdir::WalkDir;

#[test]
fn test_builtin_prompts_validate_directly() -> Result<()> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let project_root = Path::new(&manifest_dir).parent().unwrap();

    let mut result = ValidationResult::new();
    let loader = PromptLoader::new();

    // Walk through builtin/prompts directory
    let prompts_dir = project_root.join("builtin/prompts");
    for entry in WalkDir::new(&prompts_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file()
            && (path.extension() == Some("md".as_ref())
                || path.extension() == Some("liquid".as_ref()))
        {
            result.files_checked += 1;

            // Read and validate the prompt
            if let Ok(content) = fs::read_to_string(path) {
                match loader.load_from_string(path.file_stem().unwrap().to_str().unwrap(), &content)
                {
                    Ok(prompt) => {
                        // Basic validation passed - prompt loaded successfully
                        println!("✓ Valid prompt: {}", prompt.name);
                    }
                    Err(e) => {
                        println!("✗ Invalid prompt at {path:?}: {e}");
                        result.errors += 1;
                    }
                }
            }
        }
    }

    println!("\nValidation Summary:");
    println!("Files checked: {}", result.files_checked);
    println!("Errors: {}", result.errors);

    assert_eq!(result.errors, 0, "All builtin prompts should be valid");
    assert!(
        result.files_checked > 0,
        "Should have validated at least some files"
    );

    Ok(())
}
