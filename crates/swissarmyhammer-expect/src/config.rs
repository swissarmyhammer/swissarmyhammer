//! The repo-level `.expect/config.toml` schema and parser.
//!
//! `config.toml` is the optional, repo-wide configuration from `ideas/expect.md`
//! §"Config Schema": the pinned grading model, embedder, Tier-2 threshold,
//! reliability and approval policy, and the driving agent's use-case. Every field
//! carries a documented default, so an absent file (or an absent section, or an
//! absent key) collapses to [`ExpectConfig::default`] — configuration only ever
//! *overrides* defaults, never *requires* them.
//!
//! Like the spec [`Frontmatter`](crate::Frontmatter), each section is a **closed**
//! set of keys: `deny_unknown_fields` makes a typo such as `similarty_threshold`
//! fail the parser loudly rather than be silently ignored.

use crate::error::ExpectError;
use crate::spec::ReliabilityPolicy;
use crate::types::Surface;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The directory, at the repo root, that holds `expect` state and config.
///
/// `pub(crate)` so sibling modules (e.g. [`crate::observe`]) anchor their own
/// `.expect/` sub-paths to this single source of truth rather than re-typing it.
pub(crate) const EXPECT_DIR: &str = ".expect";

/// The repo-level config file within [`EXPECT_DIR`].
const CONFIG_FILE: &str = "config.toml";

/// The default Tier-2 cosine similarity cutoff for [`EmbedderConfig`].
const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.80;

/// The default grader-confidence floor for [`ApprovalConfig`]: below this, a
/// criterion is routed to the human escalation queue.
const DEFAULT_ESCALATE_BELOW_CONFIDENCE: f32 = 0.6;

/// The repo-level `.expect/config.toml` configuration.
///
/// The whole struct is `#[serde(default)]`, so any missing section is filled from
/// that section's own [`Default`] — which is how an absent file and a partial file
/// both collapse onto the documented defaults. `deny_unknown_fields` rejects a
/// stray top-level table so a mistyped section header fails loudly.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExpectConfig {
    /// The grading model: which named sah model renders Tier-3 verdicts.
    pub model: ModelConfig,
    /// How the system under test is provisioned per `check`.
    pub provision: ProvisionConfig,
    /// The pinned embedder and its Tier-2 cosine cutoff.
    pub embedder: EmbedderConfig,
    /// The repo-wide reliability policy and non-deterministic surfaces.
    pub reliability: ReliabilityConfig,
    /// The drift-approval and human-escalation policy.
    pub approval: ApprovalConfig,
    /// The driving agent that perceives and acts on the system under test.
    pub agent: AgentConfig,
}

/// The `[model]` section: the model that **grades** criteria (Tier 3).
///
/// Distinct from [`AgentConfig`], which only *drives* the system under test.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelConfig {
    /// Named sah model for grading; empty ⇒ the sah model default.
    pub default: String,
    /// Optional extra named models consulted for borderline criteria.
    pub panel: Vec<String>,
    /// What to do when the pinned model is gone. Default [`OnMissing::Fallback`].
    pub on_missing: OnMissing,
}

/// What `expect` does when the pinned grading model is no longer available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnMissing {
    /// Warn and fall back to the sah model default (the documented default).
    #[default]
    Fallback,
    /// Treat the missing pinned model as a hard error.
    Error,
}

/// The `[provision]` section: how the system under test is stood up per `check`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProvisionConfig {
    /// Provisioning granularity. Default [`Granularity::PerCheck`].
    pub granularity: Granularity,
}

/// How many systems under test a single `check` provisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Granularity {
    /// One shared SUT per `check`; `isolation: fresh` overrides per-spec.
    #[default]
    PerCheck,
}

/// The `[embedder]` section: the pinned embedding model and Tier-2 cutoff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EmbedderConfig {
    /// The pinned embedding model — the checkpoint matters for reproducibility.
    pub model: String,
    /// The Tier-2 cosine similarity cutoff. Default `0.80`.
    pub similarity_threshold: f32,
}

impl Default for EmbedderConfig {
    fn default() -> Self {
        EmbedderConfig {
            model: "text-embedding-3-large".to_string(),
            similarity_threshold: DEFAULT_SIMILARITY_THRESHOLD,
        }
    }
}

/// The `[reliability]` section: the repo-wide `pass^k` default and the surfaces
/// whose runs are inherently non-deterministic.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReliabilityConfig {
    /// The default `pass^k` policy when a spec omits `reliability:`. Default `pass^1`.
    pub default: ReliabilityPolicy,
    /// Surfaces that drive non-deterministically; empty ⇒ all drive mechanically.
    pub nondeterministic_surfaces: Vec<Surface>,
}

/// The `[approval]` section: the drift-approval and human-escalation policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApprovalConfig {
    /// Whether CI auto-approves drift. Default `false` (unapproved drift fails CI).
    pub ci_autoapprove: bool,
    /// Route a criterion to the human queue below this grader confidence. Default `0.6`.
    pub escalate_below_confidence: f32,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        ApprovalConfig {
            ci_autoapprove: false,
            escalate_below_confidence: DEFAULT_ESCALATE_BELOW_CONFIDENCE,
        }
    }
}

/// The `[agent]` section: the agent that **drives** the system under test.
///
/// Distinct from [`ModelConfig`], which only *grades* criteria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    /// The `AgentUseCase` that perceives and acts on the SUT. Default `expectations`.
    pub use_case: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            use_case: "expectations".to_string(),
        }
    }
}

impl ExpectConfig {
    /// Parse an [`ExpectConfig`] from the raw `config.toml` contents.
    ///
    /// Any omitted section, or any omitted key within a section, takes its
    /// documented default. An unknown key in any section is an error.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Toml`] when `contents` is not valid TOML or carries
    /// an unknown key.
    pub fn parse(contents: &str) -> Result<Self, ExpectError> {
        Ok(toml::from_str(contents)?)
    }

    /// Load the [`ExpectConfig`] from `<expect_dir>/config.toml`.
    ///
    /// The config is optional: a missing file yields [`ExpectConfig::default`]
    /// rather than an error.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Io`] when the file exists but cannot be read, or
    /// [`ExpectError::Toml`] when it is present but malformed.
    pub fn load(expect_dir: &Path) -> Result<Self, ExpectError> {
        let path = expect_dir.join(CONFIG_FILE);
        match std::fs::read_to_string(&path) {
            Ok(contents) => Self::parse(&contents),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ExpectConfig::default()),
            Err(err) => Err(ExpectError::Io(err)),
        }
    }
}

/// Locate the `.expect/` directory at the git repository root enclosing
/// `start_dir`, or `None` when `start_dir` is not within a git repository.
///
/// The directory is *not* required to exist — the caller hands the result to
/// [`ExpectConfig::load`], which treats an absent `config.toml` as all-defaults.
pub fn find_expect_dir(start_dir: &Path) -> Option<PathBuf> {
    swissarmyhammer_directory::find_git_repository_root_from(start_dir)
        .map(|root| root.join(EXPECT_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// The full worked example from `ideas/expect.md` §"Config Schema".
    const FULL_CONFIG: &str = r#"
[model]
default = "qwen-coder-flash"
panel = ["claude-sonnet", "gpt-5"]
on_missing = "error"

[provision]
granularity = "per-check"

[embedder]
model = "text-embedding-3-small"
similarity_threshold = 0.9

[reliability]
default = "pass^3"
nondeterministic_surfaces = ["browser", "gui"]

[approval]
ci_autoapprove = true
escalate_below_confidence = 0.42

[agent]
use_case = "driving"
"#;

    #[test]
    fn parses_a_full_config_into_all_fields() {
        let config = ExpectConfig::parse(FULL_CONFIG).expect("parse full config");

        assert_eq!(config.model.default, "qwen-coder-flash");
        assert_eq!(config.model.panel, vec!["claude-sonnet", "gpt-5"]);
        assert_eq!(config.model.on_missing, OnMissing::Error);

        assert_eq!(config.provision.granularity, Granularity::PerCheck);

        assert_eq!(config.embedder.model, "text-embedding-3-small");
        assert_eq!(config.embedder.similarity_threshold, 0.9);

        assert_eq!(config.reliability.default.required(), 3);
        assert_eq!(
            config.reliability.nondeterministic_surfaces,
            vec![Surface::Browser, Surface::Gui]
        );

        assert!(config.approval.ci_autoapprove);
        assert_eq!(config.approval.escalate_below_confidence, 0.42);

        assert_eq!(config.agent.use_case, "driving");
    }

    #[test]
    fn missing_config_file_yields_defaults() {
        let dir = TempDir::new().unwrap();
        // No config.toml written.
        let config = ExpectConfig::load(dir.path()).expect("load missing config");
        assert_eq!(config, ExpectConfig::default());
    }

    #[test]
    fn default_config_matches_the_documented_defaults() {
        let config = ExpectConfig::default();
        assert_eq!(config.model.default, "");
        assert!(config.model.panel.is_empty());
        assert_eq!(config.model.on_missing, OnMissing::Fallback);
        assert_eq!(config.provision.granularity, Granularity::PerCheck);
        assert_eq!(config.embedder.model, "text-embedding-3-large");
        assert_eq!(
            config.embedder.similarity_threshold,
            DEFAULT_SIMILARITY_THRESHOLD
        );
        assert_eq!(config.reliability.default.required(), 1);
        assert!(config.reliability.nondeterministic_surfaces.is_empty());
        assert!(!config.approval.ci_autoapprove);
        assert_eq!(
            config.approval.escalate_below_confidence,
            DEFAULT_ESCALATE_BELOW_CONFIDENCE
        );
        assert_eq!(config.agent.use_case, "expectations");
    }

    #[test]
    fn partial_config_merges_set_keys_over_defaults() {
        // Only one key in one section is set; everything else must default.
        let config = ExpectConfig::parse("[embedder]\nsimilarity_threshold = 0.5\n")
            .expect("parse partial config");

        // The set key wins.
        assert_eq!(config.embedder.similarity_threshold, 0.5);
        // The unset key in the same section keeps its default.
        assert_eq!(config.embedder.model, "text-embedding-3-large");
        // An entirely absent section is the default.
        assert_eq!(config.agent.use_case, "expectations");
        assert_eq!(
            config.approval.escalate_below_confidence,
            DEFAULT_ESCALATE_BELOW_CONFIDENCE
        );
    }

    #[test]
    fn loads_a_full_config_from_an_expect_dir() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.toml"), FULL_CONFIG).unwrap();

        let config = ExpectConfig::load(dir.path()).expect("load full config");
        assert_eq!(config.agent.use_case, "driving");
        assert_eq!(config.model.on_missing, OnMissing::Error);
    }

    #[test]
    fn rejects_an_unknown_key_in_a_section() {
        let toml = "[embedder]\nsimilarty_threshold = 0.5\n";
        let err = ExpectConfig::parse(toml).expect_err("unknown key must fail");
        let message = err.to_string();
        assert!(
            message.contains("similarty_threshold"),
            "error should name the bad key, got: {message}"
        );
    }

    #[test]
    fn rejects_an_unknown_top_level_section() {
        let toml = "[embeder]\nmodel = \"x\"\n";
        let err = ExpectConfig::parse(toml).expect_err("unknown section must fail");
        assert!(
            err.to_string().contains("embeder"),
            "error should name the bad section: {err}"
        );
    }

    #[test]
    fn rejects_a_malformed_reliability_policy() {
        let toml = "[reliability]\ndefault = \"always\"\n";
        let err = ExpectConfig::parse(toml).expect_err("bad pass^k must fail");
        assert!(
            err.to_string().contains("pass^"),
            "error should explain the pass^N form: {err}"
        );
    }

    #[test]
    fn find_expect_dir_resolves_to_repo_root() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        let nested = repo.path().join("crates").join("inner");
        fs::create_dir_all(&nested).unwrap();

        let expect_dir = find_expect_dir(&nested).expect("repo root found");
        assert_eq!(expect_dir, repo.path().join(".expect"));
    }

    #[test]
    fn find_expect_dir_is_none_outside_a_repo() {
        let not_a_repo = TempDir::new().unwrap();
        assert!(find_expect_dir(not_a_repo.path()).is_none());
    }
}
