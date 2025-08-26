use swissarmyhammer::prompts::PromptLoader;

use swissarmyhammer::test_utils::IsolatedTestHome;

#[test]
fn test_comprehensive_prompt_library() {
    let _guard = IsolatedTestHome::new();
    
    let mut loader = PromptLoader::new();
    let result = loader.load_all();
    assert!(result.is_ok());
    
    // Get all loaded prompts count
    let prompt_count = loader.storage.len();
    
    // Print all loaded prompts for debugging
    let mut prompt_list: Vec<_> = loader.storage.iter()
        .map(|(name, _)| name)
        .collect();
    prompt_list.sort();
    
    println!("\nLoaded prompts:");
    for name in &prompt_list {
        println!("  - {}", name);
    }
    
    // We should have at least 15 prompts as per requirements
    assert!(prompt_count >= 15, "Expected at least 15 prompts, found {}", prompt_count);
    
    // List of expected prompts (using names from YAML front matter)
    let expected_prompts = vec![
        // Root level
        "example",
        "help",
        "plan",
        
        // Debug category
        "debug-error",
        "debug-performance",
        "debug-logs",
        
        // Refactor category
        "refactor-patterns",
        "refactor-clean",
        "refactor-extract",
        
        // Review category
        "code-review",
        "review-security",
        "review-accessibility",
        
        // Docs category
        "docs-readme",
        "docs-api",
        "docs-comments",
        
        // Test category
        "test-unit",
        "test-integration", 
        "test-property",
        
        // Prompts category
        "prompts-create",
        "prompts-improve",
        
        // New Liquid example prompts
        "data/array-processor",
        "formatting/table-generator", 
        "communication/email-composer",
        "analysis/statistics-calculator",
        "review/code-dynamic",
        "productivity/task-formatter",
    ];
    
    // Check each expected prompt exists
    for expected in &expected_prompts {
        let found = loader.storage.iter().any(|(name, _)| {
            &name == expected || name.ends_with(expected)
        });
        assert!(found, "Expected prompt '{}' not found", expected);
    }
    
    // Verify all prompts have required metadata
    for (name, prompt) in loader.storage.iter() {
        // Check required fields
        assert!(!prompt.name.is_empty(), "Prompt '{}' has empty name", name);
        assert!(!prompt.content.is_empty(), "Prompt '{}' has empty content", name);
        assert!(!prompt.source_path.is_empty(), "Prompt '{}' has empty source_path", name);
        
        // Verify title exists for all prompts
        assert!(prompt.title.is_some(), "Prompt '{}' missing title", name);
        
        // Verify description exists for all prompts  
        assert!(prompt.description.is_some(), "Prompt '{}' missing description", name);
    }
    
    println!("Successfully loaded {} prompts", prompt_count);
    
    // Print all loaded prompts for verification
    let mut prompt_list: Vec<_> = loader.storage.iter()
        .map(|(name, _)| name)
        .collect();
    prompt_list.sort();
    
    println!("\nLoaded prompts:");
    for name in prompt_list {
        println!("  - {}", name);
    }
}