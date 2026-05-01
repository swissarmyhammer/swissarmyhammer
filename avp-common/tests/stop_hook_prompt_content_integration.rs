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
//! Strategy: wrap a [`PlaybackAgent`] in a `ConnectTo<Client>` middleware
//! that snapshots the text payload of every `session/prompt` request as it
//! flows through. After running `AvpContext::execute_rulesets`, assert on
//! the captured prompt text.
//!
//! In ACP 0.11 the `Agent` trait was removed in favour of a builder/handler
//! runtime, so the previous "wrap as `impl Agent`" approach is no longer
//! possible — the wrapper is now a duplex-channel middleware that observes
//! JSON-RPC messages on the wire, mirroring the shape of
//! [`agent_client_protocol_extras::RecordingAgent`].

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_client_protocol::jsonrpcmsg::{Message, Params};
use agent_client_protocol::schema::{ContentBlock, PromptRequest};
use agent_client_protocol::{Channel, Client, ConnectTo, Result as AcpResult};
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

/// `ConnectTo<Client>` middleware that records the text payload of every
/// `session/prompt` request as it flows through to the inner agent.
///
/// Mirrors the duplex-channel wiring used by
/// [`agent_client_protocol_extras::RecordingAgent`]: client and inner agent
/// are connected through an internal pipe, and every JSON-RPC message
/// observed in the client→agent direction is inspected for `session/prompt`
/// requests. The full JSON params are captured so the test can assert on
/// any field of the request, not just the text content blocks.
struct PromptCapturingAgent<A> {
    inner: A,
    /// Captured `PromptRequest` instances in arrival order.
    captured: Arc<Mutex<Vec<PromptRequest>>>,
}

impl<A> PromptCapturingAgent<A> {
    /// Wrap `inner` in a capturing tee. Returns the wrapper plus a shared
    /// handle to the captured-prompts vector for the test to read after
    /// the connection completes.
    fn new(inner: A) -> (Self, Arc<Mutex<Vec<PromptRequest>>>) {
        let captured = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                inner,
                captured: Arc::clone(&captured),
            },
            captured,
        )
    }
}

impl<A> ConnectTo<Client> for PromptCapturingAgent<A>
where
    A: ConnectTo<Client> + Send + 'static,
{
    /// Wire the client transport to the inner agent through a capturing tee.
    ///
    /// Builds an internal duplex channel between us and the inner component
    /// and runs three concurrent loops: the inner agent's own future,
    /// copy-and-capture client→inner, and a plain copy of inner→client.
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
    ) -> AcpResult<()> {
        // Internal pipe between us and the inner agent.
        let (to_inner, inner_side) = Channel::duplex();

        // Drive the inner agent on its end of the duplex channel.
        let inner_future = self.inner.connect_to(inner_side);

        // Drive the real client transport — we expose ourselves as the agent
        // it talks to.
        let (client_channel, client_future) = client.into_channel_and_future();

        let captured = self.captured;

        // client → inner: peek at every message; capture prompt requests.
        let capture_client_to_inner = capture_prompts(client_channel.rx, to_inner.tx, captured);

        // inner → client: pass-through.
        let copy_inner_to_client = copy_messages(to_inner.rx, client_channel.tx);

        match futures::try_join!(
            inner_future,
            client_future,
            capture_client_to_inner,
            copy_inner_to_client,
        ) {
            Ok(((), (), (), ())) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

/// Copy every message from `rx` to `tx`. Used for the inner→client direction
/// where we have no inspection to perform.
async fn copy_messages(
    mut rx: futures::channel::mpsc::UnboundedReceiver<AcpResult<Message>>,
    tx: futures::channel::mpsc::UnboundedSender<AcpResult<Message>>,
) -> AcpResult<()> {
    use futures::StreamExt;

    while let Some(msg) = rx.next().await {
        tx.unbounded_send(msg)
            .map_err(|e| agent_client_protocol::util::internal_error(e.to_string()))?;
    }
    Ok(())
}

/// Copy every message from `rx` to `tx`, capturing the parsed `PromptRequest`
/// of every `session/prompt` request seen on the way through.
///
/// Failure to parse the params is logged and ignored — capturing must never
/// break the wrapped agent's call.
async fn capture_prompts(
    mut rx: futures::channel::mpsc::UnboundedReceiver<AcpResult<Message>>,
    tx: futures::channel::mpsc::UnboundedSender<AcpResult<Message>>,
    captured: Arc<Mutex<Vec<PromptRequest>>>,
) -> AcpResult<()> {
    use futures::StreamExt;

    while let Some(msg) = rx.next().await {
        if let Ok(Message::Request(req)) = &msg {
            if req.id.is_some() && req.method == "session/prompt" {
                if let Some(prompt) = decode_prompt(req.params.as_ref()) {
                    captured.lock().unwrap().push(prompt);
                } else {
                    tracing::warn!("PromptCapturingAgent: failed to decode session/prompt params");
                }
            }
        }
        tx.unbounded_send(msg)
            .map_err(|e| agent_client_protocol::util::internal_error(e.to_string()))?;
    }
    Ok(())
}

/// Decode the `params` of a `session/prompt` JSON-RPC request into a typed
/// [`PromptRequest`]. Returns `None` on shape mismatch.
fn decode_prompt(params: Option<&Params>) -> Option<PromptRequest> {
    let value = match params? {
        Params::Object(map) => serde_json::Value::Object(map.clone()),
        Params::Array(_) => return None,
    };
    serde_json::from_value(value).ok()
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

    // Wrap the playback fixture in a capturing tee.
    let playback = PlaybackAgent::new(fixture_path(), "claude");
    let (capturing_agent, captured) = PromptCapturingAgent::new(playback);

    let ctx = AvpContext::with_agent(capturing_agent).expect("with_agent");

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

    // The capturing tee should have seen exactly one prompt (one rule).
    let prompts = captured.lock().unwrap().clone();
    assert!(
        !prompts.is_empty(),
        "expected at least one prompt to reach the agent, got none"
    );
    // Concatenate every text block in the first observed prompt.
    let prompt_text: String = prompts[0]
        .prompt
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

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
