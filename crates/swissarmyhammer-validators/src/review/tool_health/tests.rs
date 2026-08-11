//! Tests for [the stored fixture verdict](super).
//!
//! Every test here proves ONE tool rule and counts how many times its `run`
//! script executed against the fixtures, so "the verdict was read back" and
//! "the verdict was proved again" are a number rather than a judgement.

use super::*;

use std::path::PathBuf;

use tempfile::TempDir;

use swissarmyhammer_common::test_utils::shell_escape_path;

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

/// How many times each test here probes the same rule: once to leave a
/// verdict, and once to learn whether that verdict was read back or the rule
/// was proved again.
const PROBES_PER_TEST: usize = 2;

/// How many times a BROKEN rule runs its script to prove itself: the fail
/// fixture alone, because the fixture contract stops at the first broken run.
const FIXTURE_RUNS_PER_BROKEN_PROOF: usize = 1;

/// The tool version the probe rule reports before a test changes it.
const FIRST_VERSION: &str = "1.0.0";

/// The tool version a test changes to, to stand for a tool upgrade.
const UPGRADED_VERSION: &str = "2.0.0";

/// A version command that exits nonzero, so the doctor reads no version.
const FAILING_VERSION_COMMAND: &str = "exit 1";

/// A version command that succeeds and prints nothing, so the doctor reads no
/// version.
const SILENT_VERSION_COMMAND: &str = "true";

/// A fixture file that is neither of the rule's own fixtures. The doctor
/// copies the whole directory into its scratch directory, so this file is part
/// of what a fixture run reads.
const NEIGHBOUR_FIXTURE: &str = "neighbour.rs";

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

    /// The state directory a review must not create until it saves a verdict.
    fn state_dir(&self) -> PathBuf {
        self.workspace()
            .join(ManagedDirectory::<SwissarmyhammerConfig>::dir_name())
    }

    /// The validator set base the health check reads `fixtures/` from.
    fn base(&self) -> PathBuf {
        self.root.path().join("set")
    }

    /// The directory the health check reads the fixture pair from.
    fn fixtures(&self) -> PathBuf {
        self.base().join(FIXTURES_DIR_NAME)
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

    /// A validator set holding the one probe tool rule, which reports the tool
    /// version [`ProbeDirs::version_file`] holds.
    ///
    /// `extra_script` is appended to the run script, so a test states a rule
    /// edit as the text it added.
    fn ruleset(&self, extra_script: &str) -> RuleSet {
        let version_command = format!("cat {}", shell_escape_path(&self.version_file()));
        self.ruleset_with_version(extra_script, Some(version_command))
    }

    /// A validator set holding the one probe tool rule, which reports no tool
    /// version.
    fn versionless_ruleset(&self) -> RuleSet {
        self.ruleset_with_version("", None)
    }

    /// A validator set holding the one probe tool rule, whose doctor block
    /// reads its version with `check_version_command`.
    fn ruleset_with_version(
        &self,
        extra_script: &str,
        check_version_command: Option<String>,
    ) -> RuleSet {
        let mut script = counting_tool_script(&self.counter());
        script.push_str(&format!(
            "if [ -f {} ]; then echo 'the tool broke' >&2; exit 3; fi\n",
            shell_escape_path(&self.break_file())
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
                    check_version_command,
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

/// Whether a verdict stands under the key of the one rule in `ruleset`.
fn verdict_is_stored(cache: &ToolHealthCache, ruleset: &RuleSet) -> bool {
    let key = verdict_key(ruleset, &ruleset.rules[0]);
    cache.verdicts().contains_key(&key)
}

/// Probe the one rule in `ruleset` twice and require that the fixtures ran
/// both times and that nothing was stored under the rule's key.
///
/// `why` names the reason the rule cannot be stored, so a failure states which
/// of the several "no usable version" paths broke.
fn assert_never_stored(dirs: &ProbeDirs, cache: &ToolHealthCache, ruleset: &RuleSet, why: &str) {
    assert!(probe_health(cache, HealthProof::Stored, ruleset).usable());
    assert!(probe_health(cache, HealthProof::Stored, ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF * PROBES_PER_TEST,
        "{why}: the fixtures must run on every probe"
    );
    assert!(
        !verdict_is_stored(cache, ruleset),
        "{why}: no verdict may be stored"
    );
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
        FIXTURE_RUNS_PER_PROOF * PROBES_PER_TEST,
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
        FIXTURE_RUNS_PER_PROOF * PROBES_PER_TEST,
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
        dirs.fixtures().join(format!("{PROBE_RULE}.pass.rs")),
        "fn still_clean() {}\n",
    )
    .expect("edit the pass fixture");
    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF * PROBES_PER_TEST,
        "a verdict proved against the old fixtures must not stand for the edited ones"
    );
}

#[test]
fn an_added_fixture_neighbour_proves_the_rule_again() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());
    std::fs::write(
        dirs.fixtures().join(NEIGHBOUR_FIXTURE),
        "fn neighbour() {}\n",
    )
    .expect("add a fixture neighbour");
    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF * PROBES_PER_TEST,
        "a fixture run reads the whole directory, so an added file must not stand under the old verdict"
    );
}

#[test]
fn a_deleted_fixture_neighbour_proves_the_rule_again() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let cache = ToolHealthCache::open(&dirs.workspace());
    let neighbour = dirs.fixtures().join(NEIGHBOUR_FIXTURE);
    std::fs::write(&neighbour, "fn neighbour() {}\n").expect("add a fixture neighbour");

    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());
    std::fs::remove_file(&neighbour).expect("delete the fixture neighbour");
    assert!(probe_health(&cache, HealthProof::Stored, &ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF * PROBES_PER_TEST,
        "a fixture run reads the whole directory, so a deleted file must not stand under the old verdict"
    );
}

#[test]
fn a_rule_that_reports_no_version_is_never_stored() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.versionless_ruleset();
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert_never_stored(&dirs, &cache, &ruleset, "a rule with no version command");
}

#[test]
fn a_version_command_that_fails_leaves_the_rule_unstored() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset_with_version("", Some(FAILING_VERSION_COMMAND.to_string()));
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert_never_stored(
        &dirs,
        &cache,
        &ruleset,
        "a version command that exits nonzero",
    );
}

#[test]
fn a_version_command_that_reports_nothing_leaves_the_rule_unstored() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset_with_version("", Some(SILENT_VERSION_COMMAND.to_string()));
    let cache = ToolHealthCache::open(&dirs.workspace());

    assert_never_stored(
        &dirs,
        &cache,
        &ruleset,
        "a version command that prints nothing",
    );
}

/// A fixture run that broke is never stored, so a rule that failed once is
/// proved again on the next check.
///
/// A `workspace`-scope rule runs a real build tool, which breaks for reasons
/// that say nothing about the tool or the rule: a lost build lock, a full
/// disk. A stored failure would put the rule on its prompt fallback until the
/// tool version or the rule content changed.
#[test]
fn a_broken_fixture_run_is_never_stored() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let cache = ToolHealthCache::open(&dirs.workspace());
    std::fs::write(dirs.break_file(), "").expect("break the tool");

    assert!(!probe_health(&cache, HealthProof::Stored, &ruleset).usable());
    assert!(!probe_health(&cache, HealthProof::Stored, &ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_BROKEN_PROOF * PROBES_PER_TEST,
        "a rule that broke its fixtures must be proved again on the next check"
    );
    assert!(
        !verdict_is_stored(&cache, &ruleset),
        "a broken fixture run must never stand as this rule's verdict"
    );
}

#[test]
fn doctor_proves_the_rule_again_and_drops_the_stored_verdict() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(ruleset.clone());
    let cache = ToolHealthCache::open(&dirs.workspace());

    // The engine proves the rule once and stores that it passed.
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

    // The engine now agrees with doctor, because doctor dropped the pass the
    // tool no longer earns and the engine proved the rule for itself.
    let runs_after_doctor = dirs.fixture_runs();
    assert!(
        !verdict_is_stored(&cache, &ruleset),
        "doctor must drop a verdict its own run did not earn"
    );
    assert!(
        !probe_health(&cache, HealthProof::Stored, &ruleset).usable(),
        "the engine must follow doctor rather than replay the old pass"
    );
    assert!(
        dirs.fixture_runs() > runs_after_doctor,
        "with no verdict standing, the engine must prove the rule for itself"
    );
}

/// A verdict doctor drops must not survive on the disk.
///
/// `sah doctor` runs in a process of its own, so its drop reaches the next
/// review only through the stored file. This test crosses that boundary:
/// each of the three steps opens its own cache and saves it, exactly as the
/// three processes do. A workspace that holds ONE verdict empties its map on
/// the drop, which is the case a save that returns early on an empty map
/// leaves untouched.
#[test]
fn a_saved_verdict_that_doctor_drops_does_not_survive_a_reopened_cache() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(ruleset.clone());

    // The review proves the rule, stores the pass, and saves it.
    let review = ToolHealthCache::open(&dirs.workspace());
    assert!(probe_health(&review, HealthProof::Stored, &ruleset).usable());
    review.save();

    // The tool breaks. Doctor reads the saved verdicts, proves the rule for
    // itself, drops the pass its own run did not earn, and saves.
    std::fs::write(dirs.break_file(), "").expect("break the tool");
    let doctor = ToolHealthCache::open(&dirs.workspace());
    let status = check_review_engine_with(&loader, &[], Some(&doctor));
    assert_eq!(status.tool_rules.len(), 1);
    assert!(
        !status.tool_rules[0].usable(),
        "sah doctor must run the fixtures whatever is stored"
    );
    doctor.save();

    // The next review reads the saved verdicts anew.
    let runs_before = dirs.fixture_runs();
    let next = ToolHealthCache::open(&dirs.workspace());
    assert!(
        !probe_health(&next, HealthProof::Stored, &ruleset).usable(),
        "a review that follows doctor must not replay the pass doctor dropped"
    );
    assert!(
        dirs.fixture_runs() > runs_before,
        "with no verdict on the disk, the review must prove the rule for itself"
    );
}

#[test]
fn opening_a_cache_creates_nothing_in_the_workspace() {
    let dirs = ProbeDirs::new();

    let _cache = ToolHealthCache::open(&dirs.workspace());

    assert!(
        !dirs.state_dir().exists(),
        "opening a cache must leave the reviewed tree as it found it"
    );
}

#[test]
fn a_cache_with_no_verdict_creates_nothing_in_the_workspace() {
    let dirs = ProbeDirs::new();
    let cache = ToolHealthCache::open(&dirs.workspace());

    cache.save();

    assert!(
        !dirs.state_dir().exists(),
        "a review that stored no verdict must write nothing into the reviewed tree"
    );
}

#[test]
fn a_stored_verdict_survives_a_reopened_cache() {
    let dirs = ProbeDirs::new();
    let ruleset = dirs.ruleset("");

    let first = ToolHealthCache::open(&dirs.workspace());
    assert!(probe_health(&first, HealthProof::Stored, &ruleset).usable());
    first.save();

    let second = ToolHealthCache::open(&dirs.workspace());
    assert!(probe_health(&second, HealthProof::Stored, &ruleset).usable());

    assert_eq!(
        dirs.fixture_runs(),
        FIXTURE_RUNS_PER_PROOF,
        "a later review must read the saved verdict rather than prove the rule again"
    );
}

#[test]
fn two_fixture_sets_that_share_one_byte_stream_digest_differently() {
    let dir = tempfile::tempdir().expect("fixture set");
    let short = dir.path().join(format!("{PROBE_RULE}.pass.rs"));
    let long = dir.path().join(format!("{PROBE_RULE}.pass.rsX"));

    std::fs::write(&short, "XY").expect("write the short name");
    let short_digest = fixture_digest(dir.path());
    std::fs::remove_file(&short).expect("remove the short name");
    std::fs::write(&long, "Y").expect("write the long name");
    let long_digest = fixture_digest(dir.path());

    assert_ne!(
        short_digest, long_digest,
        "a name that grows by the byte its content loses must not digest the same; \
         the doctor reads both names as this rule's pass fixture"
    );
}

/// The mode that takes every permission off a file.
#[cfg(unix)]
const NO_PERMISSIONS: u32 = 0o000;

#[cfg(unix)]
#[test]
fn an_unreadable_fixture_does_not_digest_as_an_empty_one() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("fixture set");
    let fixture = dir.path().join(format!("{PROBE_RULE}.pass.rs"));

    std::fs::write(&fixture, "").expect("write an empty fixture");
    let empty_digest = fixture_digest(dir.path());

    std::fs::write(&fixture, "fn clean() {}\n").expect("fill the fixture");
    std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(NO_PERMISSIONS))
        .expect("take the permissions off the fixture");
    let unreadable_digest = fixture_digest(dir.path());

    assert_ne!(
        empty_digest, unreadable_digest,
        "a fixture the digest cannot read must not stand for an empty one; \
         this test needs a user that cannot read a file of mode 0"
    );
}
