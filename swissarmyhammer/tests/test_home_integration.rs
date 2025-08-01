/// Integration test for test home directory setup
use swissarmyhammer::test_utils::{create_test_home_guard, get_test_swissarmyhammer_dir};

#[test]
fn test_home_directory_override_works() {
    let original_home = std::env::var("HOME").ok();

    {
        let _guard = create_test_home_guard();

        let home = std::env::var("HOME").expect("HOME not set");
        assert!(home.contains("test-home"));

        let swissarmyhammer_dir = get_test_swissarmyhammer_dir();
        assert!(swissarmyhammer_dir.exists());
        assert!(swissarmyhammer_dir.join("prompts").exists());
        assert!(swissarmyhammer_dir.join("workflows").exists());

        // Check that test prompts exist
        let test_prompt = swissarmyhammer_dir.join("prompts").join("test-prompt.md");
        assert!(test_prompt.exists());

        let another_test = swissarmyhammer_dir
            .join("prompts")
            .join("another-test.md.liquid");
        assert!(another_test.exists());

        // Check that test workflow exists
        let test_workflow = swissarmyhammer_dir
            .join("workflows")
            .join("test-workflow.yaml");
        assert!(test_workflow.exists());
    }

    // Check that HOME is restored after guard is dropped
    let restored_home = std::env::var("HOME").ok();
    assert_eq!(original_home, restored_home);
}

#[test]
fn test_prompt_loading_with_test_home() {
    use swissarmyhammer::prompts::PromptLoader;

    let _guard = create_test_home_guard();

    let loader = PromptLoader::new();
    let test_dir = get_test_swissarmyhammer_dir().join("prompts");
    let prompts = loader
        .load_directory(&test_dir)
        .expect("Failed to load prompts");

    // We should have loaded our test prompts
    assert_eq!(prompts.len(), 2);

    let prompt_names: Vec<String> = prompts.iter().map(|p| p.name.clone()).collect();
    assert!(prompt_names.contains(&"test-prompt".to_string()));
    assert!(prompt_names.contains(&"another-test".to_string()));
}

#[test]
fn test_prompt_resolver_with_test_home() {
    use swissarmyhammer::{PromptLibrary, PromptResolver};

    let _guard = create_test_home_guard();

    // Verify HOME is set correctly
    let home = std::env::var("HOME").expect("HOME not set");
    println!("HOME is set to: {home}");

    let test_prompts_dir = get_test_swissarmyhammer_dir().join("prompts");
    println!("Test prompts dir: {test_prompts_dir:?}");
    println!("Test prompts dir exists: {}", test_prompts_dir.exists());

    let mut resolver = PromptResolver::new();
    let mut library = PromptLibrary::new();

    // Load user prompts (which should now come from test home)
    resolver
        .load_all_prompts(&mut library)
        .expect("Failed to load user prompts");

    let prompts = library.list().expect("Failed to list prompts");
    let user_prompt_names: Vec<String> = prompts.iter().map(|p| p.name.clone()).collect();

    // Debug output to see what prompts were loaded
    println!("Loaded prompts: {user_prompt_names:?}");

    // Should have loaded our test prompts
    assert!(
        user_prompt_names.contains(&"test-prompt".to_string()),
        "Missing test-prompt"
    );
    assert!(
        user_prompt_names.contains(&"another-test".to_string()),
        "Missing another-test"
    );
}
