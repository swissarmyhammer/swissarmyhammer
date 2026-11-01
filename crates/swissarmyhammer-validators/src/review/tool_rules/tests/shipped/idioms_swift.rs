//! Acceptance tests for the shipped `idioms-swift` tool rule.
//!
//! Each test drives the SHIPPED script over a probe repository and reads what
//! the real `swift format` reported.
//!
//! Three answers stand apart from every other Swift rule of this set, and one
//! test holds each. `swift format lint` writes NINE tags that no key of the
//! configuration can stop, so the script reads the tag off each output line
//! and keeps only a tag its own ALLOWLIST names. A path that holds no file
//! exits 0 and writes NOTHING, so the script tests each path before the tool
//! starts. And a path that names a DIRECTORY costs a shared run every finding
//! it made, so the script hands `swift format` one path for each run.
//!
//! One more test stands apart because of what it protects rather than what it
//! measures. This gate took seven bullets out of the Swift prompt rules, and a
//! requirement deleted without a tool that reads it is one the set states
//! nowhere. `the_shipped_swift_idioms_tool_rule_owns_each_bullet_it_took`
//! holds each of the seven to being reported by its tool, over Swift written in
//! the SHAPE the bullet named.

use super::*;

/// Acceptance: every shipped idioms tool rule passes its fixture pair in
/// doctor, and supersedes nothing.
///
/// The pass fixture is the load-bearing half. It holds the same declarations
/// the fail fixture holds, each written in the form its rule asks for, so a
/// SwiftFormat release that changed what one rule asks for makes the pair
/// fail and takes the rule out of the review rather than moving a finding
/// without a word.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_idioms_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_IDIOMS_RULES, IDIOMS_RULE_KIND);
}

/// How many rule tags the shipped allowlist holds.
///
/// Seven, which is the part of the toolchain's fixed set of 43 rules that
/// decides an IDIOM. The count is the assertion that a tag added or dropped
/// later reaches this guard rather than moving the gate without a word.
const SWIFT_IDIOMS_ALLOWLIST_SIZE: usize = 7;

/// The rule that rewrites a `for` loop's nested `if` into a `where` clause.
///
/// The gate leaves it OFF: the tool cannot tell a loop that acts from a loop
/// that builds a collection.
const SWIFT_IDIOMS_WHERE_CLAUSE_RULE: &str = "UseWhereClausesInForLoops";

/// A `for` loop that filters with a nested `if`.
///
/// [`SWIFT_IDIOMS_WHERE_CLAUSE_RULE`] reports this shape when it is ON.
const SWIFT_IDIOMS_NESTED_IF_FOR_LOOP: &str = concat!(
    "public enum Nesting {\n",
    "    public static func walk(_ things: [Int]) {\n",
    "        for thing in things { if thing > 2 { print(thing) } }\n",
    "    }\n",
    "}\n",
);

/// Acceptance: the gate leaves `UseWhereClausesInForLoops` OFF.
///
/// Measured with that rule ON, it reports the nested `if` of the probe below.
/// The gate must stay silent over the probe, which holds the rule OFF.
#[test]
fn the_shipped_swift_idioms_tool_rule_leaves_the_where_clause_rule_off() {
    let reported = swift_idioms_reporting_rules(SWIFT_IDIOMS_NESTED_IF_FOR_LOOP, NO_SUPPORT_FILES);
    assert!(
        !reported.contains(&SWIFT_IDIOMS_WHERE_CLAUSE_RULE.to_string()),
        "the gate must stay silent over a `for` loop that filters with a nested `if`; \
         the run reported {reported:?}"
    );
}

/// The probe repository stages no file as a `(path, text)` pair.
///
/// Each probe below stages the shipped fixture through `prepare` instead, so
/// the run measures the SHIPPED bytes rather than a copy this file wrote.
const NO_STAGED_TEXT: &[(&str, &str)] = &[];

/// The shipped failing fixture, as [`copy_shipped_fixture`] asks for it.
const SWIFT_IDIOMS_FAIL_FIXTURE: &str = "idioms-swift.fail.swift";

/// Where a probe stages the failing fixture.
const SWIFT_IDIOMS_JUDGED_PATH: &str = "Judged.swift";

/// A path the probe repository holds no file at.
const SWIFT_IDIOMS_ABSENT_PATH: &str = "Absent.swift";

/// How many findings the shipped failing fixture reports.
///
/// Measured with the shipped script and Apple Swift 6.4: 11 findings, at exit
/// 0, carrying all seven rules of the allowlist. The count states that a
/// refusing path beside the fixture costs the run nothing, because the run
/// reports the same 11 either way.
const SWIFT_IDIOMS_FAIL_FIXTURE_FINDINGS: usize = 11;

/// How many rules the findings of the shipped failing fixture carry.
///
/// All seven of the allowlist, so the fixture measures each rule the gate
/// keeps rather than a part of them.
const SWIFT_IDIOMS_FAIL_FIXTURE_RULES: usize = SWIFT_IDIOMS_ALLOWLIST_SIZE;

/// What the marked line the script writes for a path it could not judge opens
/// with, after the engine takes the `sah-diagnostic:` marker off.
const SWIFT_IDIOMS_DECLINED_HEAD: &str = "idioms-swift found no file at";

/// What the marked line opens with for a file the run READ and no rule judged.
///
/// A path that holds no file and a file the parser cannot read are two shapes
/// and two messages, so a probe of one must not pass over the other.
const SWIFT_IDIOMS_DECLINED_FILE_HEAD: &str = "idioms-swift declined";

/// Drives the shipped script over the failing fixture beside `files`, and
/// answers the findings and the declined items of that one run.
///
/// The fixture is staged through `prepare` so the run reads the bytes the set
/// ships. `files` is the argument list the run carries, which is where a probe
/// names a path the repository holds no file at.
fn swift_idioms_run(files: &[&str]) -> SwiftIdiomsRun {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_IDIOMS_RULE);

    let stage_fixture = |repo: &Path| {
        copy_shipped_fixture(
            &loader,
            repo,
            SWIFT_IDIOMS_FAIL_FIXTURE,
            SWIFT_IDIOMS_JUDGED_PATH,
        );
    };
    let staging = ShippedStaging {
        staged: NO_STAGED_TEXT,
        links: NO_PROBE_LINKS,
        outside: NO_PROBE_OUTSIDE,
        prepare: &stage_fixture,
        restore: &stage_nothing_more,
    };

    swift_idioms_from_staging(&loader, &staging, files)
}

/// Drives the shipped script over `source` staged at `path`, with `path` as
/// the whole work list, and answers what that one run said.
///
/// [`swift_gate_reporting_rules`] answers the finding names alone, so a probe
/// of what the run DECLINED reaches the script through here instead.
fn swift_idioms_staged_run(path: ProbePath<'_>, source: &str) -> SwiftIdiomsRun {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_IDIOMS_RULE);

    let staged = [(path.0, source)];

    swift_idioms_from_staging(&loader, &ShippedStaging::of(&staged), &[path.0])
}

/// Drives the shipped script over `staging` with `files` as the work list, and
/// answers the whole of what that one run said.
///
/// The two helpers above differ in their STAGING alone — one copies the
/// shipped fixture into the probe repository, and the other writes a probe
/// file of its own — so the run itself and the three readings a probe takes
/// off it stand here, once.
fn swift_idioms_from_staging(
    loader: &ValidatorLoader,
    staging: &ShippedStaging<'_>,
    files: &[&str],
) -> SwiftIdiomsRun {
    let run = drive_shipped_script_whole(loader, SWIFT_IDIOMS_RULE, staging, files);
    let outcome = run
        .outcome
        .expect("the shipped Swift idioms script must judge the probe files and exit 0");

    SwiftIdiomsRun {
        findings: finding_rows(&outcome, &run.repo_root),
        rules: finding_rule_names(&outcome, &run.repo_root),
        declined: script_diagnostics(&outcome, &run.repo_root),
    }
}

/// What one run of the shipped script said.
struct SwiftIdiomsRun {
    /// Each finding as the `path:line` row a probe states.
    findings: Vec<String>,

    /// The rule name each of those findings carries.
    rules: Vec<String>,

    /// Each path the run could not judge.
    declined: Vec<String>,
}

/// Acceptance: a path the shipped Swift idioms rule cannot judge costs the run
/// that one path, and no finding of the files it did judge.
///
/// A path that holds no file is the dangerous shape, and it is dangerous for a
/// reason no status carries: measured with Apple Swift 6.4,
/// `swift format lint --strict` over such a path exits 0 and writes NOTHING,
/// which reads exactly like a clean pass over a file the run never opened. The
/// script therefore tests each path itself with `[ ! -e "$file" ]` before the
/// tool starts.
///
/// The one path for each run is what keeps the rest of the work list. Measured
/// over a file holding one finding beside a path that names a DIRECTORY:
/// a shared run exits 64, reports ZERO findings, and judges the healthy file
/// in none of it. That is the answer `builtin/validators/README.md` refuses —
/// "a nonzero exit fails the WHOLE run, so one unjudged path throws away every
/// finding the run did make".
///
/// The two halves of this test are both load-bearing: the run keeps every
/// finding of the file it judged, AND it states the path it declined on the
/// marked stderr channel. A run that lost either half is a run that answered
/// for a file it never read.
#[test]
fn the_shipped_swift_idioms_tool_rule_reports_a_file_beside_one_it_declined() {
    let alone = swift_idioms_run(&[SWIFT_IDIOMS_JUDGED_PATH]);
    assert_eq!(
        alone.findings.len(),
        SWIFT_IDIOMS_FAIL_FIXTURE_FINDINGS,
        "the shipped failing fixture must report \
         {SWIFT_IDIOMS_FAIL_FIXTURE_FINDINGS} findings on its own; it reported {:?}",
        alone.findings
    );

    let carried: std::collections::BTreeSet<&String> = alone.rules.iter().collect();
    assert_eq!(
        carried.len(),
        SWIFT_IDIOMS_FAIL_FIXTURE_RULES,
        "the findings of the shipped failing fixture must carry all \
         {SWIFT_IDIOMS_FAIL_FIXTURE_RULES} rules of the allowlist; they carry {carried:?}"
    );

    let beside = swift_idioms_run(&[SWIFT_IDIOMS_ABSENT_PATH, SWIFT_IDIOMS_JUDGED_PATH]);

    assert_eq!(
        beside.findings, alone.findings,
        "a path the run cannot judge must cost that path alone; the run beside \
         `{SWIFT_IDIOMS_ABSENT_PATH}` reported another finding list than the run without it"
    );
    assert_eq!(
        beside.declined.len(),
        1,
        "the run must state the one path it declined; it stated {:?}",
        beside.declined
    );
    assert!(
        beside.declined[0].starts_with(SWIFT_IDIOMS_DECLINED_HEAD)
            && beside.declined[0].contains(SWIFT_IDIOMS_ABSENT_PATH),
        "the marked line must name the path it declined; it reads `{}`",
        beside.declined[0]
    );
}

/// Where a probe stages the Swift file it measures a rule set over.
const SWIFT_IDIOMS_PROBE_PATH: &str = "Probe.swift";

/// Where a probe stages a SECOND Swift file of the same run.
const SWIFT_IDIOMS_SECOND_PROBE_PATH: &str = "Second.swift";

/// Drives the shipped script over `source` staged at
/// [`SWIFT_IDIOMS_PROBE_PATH`], beside `support`, and answers the rule name of
/// each finding it reported.
fn swift_idioms_reporting_rules(source: &str, support: &[(&str, &str)]) -> Vec<String> {
    swift_gate_reporting_rules(SWIFT_IDIOMS_RULE, SWIFT_IDIOMS_PROBE_PATH, source, support)
}

/// A Swift file holding one defect of the gate allowlist.
///
/// The empty-collection call is the shape `AlwaysUseLiteralForEmptyCollection\
/// Init` reports, and the rule is OFF in the toolchain's own configuration, so
/// a run that wrote no configuration of its own stays silent here.
const SWIFT_IDIOMS_REPORTING_SOURCE: &str = concat!(
    "public struct Holder {\n",
    "    public var items = [Int]()\n",
    "}\n",
);

/// Acceptance: a run over two files that both report gives the findings of
/// both.
///
/// The script hands `swift format` one path for each run, so the findings of
/// one file reach the report through a run of their own. A loop that stopped
/// at the first file that reported, or that let `set -e` end the run on the
/// status 1 a finding answers, would report the first file alone — and the
/// second file would read as clean.
///
/// Both halves are load-bearing. Each file must carry a finding of its own, so
/// the run states a row for each path, and the run must exit 0 so the engine
/// reads the rows at all.
#[test]
fn the_shipped_swift_idioms_tool_rule_reports_every_file_that_reports() {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_IDIOMS_RULE);

    let staged = [
        (SWIFT_IDIOMS_PROBE_PATH, SWIFT_IDIOMS_REPORTING_SOURCE),
        (
            SWIFT_IDIOMS_SECOND_PROBE_PATH,
            SWIFT_IDIOMS_REPORTING_SOURCE,
        ),
    ];
    let rows = shipped_script_findings(
        &loader,
        SWIFT_IDIOMS_RULE,
        &staged,
        &[SWIFT_IDIOMS_PROBE_PATH, SWIFT_IDIOMS_SECOND_PROBE_PATH],
    )
    .expect("the shipped Swift idioms script must judge both probe files and exit 0");

    for path in [SWIFT_IDIOMS_PROBE_PATH, SWIFT_IDIOMS_SECOND_PROBE_PATH] {
        assert!(
            rows.iter().any(|row| row.starts_with(path)),
            "a run over two files that both report must state a row for `{path}`; the run \
             reported {rows:?}"
        );
    }
}

/// Acceptance: the shipped Swift idioms rule drops every tag its allowlist
/// does not name.
///
/// `swift format lint` writes nine tags that come from the PRETTY-PRINTER and
/// the WHITESPACE LINTER rather than from the 43 rules, and no key of the
/// configuration reaches any of them. The script therefore reads the tag off
/// each output line and keeps only a tag the allowlist names.
///
/// Each probe of [`SWIFT_FORMAT_TAG_PROBES`] is the smallest file that draws
/// one of those nine tags, and
/// `the_shipped_swift_idioms_rule_body_names_every_tag_swift_format_writes`
/// holds each one to drawing it from the live tool. This test drives the same
/// probes through the SHIPPED SCRIPT, so a script that dropped the filter
/// reports layout here and fails BY NAME.
#[test]
fn the_shipped_swift_idioms_tool_rule_drops_every_tag_outside_its_allowlist() {
    for (tag, source) in SWIFT_FORMAT_TAG_PROBES {
        let reported = swift_idioms_reporting_rules(source, NO_SUPPORT_FILES);
        assert!(
            reported.is_empty(),
            "the `[{tag}]` probe holds a layout defect and no idiom, so the gate must report \
             nothing over it; the run reported {reported:?}"
        );
    }
}

/// A file name that carries the head of an allowlisted `swift format`
/// finding.
///
/// `swift format lint --strict` writes `<path>:<line>:<column>: error:
/// [<Tag>] <reason>`, and it writes the PATH FIRST. So every reading of that
/// output that matches the tag as loose text reads the NAME of the file as
/// well. This name holds `: error: [UseShorthandTypeNames] `, and that tag
/// stands in the allowlist, so a loose reading takes every line this file
/// draws for an allowlisted finding.
const SWIFT_IDIOMS_DIAGNOSTIC_NAME_PATH: &str = "x: error: [UseShorthandTypeNames] y.swift";

/// The same name, for the probe that measures the decline guard.
///
/// A second name rather than the same one, because the two probes measure two
/// readings of the same output and a shared path would tie them together.
const SWIFT_IDIOMS_DIAGNOSTIC_NAME_BROKEN_PATH: &str = "z: error: [UseShorthandTypeNames] q.swift";

/// Acceptance: the gate reads the tag off a `swift format` line and never off
/// the name of the file that line reports.
///
/// `swift format` writes the path at the head of every line, so a reading
/// spelled `grep -E ": error: \[($allowed)\] "` matches the written PATH. A
/// file named for an allowlisted diagnostic then carries every tag through the
/// gate. Measured with Apple Swift 6.4 over the `[Indentation]` probe staged
/// at [`SWIFT_IDIOMS_DIAGNOSTIC_NAME_PATH`]: the loose reading wrote
/// `x: error: [UseShorthandTypeNames] y.swift:3: Indentation: unindent by 6
/// spaces`, which is a layout finding the allowlist does not name and review
/// must never carry.
///
/// Both halves are load-bearing. The layout probe must report NOTHING, or the
/// filter reads the written path; and the idiom probe must still report, or an
/// anchor tight enough to drop the first also drops every finding a file with
/// an unusual name draws.
#[test]
fn the_shipped_swift_idioms_tool_rule_measures_a_file_named_for_a_diagnostic_head() {
    let layout = swift_gate_reporting_rules(
        SWIFT_IDIOMS_RULE,
        SWIFT_IDIOMS_DIAGNOSTIC_NAME_PATH,
        SWIFT_FORMAT_INDENTATION_PROBE,
        NO_SUPPORT_FILES,
    );
    assert!(
        layout.is_empty(),
        "the `[{SWIFT_FORMAT_INDENTATION_TAG}]` probe holds a layout defect and no idiom, so \
         the gate must report nothing over it whatever the file is named; staged at \
         `{SWIFT_IDIOMS_DIAGNOSTIC_NAME_PATH}` the run reported {layout:?}"
    );

    let idiom = swift_gate_reporting_rules(
        SWIFT_IDIOMS_RULE,
        SWIFT_IDIOMS_DIAGNOSTIC_NAME_PATH,
        SWIFT_IDIOMS_LONG_TYPE,
        NO_SUPPORT_FILES,
    );
    assert!(
        idiom.contains(&SWIFT_SHORTHAND_TYPE_RULE.to_string()),
        "`{SWIFT_SHORTHAND_TYPE_RULE}` must report a long type name whatever the file is \
         named; staged at `{SWIFT_IDIOMS_DIAGNOSTIC_NAME_PATH}` the run reported {idiom:?}"
    );
}

/// Acceptance: the gate declines a file the parser cannot read, whatever that
/// file is named.
///
/// The decline guard reads the same output as the tag filter, and it carried
/// the same defect. A reading spelled `grep -v ': error: \['` drops a line by
/// loose text, so a file named for an allowlisted diagnostic makes an
/// UNTAGGED tool error look tagged and the file is never declined. Measured
/// with Apple Swift 6.4 over Swift the parser cannot read, staged at
/// [`SWIFT_IDIOMS_DIAGNOSTIC_NAME_BROKEN_PATH`]: the loose reading wrote no
/// marked line and gave the tool's own `error: expected name in attribute`
/// lines as findings, unrewritten.
///
/// Both halves are load-bearing. The run must report NOTHING, because a file
/// no rule judged carries no finding; and it must state the path on the marked
/// channel, because a run that judged nothing and said nothing reads as a
/// clean file.
#[test]
fn the_shipped_swift_idioms_tool_rule_declines_a_file_named_for_a_diagnostic_head() {
    let run = swift_idioms_staged_run(
        ProbePath(SWIFT_IDIOMS_DIAGNOSTIC_NAME_BROKEN_PATH),
        SWIFT_FORMAT_UNPARSABLE_SOURCE,
    );

    assert!(
        run.findings.is_empty(),
        "a file the parser cannot read is a file no rule judged, so the run must report \
         nothing over it whatever the file is named; staged at \
         `{SWIFT_IDIOMS_DIAGNOSTIC_NAME_BROKEN_PATH}` the run reported {:?}",
        run.findings
    );
    assert_eq!(
        run.declined.len(),
        1,
        "the run must state the one path it declined; it stated {:?}",
        run.declined
    );
    assert!(
        run.declined[0].starts_with(SWIFT_IDIOMS_DECLINED_FILE_HEAD)
            && run.declined[0].contains(SWIFT_IDIOMS_DIAGNOSTIC_NAME_BROKEN_PATH),
        "the marked line must name the file it declined; it reads `{}`",
        run.declined[0]
    );
}

/// A file name a real repository holds that carries a PRECOMPOSED character.
///
/// `swift format` writes the path back in NFD, and the argument list carries
/// it in NFC. Measured with Apple Swift 6.4: the argument list holds
/// `63 61 66 c3a9 2e 73 77 69 66 74`, and the head of the output line holds
/// `63 61 66 65 cc81 2e 73 77 69 66 74`. So a reading that compares the path
/// the tool WROTE with the path the argument list SPELLED matches no line of
/// this file, and every finding it draws is lost.
const SWIFT_IDIOMS_ACCENTED_PATH: &str = "café.swift";

/// A file name a real repository holds that carries an alternation bar.
///
/// `|` is the alternation operator of an extended regular expression and the
/// delimiter an `s` command reaches for, so a reading that writes the path
/// into either one has to escape it — and an escaped delimiter is a delimiter
/// still. Of the 22 characters measured against the earlier reading, this is
/// the one that defeated it.
const SWIFT_IDIOMS_ALTERNATION_PATH: &str = "a|b.swift";

/// The declaration of [`SWIFT_IDIOMS_LONG_TYPE`] whose row a probe of an
/// unusual file name states.
const SWIFT_IDIOMS_LONG_TYPE_HEAD: &str = "public static func read(";

/// Holds `run` to reporting the long type names of [`SWIFT_IDIOMS_LONG_TYPE`]
/// over the file staged at `path`, in the shape the script rewrites the tool's
/// own line into.
///
/// Three readings, and each one holds a half no other holds. A run that
/// DECLINED the file reports nothing and states the path on the marked
/// channel. A run that wrote the tool's line UNREWRITTEN states the same
/// `path:line` row all the same, because the engine reads the first
/// `:<digits>:` of a line, so the row cannot separate the two shapes. The rule
/// NAME can: the rewritten line carries `<path>:<line>: <RuleName>: <reason>`,
/// and the raw line carries the COLUMN number where the name stands.
fn assert_swift_idioms_reports_the_long_type(run: &SwiftIdiomsRun, path: &str) {
    assert!(
        run.declined.is_empty(),
        "the gate must judge `{path}` rather than decline it; it stated {:?}",
        run.declined
    );

    let expected = expected_row(path, SWIFT_IDIOMS_LONG_TYPE, SWIFT_IDIOMS_LONG_TYPE_HEAD);
    assert!(
        run.findings.contains(&expected),
        "the gate must state the row `{expected}` for the long type name of `{path}`; it \
         reported {:?}",
        run.findings
    );

    let carried: std::collections::BTreeSet<&str> = run.rules.iter().map(String::as_str).collect();
    assert_eq!(
        carried,
        std::collections::BTreeSet::from([SWIFT_SHORTHAND_TYPE_RULE]),
        "every finding over `{path}` must carry `{SWIFT_SHORTHAND_TYPE_RULE}`, in the shape \
         the script rewrites the tool's own line into; the run reported {:?}",
        run.rules
    );
}

/// Acceptance: the gate reports a file whose name carries a precomposed
/// character.
///
/// The name is a legitimate one, and the reading that anchors on the head of a
/// diagnostic must not cost it its findings. Measured with Apple Swift 6.4 and
/// a reading that compared the two spellings of the path: the run wrote
/// `sah-diagnostic: idioms-swift declined café.swift: ...` and reported
/// NOTHING, because the tool writes the path in NFD and the argument list
/// carries it in NFC. The `ünïcodé.swift` name broke the same way, and
/// `日本語.swift` did not, because those characters have no decomposed form.
/// So the break is the precomposed Latin letter a real repository writes.
#[test]
fn the_shipped_swift_idioms_tool_rule_reports_a_file_whose_name_carries_an_accent() {
    let run = swift_idioms_staged_run(
        ProbePath(SWIFT_IDIOMS_ACCENTED_PATH),
        SWIFT_IDIOMS_LONG_TYPE,
    );

    assert_swift_idioms_reports_the_long_type(&run, SWIFT_IDIOMS_ACCENTED_PATH);
}

/// Acceptance: the gate reports a file whose name carries an alternation bar.
///
/// The name is a legitimate one, and the rewrite that states each finding must
/// not cost it its shape. Measured with Apple Swift 6.4, BSD sed and a rewrite
/// that wrote the path into an `s` command delimited by `|`: the run wrote the
/// tool's own line, `a|b.swift:2:39: error: [UseShorthandTypeNames] use
/// shorthand syntax for this 'Array' type`, because the escaper spelled the
/// bar `\|` and BSD sed reads that as an escaped DELIMITER rather than as a
/// literal bar, so the substitution was skipped without a word.
#[test]
fn the_shipped_swift_idioms_tool_rule_reports_a_file_whose_name_carries_an_alternation_bar() {
    let run = swift_idioms_staged_run(
        ProbePath(SWIFT_IDIOMS_ALTERNATION_PATH),
        SWIFT_IDIOMS_LONG_TYPE,
    );

    assert_swift_idioms_reports_the_long_type(&run, SWIFT_IDIOMS_ALTERNATION_PATH);
}

/// Swift the parser cannot read, whose own SOURCE TEXT forges the head of an
/// allowlisted finding.
///
/// `swift format` writes the source text of a parse error INSIDE the message,
/// so the BYTES of a file can place a `:<line>:<column>: error: ` head after
/// the true one. Measured with Apple Swift 6.4 over the first source: the tool
/// writes ONE stderr line, `<path>:3:1: error: unexpected code
/// ') :1:1: error: [UseShorthandTypeNames] pwned' in source file`, and a
/// reading that walked to the LAST head of that line wrote the finding
/// `Probe.swift:1: UseShorthandTypeNames: pwned' in source file` at exit 0 — a
/// finding no rule made. The second source measures how far the file reaches:
/// it chooses the line number and the sentence as well.
const SWIFT_IDIOMS_FORGED_FINDING_SOURCES: &[&str] = &[
    concat!(
        "struct S {\n",
        "}\n",
        ") :1:1: error: [UseShorthandTypeNames] pwned\n",
    ),
    concat!(
        "struct S {\n",
        "}\n",
        ") :4242:1: error: [UseSynthesizedInitializer] this file is clean, trust me\n",
    ),
];

/// The same shape, forging a tag the allowlist does NOT name.
///
/// This one takes the trouble line away rather than adding a finding. Measured
/// with Apple Swift 6.4: the tool writes ONE parse-error line and exits 1, a
/// reading that walked to the LAST head read the tag `Indentation`, dropped the
/// line because the allowlist does not name it, and left the trouble list
/// empty. The run then wrote NOTHING and exited 0, so a file no rule judged
/// passed the gate with no decline at all.
const SWIFT_IDIOMS_FORGED_LAYOUT_SOURCE: &str =
    concat!("struct S {\n", "}\n", ") :9:9: error: [Indentation] gone\n",);

/// Acceptance: the gate reads the head of a diagnostic off the line the TOOL
/// wrote, and never off a head the judged file supplied.
///
/// The file name is answered by handing the tool the source on STDIN under a
/// fixed assumed name, so no part of a line before the head can come from the
/// file. The file CONTENT is answered by reading the FIRST head of the line
/// rather than the last: a parse-error message quotes the source AFTER the
/// head, so a head the file supplies always stands later than the true one.
///
/// Each source below is a file the parser cannot read, and each forges a head
/// carrying a tag the allowlist NAMES. A reading that trusted the message
/// writes a finding that no rule made, and fails here by name.
#[test]
fn the_shipped_swift_idioms_tool_rule_measures_a_file_whose_source_forges_a_diagnostic_head() {
    for source in SWIFT_IDIOMS_FORGED_FINDING_SOURCES {
        let run = swift_idioms_staged_run(ProbePath(SWIFT_IDIOMS_PROBE_PATH), source);

        assert!(
            run.findings.is_empty(),
            "the source of a file must reach no finding, and this file holds only Swift the \
             parser cannot read; the run reported {:?} carrying {:?}",
            run.findings,
            run.rules
        );
    }
}

/// Acceptance: the gate declines a file the parser cannot read, whatever that
/// file's own bytes say.
///
/// The decline guard and the tag filter are ONE reading, so a head the file
/// supplies takes the trouble line out of the guard as well as putting a
/// finding into the filter. Measured with Apple Swift 6.4 over
/// [`SWIFT_IDIOMS_FORGED_LAYOUT_SOURCE`]: a reading that walked to the LAST
/// head wrote NOTHING and exited 0, and the same file with its third line
/// spelled `) garbage here` was declined correctly, so the payload alone made
/// the difference.
///
/// Both halves are load-bearing. The run must report nothing, because a file no
/// rule judged carries no finding; and it must state the path on the marked
/// channel, because a run that judged nothing and said nothing reads as a clean
/// file.
#[test]
fn the_shipped_swift_idioms_tool_rule_declines_a_file_whose_source_forges_a_diagnostic_head() {
    let run = swift_idioms_staged_run(
        ProbePath(SWIFT_IDIOMS_PROBE_PATH),
        SWIFT_IDIOMS_FORGED_LAYOUT_SOURCE,
    );

    assert!(
        run.findings.is_empty(),
        "a file the parser cannot read is a file no rule judged, so the run must report \
         nothing over it whatever its bytes say; the run reported {:?}",
        run.findings
    );
    assert_eq!(
        run.declined.len(),
        1,
        "the run must state the one path it declined; it stated {:?}",
        run.declined
    );
    assert!(
        run.declined[0].starts_with(SWIFT_IDIOMS_DECLINED_FILE_HEAD)
            && run.declined[0].contains(SWIFT_IDIOMS_PROBE_PATH),
        "the marked line must name the file it declined; it reads `{}`",
        run.declined[0]
    );
}

/// The path a probe stages its one file at.
///
/// A distinct type, so a path and the source staged at it cannot trade places.
#[derive(Clone, Copy)]
struct ProbePath<'a>(&'a str);

/// A Swift file written the way the two Swift prompt rules ASK for.
///
/// Every declaration is a DO one of them states: the empty-collection
/// properties carry the type annotation `idioms.md` requires, and the test
/// unwraps with `#require` rather than with the `guard` `optionals.md` now
/// refuses in a test.
const SWIFT_PROMPT_RULE_ANSWER: &str = concat!(
    "import Testing\n\n",
    "public struct Holder {\n",
    "    public var items: [Int] = []\n",
    "    public var ids: Set<String> = []\n",
    "}\n\n",
    "struct HolderTests {\n",
    "    @Test func `the holder holds its readings`() throws {\n",
    "        let holder = Holder()\n",
    "        let value = try #require(holder.items.first)\n",
    "        #expect(value == 1)\n",
    "    }\n",
    "}\n",
);

/// Acceptance: Swift written the way the prompt rules ask for draws no finding
/// from this gate.
///
/// A tool and a prompt rule that disagree produce churn on every review round,
/// so agreement is a fact to measure rather than one to assume. The answer
/// file must report NOTHING, or the gate fights the prompt rule the author is
/// reading.
///
/// The `Set<String>` row is the load-bearing one. `idioms.md` states
/// `var ids: Set<String> = []` as its DO, and measured with Apple Swift 6.4,
/// `AlwaysUseLiteralForEmptyCollectionInit` reports `[Int]()` and
/// `[String: Int]()` and stays SILENT for `Set<String>()`. So the gate decides
/// the array and the dictionary and leaves the set alone, in the direction the
/// prompt rule asks for either way.
#[test]
fn the_shipped_swift_idioms_tool_rule_agrees_with_the_swift_prompt_rules() {
    let answered = swift_idioms_reporting_rules(SWIFT_PROMPT_RULE_ANSWER, NO_SUPPORT_FILES);
    assert!(
        answered.is_empty(),
        "Swift written the way the prompt rules ask for must draw no finding from this \
         gate; the run reported {answered:?}"
    );
}

/// The rule that reports an explicit memberwise initializer the compiler
/// would synthesize.
const SWIFT_SYNTHESIZED_INITIALIZER_RULE: &str = "UseSynthesizedInitializer";

/// A `public` struct carrying a `public` memberwise initializer.
///
/// Swift synthesizes an INTERNAL memberwise initializer and never a `public`
/// one, so this initializer is identical to nothing the compiler writes.
const SWIFT_IDIOMS_PUBLIC_INIT: &str = concat!(
    "public struct Point {\n",
    "    public var x: Int\n",
    "    public var y: Int\n",
    "\n",
    "    public init(x: Int, y: Int) {\n",
    "        self.x = x\n",
    "        self.y = y\n",
    "    }\n",
    "}\n",
);

/// Acceptance: the gate owns the INTERNAL half of the redundant memberwise
/// initializer and no more.
///
/// Row 4 of the measurement table of `idioms-swift.md` holds both answers, and
/// the silent half is correct rather than a hole: Swift synthesizes an
/// internal memberwise initializer and never a `public` one, so a
/// `public init` is identical to nothing and deleting it would take a
/// declaration off the package surface.
///
/// Both halves are load-bearing. The internal form must report, or the bullet
/// `idioms.md` no longer states has no owner; the public form must stay
/// silent, or the gate asks for an edit that breaks every caller outside the
/// package.
#[test]
fn the_shipped_swift_idioms_tool_rule_reads_the_internal_synthesized_initializer() {
    let internal = swift_idioms_reporting_rules(SWIFT_IDIOMS_REDUNDANT_INIT, NO_SUPPORT_FILES);
    assert!(
        internal.contains(&SWIFT_SYNTHESIZED_INITIALIZER_RULE.to_string()),
        "`{SWIFT_SYNTHESIZED_INITIALIZER_RULE}` must report an internal memberwise \
         initializer the compiler would synthesize; the run reported {internal:?}"
    );

    let public = swift_idioms_reporting_rules(SWIFT_IDIOMS_PUBLIC_INIT, NO_SUPPORT_FILES);
    assert!(
        !public.contains(&SWIFT_SYNTHESIZED_INITIALIZER_RULE.to_string()),
        "Swift synthesizes no `public` memberwise initializer, so \
         `{SWIFT_SYNTHESIZED_INITIALIZER_RULE}` must stay silent for a `public init`; the \
         run reported {public:?}"
    );
}

/// The empty-collection form `idioms.md` names as its DON'T.
///
/// It differs from the two properties of [`SWIFT_PROMPT_RULE_ANSWER`] in that
/// each annotated literal is written as a constructor call instead.
const SWIFT_IDIOMS_INFERRED_PROPERTY: &str = concat!(
    "public struct Holder {\n",
    "    public var items = [Int]()\n",
    "    public var table = [String: Int]()\n",
    "    public var ids = Set<String>()\n",
    "}\n",
);

/// The rule that reports the empty-collection call.
const SWIFT_EMPTY_COLLECTION_RULE: &str = "AlwaysUseLiteralForEmptyCollectionInit";

/// Acceptance: the gate reports the empty-collection call, and it reports it
/// in the direction `idioms.md` asks for.
///
/// The rule is OFF in the toolchain's own configuration, so the configuration
/// the script writes is what turns it ON. The direction is the whole point:
/// its message reads `replace '[Int]()' with ': [Int] = []'`, which is the DO
/// of `idioms.md` word for word, and it is the OPPOSITE direction from
/// SwiftFormat's `propertyTypes` under `--property-types inferred`, which this
/// gate refused for exactly that reason.
///
/// The `Set` row is load-bearing beside them. Measured with Apple Swift 6.4,
/// the rule reports `[Int]()` and `[String: Int]()` and stays SILENT for
/// `Set<String>()`, so the set form has no owner in this gate and
/// `idioms.md` keeps it.
#[test]
fn the_shipped_swift_idioms_tool_rule_reports_the_empty_collection_call() {
    let rows = swift_idioms_reporting_rules(SWIFT_IDIOMS_INFERRED_PROPERTY, NO_SUPPORT_FILES);

    let reported = rows
        .iter()
        .filter(|rule| *rule == SWIFT_EMPTY_COLLECTION_RULE)
        .count();

    assert_eq!(
        reported, SWIFT_EMPTY_COLLECTION_REPORTED_FORMS,
        "`{SWIFT_EMPTY_COLLECTION_RULE}` must report the array call and the dictionary call \
         and stay silent for the `Set` call, which is \
         {SWIFT_EMPTY_COLLECTION_REPORTED_FORMS} findings over the three; the run reported \
         {rows:?}"
    );
}

/// How many of the three empty-collection calls the gate reports.
///
/// Two: the array and the dictionary. Measured with Apple Swift 6.4,
/// `Set<String>()` draws nothing.
const SWIFT_EMPTY_COLLECTION_REPORTED_FORMS: usize = 2;

/// The rule that asks for a `for` loop where the code calls `forEach`.
const SWIFT_REPLACE_FOR_EACH_RULE: &str = "ReplaceForEachWithForLoop";

/// A `forEach` whose closure holds an `if`, written on ONE line.
///
/// This is the shape the deleted `idioms.md` bullet named as its DON'T:
/// `forEach` + `if` where the author needs control flow, and `forEach` can
/// `break`, `continue` or `return` out of nothing.
const SWIFT_IDIOMS_SINGLE_LINE_FOR_EACH: &str = concat!(
    "public enum Looping {\n",
    "    public static func walk(_ things: [Int]) {\n",
    "        things.forEach { if $0 > 2 { print($0) } }\n",
    "    }\n",
    "}\n",
);

/// The same walk written as a `filter` chain feeding a `forEach`.
///
/// A `forEach` after a `filter` is the walk `ReplaceForEachWithForLoop` reports.
const SWIFT_IDIOMS_FILTER_CHAIN: &str = concat!(
    "public enum Filtering {\n",
    "    public static func walk(_ things: [Int]) {\n",
    "        things.filter { $0 > 2 }.forEach { thing in print(thing) }\n",
    "    }\n",
    "}\n",
);

/// Acceptance: `ReplaceForEachWithForLoop` reports the `filter` chain.
///
/// This is the row the body of `idioms-swift.md` records under "Which rule
/// reports a filter chain".
///
/// The tool CORRECTS nothing here. Measured with Apple Swift 6.4,
/// `swift format --in-place` with this rule ON leaves the file byte for byte
/// as it was, so the message is the whole of what the author receives.
///
#[test]
fn the_shipped_swift_idioms_tool_rule_reports_a_filter_chain() {
    let reported = swift_idioms_reporting_rules(SWIFT_IDIOMS_FILTER_CHAIN, NO_SUPPORT_FILES);
    assert!(
        reported.contains(&SWIFT_REPLACE_FOR_EACH_RULE.to_string()),
        "`{SWIFT_REPLACE_FOR_EACH_RULE}` must report a `filter` chain feeding a `forEach`; \
         the run reported {reported:?}"
    );
}

/// The rule that asks for a signature with no return clause at all.
const SWIFT_VOID_RETURN_RULE: &str = "NoVoidReturnOnFunctionSignature";

/// A function whose return clause names `Void` where it could name nothing.
const SWIFT_IDIOMS_VOID_RETURN: &str = concat!(
    "public enum Typed {\n",
    "    public static func f() -> Void {}\n",
    "}\n",
);

/// A closure PARAMETER whose result names the empty tuple.
///
/// The rule reads a function SIGNATURE, so this shape belongs to
/// `ReturnVoidInsteadOfEmptyTuple`, which the allowlist does not name.
const SWIFT_IDIOMS_CLOSURE_RETURN: &str = concat!(
    "public enum Handlers {\n",
    "    public static func handler(_ body: (Int) -> ()) {}\n",
    "}\n",
);

/// Acceptance: the gate decides BOTH halves of the `Void` return clause.
///
/// `idioms.md` stated two requirements in one bullet: write `Void` rather than
/// `()`, and omit the return clause entirely when it is `Void`. Measured with
/// Apple Swift 6.4, `NoVoidReturnOnFunctionSignature` reports BOTH shapes —
/// `remove the explicit return type '()' from this function` and
/// `remove the explicit return type 'Void' from this function` — so one rule
/// answers the whole bullet, and the shape its message asks for is the shape
/// the bullet's own DO names.
///
/// The closure row is the boundary. The rule reads a function SIGNATURE, so a
/// closure parameter written `(Int) -> ()` draws nothing from it; that shape
/// belongs to `ReturnVoidInsteadOfEmptyTuple`, and the allowlist does not name
/// that rule. A gate that reported it would decide a shape no bullet of this
/// set states.
#[test]
fn the_shipped_swift_idioms_tool_rule_decides_both_halves_of_the_void_return_clause() {
    for probe in [SWIFT_IDIOMS_PAREN_RETURN, SWIFT_IDIOMS_VOID_RETURN] {
        let reported = swift_idioms_reporting_rules(probe, NO_SUPPORT_FILES);
        assert!(
            reported.contains(&SWIFT_VOID_RETURN_RULE.to_string()),
            "`{SWIFT_VOID_RETURN_RULE}` must report every return clause that names nothing, \
             and this probe holds one; the run reported {reported:?}"
        );
    }

    let closure = swift_idioms_reporting_rules(SWIFT_IDIOMS_CLOSURE_RETURN, NO_SUPPORT_FILES);
    assert!(
        !closure.contains(&SWIFT_VOID_RETURN_RULE.to_string()),
        "`{SWIFT_VOID_RETURN_RULE}` reads a function signature, so it must stay silent for a \
         closure parameter; the run reported {closure:?}"
    );
}

/// The rule that asks for the shorthand spelling of a long type name.
const SWIFT_SHORTHAND_TYPE_RULE: &str = "UseShorthandTypeNames";

/// One declaration for each long spelling the deleted `idioms.md` bullet
/// named as its DON'T.
///
/// The bullet enumerated three — `Array<Int>`, `Dictionary<Key, Value>` and
/// `Optional<String>` — so a probe holding one of them would prove a third of
/// it. Measured on 0.62.1 under the shipped script, `typeSugar` reports each of
/// the three on its own line.
const SWIFT_IDIOMS_LONG_TYPE: &str = concat!(
    "public enum Sugar {\n",
    "    public static func read(_ values: Array<Int>) -> Int {\n",
    "        values.count\n",
    "    }\n\n",
    "    public static func table(_ pairs: Dictionary<String, Int>) -> Int {\n",
    "        pairs.count\n",
    "    }\n\n",
    "    public static func name(_ value: Optional<String>) -> Int {\n",
    "        value?.count ?? 0\n",
    "    }\n",
    "}\n",
);

/// A function whose return clause names the empty tuple rather than `Void`.
const SWIFT_IDIOMS_PAREN_RETURN: &str = concat!(
    "public enum Returns {\n",
    "    public static func run() -> () {}\n",
    "}\n",
);

/// A struct carrying an internal initializer the compiler would synthesize.
const SWIFT_IDIOMS_REDUNDANT_INIT: &str = concat!(
    "public struct Reading {\n",
    "    public var count: Int\n",
    "\n",
    "    init(count: Int) {\n",
    "        self.count = count\n",
    "    }\n",
    "}\n",
);

/// A case whose `let` binds the whole pattern rather than each variable.
const SWIFT_IDIOMS_HOISTED_LET: &str = concat!(
    "public enum Corner {\n",
    "    case at(Int, Int)\n",
    "}\n",
    "\n",
    "public func describe(_ corner: Corner) -> Int {\n",
    "    switch corner {\n",
    "    case let .at(x, y):\n",
    "        return x + y\n",
    "    }\n",
    "}\n",
);

/// Every requirement this gate took out of a Swift prompt rule.
///
/// Each `defect` is written in the SHAPE the deleted bullet named, so a row
/// reports what the bullet was about rather than a neighbouring shape the same
/// rule happens to read.
///
/// The `NoVoidReturnOnFunctionSignature` rows carry BOTH halves of the bullet
/// `idioms.md` split: the `()` return clause and the omit-the-clause form. The
/// empty-collection row carries the array and dictionary forms; `Set` stays
/// with the prompt rule, and `the_shipped_swift_idioms_tool_rule_reports_the_empty_collection_call`
/// measures that boundary.
///
/// `ReplaceForEachWithForLoop` carries the `forEach` + `if` half, and it
/// reports the `filter` chain as well —
/// `the_shipped_swift_idioms_tool_rule_reports_a_filter_chain` measures that.
///
/// One bullet this gate USED to own is not here at all. `value-semantics.md`
/// "Mark a `class` that is not designed for subclassing `final`" was
/// `preferFinalClasses`, and the toolchain carries no rule like it, so the
/// prompt rule states it again and `swift_judgment_rules.rs` holds the gate
/// silent over it.
const SWIFT_IDIOMS_SUPERSEDED_BULLETS: &[SupersededSwiftBullet] = &[
    SupersededSwiftBullet {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        tool_rule: SWIFT_SHORTHAND_TYPE_RULE,
        defect: SWIFT_IDIOMS_LONG_TYPE,
        words: "shorthand type sugar",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        tool_rule: SWIFT_VOID_RETURN_RULE,
        defect: SWIFT_IDIOMS_PAREN_RETURN,
        words: "Return `Void`, not `()`",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        tool_rule: SWIFT_VOID_RETURN_RULE,
        defect: SWIFT_IDIOMS_VOID_RETURN,
        words: "Omit the return clause entirely when it is `Void`",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        tool_rule: SWIFT_EMPTY_COLLECTION_RULE,
        defect: SWIFT_IDIOMS_INFERRED_PROPERTY,
        words: "Empty-collection variables use a literal",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        tool_rule: SWIFT_SYNTHESIZED_INITIALIZER_RULE,
        defect: SWIFT_IDIOMS_REDUNDANT_INIT,
        words: "memberwise initializer identical to the synthesized one",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        tool_rule: SWIFT_REPLACE_FOR_EACH_RULE,
        defect: SWIFT_IDIOMS_SINGLE_LINE_FOR_EACH,
        words: "over `forEach` + `if`",
    },
    SupersededSwiftBullet {
        prompt_rule: SWIFT_IDIOMS_PROMPT_RULE,
        tool_rule: "UseLetInEveryBoundCaseVariable",
        defect: SWIFT_IDIOMS_HOISTED_LET,
        words: "Bind each case variable with its own `let`",
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
/// Every probe holds ONE KIND of defect, so the rule the run names is the rule
/// that read the shape rather than a neighbour that read the same file, and it
/// holds that defect in the SHAPE the bullet named — a probe staging some other
/// shape the same rule happens to report proves ownership of a requirement the
/// bullet never stated.
#[test]
fn the_shipped_swift_idioms_tool_rule_owns_each_bullet_it_took() {
    for bullet in SWIFT_IDIOMS_SUPERSEDED_BULLETS {
        let reported = swift_idioms_reporting_rules(bullet.defect, NO_SUPPORT_FILES);
        verify_superseded_swift_bullet(bullet, &reported);
    }
}

/// The rule that reports a static member whose name repeats its own type.
const SWIFT_TYPE_NAME_RULE: &str = "DontRepeatTypeInStaticProperties";

/// A static member whose initializer CALLS the type that holds it.
const SWIFT_IDIOMS_REPEATED_TYPE_NAME: &str = concat!(
    "public struct Color {\n",
    "    public static let redColor = Color()\n",
    "}\n",
);

/// The same member with an initializer that infers `Int`.
///
/// The rule reads the member's own TYPE off an initializer that calls the
/// enclosing type, so a literal of another type draws nothing.
const SWIFT_IDIOMS_INFERRED_TYPE_NAME: &str = concat!(
    "public struct Color {\n",
    "    public static let redColor = 1\n",
    "}\n",
);

/// Acceptance: the gate reads ONE shape of type-name repetition.
///
/// Row 3 of the measurement table of `idioms-swift.md` holds eleven shapes,
/// and the rule reports four of them. It reads three facts at one time: the
/// member is `static`, its own type is the type that holds it, and its name
/// ends in that type name as a SUFFIX. The bullet of `idioms.md` names no
/// type, so the rule decides one shape of many and the prompt rule keeps the
/// bullet.
///
/// Both halves are load-bearing. The calling form must report, or the row is
/// not true; the inferred form must stay silent, or the row that records the
/// limit is not true either.
#[test]
fn the_shipped_swift_idioms_tool_rule_reads_one_shape_of_type_name_repetition() {
    let repeated = swift_idioms_reporting_rules(SWIFT_IDIOMS_REPEATED_TYPE_NAME, NO_SUPPORT_FILES);
    assert!(
        repeated.contains(&SWIFT_TYPE_NAME_RULE.to_string()),
        "`{SWIFT_TYPE_NAME_RULE}` must report a static member whose initializer calls the \
         type that holds it; the run reported {repeated:?}"
    );

    let inferred = swift_idioms_reporting_rules(SWIFT_IDIOMS_INFERRED_TYPE_NAME, NO_SUPPORT_FILES);
    assert!(
        !inferred.contains(&SWIFT_TYPE_NAME_RULE.to_string()),
        "`{SWIFT_TYPE_NAME_RULE}` reads the member's own type off its initializer, so a \
         member that infers `Int` must draw nothing; the run reported {inferred:?}"
    );
}

/// A line the pretty-printer reports for its length alone.
///
/// The default `lineLength` is 100 columns, so the name and the literal beside
/// it have to run past that.
const SWIFT_FORMAT_LINE_LENGTH_PROBE: &str = concat!(
    "let overLongName = ",
    "\"this string literal and the name beside it run well past the hundred \
     column line the default configuration states\"\n",
);

/// Swift holding one defect the shipped configuration reports.
///
/// The `Spacing` probe below and the "a file with findings" status probe
/// further down measure two different answers over this ONE declaration, so it
/// is named here, ahead of both of them.
const SWIFT_FORMAT_DIRTY_SOURCE: &str = "let alpha = 1+2\n";

/// The tag the whitespace linter writes for a member indented past the column
/// the configuration asks for.
const SWIFT_FORMAT_INDENTATION_TAG: &str = "Indentation";

/// A file holding one member indented 8 spaces where 2 are asked for.
///
/// The tag-probe list below and
/// `the_shipped_swift_idioms_tool_rule_measures_a_file_named_for_a_diagnostic_head`
/// measure two different answers over this ONE file, so it is named here,
/// ahead of both of them.
const SWIFT_FORMAT_INDENTATION_PROBE: &str =
    "struct Probe {\n  let alpha = 1\n        let beta = 2\n}\n";

/// A file whose spacing is a TAB where the tool asks for a space.
///
/// The TAB stands between the `=` and the value, so it is SPACING rather than
/// indentation, which is what separates `SpacingCharacter` from `Spacing`.
const SWIFT_FORMAT_SPACING_CHARACTER_PROBE: &str = "struct A {\n  let x =\t1\n}\n";

/// Swift that draws ONE tag no configuration can stop, for each tag the rule
/// body names.
///
/// Each probe is the smallest file that draws its own tag, so the run names
/// the tag that read the shape rather than a neighbour that read the same
/// file. A tag the body names and this list does not, and a probe this list
/// holds and the body does not, each fail the test below by name. Neither list
/// decides what the SET is: the test reads that out of the toolchain binary,
/// through `swift_reflection::tags_no_configuration_stops`.
const SWIFT_FORMAT_TAG_PROBES: &[(&str, &str)] = &[
    (
        "AddLines",
        "struct Probe { let alpha: Int\n  let beta: Int }\n",
    ),
    (
        "EndOfLineComment",
        "let alpha = 1  // an end of line comment long enough to push this line well past the \
         hundred column line the configuration states\n",
    ),
    (SWIFT_FORMAT_INDENTATION_TAG, SWIFT_FORMAT_INDENTATION_PROBE),
    ("LineLength", SWIFT_FORMAT_LINE_LENGTH_PROBE),
    ("RemoveLine", "let alpha = 1\n\n\n\nlet beta = 2\n"),
    ("Spacing", SWIFT_FORMAT_DIRTY_SOURCE),
    ("SpacingCharacter", SWIFT_FORMAT_SPACING_CHARACTER_PROBE),
    ("TrailingComma", "let alpha = [\n  1,\n  2\n]\n"),
    ("TrailingWhitespace", "let alpha = 1   \n"),
];

/// Swift the parser cannot read, because `@@@` opens an attribute that names
/// nothing.
const SWIFT_FORMAT_UNPARSABLE_SOURCE: &str =
    concat!("public struct Broken {\n", "  @@@ let alpha = 1\n", "}\n");
