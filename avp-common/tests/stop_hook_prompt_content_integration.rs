//! Integration test: Stop-hook validator prompts include the changed-files
//! list and the per-file diff content.
//!
//! Regression coverage for the bug where:
//!
//! 1. `prepare_validator_context` short-circuited for Stop hooks (no
//!    `tool_name`), dropping caller-supplied diffs on the floor.
//! 2. `changed_files` never reached the rule prompt, so validators saw no
//!    explicit list of files to focus on.
//!
//! With both defects fixed, a Stop-hook run with two changed files and a
//! diff per file produces a rule prompt that contains:
//!
//! - A `## Files Changed This Turn` section listing both paths.
//! - The unified-diff content for each file in a fenced ```diff block.
//!
//! Strategy: wrap a [`PlaybackAgent`] in a recording proxy that snapshots
//! every `PromptRequest` text payload as it passes through. After running
//! `AvpContext::execute_rulesets`, assert on the captured prompt text.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::{
    AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SessionNotification, SetSessionModeRequest, SetSessionModeResponse,
};
use agent_client_protocol::Agent;
use agent_client_protocol_extras::PlaybackAgent;
use avp_common::context::AvpContext;
use avp_common::turn::FileDiff;
use avp_common::types::HookType;
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use tempfile::TempDir;

/// Path to the playback fixture used by this test.
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("recordings")
        .join("rule_clean_pass.json")
}

/// Agent proxy that records every `PromptRequest` passed to `prompt()`.
///
/// Forwards all calls to the wrapped `PlaybackAgent` and exposes the captured
/// prompt text payloads through [`captured_prompts`](Self::captured_prompts).
struct PromptCapturingAgent {
    inner: PlaybackAgent,
    /// Concatenated text from every `PromptRequest` ever sent.
    captured: Arc<Mutex<Vec<String>>>,
}

impl PromptCapturingAgent {
    fn new(inner: PlaybackAgent) -> (Self, Arc<Mutex<Vec<String>>>) {
        let captured = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                inner,
                captured: Arc::clone(&captured),
            },
            captured,
        )
    }

    fn subscribe_notifications(&self) -> tokio::sync::broadcast::Receiver<SessionNotification> {
        self.inner.subscribe_notifications()
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for PromptCapturingAgent {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        self.inner.initialize(request).await
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        self.inner.authenticate(request).await
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        self.inner.new_session(request).await
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> agent_client_protocol::Result<PromptResponse> {
        // Snapshot every text block in the prompt before delegating.
        let text: String = request
            .prompt
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.captured.lock().unwrap().push(text);

        self.inner.prompt(request).await
    }

    async fn cancel(&self, request: CancelNotification) -> agent_client_protocol::Result<()> {
        self.inner.cancel(request).await
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        self.inner.load_session(request).await
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        self.inner.set_session_mode(request).await
    }

    async fn ext_method(&self, request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        self.inner.ext_method(request).await
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> agent_client_protocol::Result<()> {
        self.inner.ext_notification(notification).await
    }
}

/// Construct a Stop-hook context, run a single-rule RuleSet through the
/// production runner, and assert the captured rule prompt contains both
/// the changed-files list AND the diff content for each file.
#[tokio::test]
#[serial_test::serial(cwd, env)]
async fn stop_hook_rule_prompt_contains_changed_files_and_diffs() {
    // Set up a fresh "git repo" temp dir so AvpContext::with_agent succeeds.
    let temp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(temp.path().join(".git")).expect("create .git");

    let original_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(temp.path()).expect("chdir to temp");

    // Wrap the playback fixture in a capturing proxy.
    let playback = PlaybackAgent::new(fixture_path(), "claude");
    let (capturing_agent, captured) = PromptCapturingAgent::new(playback);
    let notifications = capturing_agent.subscribe_notifications();
    let agent_arc: Arc<dyn Agent + Send + Sync> = Arc::new(capturing_agent);

    let ctx = AvpContext::with_agent(agent_arc, notifications).expect("with_agent");

    // Lay down a single-rule RuleSet under <temp>/.avp/validators/.
    let avp_validators = temp
        .path()
        .join(".avp")
        .join("validators")
        .join("stop-prompt-test");
    let rules_dir = avp_validators.join("rules");
    std::fs::create_dir_all(&rules_dir).expect("create rules dir");
    std::fs::write(
        avp_validators.join("VALIDATOR.md"),
        "---\nname: stop-prompt-test\ndescription: Stop-hook prompt content test\nversion: 1.0.0\ntrigger: Stop\nseverity: error\n---\n\n# stop-prompt-test\n\nProbe ruleset.\n",
    )
    .expect("write manifest");
    std::fs::write(
        rules_dir.join("probe.md"),
        "---\nname: probe\ndescription: Probe rule\n---\n\n# Probe\n\nReturn passed.\n",
    )
    .expect("write rule");

    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            ValidatorSource::Project,
        )
        .expect("load rulesets");
    let rulesets = loader.list_rulesets();
    assert_eq!(rulesets.len(), 1, "expected exactly one ruleset on disk");

    // Stop-hook input: no tool_name, just the turn-end metadata.
    let input = serde_json::json!({
        "session_id": "stop-prompt-session",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": true,
    });

    // Two changed files with two diffs — the runner is responsible for
    // embedding both into the rule prompt.
    let changed_files = vec!["src/alpha.rs".to_string(), "src/beta.rs".to_string()];
    let diffs = vec![
        FileDiff {
            path: PathBuf::from("src/alpha.rs"),
            diff_text: "--- src/alpha.rs\n+++ src/alpha.rs\n@@ -1 +1 @@\n-old_alpha\n+new_alpha\n"
                .to_string(),
            is_new_file: false,
            is_binary: false,
        },
        FileDiff {
            path: PathBuf::from("src/beta.rs"),
            diff_text: "--- src/beta.rs\n+++ src/beta.rs\n@@ -1 +1 @@\n-old_beta\n+new_beta\n"
                .to_string(),
            is_new_file: false,
            is_binary: false,
        },
    ];

    // Avoid CLAUDE_ACP short-circuiting the runner.
    let saved_claude_acp = std::env::var("CLAUDE_ACP").ok();
    std::env::remove_var("CLAUDE_ACP");

    let _executed = ctx
        .execute_rulesets(
            &rulesets,
            HookType::Stop,
            &input,
            Some(&changed_files),
            Some(&diffs),
        )
        .await;

    if let Some(val) = saved_claude_acp {
        std::env::set_var("CLAUDE_ACP", val);
    }
    std::env::set_current_dir(&original_cwd).expect("restore cwd");

    // The capturing proxy should have seen exactly one prompt (one rule).
    let prompts = captured.lock().unwrap().clone();
    assert!(
        !prompts.is_empty(),
        "expected at least one prompt to reach the agent, got none"
    );
    let prompt_text = &prompts[0];

    // 1. The `## Files Changed This Turn` section must appear with both paths.
    assert!(
        prompt_text.contains("## Files Changed This Turn"),
        "prompt should contain the Files Changed section, got:\n{}",
        prompt_text
    );
    assert!(
        prompt_text.contains("src/alpha.rs"),
        "prompt should list src/alpha.rs in the changed-files section, got:\n{}",
        prompt_text
    );
    assert!(
        prompt_text.contains("src/beta.rs"),
        "prompt should list src/beta.rs in the changed-files section, got:\n{}",
        prompt_text
    );

    // 2. The diff content for each file must appear in a fenced ```diff block.
    assert!(
        prompt_text.contains("```diff"),
        "prompt should contain a ```diff fence, got:\n{}",
        prompt_text
    );
    assert!(
        prompt_text.contains("-old_alpha"),
        "prompt should contain alpha.rs removed line, got:\n{}",
        prompt_text
    );
    assert!(
        prompt_text.contains("+new_alpha"),
        "prompt should contain alpha.rs added line, got:\n{}",
        prompt_text
    );
    assert!(
        prompt_text.contains("-old_beta"),
        "prompt should contain beta.rs removed line, got:\n{}",
        prompt_text
    );
    assert!(
        prompt_text.contains("+new_beta"),
        "prompt should contain beta.rs added line, got:\n{}",
        prompt_text
    );
}
