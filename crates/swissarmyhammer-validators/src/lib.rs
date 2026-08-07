//! SwissArmyHammer validators — the pluggable review engine.
//!
//! The hook-triggered execution path (per-tool-call validator dispatch via
//! PreToolUse/PostToolUse/Stop hooks) has been retired. This crate now provides
//! only the genuinely reusable, hook-free pieces of the review engine:
//!
//! 1. The **rules-as-data loader** ([`validators::ValidatorLoader`], the
//!    [`validators::types`] data model, and the [`validators::parser`]):
//!    file/glob matching, directory-precedence stacking, and `@`-include
//!    expansion.
//! 2. A hook-free **shared bounded agent pool** ([`validators::AgentPool`]): the
//!    single place parallelism is controlled for the whole review pipeline.
//!
//! Frontmatter in VALIDATOR.md and rule files supports Liquid templating.
//! Use `{{ version }}` to reference the workspace version.
//!
//! # Engine API
//!
//! The crate root exposes a small, decoupled, hook-free engine surface that
//! downstream stages consume — none of these take a hook or ACP-hook argument:
//!
//! - [`load_rules`] — load every builtin/user/project RuleSet with the correct
//!   directory precedence into a ready [`validators::ValidatorLoader`].
//! - [`match_rules`] — given a file path and its workspace root, return the
//!   RuleSets whose match criteria select that file.
//! - [`workspace_project_types`] — resolve an optional workspace root into the
//!   detected project type keys a match context carries.
//! - [`execute_agents`] — fan a batch of prompts out to a shared
//!   [`validators::AgentPool`] and collect their results.

use std::path::Path;

/// Builtin RuleSets and YAML includes embedded in the binary at build time.
pub mod builtin;
/// Review-engine status facts for doctor surfaces.
///
/// Call [`doctor::check_review_engine`] with a workspace root to get a
/// [`doctor::ReviewEngineStatus`] — the detected project types, each
/// validator set with its applicability, and each tool rule with its tool
/// presence and fixture result. Convert the facts into doctor check rows
/// with [`doctor::to_checks`].
pub mod doctor;
/// The crate error type, [`AvpError`], for load, parse, and match failures.
pub mod error;
/// The local multi-agent review pipeline: scope resolution, probes, fleet
/// prompts, tool output parsing, verification, and synthesis.
pub mod review;
/// The rules-as-data model: [`ValidatorLoader`], the [`validators::types`]
/// data model, the parser, and the shared [`AgentPool`].
pub mod validators;

pub use builtin::load_builtins;
pub use error::AvpError;
pub use validators::{
    AgentPool, MatchContext, PoolConfig, PoolError, PromptResult, RuleSet, Validator,
    ValidatorLoader, ValidatorResult,
};

/// Workspace version, inherited from the workspace Cargo.toml.
///
/// Available in VALIDATOR.md and rule frontmatter as the Liquid variable `{{ version }}`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Load every RuleSet with builtin → user → project precedence.
///
/// This is the standalone, hook-free entry point for getting a fully populated
/// loader. It loads, in order:
///
/// 1. Builtin RuleSets embedded with the engine (lowest precedence).
/// 2. User and project RuleSets from their on-disk directories, with later
///    sources overriding earlier ones of the same name (highest precedence).
///
/// Returns the populated [`ValidatorLoader`] ready for [`match_rules`] queries
/// or direct inspection.
///
/// # Errors
///
/// Returns an [`AvpError`] if loading the user/project directories fails. Builtin
/// loading failures are logged and skipped rather than propagated, matching the
/// loader's builtin-precedence contract.
pub fn load_rules(workspace_root: Option<&Path>) -> Result<ValidatorLoader, AvpError> {
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);
    loader.load_all(workspace_root)?;
    Ok(loader)
}

/// Return the RuleSets that match a given file path in a given workspace.
///
/// This is the decoupled, standalone rule-matching surface: it takes a file path
/// and the workspace root that file belongs to — no hook event, no tool name, no
/// ACP-hook context — loads the full rule stack via [`load_rules`], and returns
/// the owned RuleSets whose match criteria select `file_path`.
///
/// `workspace_root` does double duty, and both uses take the same root. It
/// selects the PROJECT layer [`load_rules`] stacks (`<root>/.validators`), and
/// it is resolved once per call into the workspace's detected project type keys
/// with [`review::detected_project_type_keys`], the same helper the review scope
/// stage uses, so a RuleSet keyed on `match.project_types` is evaluated against
/// the real workspace. Pass the session working directory's root; never the
/// process current directory.
///
/// A `None` root fails closed twice over: no project layer loads, and no project
/// types resolve, so a `project_types`-keyed RuleSet does not match.
///
/// # Errors
///
/// Returns an [`AvpError`] if [`load_rules`] fails.
pub fn match_rules(
    file_path: impl Into<String>,
    workspace_root: Option<&Path>,
) -> Result<Vec<RuleSet>, AvpError> {
    let loader = load_rules(workspace_root)?;
    let ctx = MatchContext::new()
        .with_file(file_path)
        .with_project_types(workspace_project_types(workspace_root));
    Ok(loader
        .matching_rulesets(&ctx)
        .into_iter()
        .cloned()
        .collect())
}

/// The detected project type keys of the workspace at `workspace_root`.
///
/// The ONE place a rule-matching surface turns an optional workspace root into
/// match-context project types: a root resolves through
/// [`review::detected_project_type_keys`], and no root resolves to no types so a
/// `project_types`-keyed RuleSet fails closed instead of matching an unknown
/// workspace.
pub fn workspace_project_types(workspace_root: Option<&Path>) -> Vec<String> {
    workspace_root
        .map(review::detected_project_type_keys)
        .unwrap_or_default()
}

/// Fan a batch of prompts out to a shared agent pool and collect their results.
///
/// This is the hook-free agent-execution primitive downstream stages consume.
/// Every prompt is submitted to `pool` (which controls parallelism via its fixed
/// worker count); the returned vector preserves submission order, with each entry
/// holding that prompt's [`PromptResult`].
///
/// Submission is non-blocking and the pool pipelines work across its workers, so
/// callers get fan-out parallelism for free without managing tasks themselves.
pub async fn execute_agents(
    pool: &AgentPool,
    prompts: impl IntoIterator<Item = String>,
) -> Vec<PromptResult> {
    let receivers: Vec<_> = prompts.into_iter().map(|p| pool.submit(p)).collect();
    let mut results = Vec::with_capacity(receivers.len());
    for rx in receivers {
        match rx.await {
            Ok(result) => results.push(result),
            Err(_) => results.push(Err(claude_agent::AgentError::Internal(
                "agent pool dropped the result before delivery".to_string(),
            )
            .into())),
        }
    }
    results
}
