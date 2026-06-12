//! Tests for [`LlamaHookEvaluator`], the llama-model-backed [`HookEvaluator`]
//! that powers `type: prompt` and `type: agent` hooks.
//!
//! The model-call step is exercised through the `HookModel` generation seam
//! with an in-test scripted fake — no real GPU or weights are loaded. This
//! mirrors the `ScriptedModel` keystone used elsewhere in the crate: the seam
//! stops at the trait that captures a single short model call.
//!
//! The acceptance criteria are about hook *decisions*, so the end-to-end tests
//! drive the evaluator through the real `HookConfig::build_registrations`
//! pipeline and fire `HookEvent`s, asserting on the resulting `HookDecision`.

use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::schema::StopReason;
use agent_client_protocol_extras::{HookConfig, HookDecision, HookEvaluator, HookEvent};
use async_trait::async_trait;
use llama_agent::acp::llama_hook_evaluator::{HookModel, LlamaHookEvaluator};

/// A weight-free [`HookModel`] that replays a fixed model output and records
/// every call. Lets the evaluator be driven without a real model.
#[derive(Clone, Default)]
struct ScriptedHookModel {
    /// Raw text the "model" returns from a generation call.
    output: String,
    /// Set true to simulate a model-call failure.
    fail: bool,
    /// Records (is_agent-irrelevant) the prompts and whether the agent path was used.
    calls: Arc<std::sync::Mutex<Vec<String>>>,
}

impl ScriptedHookModel {
    fn returning(output: &str) -> Self {
        Self {
            output: output.to_string(),
            fail: false,
            calls: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn failing() -> Self {
        Self {
            output: String::new(),
            fail: true,
            calls: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl HookModel for ScriptedHookModel {
    async fn generate_eval(
        &self,
        system_instruction: &str,
        user_prompt: &str,
        _max_tokens: usize,
    ) -> Result<String, String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{system_instruction}\n{user_prompt}"));
        if self.fail {
            Err("model call failed".to_string())
        } else {
            Ok(self.output.clone())
        }
    }

    async fn generate_agent_eval(
        &self,
        _system_instruction: &str,
        _user_prompt: &str,
        _max_turns: usize,
        max_tokens: usize,
    ) -> Result<String, String> {
        // Without an override, the agent path mirrors the single-turn output.
        self.generate_eval(_system_instruction, _user_prompt, max_tokens)
            .await
    }
}

/// A weight-free [`HookModel`] that records which path the evaluator drove and
/// replays a fixed verdict from the agent path.
///
/// This is the `generate_agent_eval` seam — the boundary the production
/// `AgentServer` implements by running the real bounded tool loop. These tests
/// own the *evaluator* behavior on top of that seam (path selection, verdict
/// normalization, error degradation); the bounded loop's own control flow —
/// turn counting, the tool branch, the forced final generation — is covered by
/// unit tests that drive the real `run_bounded_tool_loop` directly
/// (`agent::tests::bounded_tool_loop`). The fake therefore does *not*
/// re-implement the loop: doing so would assert the acceptance criteria against
/// a parallel algorithm rather than the production code.
#[derive(Clone)]
struct ScriptedAgentModel {
    /// The verdict the agent path returns.
    verdict: String,
    /// Number of times the agent path (`generate_agent_eval`) was invoked.
    agent_calls: Arc<std::sync::Mutex<usize>>,
}

impl ScriptedAgentModel {
    fn returning(verdict: &str) -> Self {
        Self {
            verdict: verdict.to_string(),
            agent_calls: Arc::new(std::sync::Mutex::new(0)),
        }
    }

    fn agent_calls(&self) -> usize {
        *self.agent_calls.lock().unwrap()
    }
}

#[async_trait]
impl HookModel for ScriptedAgentModel {
    async fn generate_eval(
        &self,
        _system_instruction: &str,
        _user_prompt: &str,
        _max_tokens: usize,
    ) -> Result<String, String> {
        Ok(self.verdict.clone())
    }

    async fn generate_agent_eval(
        &self,
        _system_instruction: &str,
        _user_prompt: &str,
        _max_turns: usize,
        _max_tokens: usize,
    ) -> Result<String, String> {
        *self.agent_calls.lock().unwrap() += 1;
        Ok(self.verdict.clone())
    }
}

/// Build a single-registration list for the given event kind + prompt hook,
/// wired to the supplied evaluator.
fn registrations(
    event: &str,
    evaluator: Arc<dyn HookEvaluator>,
) -> Vec<agent_client_protocol_extras::HookRegistration> {
    let json = format!(
        r#"{{ "hooks": {{ "{event}": [{{ "hooks": [{{ "type": "prompt", "prompt": "Check: $ARGUMENTS" }}] }}] }} }}"#
    );
    let config: HookConfig = serde_json::from_str(&json).expect("valid hook config");
    config
        .build_registrations(Some(evaluator))
        .expect("registrations build")
}

fn stop_event() -> HookEvent {
    HookEvent::Stop {
        session_id: "s1".to_string(),
        stop_reason: StopReason::EndTurn,
        stop_hook_active: false,
        cwd: PathBuf::from("/tmp"),
    }
}

fn user_prompt_submit_event() -> HookEvent {
    HookEvent::UserPromptSubmit {
        session_id: "s1".to_string(),
        prompt: vec![agent_client_protocol::schema::ContentBlock::from("hello")],
        cwd: PathBuf::from("/tmp"),
    }
}

async fn fire(
    event: HookEvent,
    regs: &[agent_client_protocol_extras::HookRegistration],
) -> HookDecision {
    regs[0].handler.handle(&event).await
}

#[tokio::test]
async fn stop_hook_block_yields_should_continue() {
    // A Stop hook whose evaluator returns ok:false must yield ShouldContinue
    // (Stop's "block" means "don't stop").
    let model = ScriptedHookModel::returning(r#"{"ok": false, "reason": "tests failing"}"#);
    let evaluator: Arc<dyn HookEvaluator> = LlamaHookEvaluator::with_model(Arc::new(model));
    let regs = registrations("Stop", evaluator);

    let decision = fire(stop_event(), &regs).await;
    match decision {
        HookDecision::ShouldContinue { reason } => assert_eq!(reason, "tests failing"),
        other => panic!("expected ShouldContinue, got {other:?}"),
    }
}

#[tokio::test]
async fn stop_hook_ok_allows() {
    let model = ScriptedHookModel::returning(r#"{"ok": true}"#);
    let evaluator: Arc<dyn HookEvaluator> = LlamaHookEvaluator::with_model(Arc::new(model));
    let regs = registrations("Stop", evaluator);

    let decision = fire(stop_event(), &regs).await;
    assert!(matches!(decision, HookDecision::Allow));
}

#[tokio::test]
async fn user_prompt_submit_block_blocks_with_reason() {
    let model = ScriptedHookModel::returning(r#"{"ok": false, "reason": "policy violation"}"#);
    let evaluator: Arc<dyn HookEvaluator> = LlamaHookEvaluator::with_model(Arc::new(model));
    let regs = registrations("UserPromptSubmit", evaluator);

    let decision = fire(user_prompt_submit_event(), &regs).await;
    match decision {
        HookDecision::Block { reason } => assert_eq!(reason, "policy violation"),
        other => panic!("expected Block, got {other:?}"),
    }
}

#[tokio::test]
async fn model_failure_allows() {
    // A model-call failure must never crash the turn: the evaluator returns a
    // valid ok:true response so the handler decides Allow.
    let model = ScriptedHookModel::failing();
    let evaluator: Arc<dyn HookEvaluator> = LlamaHookEvaluator::with_model(Arc::new(model));
    let regs = registrations("UserPromptSubmit", evaluator);

    let decision = fire(user_prompt_submit_event(), &regs).await;
    assert!(matches!(decision, HookDecision::Allow));
}

#[tokio::test]
async fn unparseable_output_allows() {
    // Garbage model output must be treated as Allow, not propagate an error.
    let model = ScriptedHookModel::returning("I think everything looks fine, no JSON here.");
    let evaluator: Arc<dyn HookEvaluator> = LlamaHookEvaluator::with_model(Arc::new(model));
    let regs = registrations("UserPromptSubmit", evaluator);

    let decision = fire(user_prompt_submit_event(), &regs).await;
    assert!(matches!(decision, HookDecision::Allow));
}

#[tokio::test]
async fn evaluate_extracts_embedded_json_object() {
    // Models often wrap JSON in prose; the evaluator must extract the object.
    let model = ScriptedHookModel::returning(
        "Sure! Here is my verdict:\n{\"ok\": false, \"reason\": \"missing tests\"}\nThanks.",
    );
    let evaluator = LlamaHookEvaluator::with_model(Arc::new(model));
    let response = HookEvaluator::evaluate(&*evaluator, "anything", false)
        .await
        .expect("evaluate succeeds");
    let parsed: serde_json::Value = serde_json::from_str(&response).expect("valid JSON returned");
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["reason"], "missing tests");
}

#[tokio::test]
async fn agent_path_routes_to_multi_turn_loop() {
    // The agent path (is_agent=true) drives the bounded tool loop via the
    // `generate_agent_eval` seam, then normalizes and returns its verdict. The
    // loop's own bound and turn-counting are unit-tested against the production
    // `run_bounded_tool_loop`; here we assert only that the agent path is taken
    // and its verdict is normalized.
    let model = Arc::new(ScriptedAgentModel::returning(
        r#"{"ok": false, "reason": "policy violation"}"#,
    ));
    let evaluator = LlamaHookEvaluator::with_model(Arc::clone(&model));

    let response = HookEvaluator::evaluate(&*evaluator, "anything", true)
        .await
        .expect("evaluate succeeds");

    let parsed: serde_json::Value = serde_json::from_str(&response).expect("valid JSON returned");
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["reason"], "policy violation");
    assert_eq!(model.agent_calls(), 1, "the agent path must be taken once");
}

#[tokio::test]
async fn agent_path_failure_allows() {
    // A failure in the agent loop must degrade to Allow, never crash the turn.
    let model = ScriptedHookModel::failing();
    let evaluator = LlamaHookEvaluator::with_model(Arc::new(model));

    let response = HookEvaluator::evaluate(&*evaluator, "anything", true)
        .await
        .expect("evaluate succeeds");

    let parsed: serde_json::Value = serde_json::from_str(&response).expect("valid JSON returned");
    assert_eq!(parsed["ok"], true);
}

#[tokio::test]
async fn prompt_path_does_not_run_agent_loop() {
    // is_agent=false must take the single-turn path, not the agent path.
    let model = Arc::new(ScriptedAgentModel::returning(r#"{"ok": true}"#));
    let evaluator = LlamaHookEvaluator::with_model(Arc::clone(&model));

    let response = HookEvaluator::evaluate(&*evaluator, "anything", false)
        .await
        .expect("evaluate succeeds");

    let parsed: serde_json::Value = serde_json::from_str(&response).expect("valid JSON returned");
    assert_eq!(parsed["ok"], true);
    assert_eq!(
        model.agent_calls(),
        0,
        "prompt path must not invoke the agent path"
    );
}
