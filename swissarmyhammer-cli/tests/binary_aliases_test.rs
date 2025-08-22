mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process;

#[tokio::test]
async fn test_swissarmyhammer_binary_exists() {
    let result = run_sah_command_in_process(&["--version"]).await.unwrap();
    assert_eq!(result.exit_code, 0, "Version command should succeed");
    assert!(
        result.stdout.contains("swissarmyhammer"),
        "Version output should contain swissarmyhammer"
    );
}

#[tokio::test]
async fn test_swissarmyhammer_has_expected_commands() {
    let result = run_sah_command_in_process(&["--help"]).await.unwrap();
    assert_eq!(result.exit_code, 0, "Help command should succeed");

    let output = result.stdout;
    assert!(
        output.contains("serve"),
        "Help should mention serve command"
    );
    assert!(
        output.contains("doctor"),
        "Help should mention doctor command"
    );
    assert!(
        output.contains("prompt"),
        "Help should mention prompt command"
    );
    assert!(output.contains("flow"), "Help should mention flow command");
    assert!(
        output.contains("completion"),
        "Help should mention completion command"
    );
    assert!(
        output.contains("validate"),
        "Help should mention validate command"
    );
}
