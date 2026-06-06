//! Engine stage 2 — the fan-out fleet.
//!
//! The shard is the validator; the **grain is the file**. This stage takes the
//! stage-1 [`WorkList`](crate::review::WorkList) and produces one agent task per
//! `(validator, file)` pair, submitting every task to the shared
//! [`AgentPool`](crate::validators::AgentPool). Each task reviews ONE file
//! against ONE validator's rules, armed with the engine-run probe evidence
//! stage 1 already gathered, and returns a `Vec<`[`Finding`]`>` tagged with the
//! validator (and, when the agent cites it, the rule).
//!
//! # Batching, not concurrency
//!
//! To bound the task count on a large diff, a handful of files are *packed* into
//! one task ([`FleetConfig::batch_size`]); the grain stays the file (each file is
//! rendered as its own self-contained block), the batch is just packing so a
//! 400-file diff does not mint 400 separate sessions. The batching applied is
//! logged via [`tracing`].
//!
//! **Parallelism is not controlled here.** Every task goes to the shared
//! [`AgentPool`], which owns the single concurrency control (worker count). This
//! stage only submits and collects; the pool queues and drains. A task that
//! errors or times out yields zero findings for its batch — logged, never a
//! panic — so one bad task never aborts the rest.
//!
//! # The prompt payload
//!
//! [`render_fleet_prompt`] assembles exactly the payload the task specifies,
//! reusing the structured data stage 1 produced (no new template engine):
//!
//! 1. **Change purpose** — [`WorkList::change_purpose`](crate::review::WorkList).
//! 2. **Validator instructions** — the mandate (the validator's `description`),
//!    each rule body verbatim, the severity default, and the output contract
//!    (every finding emits `rule` + `claim` + `evidence` + `suggestion`, matching
//!    the [`Finding`] type).
//! 3. **The file(s) under review** — for each file in the batch: its path, the
//!    structured semantic diff, the bounded source slice, and the probe results
//!    rendered as evidence blocks.
//!
//! Excluded by design: other validators' rules and any file outside the batch.

use std::fmt::Write as _;

use crate::review::scope::{FileWork, ValidatorWork, WorkList};
use crate::review::types::{parse_findings, Finding};
use crate::validators::{AgentPool, RuleSet, Severity, ValidatorLoader};

/// Default number of files packed into a single fan-out task.
///
/// Small enough that one task's prompt stays well inside an agent's context
/// window (the grain is still the file), large enough that a big diff does not
/// mint a separate session per file.
pub const DEFAULT_BATCH_SIZE: usize = 4;

/// Configuration for a fan-out run.
#[derive(Debug, Clone, Copy)]
pub struct FleetConfig {
    /// How many files to pack into one agent task. Clamped to at least 1.
    pub batch_size: usize,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

impl FleetConfig {
    /// The effective, clamped batch size (never zero).
    fn effective_batch_size(&self) -> usize {
        self.batch_size.max(1)
    }
}

/// Fan a [`WorkList`] out across the shared [`AgentPool`] and collect the merged,
/// validator-tagged findings.
///
/// One task is built per `(validator, batch-of-files)`: every validator's files
/// are packed into batches of [`FleetConfig::batch_size`], each batch rendered
/// into one prompt by [`render_fleet_prompt`] and submitted to `pool`. As each
/// task returns, its response is parsed by [`parse_findings`] and every finding
/// is tagged with the validator. A task that errors or returns unparseable
/// content contributes zero findings for its batch and is logged — never a panic.
///
/// `loader` is the same fully-loaded [`ValidatorLoader`] stage 1 matched against,
/// reused here as the authoritative source of each validator's mandate and rule
/// bodies (the [`WorkList`] carries only the per-file work and the rule *names*).
/// A validator in the work-list with no matching RuleSet in the loader is logged
/// and skipped rather than rendered with empty instructions.
///
/// The returned findings are ordered by validator (work-list order), then by the
/// order the pool delivered each batch.
pub async fn run_fleet(
    work: &WorkList,
    loader: &ValidatorLoader,
    pool: &AgentPool,
    config: FleetConfig,
) -> Vec<Finding> {
    let batch_size = config.effective_batch_size();

    // Build every (validator, batch) task and submit it. Submission is
    // non-blocking; the pool owns the concurrency. We keep the validator name
    // and the batch's file paths alongside each receiver so a parse failure can
    // be attributed and tagged.
    struct Pending {
        validator: String,
        files: Vec<String>,
        rx: tokio::sync::oneshot::Receiver<crate::validators::PromptResult>,
    }

    let mut pending: Vec<Pending> = Vec::new();
    for validator in &work.validators {
        let Some(ruleset) = loader.get_ruleset(&validator.validator_name) else {
            tracing::warn!(
                validator = %validator.validator_name,
                "fleet fan-out: no RuleSet for validator in loader; skipping its files"
            );
            continue;
        };
        let total_batches = batch_count(validator.files.len(), batch_size);
        // The rule names being applied come from the loader's RuleSet (the
        // authoritative source), so the log shows exactly which validator×rules
        // ran — not just the validator name.
        let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
        tracing::info!(
            validator = %validator.validator_name,
            files = validator.files.len(),
            batch_size,
            batches = total_batches,
            rules = ?rule_names,
            "fleet fan-out: batching files into agent tasks"
        );
        for batch in validator.files.chunks(batch_size) {
            let prompt = render_fleet_prompt(&work.change_purpose, validator, ruleset, batch);
            let files: Vec<String> = batch.iter().map(|f| f.path.clone()).collect();
            tracing::debug!(
                validator = %validator.validator_name,
                files = ?files,
                rules = ?rule_names,
                "fleet fan-out: submitting validator×files×rules task"
            );
            pending.push(Pending {
                validator: validator.validator_name.clone(),
                files,
                rx: pool.submit(prompt),
            });
        }
    }

    // Collect every task. The pool drains them in parallel up to its worker
    // count; we await them in submission order, which is fine because each
    // receiver resolves independently.
    let mut findings: Vec<Finding> = Vec::new();
    for task in pending {
        let parsed = collect_task(task.rx.await, &task.validator, &task.files);
        findings.extend(parsed);
    }
    findings
}

/// Resolve one task's delivered result into tagged findings, degrading any
/// failure to an empty vec.
fn collect_task(
    delivered: Result<crate::validators::PromptResult, tokio::sync::oneshot::error::RecvError>,
    validator: &str,
    files: &[String],
) -> Vec<Finding> {
    let response = match delivered {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => {
            tracing::warn!(
                validator = %validator,
                files = ?files,
                error = %err,
                "fleet task failed; yielding zero findings for this batch"
            );
            return Vec::new();
        }
        Err(_) => {
            tracing::warn!(
                validator = %validator,
                files = ?files,
                "fleet task result was dropped before delivery; yielding zero findings"
            );
            return Vec::new();
        }
    };

    match parse_findings(&response.content) {
        Ok(parsed) => tag_findings(parsed, validator),
        Err(err) => {
            tracing::warn!(
                validator = %validator,
                files = ?files,
                error = %err,
                "fleet task response did not parse into findings; yielding zero findings"
            );
            Vec::new()
        }
    }
}

/// Tag every finding with its source `validator` name, overriding whatever the
/// agent emitted so the validator attribution is always authoritative.
fn tag_findings(mut findings: Vec<Finding>, validator: &str) -> Vec<Finding> {
    for finding in &mut findings {
        finding.validator = validator.to_string();
    }
    findings
}

/// How many batches `file_count` files split into at `batch_size` per batch.
fn batch_count(file_count: usize, batch_size: usize) -> usize {
    file_count.div_ceil(batch_size.max(1))
}

/// Render the fan-out prompt for one `(validator, batch-of-files)` task.
///
/// The payload is assembled directly from the structured stage-1 data — there is
/// no template engine. The three sections are, in order: the change purpose, the
/// validator's instructions (mandate + rule bodies + severity default + the
/// output contract), and one self-contained block per file in the batch (path +
/// semantic diff + bounded source slice + probe evidence).
///
/// `validator` is the work-list entry (its name and the file work); `ruleset` is
/// the same validator's loaded [`RuleSet`], the authoritative source of the
/// mandate (its description) and the verbatim rule bodies.
pub fn render_fleet_prompt(
    change_purpose: &str,
    validator: &ValidatorWork,
    ruleset: &RuleSet,
    files: &[FileWork],
) -> String {
    let mut out = String::new();

    // 1. Change purpose.
    out.push_str("# Change purpose\n\n");
    out.push_str(change_purpose.trim());
    out.push_str("\n\n");

    // 2. Validator instructions.
    render_validator_instructions(&mut out, validator, ruleset);

    // 3. The file(s) under review.
    out.push_str("# Files under review\n\n");
    for file in files {
        render_file_block(&mut out, file);
    }

    out
}

/// Append the validator-instructions section: mandate, rule bodies, severity
/// default, and the finding output contract.
fn render_validator_instructions(out: &mut String, validator: &ValidatorWork, ruleset: &RuleSet) {
    let _ = writeln!(out, "# Validator: {}\n", validator.validator_name);
    out.push_str("## Mandate\n\n");
    out.push_str(ruleset.description().trim());
    out.push_str("\n\n");

    out.push_str("## Rules\n\n");
    for rule in &ruleset.rules {
        let _ = writeln!(out, "### Rule: {}\n", rule.name);
        out.push_str(rule.body.trim());
        out.push_str("\n\n");
    }

    let _ = writeln!(
        out,
        "## Default severity\n\nUnless a rule states otherwise, findings default to severity `{}`.\n",
        severity_default(validator.severity)
    );

    out.push_str(OUTPUT_CONTRACT);
    out.push('\n');
}

/// The validator's default severity as the `blocker`/`warning`/`nit` word the
/// [`Finding`] severity field uses, so the contract speaks the agent's output
/// vocabulary rather than the loader's internal `info`/`warn`/`error`.
fn severity_default(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "blocker",
        Severity::Warn => "warning",
        Severity::Info => "nit",
    }
}

/// The finding output contract, shared verbatim by every fan-out prompt.
///
/// It instructs the agent to emit a JSON array of findings, each carrying the
/// four load-bearing fields the [`Finding`] type and the verify stage require:
/// `rule`, `claim` (what + why it matters), `evidence` (a cited probe proof), and
/// `suggestion` (the fix).
const OUTPUT_CONTRACT: &str = "\
## Output contract

Emit your findings as a JSON array. Each finding is one object with these fields:

- `file`: the path of the file the finding is about.
- `line`: the 1-based line number the finding points at.
- `rule`: which rule of this validator fired.
- `severity`: one of `blocker`, `warning`, `nit`.
- `claim`: what is wrong AND why it matters — one concern per finding.
- `evidence`: the proof the issue is real — cite the injected probe result \
(e.g. \"per `duplicates`: 0.94 at `bar.rs:88`\") or a `file:line` citation.
- `suggestion`: the fix.

Report only real issues. If you find none, emit an empty array `[]`.
";

/// Append one file's review block: path, semantic diff, bounded source slice,
/// and the probe results rendered as evidence.
fn render_file_block(out: &mut String, file: &FileWork) {
    let _ = writeln!(out, "## File: {}\n", file.path);

    out.push_str("### Semantic diff\n\n");
    render_semantic_diff(out, file);

    out.push_str("### Source slice\n\n");
    out.push_str("```\n");
    out.push_str(file.source_slice.trim_end());
    out.push_str("\n```\n\n");

    out.push_str("### Probe evidence\n\n");
    render_probe_evidence(out, file);
}

/// Append the structured semantic diff for a file as a list of changed entities.
fn render_semantic_diff(out: &mut String, file: &FileWork) {
    if file.semantic_diff.is_empty() {
        out.push_str("_No structured entity changes._\n\n");
        return;
    }
    for change in &file.semantic_diff {
        let _ = writeln!(
            out,
            "- {} {} `{}`",
            change.change_type, change.entity_type, change.entity_name
        );
    }
    out.push('\n');
}

/// Append the probe results for a file as evidence blocks.
fn render_probe_evidence(out: &mut String, file: &FileWork) {
    if file.probe_results.is_empty() {
        out.push_str("_No probe evidence._\n\n");
        return;
    }
    for result in &file.probe_results {
        let _ = writeln!(out, "- probe `{}` on `{}`:", result.name, result.target);
        if result.rows.is_empty() {
            out.push_str("  - (no rows)\n");
            continue;
        }
        for row in &result.rows {
            out.push_str("  - ");
            out.push_str(&row.file_path);
            if let Some(line) = row.line {
                let _ = write!(out, ":{line}");
            }
            if let Some(symbol) = &row.symbol {
                let _ = write!(out, " `{symbol}`");
            }
            if let Some(similarity) = row.similarity {
                let _ = write!(out, " @ {similarity:.2}");
            }
            if let Some(detail) = &row.detail {
                let _ = write!(out, " — {detail}");
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use agent_client_protocol::schema::{
        ContentBlock, ContentChunk, InitializeResponse, NewSessionResponse, PromptRequest,
        PromptResponse, SessionNotification, SessionUpdate, TextContent,
    };
    use agent_client_protocol::{Channel, Client, ConnectTo, ConnectionTo, Role};

    use swissarmyhammer_sem::model::change::{ChangeType, SemanticChange};

    use crate::review::probes::{ProbeKind, ProbeResult, ProbeRow};
    use crate::review::scope::WorkList;
    use crate::validators::types::{
        Rule, RuleSet, RuleSetManifest, RuleSetMetadata, ValidatorMatch,
    };
    use crate::validators::{AgentPool, PoolConfig, ValidatorLoader, ValidatorSource};

    // ---- fixtures --------------------------------------------------------

    /// A RuleSet whose mandate (description) and rule bodies are distinctive so
    /// the rendered prompt can be asserted against them verbatim.
    fn ruleset(name: &str, mandate: &str, rules: &[(&str, &str)]) -> RuleSet {
        RuleSet {
            manifest: RuleSetManifest {
                name: name.to_string(),
                description: mandate.to_string(),
                metadata: RuleSetMetadata {
                    version: "1.0.0".to_string(),
                },
                match_criteria: Some(ValidatorMatch {
                    tools: vec![],
                    files: vec!["*.rs".to_string()],
                }),
                trigger_matcher: None,
                tags: vec![],
                probes: vec![],
                severity: Severity::Warn,
                timeout: 30,
                once: false,
            },
            rules: rules
                .iter()
                .map(|(rname, body)| Rule {
                    name: rname.to_string(),
                    description: format!("{rname} description"),
                    body: body.to_string(),
                    severity: None,
                    timeout: None,
                })
                .collect(),
            source: ValidatorSource::Builtin,
            base_path: PathBuf::from("/test"),
        }
    }

    /// A loader carrying the given rulesets, matched by name in `run_fleet`.
    fn loader_with(rulesets: Vec<RuleSet>) -> ValidatorLoader {
        let mut loader = ValidatorLoader::new();
        for rs in rulesets {
            loader.add_builtin_ruleset(rs);
        }
        loader
    }

    /// A `FileWork` carrying a distinctive added entity, a source slice tagged
    /// with the path, and one `duplicates` probe row.
    fn file_work(path: &str, symbol: &str, dup_at: &str) -> FileWork {
        FileWork {
            path: path.to_string(),
            semantic_diff: vec![SemanticChange {
                id: format!("{path}:{symbol}"),
                entity_id: symbol.to_string(),
                change_type: ChangeType::Added,
                entity_type: "function".to_string(),
                entity_name: symbol.to_string(),
                file_path: path.to_string(),
                old_file_path: None,
                before_content: None,
                after_content: Some(format!("fn {symbol}() {{}}")),
                commit_sha: None,
                author: None,
                timestamp: None,
                structural_change: None,
            }],
            changed_symbols: vec![symbol.to_string()],
            source_slice: format!("// slice for {path}\nfn {symbol}() {{}}"),
            probe_results: vec![ProbeResult {
                name: "duplicates".to_string(),
                kind: ProbeKind::Fact,
                target: path.to_string(),
                rows: vec![ProbeRow {
                    file_path: dup_at.to_string(),
                    symbol: Some(symbol.to_string()),
                    line: Some(88),
                    similarity: Some(0.94),
                    detail: None,
                }],
            }],
        }
    }

    fn validator_work(name: &str, files: Vec<FileWork>) -> ValidatorWork {
        ValidatorWork {
            validator_name: name.to_string(),
            severity: Severity::Warn,
            rules: vec![format!("{name}-rule")],
            probes: vec!["duplicates".to_string()],
            files,
        }
    }

    // ---- scripted mock agent harness -------------------------------------
    //
    // A minimal ACP agent that maps each incoming prompt onto a scripted
    // response by substring match, delivering the response text as a streamed
    // `agent_message_chunk` (the shape the production agents emit and the pool's
    // collector reads). One script entry can be set to error, proving a failing
    // task degrades to zero findings without deadlocking the rest.

    struct ScriptedAgent {
        next_session: AtomicUsize,
        /// (prompt-substring, Some(response) | None=error), matched in order.
        script: Vec<(String, Option<String>)>,
        /// Prompts seen, for assertions about what was submitted.
        seen: Mutex<Vec<String>>,
    }

    impl ScriptedAgent {
        fn new(script: Vec<(String, Option<String>)>) -> Arc<Self> {
            Arc::new(Self {
                next_session: AtomicUsize::new(0),
                script,
                seen: Mutex::new(Vec::new()),
            })
        }

        fn seen_prompts(&self) -> Vec<String> {
            self.seen.lock().unwrap().clone()
        }

        fn response_for(&self, prompt: &str) -> Option<String> {
            for (needle, response) in &self.script {
                if prompt.contains(needle) {
                    return response.clone();
                }
            }
            // No script entry → empty findings array.
            Some("[]".to_string())
        }

        fn is_error(&self, prompt: &str) -> bool {
            self.script
                .iter()
                .find(|(needle, _)| prompt.contains(needle))
                .map(|(_, response)| response.is_none())
                .unwrap_or(false)
        }
    }

    /// Adapter wiring a [`ScriptedAgent`] as an ACP server over a channel.
    struct ScriptedAdapter(Arc<ScriptedAgent>);

    impl ConnectTo<Client> for ScriptedAdapter {
        async fn connect_to(
            self,
            client: impl ConnectTo<<Client as Role>::Counterpart>,
        ) -> agent_client_protocol::Result<()> {
            let mock = Arc::clone(&self.0);
            agent_client_protocol::Agent
                .builder()
                .name("scripted-agent")
                .on_receive_request(
                    {
                        let mock = Arc::clone(&mock);
                        async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                            dispatch(&mock, req, responder, &cx)
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |_n: agent_client_protocol::ClientNotification, _cx| Ok(()),
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_to(client)
                .await
        }
    }

    fn dispatch(
        mock: &Arc<ScriptedAgent>,
        request: agent_client_protocol::ClientRequest,
        responder: agent_client_protocol::Responder<serde_json::Value>,
        cx: &ConnectionTo<Client>,
    ) -> agent_client_protocol::Result<()> {
        use agent_client_protocol::ClientRequest as Req;

        let mock = Arc::clone(mock);
        let cx = cx.clone();
        cx.clone().spawn(async move {
            match request {
                Req::InitializeRequest(_) => responder
                    .cast()
                    .respond_with_result(Ok(InitializeResponse::new(1.into()))),
                Req::NewSessionRequest(_req) => {
                    let n = mock.next_session.fetch_add(1, Ordering::SeqCst);
                    let id = agent_client_protocol::schema::SessionId::new(format!("sess-{n}"));
                    responder
                        .cast()
                        .respond_with_result(Ok(NewSessionResponse::new(id)))
                }
                Req::PromptRequest(req) => {
                    let prompt = prompt_text(&req);
                    mock.seen.lock().unwrap().push(prompt.clone());
                    if mock.is_error(&prompt) {
                        return responder
                            .cast::<PromptResponse>()
                            .respond_with_error(agent_client_protocol::Error::internal_error());
                    }
                    if let Some(text) = mock.response_for(&prompt) {
                        // Stream the scripted content as an assistant chunk.
                        let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(
                            ContentBlock::Text(TextContent::new(text)),
                        ));
                        let notif = SessionNotification::new(req.session_id.clone(), update);
                        let _ = cx.send_notification(notif);
                    }
                    responder.cast().respond_with_result(Ok(PromptResponse::new(
                        agent_client_protocol::schema::StopReason::EndTurn,
                    )))
                }
                _ => responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found()),
            }
        })
    }

    fn prompt_text(req: &PromptRequest) -> String {
        req.prompt
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn new_notifier() -> Arc<claude_agent::NotificationSender> {
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        Arc::new(notifier)
    }

    /// Run `body` against a pool backed by the scripted agent.
    async fn with_pool<F, Fut, R>(agent: Arc<ScriptedAgent>, config: PoolConfig, body: F) -> R
    where
        F: FnOnce(AgentPool) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);
        let (channel_a, channel_b) = Channel::duplex();

        let agent_task = tokio::spawn(async move {
            let _ = ScriptedAdapter(agent).connect_to(channel_a).await;
        });

        let notifier_for_handler = Arc::clone(&notifier);
        let result = Client
            .builder()
            .name("fleet-test-client")
            .on_receive_notification(
                async move |notif: SessionNotification, _cx| {
                    let _ = notifier_for_handler.send_update(notif).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(channel_b, async move |conn: ConnectionTo<_>| {
                let pool = AgentPool::new(conn, notifier_body, config);
                Ok(body(pool).await)
            })
            .await
            .expect("client connect_with failed");

        agent_task.abort();
        let _ = agent_task.await;
        result
    }

    /// A findings array as an agent would emit it, fenced in prose.
    fn findings_json(file: &str, rule: &str, claim: &str) -> String {
        format!(
            "Here are my findings:\n\n```json\n[{{\"file\":\"{file}\",\"line\":42,\
             \"validator\":\"ignored-by-agent\",\"rule\":\"{rule}\",\"severity\":\"warning\",\
             \"claim\":\"{claim}\",\"evidence\":\"per `duplicates`: 0.94\",\
             \"suggestion\":\"extract a helper\"}}]\n```\n"
        )
    }

    // ---- renderer tests (pure) -------------------------------------------

    #[test]
    fn prompt_contains_change_purpose_mandate_rules_and_output_contract() {
        let rs = ruleset(
            "deduplicate",
            "DEDUP_MANDATE: never copy-paste logic.",
            &[(
                "no-copy-paste",
                "RULE_BODY: extract shared helpers verbatim.",
            )],
        );
        let vw = validator_work(
            "deduplicate",
            vec![file_work("src/a.rs", "alpha", "src/x.rs")],
        );

        let prompt = render_fleet_prompt("PURPOSE: scaffolding the parser.", &vw, &rs, &vw.files);

        assert!(
            prompt.contains("PURPOSE: scaffolding the parser."),
            "{prompt}"
        );
        assert!(
            prompt.contains("DEDUP_MANDATE: never copy-paste logic."),
            "{prompt}"
        );
        assert!(
            prompt.contains("RULE_BODY: extract shared helpers verbatim."),
            "rule body must appear verbatim: {prompt}"
        );
        // Output contract: the four load-bearing finding fields.
        assert!(prompt.contains("`rule`"), "{prompt}");
        assert!(prompt.contains("`claim`"), "{prompt}");
        assert!(prompt.contains("`evidence`"), "{prompt}");
        assert!(prompt.contains("`suggestion`"), "{prompt}");
        // Severity default rendered from the validator severity (warn → warning).
        assert!(prompt.contains("severity `warning`"), "{prompt}");
    }

    #[test]
    fn prompt_renders_the_files_probe_evidence_and_excludes_other_files() {
        let rs = ruleset("deduplicate", "mandate", &[("r", "rule body")]);
        let vw = validator_work(
            "deduplicate",
            vec![
                file_work("src/a.rs", "alpha", "src/dup_of_a.rs"),
                file_work("src/b.rs", "beta", "src/dup_of_b.rs"),
            ],
        );

        // Render a batch of JUST the first file.
        let prompt = render_fleet_prompt("purpose", &vw, &rs, &vw.files[..1]);

        // This file's path, symbol, slice, and probe evidence are present.
        assert!(prompt.contains("src/a.rs"), "{prompt}");
        assert!(prompt.contains("alpha"), "{prompt}");
        assert!(prompt.contains("// slice for src/a.rs"), "{prompt}");
        assert!(
            prompt.contains("probe `duplicates`"),
            "probe evidence must be rendered: {prompt}"
        );
        assert!(prompt.contains("src/dup_of_a.rs:88"), "{prompt}");
        assert!(prompt.contains("@ 0.94"), "{prompt}");

        // The OTHER file's content is excluded from this task's prompt.
        assert!(
            !prompt.contains("src/b.rs"),
            "other file must be excluded: {prompt}"
        );
        assert!(
            !prompt.contains("beta"),
            "other file's symbol must be excluded: {prompt}"
        );
        assert!(!prompt.contains("src/dup_of_b.rs"), "{prompt}");
    }

    #[test]
    fn severity_default_maps_to_finding_vocabulary() {
        assert_eq!(severity_default(Severity::Error), "blocker");
        assert_eq!(severity_default(Severity::Warn), "warning");
        assert_eq!(severity_default(Severity::Info), "nit");
    }

    // ---- batching tests (pure) -------------------------------------------

    #[test]
    fn batch_count_packs_files_into_bounded_batches() {
        assert_eq!(batch_count(0, 4), 0);
        assert_eq!(batch_count(1, 4), 1);
        assert_eq!(batch_count(4, 4), 1);
        assert_eq!(batch_count(5, 4), 2);
        assert_eq!(batch_count(8, 4), 2);
        assert_eq!(batch_count(9, 4), 3);
        // A zero batch size is clamped to 1 (one task per file), never a panic.
        assert_eq!(batch_count(3, 0), 3);
    }

    // ---- orchestrator tests (scripted mock agent) ------------------------

    #[tokio::test]
    async fn fan_out_two_validators_two_files_submits_at_most_four_tasks() {
        let rs_a = ruleset("val-a", "mandate a", &[("ra", "body a")]);
        let rs_b = ruleset("val-b", "mandate b", &[("rb", "body b")]);
        let loader = loader_with(vec![rs_a, rs_b]);

        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![
                validator_work(
                    "val-a",
                    vec![
                        file_work("src/a.rs", "alpha", "src/x.rs"),
                        file_work("src/b.rs", "beta", "src/y.rs"),
                    ],
                ),
                validator_work(
                    "val-b",
                    vec![
                        file_work("src/a.rs", "alpha", "src/x.rs"),
                        file_work("src/b.rs", "beta", "src/y.rs"),
                    ],
                ),
            ],
        };

        // Script: a finding for val-a on src/a.rs, a finding for val-b on
        // src/b.rs, empty for the rest (matched by validator + file in prompt).
        let agent = ScriptedAgent::new(vec![
            (
                "# Validator: val-a".to_string() + "\n\n## Mandate",
                Some(findings_json("src/a.rs", "ra", "dup in a")),
            ),
            (
                "# Validator: val-b".to_string() + "\n\n## Mandate",
                Some(findings_json("src/b.rs", "rb", "dup in b")),
            ),
        ]);
        let agent_probe = Arc::clone(&agent);

        // batch_size=1 → file-grain: 2 validators × 2 files = 4 tasks.
        let findings = with_pool(agent, PoolConfig::remote(4), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        assert_eq!(
            agent_probe.seen_prompts().len(),
            4,
            "2 validators × 2 files at batch_size 1 = 4 tasks"
        );

        // Every finding is tagged with its validator (overriding the agent's
        // self-reported `ignored-by-agent`), and the rule tag survives.
        let a = findings
            .iter()
            .find(|f| f.claim == "dup in a")
            .expect("val-a finding");
        assert_eq!(a.validator, "val-a");
        assert_eq!(a.rule.as_deref(), Some("ra"));
        let b = findings
            .iter()
            .find(|f| f.claim == "dup in b")
            .expect("val-b finding");
        assert_eq!(b.validator, "val-b");
        assert_eq!(b.rule.as_deref(), Some("rb"));
        assert!(
            findings.iter().all(|f| f.validator != "ignored-by-agent"),
            "the agent's self-reported validator must be overridden"
        );
    }

    #[tokio::test]
    async fn many_small_files_collapse_into_fewer_batched_tasks() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);

        // 10 files for one validator.
        let files: Vec<FileWork> = (0..10)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        let agent = ScriptedAgent::new(vec![]);
        let agent_probe = Arc::clone(&agent);

        // batch_size=4 → 10 files collapse into ceil(10/4) = 3 tasks.
        let _findings = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 4 }).await
        })
        .await;

        assert_eq!(
            agent_probe.seen_prompts().len(),
            3,
            "10 small files at batch_size 4 collapse into 3 tasks, not 10"
        );
        // Each batched task carries multiple files (the grain stays the file:
        // each is its own block, the batch just packs them).
        let first = &agent_probe.seen_prompts()[0];
        let file_blocks = first.matches("## File: ").count();
        assert_eq!(file_blocks, 4, "the first batch packs 4 file blocks");
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn batching_applied_is_logged() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);

        let files: Vec<FileWork> = (0..5)
            .map(|i| file_work(&format!("src/f{i}.rs"), &format!("sym{i}"), "src/x.rs"))
            .collect();
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("val", files)],
        };

        let agent = ScriptedAgent::new(vec![]);
        let _findings = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 2 }).await
        })
        .await;

        // The fan-out logs the batching it applied: 5 files at batch_size 2 → 3
        // batches, attributed to the validator.
        assert!(logs_contain(
            "fleet fan-out: batching files into agent tasks"
        ));
        assert!(logs_contain("batches=3"));
        assert!(logs_contain("batch_size=2"));
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn fan_out_logs_the_rule_names_being_applied_per_validator() {
        // A validator with two distinctively-named rules; the fan-out log must
        // name the rules being applied (sourced from the loader's RuleSet) so the
        // logs show exactly which validator×rules ran.
        let rs = ruleset(
            "deduplicate",
            "mandate",
            &[("no-copy-paste", "body a"), ("prefer-reuse", "body b")],
        );
        let loader = loader_with(vec![rs]);

        let files: Vec<FileWork> = vec![file_work("src/a.rs", "alpha", "src/x.rs")];
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work("deduplicate", files)],
        };

        let agent = ScriptedAgent::new(vec![]);
        let _findings = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig::default()).await
        })
        .await;

        // The batching log carries the rule names from the loader's RuleSet as a
        // structured field (the exact bracketed list only this log emits — the
        // rendered prompt spells rules as `### Rule: ...` prose, not this shape).
        assert!(logs_contain("rules=[\"no-copy-paste\", \"prefer-reuse\"]"));
    }

    #[tokio::test]
    async fn one_failing_task_yields_zero_findings_without_aborting_the_rest() {
        let rs = ruleset("val", "mandate", &[("r", "body")]);
        let loader = loader_with(vec![rs]);

        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "val",
                vec![
                    file_work("src/good.rs", "good", "src/x.rs"),
                    file_work("src/bad.rs", "bad", "src/y.rs"),
                ],
            )],
        };

        // The task whose prompt mentions src/bad.rs errors; the good one returns
        // a finding.
        let agent = ScriptedAgent::new(vec![
            ("## File: src/bad.rs".to_string(), None),
            (
                "## File: src/good.rs".to_string(),
                Some(findings_json("src/good.rs", "r", "real issue")),
            ),
        ]);

        let findings = with_pool(agent, PoolConfig::remote(2), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig { batch_size: 1 }).await
        })
        .await;

        // The erroring task contributed nothing; the good one still returned.
        assert_eq!(
            findings.len(),
            1,
            "the failing task degrades to zero findings"
        );
        assert_eq!(findings[0].claim, "real issue");
        assert_eq!(findings[0].validator, "val");
    }

    #[tokio::test]
    async fn validator_missing_from_loader_is_skipped_not_panicked() {
        // The work-list names a validator the loader does not know.
        let loader = loader_with(vec![ruleset("known", "mandate", &[("r", "body")])]);
        let work = WorkList {
            change_purpose: "purpose".to_string(),
            validators: vec![validator_work(
                "unknown",
                vec![file_work("src/a.rs", "alpha", "src/x.rs")],
            )],
        };

        let agent = ScriptedAgent::new(vec![]);
        let agent_probe = Arc::clone(&agent);

        let findings = with_pool(agent, PoolConfig::remote(1), move |pool| async move {
            run_fleet(&work, &loader, &pool, FleetConfig::default()).await
        })
        .await;

        assert!(
            findings.is_empty(),
            "an unknown validator yields no findings"
        );
        assert!(
            agent_probe.seen_prompts().is_empty(),
            "no task is submitted for a validator missing from the loader"
        );
    }
}
