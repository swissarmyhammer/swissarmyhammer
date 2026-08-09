//! Tests for [the stored fixture verdict](super).
//!
//! Every test here proves ONE tool rule and counts how many times its `run`
//! script executed against the fixtures, so "the verdict was read back" and
//! "the verdict was proved again" are a number rather than a judgement.

use super::*;

use std::path::PathBuf;

use tempfile::TempDir;

use crate::doctor::check_review_engine_with;
use crate::review::test_support::{
    counting_tool_script, fixture_runs, ruleset, write_counted_tool_rule_fixtures,
    FIXTURE_RUNS_PER_PROOF,
};
use crate::validators::types::{ToolDoctor, ToolScope};
use crate::validators::ValidatorLoader;

/// The validator set the probe rule lives in.
const PROBE_SET: &str = "probe";

/// The one tool rule every test here proves.
const PROBE_RULE: &str = "probe-tool";

/// How many times a rule is proved when something it turns on changed: once
/// before the change, and once after.
const PROOFS_ACROSS_A_CHANGE: usize = 2;

/// The tool version the probe rule reports before a test changes it.
const FIRST_VERSION: &str = "1.0.0";

/// The tool version a test changes to, to stand for a tool upgrade.
const UPGRADED_VERSION: &str = "2.0.0";

/// The directories one probe rule needs: a workspace for the cache, and a
/// validator set base holding the fixture pair.
struct ProbeDirs {
    /// The temporary root both directories live under.
    root: TempDir,
}

impl ProbeDirs {
    /// Lay out the workspace, the set base, the fixture pair, and the fixture
    /// marker the run script counts on.
    fn new() -> Self {
        let root = tempfile::tempdir().expect("probe root");
        std::fs::create_dir_all(root.path().join("workspace")).expect("probe workspace");
        std::fs::create_dir_all(root.path().join("set")).expect("probe set base");
        let dirs = Self { root };
        write_counted_tool_rule_fixtures(&dirs.base(), PROBE_RULE);
        std::fs::write(dirs.version_file(), FIRST_VERSION).expect("write the first version");
        dirs
    }

    /// The workspace the stored verdicts are kept under.
    fn workspace(&self) -> PathBuf {
        self.root.path().join("workspace")
    }

    /// The validator set base the health check reads `fixtures/` from.
    fn base(&self) -> PathBuf {
        self.root.path().join("set")
    }

    /// The file the probe rule's version command reads, so a test changes the
    /// tool version without touching the rule.
    fn version_file(&self) -> PathBuf {
        self.base().join("tool-version")
    }

    /// The file the probe rule's run script appends to on a fixture run.
    fn counter(&self) -> PathBuf {
        self.base().join("fixture-runs")
    }

    /// The file whose presence makes the probe rule's script exit nonzero.
    ///
    /// It sits beside the fixtures directory rather than inside it, so
    /// creating it breaks the rule without changing anything the stored
    /// verdict is keyed on.
    fn break_file(&self) -> PathBuf {
        self.base().join("break-the-tool")
    }

    /// How many fixture runs the probe rule's script has recorded.
    fn fixture_runs(&self) -> usize {
        fixture_runs(&self.counter())
    }

    /// A validator set holding the one probe tool rule.
    ///
    /// `extra_script` is appended to the run script, so a test states a rule
    /// edit as the text it added.
    fn ruleset(&self, extra_script: &str) -> RuleSet {
        let mut script = counting_tool_script(&self.counter());
        script.push_str(&format!(
            "if [ -f \"{}\" ]; then echo 'the tool broke' >&2; exit 3; fi\n",
            self.break_file().display()
        ));
        script.push_str(extra_script);

        let mut ruleset = ruleset(PROBE_SET, "*.rs", &[]);
        ruleset.base_path = self.base();
        ruleset.rules = vec![Rule {
            name: PROBE_RULE.to_string(),
            description: "findings by tool".to_string(),
            body: "the tool reads the code".to_string(),
            tool: Some(ToolSpec {
                scope: ToolScope::Files,
                run: script,
                doctor: Some(ToolDoctor {
                    check_command: "true".to_string(),
                    check_version_command: Some(format!("cat {}", self.version_file().display())),
                    fix_hint: None,
                }),
                install: None,
            }),
            ..Rule::default()
        }];
        ruleset
    }
}

/// The health of the one tool rule in `ruleset` under `proof`.
fn probe_health(cache: &ToolHealthCache, proof: HealthProof, ruleset: &RuleSet) -> ToolRuleStatus {
    let rule = &ruleset.rules[0];
    let spec = rule
        .tool
        .as_ref()
        .expect("the probe rule carries a tool block");
    tool_rule_health(Some(cache), proof, ruleset, rule, spec)
}

#[test]
fn an_unchanged_tool_rule_is_proved_once() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());
    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF,
        "the second check must read the stored verdict instead of proving the rule again"
    );
}

#[test]
fn a_changed_tool_version_proves_the_rule_again() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());
    std::fs::write(dirs.version_file(), UPGRADED_VERSION).expect("upgrade the tool");
    let after = probe_health(&cache, HealthProof::Stored, &ruleset);

    assert!(after.usable());
    assert_eq!(
        after.version.as_deref(),
        Some(UPGRADED_VERSION),
        "the reported version must be the one the tool reports now"
    );
    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF * PROOFS_ACROSS_A_CHANGE,
        "a verdict proved against the old tool version must not stand for the new one"
    );
}

#[test]
fn an_edited_run_script_proves_the_rule_again() {
    let dirs = ProbeDirs::new();
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert!(probe_health(&cache, HealthProof::Stored, &dirs.ruleset("")).usable());
    let edited = dirs.ruleset("# the rule author changed the pipe\n");
    assert!(probe_health(&cache, HealthProof::Stored, &edited).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF * PROOFS_ACROSS_A_CHANGE,
        "a verdict proved against the old run script must not stand for the edited one"
    );
}

#[test]
fn an_edited_fixture_proves_the_rule_again() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());
    std::fs::write(
        dirs.base().join("fixtures").join("probe-tool.pass.rs"),
        "fn still_clean() {}\n",
    )
    .expect("edit the pass fixture");
    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF * PROOFS_ACROSS_A_CHANGE,
        "a verdict proved against the old fixtures must not stand for the edited ones"
    );
}

#[test]
fn doctor_proves_the_rule_again_and_replaces_the_stored_verdict() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(ruleset.clone());
    let cache = ToolHealthCache::open(&dirs.workspace());

    // The engine proves the rule once and stores that it is healthy.
    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());

    // The tool breaks in a way the stored verdict is not keyed on, so the
    // engine keeps reporting the rule healthy from the store.
    std::fs::write(dirs.break_file(), "").expect("break the tool");
    assert!(
        probe_health(&cache, HealthProof::Stored, &ruleset).usable(),
        "the engine must still be reading the stored verdict for this to prove anything"
    );

    // Doctor proves the rule itself and sees the break.
    let status = check_review_engine_with(&loader, &[], Some(&cache));
    assert_eq!(status.tool_rules.len(), 1);
    assert!(
        !status.tool_rules[0].usable(),
        "sah doctor must run the fixtures whatever is stored"
    );

    // The engine now reads doctor's verdict, and reads it without proving the
    // rule for itself.
    let runs_after_doctor = dirs.fixture_runs();
    assert!(
        !probe_health(&cache, HealthProof::Stored, &ruleset).usable(),
        "doctor must replace the stored verdict, so the engine follows it"
    );
    assert_eq!(
        dirs.fixture_runs(),
        runs_after_doctor,
        "the engine must read doctor's stored verdict rather than prove the rule again"
    );
}
