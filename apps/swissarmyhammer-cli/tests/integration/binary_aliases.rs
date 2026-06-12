use crate::in_process_test_utils::run_sah_command_in_process;

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
    for cmd in ["serve", "doctor", "completion", "validate"] {
        assert!(output.contains(cmd), "Help should mention {cmd} command");
    }
}
