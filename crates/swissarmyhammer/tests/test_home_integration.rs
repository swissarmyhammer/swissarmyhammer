/// Integration test for test home directory setup
use swissarmyhammer::test_utils::IsolatedTestEnvironment;

#[test]
fn test_home_directory_override_works() {
    let original_home = std::env::var("HOME").ok();

    {
        let guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

        let home = std::env::var("HOME").expect("HOME not set");
        assert!(home.contains("tmp") || home.contains("temp")); // Should be in a temp directory

        let swissarmyhammer_dir = guard.swissarmyhammer_dir();
        assert!(swissarmyhammer_dir.exists());
        assert!(swissarmyhammer_dir.join("workflows").exists());

        // Create a test workflow to verify the structure works
        std::fs::write(
            swissarmyhammer_dir
                .join("workflows")
                .join("test-workflow.yaml"),
            "name: test\nsteps: []",
        )
        .expect("Failed to create test workflow");

        let test_workflow = swissarmyhammer_dir
            .join("workflows")
            .join("test-workflow.yaml");
        assert!(test_workflow.exists());
    }

    // Check that HOME is restored after guard is dropped
    let restored_home = std::env::var("HOME").ok();
    assert_eq!(original_home, restored_home);
}
