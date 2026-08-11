//! Tests for [tool-rule planning and execution](super).
//!
//! The tests are split by subject, one module for each. The review engine
//! renders a whole file into one agent prompt, and a file over the per-file
//! prompt cap is not reviewed at all, so a test tree this size has to be
//! several files rather than one.
//!
//! - [`plan`] — planning over built specs: what a healthy rule suppresses,
//!   what a missing tool falls back to, and which files a rule matches.
//! - [`execute`] — running the planned scripts, and the report types the run
//!   fills.
//! - [`preconditions`] — how a shipped-rule test states the tool it needs, and
//!   what it prints when the machine lacks it.
//! - [`shipped`] — each shipped tool rule driven end to end against its own
//!   language.
//!
//! This module carries what those four share: the imports, the shipped
//! tool-rule rosters, and the helpers that build a work-list and hold a run to
//! its finding.

mod execute;
mod plan;
mod preconditions;
mod shipped;

use super::*;

use crate::review::test_support::tool_rule_work;

/// A `files`-scope script that reports one `path:line: message` finding
/// per line containing `TODO`, and exits 0 whether or not it found any.
const TODO_SCRIPT: &str = r#"for f in "$@"; do awk -v f="$f" '/TODO/ { print f ":" NR ": TODO left in code" }' "$f"; done"#;

/// The builtin `code-hygiene` set, the one that carries the shipped
/// missing-docs tool rules.
const CODE_HYGIENE_SET: &str = "code-hygiene";

/// The prompt rule every shipped missing-docs tool rule supersedes.
const MISSING_DOCS_PROMPT_RULE: &str = "missing-docs";

/// The prompt rule that owns the length gate. The Rust complexity tool rule
/// supersedes it beside `cognitive-complexity`, and the Python one owns it
/// alone.
const FUNCTION_LENGTH_PROMPT_RULE: &str = "function-length";

/// The prompt rule that owns the branching gate.
const COGNITIVE_COMPLEXITY_PROMPT_RULE: &str = "cognitive-complexity";

/// What a missing-docs tool rule supersedes.
const SUPERSEDES_MISSING_DOCS: &[&str] = &[MISSING_DOCS_PROMPT_RULE];

/// What a dead-code tool rule supersedes.
const SUPERSEDES_DEAD_CODE: &[&str] = &[DEAD_CODE_PROMPT_RULE];

/// What a magic-numbers tool rule supersedes.
const SUPERSEDES_MAGIC_NUMBERS: &[&str] = &[MAGIC_NUMBERS_PROMPT_RULE];

/// What a tool rule that decides both complexity gates supersedes.
const SUPERSEDES_BOTH_COMPLEXITY_GATES: &[&str] = &[
    COGNITIVE_COMPLEXITY_PROMPT_RULE,
    FUNCTION_LENGTH_PROMPT_RULE,
];

/// What a tool rule that decides the branching gate alone supersedes.
const SUPERSEDES_COGNITIVE_COMPLEXITY: &[&str] = &[COGNITIVE_COMPLEXITY_PROMPT_RULE];

/// What a tool rule that decides the length gate alone supersedes.
const SUPERSEDES_FUNCTION_LENGTH: &[&str] = &[FUNCTION_LENGTH_PROMPT_RULE];

/// The shipped missing-docs tool rule for Rust, the one the pipeline
/// acceptance test drives end to end.
const RUST_MISSING_DOCS_RULE: &str = "missing-docs-rust";

/// The shipped missing-docs tool rule for Dart. `public_member_api_docs` reads
/// only a package's `lib/` directory, so the rule stages each changed file
/// under a probe `lib/` of its own. Two more acceptance tests drive it end to
/// end: one names every member its fail fixture leaves undocumented, and one
/// holds the probe's exclude list to the positions the project's own analyzer
/// reads.
const DART_MISSING_DOCS_RULE: &str = "missing-docs-dart";

/// Every shipped missing-docs tool rule, with the project type it serves
/// and the prompt rules it supersedes.
const SHIPPED_MISSING_DOCS_RULES: &[(&str, &str, &[&str])] = &[
    ("rust", RUST_MISSING_DOCS_RULE, SUPERSEDES_MISSING_DOCS),
    ("python", "missing-docs-python", SUPERSEDES_MISSING_DOCS),
    ("nodejs", "missing-docs-typescript", SUPERSEDES_MISSING_DOCS),
    ("go", "missing-docs-go", SUPERSEDES_MISSING_DOCS),
    ("swift", "missing-docs-swift", SUPERSEDES_MISSING_DOCS),
    ("flutter", DART_MISSING_DOCS_RULE, SUPERSEDES_MISSING_DOCS),
];

/// The prompt rule every shipped dead-code tool rule supersedes.
const DEAD_CODE_PROMPT_RULE: &str = "dead-code";

/// The shipped dead-code tool rule for Python, the one the pipeline
/// acceptance test drives end to end.
const PYTHON_DEAD_CODE_RULE: &str = "dead-code-python";

/// Every shipped dead-code tool rule, with the project type it serves.
///
/// Each supersedes the `dead-code` prompt rule for its language. Three of
/// that rule's four carve-outs are compiler behavior — an exported item, a
/// test, and an entry point are exempt because the compiler already sees
/// which callers exist and which cannot. The fourth, work-in-process
/// scaffolding, is an annotation contract: staged code carries the
/// language's own suppression marker with a reason, or it is dead.
const SHIPPED_DEAD_CODE_RULES: &[(&str, &str, &[&str])] = &[
    ("rust", "dead-code-rust", SUPERSEDES_DEAD_CODE),
    ("go", "unused-code-go", SUPERSEDES_DEAD_CODE),
    ("nodejs", "dead-code-typescript", SUPERSEDES_DEAD_CODE),
    ("python", PYTHON_DEAD_CODE_RULE, SUPERSEDES_DEAD_CODE),
    ("flutter", "dead-code-dart", SUPERSEDES_DEAD_CODE),
    ("swift", "dead-code-swift", SUPERSEDES_DEAD_CODE),
];

/// The prompt rule every shipped magic-numbers tool rule supersedes.
const MAGIC_NUMBERS_PROMPT_RULE: &str = "magic-numbers";

/// The shipped magic-numbers tool rule for Python. `ruff` exposes no value
/// allow-list, so a second acceptance test drives its fail fixture end to end
/// and names every literal the fixture holds unnamed.
const PYTHON_MAGIC_NUMBERS_RULE: &str = "magic-numbers-python";

/// The shipped magic-numbers tool rule for TypeScript and JavaScript. Its
/// value allow-list carries `100` and cannot carry a shift operand, so a
/// second acceptance test drives its fail fixture end to end and names every
/// literal the fixture holds unnamed.
const TYPESCRIPT_MAGIC_NUMBERS_RULE: &str = "magic-numbers-typescript";

/// The shipped magic-numbers tool rule for Go. Its value allow-list carries
/// `100` and cannot carry a shift operand, so a second acceptance test drives
/// its fail fixture end to end and names every literal the fixture holds
/// unnamed.
const GO_MAGIC_NUMBERS_RULE: &str = "magic-numbers-go";

/// The shipped magic-numbers tool rule for Swift. Its value allow-list carries
/// `100`, and `swiftlint` reads the shift OPERATOR, so this is the one rule of
/// the four that expresses the shift carve-out. A second acceptance test drives
/// its fail fixture end to end and names every line the fixture holds unnamed,
/// the edge of that carve-out included.
const SWIFT_MAGIC_NUMBERS_RULE: &str = "magic-numbers-swift";

/// The shipped magic-numbers tool rule for Dart. `solid_lints` 0.3.3 cannot
/// read its own value allow-list, so `100` reports, and a second acceptance
/// test drives its fail fixture end to end and names every line the fixture
/// holds unnamed.
const DART_MAGIC_NUMBERS_RULE: &str = "magic-numbers-dart";

/// Every shipped magic-numbers tool rule, with the project type it serves.
///
/// Rust is absent on purpose. The one Rust lint that reports an unnamed
/// literal is dylint's `unnamed_constant`, an unpublished example crate that
/// is built from a git checkout against a pinned nightly toolchain with
/// `rustc-dev`, so Rust keeps the `magic-numbers` prompt rule.
const SHIPPED_MAGIC_NUMBERS_RULES: &[(&str, &str, &[&str])] = &[
    (
        "python",
        PYTHON_MAGIC_NUMBERS_RULE,
        SUPERSEDES_MAGIC_NUMBERS,
    ),
    (
        "nodejs",
        TYPESCRIPT_MAGIC_NUMBERS_RULE,
        SUPERSEDES_MAGIC_NUMBERS,
    ),
    ("go", GO_MAGIC_NUMBERS_RULE, SUPERSEDES_MAGIC_NUMBERS),
    ("swift", SWIFT_MAGIC_NUMBERS_RULE, SUPERSEDES_MAGIC_NUMBERS),
    ("flutter", DART_MAGIC_NUMBERS_RULE, SUPERSEDES_MAGIC_NUMBERS),
];

/// The shipped complexity tool rule for Rust, the one the pipeline
/// acceptance test drives end to end.
const RUST_COMPLEXITY_RULE: &str = "complexity-rust";

/// The shipped complexity tool rule for TypeScript and JavaScript. It carries
/// the test carve-out both prompt rules state, so a second acceptance test
/// drives its fail fixture end to end and names every guard the fixture holds.
const TYPESCRIPT_COMPLEXITY_RULE: &str = "complexity-typescript";

/// Every shipped complexity tool rule, with the project type it serves and
/// the prompt rules it supersedes.
///
/// This is the one roster whose rows do not share a `supersedes` list. One
/// run decides both gates for Rust, TypeScript and Swift, so those rules
/// replace both prompt rules; Python and Go name one tool for each gate, so
/// each takes one rule for each. Dart keeps the `complexity` probe and both
/// prompt rules, because its only metrics tool is commercial.
const SHIPPED_COMPLEXITY_RULES: &[(&str, &str, &[&str])] = &[
    (
        "rust",
        RUST_COMPLEXITY_RULE,
        SUPERSEDES_BOTH_COMPLEXITY_GATES,
    ),
    (
        "python",
        "complexity-python",
        SUPERSEDES_COGNITIVE_COMPLEXITY,
    ),
    (
        "python",
        "function-length-python",
        SUPERSEDES_FUNCTION_LENGTH,
    ),
    (
        "nodejs",
        TYPESCRIPT_COMPLEXITY_RULE,
        SUPERSEDES_BOTH_COMPLEXITY_GATES,
    ),
    (
        "swift",
        "complexity-swift",
        SUPERSEDES_BOTH_COMPLEXITY_GATES,
    ),
    ("go", "complexity-go", SUPERSEDES_COGNITIVE_COMPLEXITY),
    ("go", "function-length-go", SUPERSEDES_FUNCTION_LENGTH),
];

/// The builtin `manifests` set, the one that matches dependency manifests
/// rather than source code.
const MANIFESTS_SET: &str = "manifests";

/// The shipped unused-dependency tool rule for Rust, the one the pipeline
/// acceptance test drives end to end.
const RUST_UNUSED_DEPENDENCIES_RULE: &str = "unused-dependencies-rust";

/// What an unused-dependency tool rule supersedes: nothing.
///
/// No shipped prompt rule asks whether a declared dependency is used, so
/// this group replaces no rule and degrades to no rule. A machine without
/// `cargo machete` gets no answer to the question rather than a worse one.
const SUPERSEDES_NOTHING: &[&str] = &[];

/// The name [`verify_shipped_tool_rules_pass_fixtures`] puts in its failure
/// messages for this group. Every other group is named for the prompt rule
/// it replaces; this one replaces none, so it is named for its own concern.
const UNUSED_DEPENDENCIES_RULE_KIND: &str = "unused-dependency";

/// Every shipped unused-dependency tool rule, with the project type it
/// serves and the prompt rules it supersedes.
const SHIPPED_UNUSED_DEPENDENCY_RULES: &[(&str, &str, &[&str])] =
    &[("rust", RUST_UNUSED_DEPENDENCIES_RULE, SUPERSEDES_NOTHING)];

/// A cargo package holding one undocumented public item and one documented
/// one. `[workspace]` keeps cargo inside the temporary directory.
const UNDOCUMENTED_PACKAGE_MANIFEST: &str = concat!(
    "[package]\nname = \"undocumented-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    "\n[workspace]\n",
);

/// The library of [`UNDOCUMENTED_PACKAGE_MANIFEST`]. The undocumented
/// struct is the finding the Rust tool rule must report.
const UNDOCUMENTED_LIB_RS: &str = concat!(
    "//! A probe crate for the shipped Rust missing-docs tool rule.\n\n",
    "/// A documented public struct.\n",
    "pub struct Documented;\n\n",
    "pub struct Undocumented;\n",
);

/// The library path inside the probe package, as the work-list holds it.
const UNDOCUMENTED_LIB_PATH: &str = "src/lib.rs";

/// A one-validator work-list over `files` for the builtin `code-hygiene`
/// set, naming both the prompt rule and the Rust tool rule.
fn code_hygiene_work(files: &[&str]) -> WorkList {
    tool_rule_work(
        "an undocumented public item",
        CODE_HYGIENE_SET,
        [
            MISSING_DOCS_PROMPT_RULE.to_string(),
            RUST_MISSING_DOCS_RULE.to_string(),
        ],
        files.iter().map(|path| (*path, UNDOCUMENTED_LIB_RS)),
    )
}

/// Executes `run` over `repo_root` and holds it to the report contract every
/// shipped tool rule keeps: the pipeline breaks nothing, and it reports
/// exactly one finding in `path` — confirmed, attributed to `set` and to
/// `rule`, carrying `claim_fragment` of the tool's own message.
///
/// This is the half every shipped-rule acceptance test shares. The half
/// above it — the probe repository, the work-list, and what the plan must
/// suppress — differs per rule and stays in the test.
fn verify_run_reports_one_finding(
    run: &ToolRun,
    repo_root: &Path,
    path: &str,
    set: &str,
    rule: &str,
    claim_fragment: &str,
) {
    let outcome = execute_tool_runs(std::slice::from_ref(run), repo_root, None);

    assert!(
        outcome.errors().is_empty(),
        "the shipped pipeline must not break; errors: {:?}",
        outcome.errors()
    );
    let findings: Vec<&VerifiedFinding> = outcome
        .findings()
        .iter()
        .filter(|verified| verified.finding.file == path)
        .collect();
    assert_eq!(
        findings.len(),
        1,
        "exactly one finding must be reported in {path}; got {:?}",
        outcome.findings()
    );
    assert!(findings[0].confirmed);
    assert_eq!(findings[0].finding.validator, set);
    assert_eq!(findings[0].finding.rule.as_deref(), Some(rule));
    assert!(
        findings[0].finding.claim.contains(claim_fragment),
        "the claim must be the tool's message carrying '{claim_fragment}'; got '{}'",
        findings[0].finding.claim
    );
}
