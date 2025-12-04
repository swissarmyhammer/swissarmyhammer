//! Test for shell_execute hanging when running background processes

use crate::actions::*;
use crate::actions_tests::create_test_context;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn test_shell_background_process_doesnt_hang() {
    // Test that shell commands with & don't cause the executor to hang

    let background_cmd = r#"(while true; do echo "background $(date)"; sleep 1; done) > /tmp/bg_test.log 2>&1 & echo "Shell exiting now""#;

    let action = ShellAction::new(background_cmd.to_string());

    let mut context = create_test_context();

    // This should complete quickly (within 5 seconds)
    // If it hangs, the test will timeout
    let result = tokio::time::timeout(Duration::from_secs(5), action.execute(&mut context)).await;

    match result {
        Ok(exec_result) => {
            assert!(exec_result.is_ok(), "Shell execution should succeed");
            println!("Test passed - shell execution completed without hanging!");
            // Clean up background process
            let _ = std::process::Command::new("pkill")
                .args(["-f", "background.*date"])
                .output();
        }
        Err(_) => {
            // Timeout - this is the bug we're trying to fix
            panic!("Shell execution hung waiting for background process!");
        }
    }
}
