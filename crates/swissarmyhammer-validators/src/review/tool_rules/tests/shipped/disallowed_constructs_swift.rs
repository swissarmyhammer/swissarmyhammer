//! Acceptance tests for the shipped `disallowed-constructs-swift` tool rule.
//!
//! Each test drives the SHIPPED script over a probe repository and reads what
//! the real swiftlint reported.
//!
//! Three answers stand apart from every other Swift rule of this set, and one
//! test holds each. Three of the twelve rules stay off inside a test target,
//! and swiftlint carries no per-rule path filter for a stock rule, so the
//! SCRIPT partitions its own paths — from the `Tests/` convention and from the
//! test targets the package manifest names. And each of the three custom regex
//! rules matches a token of the KIND its `match_kinds` names, so a comment and
//! a string literal holding the same words stay silent.
//!
//! One more test stands apart because of what it protects rather than what it
//! measures. This gate took five bullets out of the Swift prompt rules, and a
//! bullet deleted without a tool that reads it is a requirement the set states
//! nowhere. The last test in this file holds each of those five to being
//! reported by its rule.

use super::*;

/// Acceptance: every shipped disallowed-construct tool rule passes its fixture
/// pair in doctor, and supersedes nothing.
///
/// The pass fixture is the load-bearing half. It holds the same declarations
/// the fail fixture holds, each written in the form its rule asks for, and two
/// of them behind the inline directive instead — so a swiftlint release that
/// changed what one rule asks for, or that stopped honouring the directive,
/// makes the pair fail and takes the rule out of the review rather than moving
/// a finding without a word.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_disallowed_constructs_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(
        SHIPPED_DISALLOWED_CONSTRUCTS_RULES,
        DISALLOWED_CONSTRUCTS_RULE_KIND,
    );
}

/// The shipped failing fixture, as [`copy_shipped_fixture`] asks for it.
const SWIFT_DISALLOWED_FAIL_FIXTURE: &str = "disallowed-constructs-swift.fail.swift";

/// Where a probe stages the failing fixture.
const SWIFT_DISALLOWED_JUDGED_PATH: &str = "Judged.swift";

/// Every rule the shipped failing fixture reports.
///
/// The fixture holds one declaration for each of the twelve rules the gate
/// enables, so the run reports each name exactly one time. Measured with the
/// shipped script and swiftlint 0.65.0: 12 findings, exit 0, and 0 bytes on
/// stderr.
const SWIFT_DISALLOWED_FAIL_FIXTURE_RULES: &[&str] = &[
    "implicitly_unwrapped_optional",
    "force_unwrapping",
    "force_try",
    "force_cast",
    "unused_optional_binding",
    "unowned_variable_capture",
    "legacy_constructor",
    "legacy_nsgeometry_functions",
    "legacy_cggeometry_functions",
    "no_direct_standard_out_logs",
    "no_file_literal",
    "no_unchecked_sendable",
];

/// Drives the shipped script over the failing fixture, and answers the rule
/// name of each finding it reported.
///
/// The fixture is staged through `prepare` so the run reads the bytes the set
/// ships. A test that wrote its own copy would answer for the copy.
fn swift_disallowed_fixture_rules() -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(
        &loader,
        SWIFT_PROJECT_TYPES,
        SWIFT_DISALLOWED_CONSTRUCTS_RULE,
    );

    let stage_fixture = |repo: &Path| {
        copy_shipped_fixture(
            &loader,
            repo,
            SWIFT_DISALLOWED_FAIL_FIXTURE,
            SWIFT_DISALLOWED_JUDGED_PATH,
        );
    };
    let staging = ShippedStaging {
        staged: NO_SUPPORT_FILES,
        links: NO_PROBE_LINKS,
        outside: NO_PROBE_OUTSIDE,
        prepare: &stage_fixture,
        restore: &stage_nothing_more,
    };

    let run = drive_shipped_script_whole(
        &loader,
        SWIFT_DISALLOWED_CONSTRUCTS_RULE,
        &staging,
        &[SWIFT_DISALLOWED_JUDGED_PATH],
    );
    let outcome = run
        .outcome
        .expect("the shipped Swift disallowed-constructs script must judge the fixture and exit 0");

    finding_rule_names(&outcome, &run.repo_root)
}

/// Acceptance: the shipped failing fixture reports every one of the twelve
/// rules the gate enables, and reports each one time.
///
/// swiftlint keeps linting past a name it does not know, so the roster guard
/// above cannot say that an enabled rule still REPORTS. This says it: a
/// release that stops reporting one construct fails here by name, and the
/// doctor then marks the rule unusable through the same fixture.
#[test]
fn the_shipped_swift_disallowed_constructs_tool_rule_reports_every_enabled_rule() {
    let reported = swift_disallowed_fixture_rules();

    for rule in SWIFT_DISALLOWED_FAIL_FIXTURE_RULES {
        assert_eq!(
            reported.iter().filter(|name| *name == rule).count(),
            1,
            "the shipped failing fixture holds one declaration for `{rule}`, so the run \
             must report it exactly one time; it reported {reported:?}"
        );
    }
    assert_eq!(
        reported.len(),
        SWIFT_DISALLOWED_FAIL_FIXTURE_RULES.len(),
        "the failing fixture must report the twelve enabled rules and nothing else; \
         it reported {reported:?}"
    );
}

/// Where a probe stages the Swift file it measures one rule over.
const SWIFT_DISALLOWED_PROBE_PATH: &str = "Probe.swift";

/// Drives the shipped script over `source` staged at
/// [`SWIFT_DISALLOWED_PROBE_PATH`], and answers the rule name of each finding
/// it reported.
///
/// The probe stages no support file. swiftlint reads no `.swift-version`, so
/// staging one beside the probe would state a condition this gate never reads.
fn swift_disallowed_reporting_rules(source: &str) -> Vec<String> {
    swift_gate_reporting_rules(
        SWIFT_DISALLOWED_CONSTRUCTS_RULE,
        SWIFT_DISALLOWED_PROBE_PATH,
        source,
        NO_SUPPORT_FILES,
    )
}

/// The rule that reports an `@unchecked Sendable` conformance.
const SWIFT_UNCHECKED_SENDABLE_RULE: &str = "no_unchecked_sendable";

/// A type that declares `@unchecked Sendable` and states no invariant.
const SWIFT_UNCHECKED_SENDABLE_PLAIN: &str = concat!(
    "public final class Box: @unchecked Sendable {\n",
    "    public let value: Int = 0\n",
    "}\n",
);

/// The same type behind the directive, with the invariant after it.
///
/// The two constants differ in that one line and in nothing else, so the
/// directive is the only thing the test below can be measuring. The test says
/// so of the constants themselves before it drives either one.
const SWIFT_UNCHECKED_SENDABLE_ANNOTATED: &str = concat!(
    "// swiftlint:disable:next no_unchecked_sendable  the value is a `let` of a value \
     type, so no write can race a read\n",
    "public final class Box: @unchecked Sendable {\n",
    "    public let value: Int = 0\n",
    "}\n",
);

/// Acceptance: the annotation IS the documentation, and the shipped rule
/// honours it.
///
/// `concurrency.md` states the `@unchecked Sendable` requirement as pure
/// judgment — the smell is the ABSENCE of a lock and a comment. No tool can
/// answer "is there a documented invariant". A tool CAN require an explicit
/// annotation, and the text of that annotation is the invariant, which is what
/// the rule's own message asks the author to write.
///
/// The two halves are both load-bearing. Without the directive the conformance
/// reports, and with it the same bytes report nothing, so the escape hatch is
/// the only difference between the two runs. A swiftlint release that stopped
/// honouring the directive turns every documented exception back into a
/// finding no edit can satisfy.
#[test]
fn the_shipped_swift_disallowed_constructs_tool_rule_reads_the_unchecked_sendable_directive() {
    assert!(
        SWIFT_UNCHECKED_SENDABLE_ANNOTATED.ends_with(SWIFT_UNCHECKED_SENDABLE_PLAIN),
        "the annotated probe must be the plain one with the directive written above it \
         and nothing else changed, or the two runs below differ in more than the \
         directive"
    );

    let plain = swift_disallowed_reporting_rules(SWIFT_UNCHECKED_SENDABLE_PLAIN);
    assert!(
        plain.contains(&SWIFT_UNCHECKED_SENDABLE_RULE.to_string()),
        "`{SWIFT_UNCHECKED_SENDABLE_RULE}` must report a conformance that states no \
         invariant; the run reported {plain:?}"
    );

    let annotated = swift_disallowed_reporting_rules(SWIFT_UNCHECKED_SENDABLE_ANNOTATED);
    assert!(
        annotated.is_empty(),
        "the same conformance under `// swiftlint:disable:next \
         {SWIFT_UNCHECKED_SENDABLE_RULE}` must report nothing, because the text after the \
         directive IS the documented invariant; the run reported {annotated:?}"
    );
}

/// The rule that reports a call writing to standard out.
const SWIFT_STANDARD_OUT_RULE: &str = "no_direct_standard_out_logs";

/// One real `print` call, beside the same word inside a string literal, inside
/// a line comment and inside a doc comment.
///
/// `match_kinds: [identifier]` is what tells the four apart. Measured with the
/// same regex and no `match_kinds`: all four report.
const SWIFT_STANDARD_OUT_PROBE: &str = concat!(
    "public func announce(_ message: String) {\n",
    "    print(message)\n",
    "    let quoted = \"print(this is a string, not a call)\"\n",
    "    // print(this is a comment)\n",
    "    _ = quoted\n",
    "}\n\n",
    "/// print(this is a doc comment)\n",
    "public func documented() {}\n",
);

/// The rule that reports the `#file` literal.
const SWIFT_FILE_LITERAL_RULE: &str = "no_file_literal";

/// One real `#file` literal, beside the same word inside a string literal,
/// inside a line comment and inside a doc comment.
///
/// `match_kinds: [keyword]` is what tells the four apart, and it is a DIFFERENT
/// kind from the one the standard-out rule names — which is the whole reason
/// this rule needs a probe of its own rather than the standard-out probe's
/// answer taken on trust. Measured with the same regex and no `match_kinds`:
/// all four report.
const SWIFT_FILE_LITERAL_PROBE: &str = concat!(
    "public func origin(path: StaticString = #file) -> StaticString {\n",
    "    let quoted = \"#file is a string, not a literal\"\n",
    "    // #file is a comment\n",
    "    _ = quoted\n",
    "    return path\n",
    "}\n\n",
    "/// #file is a doc comment\n",
    "public func documented() {}\n",
);

/// One real `@unchecked Sendable` conformance, beside the same words inside a
/// string literal, inside a line comment and inside a doc comment.
///
/// `match_kinds: [attribute.builtin, typeidentifier]` is what tells the four
/// apart, and it names TWO kinds because the construct spans two tokens.
/// Measured with the same regex and no `match_kinds`: all four report.
const SWIFT_UNCHECKED_SENDABLE_PROBE: &str = concat!(
    "public final class Box: @unchecked Sendable {\n",
    "    public let note = \"@unchecked Sendable is a string, not a conformance\"\n",
    "    // @unchecked Sendable is a comment\n",
    "}\n\n",
    "/// @unchecked Sendable is a doc comment\n",
    "public struct Plain {}\n",
);

/// Each custom regex rule, with the probe that holds its construct in the four
/// positions.
///
/// Each probe holds ONE real construct and three copies of the same words —
/// one in a string literal, one in a line comment and one in a doc comment —
/// so a run over it reports its rule one time and nothing else.
const SWIFT_CUSTOM_RULE_PROBES: &[(&str, &str)] = &[
    (SWIFT_STANDARD_OUT_RULE, SWIFT_STANDARD_OUT_PROBE),
    (SWIFT_FILE_LITERAL_RULE, SWIFT_FILE_LITERAL_PROBE),
    (
        SWIFT_UNCHECKED_SENDABLE_RULE,
        SWIFT_UNCHECKED_SENDABLE_PROBE,
    ),
];

/// Acceptance: each custom regex rule matches a token of the kind its
/// `match_kinds` names, and never a comment and never a string literal.
///
/// A regex over source text reads all four positions alike. `match_kinds` is
/// swiftlint's own answer, and no reading of the regex could replace it — the
/// same words stand in each of the four positions. Measured over the three
/// probes with the same regexes and every `match_kinds` line dropped: each
/// rule reports four times rather than one.
///
/// The three rules name three DIFFERENT kinds, so one rule's answer says
/// nothing about the other two, and each carries a probe of its own.
#[test]
fn the_shipped_swift_disallowed_constructs_tool_rule_reads_no_comment_and_no_string_literal() {
    for (rule, probe) in SWIFT_CUSTOM_RULE_PROBES {
        let reported = swift_disallowed_reporting_rules(probe);
        assert_eq!(
            reported,
            vec![(*rule).to_string()],
            "`{rule}` must report the one real construct and none of the three copies of \
             the same words in a string literal, a line comment and a doc comment; the \
             run reported {reported:?}"
        );
    }
}

/// Each rule the gate keeps out of a test target, with the text the line its
/// construct stands on opens with.
///
/// The second half of each pair is what [`declaration_line`] turns into the
/// line the run must report, so no line number is written here as a number an
/// edit to the source below would move.
const SWIFT_FORCE_CONSTRUCTS: &[(&str, &str)] = &[
    ("force_unwrapping", "return value!"),
    ("force_try", "return try! make()"),
    ("force_cast", "return value as! Int"),
];

/// One file holding all three force constructs, which the three force rules
/// report in a product target and never inside a test target.
///
/// The three stand in ONE file so a single run answers for all three, and so
/// the product copy and the test copy of the file hold identical bytes.
const SWIFT_FORCE_CONSTRUCTS_SOURCE: &str = concat!(
    "public func unwrap(_ value: Int?) -> Int {\n",
    "    return value!\n",
    "}\n\n",
    "public func attempt(_ make: () throws -> Int) -> Int {\n",
    "    return try! make()\n",
    "}\n\n",
    "public func convert(_ value: Any) -> Int {\n",
    "    return value as! Int\n",
    "}\n",
);

/// Where a probe stages the product-target copy of those bytes.
const SWIFT_PRODUCT_TARGET_PATH: &str = "Sources/Probe/Forced.swift";

/// Where a probe stages the test-target copy of the SAME bytes, under the
/// `Tests/` directory SPM lays a package out with.
const SWIFT_TESTS_DIRECTORY_PATH: &str = "Tests/ProbeTests/Forced.swift";

/// Where a probe stages a third copy, under a test target the package manifest
/// names at a path no convention would find.
const SWIFT_MANIFEST_TARGET_PATH: &str = "Custom/OddTests/Forced.swift";

/// The manifest of the probe package.
///
/// It declares one product target and two test targets, the second of them at
/// an explicit `path:` outside `Tests/`. That second target is the whole
/// reason the run reads the manifest at all.
const SWIFT_PROBE_MANIFEST: &str = concat!(
    "// swift-tools-version:5.9\n",
    "import PackageDescription\n\n",
    "let package = Package(\n",
    "    name: \"Probe\",\n",
    "    targets: [\n",
    "        .target(name: \"Probe\"),\n",
    "        .testTarget(name: \"ProbeTests\", dependencies: [\"Probe\"]),\n",
    "        .testTarget(name: \"OddTests\", dependencies: [\"Probe\"], path: \"Custom/OddTests\"),\n",
    "    ]\n",
    ")\n",
);

/// Where a package states its manifest.
const SWIFT_PACKAGE_MANIFEST_PATH: &str = "Package.swift";

/// What a probe of the partition writes between a finding's `path:line` row
/// and the name of the rule that placed it.
const SWIFT_FORCE_ROW_SEPARATOR: &str = " ";

/// Each finding of `outcome` as the `path:line` row the work-list holds, with
/// the name of the rule that placed it after it.
///
/// A probe of the test-target partition must say WHICH rule placed each row.
/// The partition drops three rules by name and keeps the other nine, so a row
/// naming a path and a line alone could not tell a rule the partition dropped
/// from a rule that never fired.
fn finding_rule_rows(outcome: &ScriptOutcome, repo_root: &Path) -> Vec<String> {
    outcome
        .findings
        .iter()
        .map(|finding| {
            let rule = finding
                .claim
                .split_once(TOOL_CLAIM_SEPARATOR)
                .map_or_else(|| finding.claim.clone(), |(name, _)| name.to_string());
            format!(
                "{}:{}{SWIFT_FORCE_ROW_SEPARATOR}{rule}",
                normalize_tool_path(&finding.file, repo_root),
                finding.line
            )
        })
        .collect()
}

/// Every row a run must report for [`SWIFT_FORCE_CONSTRUCTS_SOURCE`] staged at
/// `path` with no carve-out over it: one for each of the three constructs.
fn swift_force_rows_of(path: &str) -> Vec<String> {
    SWIFT_FORCE_CONSTRUCTS
        .iter()
        .map(|(rule, construct)| {
            format!(
                "{path}:{}{SWIFT_FORCE_ROW_SEPARATOR}{rule}",
                declaration_line(SWIFT_FORCE_CONSTRUCTS_SOURCE, construct)
            )
        })
        .collect()
}

/// Drives the shipped script over `files`, staged as `staged` states, and
/// answers each finding as the `path:line` row the work-list holds with the
/// name of the rule that placed it after it.
///
/// The rows come back in swiftlint's own order, which groups a file's findings
/// by rule rather than by line — measured over one file holding all three
/// force constructs, the report reads `force_cast`, `force_try`,
/// `force_unwrapping` while their lines run 10, 6, 2. So each caller sorts
/// both sides rather than asserting that grouping.
fn swift_disallowed_force_rows(staged: &[(&str, &str)], files: &[&str]) -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(
        &loader,
        SWIFT_PROJECT_TYPES,
        SWIFT_DISALLOWED_CONSTRUCTS_RULE,
    );

    drive_shipped_script(
        &loader,
        SWIFT_DISALLOWED_CONSTRUCTS_RULE,
        &ShippedStaging::of(staged),
        files,
        finding_rule_rows,
    )
    .expect("the shipped Swift disallowed-constructs script must judge the probe files")
}

/// Acceptance: each of the three force constructs reports in a product target
/// and stays silent in a test target, for the SAME bytes.
///
/// The two bullets the gate took each say "in non-test code" and mean it,
/// and swiftlint carries no per-rule path filter for a stock rule — measured on
/// 0.65.0, an `excluded:` key under `force_unwrapping:` is answered with
/// `warning: Configuration for 'force_unwrapping' rule contains the invalid
/// key(s) 'excluded'.` and both files still report. So the SCRIPT partitions
/// its own paths.
///
/// All three rules stand in the run rather than one of them. The partition is
/// the script's own list, so a rule dropped from that list, and a rule the
/// `jq` filter reads differently from its neighbours, is visible only where
/// every named rule is measured.
///
/// The two halves are both load-bearing. The product path must report all
/// three, or the partition swallowed a real finding; the test path must stay
/// silent for all three, or the carve-out the prompt rules state was never
/// honoured. The two files hold identical bytes, so the PATH is the only
/// difference between them.
#[test]
fn the_shipped_swift_disallowed_constructs_tool_rule_keeps_the_force_rules_out_of_a_test_target() {
    let rows = swift_disallowed_force_rows(
        &[
            (SWIFT_PRODUCT_TARGET_PATH, SWIFT_FORCE_CONSTRUCTS_SOURCE),
            (SWIFT_TESTS_DIRECTORY_PATH, SWIFT_FORCE_CONSTRUCTS_SOURCE),
        ],
        &[SWIFT_PRODUCT_TARGET_PATH, SWIFT_TESTS_DIRECTORY_PATH],
    );

    assert_eq!(
        sorted_names(&rows),
        sorted_names(&swift_force_rows_of(SWIFT_PRODUCT_TARGET_PATH)),
        "each of the three constructs under `{SWIFT_PRODUCT_TARGET_PATH}` must report, \
         and the same bytes under `{SWIFT_TESTS_DIRECTORY_PATH}` must not; the run \
         reported {rows:?}"
    );
}

/// Acceptance: the run reads the test targets the package manifest names, so a
/// test target outside `Tests/` keeps the carve-out too.
///
/// The `Tests/` convention answers SPM's default layout and nothing else. A
/// package is free to state an explicit `path:`, and `swift package describe`
/// is the fact that names it — measured over this manifest, it reports
/// `Tests/ProbeTests` and `Custom/OddTests`.
///
/// The product path stands in the same run, so the test says the manifest
/// widened the carve-out rather than switched the gate off. All three
/// constructs stand in each copy, so the manifest's answer is measured for
/// every rule the carve-out names rather than for one of them.
///
/// The probe stages a file under `Tests/ProbeTests` as well, and names it in
/// no argument list. SPM refuses to describe a package whose declared target
/// directory holds no source — measured, `swift package describe` then exits 1
/// with `error: Source files for target ProbeTests should be located under
/// 'Tests/ProbeTests'`, and the run falls back to the convention, which is the
/// answer this test must not be given for free.
#[test]
fn the_shipped_swift_disallowed_constructs_tool_rule_reads_a_test_target_the_manifest_names() {
    let rows = swift_disallowed_force_rows(
        &[
            (SWIFT_PACKAGE_MANIFEST_PATH, SWIFT_PROBE_MANIFEST),
            (SWIFT_PRODUCT_TARGET_PATH, SWIFT_FORCE_CONSTRUCTS_SOURCE),
            (SWIFT_TESTS_DIRECTORY_PATH, SWIFT_FORCE_CONSTRUCTS_SOURCE),
            (SWIFT_MANIFEST_TARGET_PATH, SWIFT_FORCE_CONSTRUCTS_SOURCE),
        ],
        &[SWIFT_PRODUCT_TARGET_PATH, SWIFT_MANIFEST_TARGET_PATH],
    );

    assert_eq!(
        sorted_names(&rows),
        sorted_names(&swift_force_rows_of(SWIFT_PRODUCT_TARGET_PATH)),
        "the manifest names `{SWIFT_MANIFEST_TARGET_PATH}`'s directory as a test target, \
         so all three constructs there must stay silent while the three under \
         `{SWIFT_PRODUCT_TARGET_PATH}` report; the run reported {rows:?}"
    );
}

/// A force unwrap of an optional, in code no test target holds.
const SWIFT_DISALLOWED_FORCE_UNWRAP: &str = concat!(
    "public enum Unwrapping {\n",
    "    public static func read(_ name: String?) -> String {\n",
    "        name!\n",
    "    }\n",
    "}\n",
);

/// A stored property declared as an implicitly unwrapped optional.
const SWIFT_DISALLOWED_IMPLICITLY_UNWRAPPED: &str = concat!(
    "public final class Holder {\n",
    "    public var name: String!\n",
    "\n",
    "    public init() {}\n",
    "}\n",
);

/// A `try!` on a call that can fail, in code no test target holds.
const SWIFT_DISALLOWED_FORCE_TRY: &str = concat!(
    "public enum Trying {\n",
    "    public static func read(_ body: () throws -> Int) -> Int {\n",
    "        try! body()\n",
    "    }\n",
    "}\n",
);

/// An `as!` force cast, in code no test target holds.
const SWIFT_DISALLOWED_FORCE_CAST: &str = concat!(
    "public enum Casting {\n",
    "    public static func read(_ value: Any) -> Int {\n",
    "        value as! Int\n",
    "    }\n",
    "}\n",
);

/// Every bullet this gate took out of a Swift prompt rule.
///
/// The `@unchecked Sendable` row takes [`SWIFT_UNCHECKED_SENDABLE_PLAIN`],
/// which the directive test above already drives, so one probe carries both
/// answers: the conformance reports, and the same bytes behind the directive
/// do not.
///
/// The gate reads a test target for the three force rules, and each probe here
/// stands at the repository root, which no test target holds. The carve-out
/// itself is measured by the two test-target tests below.
const SWIFT_DISALLOWED_SUPERSEDED_BULLETS: &[SupersededSwiftBullet] = &[
    SupersededSwiftBullet {
        prompt_rule: SWIFT_OPTIONALS_PROMPT_RULE,
        tool_rule: "force_unwrapping",
        defect: SWIFT_DISALLOWED_FORCE_UNWRAP,
        words: "No force unwrap (`!`) in non-test code.",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_OPTIONALS_PROMPT_RULE,
        tool_rule: "implicitly_unwrapped_optional",
        defect: SWIFT_DISALLOWED_IMPLICITLY_UNWRAPPED,
        words: "No implicitly unwrapped optionals (`Type!`).",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_ERROR_HANDLING_PROMPT_RULE,
        tool_rule: "force_try",
        defect: SWIFT_DISALLOWED_FORCE_TRY,
        words: "No `try!` in non-test code.",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_ERROR_HANDLING_PROMPT_RULE,
        tool_rule: "force_cast",
        defect: SWIFT_DISALLOWED_FORCE_CAST,
        words: "No `as!` force-cast in non-test code.",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_CONCURRENCY_PROMPT_RULE,
        tool_rule: SWIFT_UNCHECKED_SENDABLE_RULE,
        defect: SWIFT_UNCHECKED_SENDABLE_PLAIN,
        words: "`@unchecked Sendable` requires a documented synchronization invariant.",
    },
];

/// Acceptance: this gate is the ONE owner of every bullet it took out of a
/// Swift prompt rule.
///
/// A bullet deleted from the prompt text without a tool that reads it is a
/// requirement the set states nowhere, and nothing downstream reports the hole.
/// A bullet the prompt rule states beside a tool that decides it is two owners,
/// which produce churn on every review round. This test is what stands between
/// the deletion and each of those.
///
/// Every probe holds ONE defect, so the rule the run names is the rule that
/// read the shape rather than a neighbour that read the same file.
#[test]
fn the_shipped_swift_disallowed_constructs_tool_rule_owns_each_bullet_it_took() {
    for bullet in SWIFT_DISALLOWED_SUPERSEDED_BULLETS {
        let reported = swift_disallowed_reporting_rules(bullet.defect);
        verify_superseded_swift_bullet(bullet, &reported);
    }
}
