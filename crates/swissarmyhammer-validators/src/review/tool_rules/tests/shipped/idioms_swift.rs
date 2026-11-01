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
//! measures. This gate took five bullets out of the Swift prompt rules, and a
//! requirement deleted without a tool that reads it is one the set states
//! nowhere. `the_shipped_swift_idioms_tool_rule_owns_each_bullet_it_took`
//! holds each of the five to being reported by its tool, over Swift written in
//! the SHAPE the bullet named, AND stated by no prompt rule.

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

/// The line of the shipped script that opens its tag allowlist.
///
/// The allowlist is the argument list of one `printf`, so the words between
/// this head and the pipe under it ARE the tags the gate keeps. Reading them
/// off the shipped script is what makes each guard below hold the rule that
/// ships rather than a copy this file wrote.
const SWIFT_IDIOMS_ALLOWLIST_HEAD: &str = r"printf '%s\n' ";

/// The word that closes the allowlist: the pipe the `printf` writes into.
const SWIFT_IDIOMS_ALLOWLIST_END: &str = "|";

/// How many rule tags the shipped allowlist holds.
///
/// Seven, which is the part of the toolchain's fixed set of 43 rules that
/// decides an IDIOM. The count is the assertion that a tag added or dropped
/// later reaches this guard rather than moving the gate without a word.
const SWIFT_IDIOMS_ALLOWLIST_SIZE: usize = 7;

/// The rule tags the shipped `idioms-swift` script keeps, read off the script
/// itself.
///
/// The words stand between [`SWIFT_IDIOMS_ALLOWLIST_HEAD`] and the pipe under
/// it, over as many lines as the script writes, each continuation line joined
/// with a trailing backslash.
fn swift_idioms_allowlist(script: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;

    for line in script.lines() {
        let trimmed = line.trim();
        let rest = match trimmed.strip_prefix(SWIFT_IDIOMS_ALLOWLIST_HEAD) {
            Some(rest) => {
                inside = true;
                rest
            }
            None if inside => trimmed,
            None => continue,
        };
        for word in rest
            .trim_end_matches(SWIFT_IDIOMS_LINE_JOIN)
            .split_whitespace()
        {
            if word == SWIFT_IDIOMS_ALLOWLIST_END {
                return names;
            }
            names.push(word.to_string());
        }
    }

    names
}

/// Every rule name the installed toolchain knows, read out of its own
/// configuration dump.
fn swift_format_rule_names() -> Vec<String> {
    swift_format_dumped_rules().keys().cloned().collect()
}

/// Acceptance: every tag the shipped allowlist names is a rule the installed
/// toolchain knows.
///
/// The allowlist is the gate. `swift format lint` writes a `[<Name>]` tag for
/// each finding, the script keeps only the tags this list names, and the
/// configuration it writes turns those same rules ON. A tag misspelled there
/// stops matching any line the tool writes, and the gate goes silent about the
/// bullet that tag decides — with no error, because a tag that matches nothing
/// looks exactly like a clean file.
///
/// This is the guard on that. Every name must stand in the `rules` table of
/// `swift format dump-configuration`, so a misspelling and a rule the
/// toolchain renames each fail here by name.
#[test]
fn the_shipped_swift_idioms_tool_rule_names_only_rules_swift_format_knows() {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_IDIOMS_RULE);
    let shipped = required_shipped_tool_rule(&loader, SWIFT_IDIOMS_RULE);

    let allowed = swift_idioms_allowlist(&shipped.script);
    assert_eq!(
        allowed.len(),
        SWIFT_IDIOMS_ALLOWLIST_SIZE,
        "the shipped allowlist must name {SWIFT_IDIOMS_ALLOWLIST_SIZE} rules; it names \
         {allowed:?}"
    );

    let known = swift_format_rule_names();
    assert_eq!(
        known.len(),
        SWIFT_FORMAT_RULE_COUNT,
        "`{SWIFT_TOOLCHAIN_TOOL} {SWIFT_FORMAT_SUBCOMMAND} {SWIFT_FORMAT_DUMP_VERB}` must write \
         the {SWIFT_FORMAT_RULE_COUNT} rules this gate is measured against; it wrote {}",
        known.len()
    );

    let unknown: Vec<&String> = allowed
        .iter()
        .filter(|name| !known.contains(name))
        .collect();

    assert!(
        unknown.is_empty(),
        "the shipped allowlist names these tags and the installed toolchain knows none of \
         them, so each one matches no output line and the gate stops measuring it: \
         {unknown:?}"
    );
}

/// The rule that rewrites a `for` loop's nested `if` into a `where` clause.
const SWIFT_IDIOMS_WHERE_CLAUSE_RULE: &str = "UseWhereClausesInForLoops";

/// A `for` loop that filters with a nested `if`.
///
/// `builtin/validators/swift/rules/immutability.md` names this shape as a
/// DON'T whose fix is `map` or `filter`, and
/// [`SWIFT_IDIOMS_WHERE_CLAUSE_RULE`] rewrites it into the `where` clause that
/// rule names as its OTHER DON'T.
const SWIFT_IDIOMS_NESTED_IF_FOR_LOOP: &str = concat!(
    "public enum Nesting {\n",
    "    public static func walk(_ things: [Int]) {\n",
    "        for thing in things { if thing > 2 { print(thing) } }\n",
    "    }\n",
    "}\n",
);

/// Acceptance: the gate leaves `UseWhereClausesInForLoops` OFF.
///
/// Row 5 of the measurement table of `idioms-swift.md` records the reason, and
/// it is a conflict rather than a preference. `immutability.md` names BOTH the
/// nested `if` and the `where` clause as DON'Ts, and its fix is `map` or
/// `filter`. Measured with that rule ON: it reports the nested `if`, and
/// `swift format --in-place` rewrites the probe below into the `where` clause
/// `immutability.md` refuses, character for character. So an author who takes
/// the finding and applies the tool's own correction lands on a DON'T.
///
/// The body of `idioms-swift.md` states two answers for that and chooses
/// neither, because the choice is a person's. Until a person makes it, the
/// rule stays OFF, and this test is what holds it there: the allowlist must
/// not name it, the script must not name it anywhere else either — a
/// configuration key would turn it on with no allowlist entry — and the gate
/// must stay silent over the shape.
#[test]
fn the_shipped_swift_idioms_tool_rule_leaves_the_where_clause_rule_off() {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_IDIOMS_RULE);
    let shipped = required_shipped_tool_rule(&loader, SWIFT_IDIOMS_RULE);

    assert!(
        !shipped.script.contains(SWIFT_IDIOMS_WHERE_CLAUSE_RULE),
        "the shipped script must name `{SWIFT_IDIOMS_WHERE_CLAUSE_RULE}` nowhere, because \
         `{SWIFT_IMMUTABILITY_PROMPT_RULE}.md` refuses the shape its own fix writes and no \
         person has chosen between the two answers `{SWIFT_IDIOMS_RULE}.md` records"
    );

    let reported = swift_idioms_reporting_rules(SWIFT_IDIOMS_NESTED_IF_FOR_LOOP, NO_SUPPORT_FILES);
    assert!(
        !reported.contains(&SWIFT_IDIOMS_WHERE_CLAUSE_RULE.to_string()),
        "the gate must stay silent over a `for` loop that filters with a nested `if`, \
         because `{SWIFT_IMMUTABILITY_PROMPT_RULE}.md` decides that shape; the run reported \
         {reported:?}"
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

/// The utilities a gate script of this set reaches for.
///
/// The list is deliberately wider than what `idioms-swift` runs today, because
/// its job is to catch the utility a LATER edit adds. Every name here that
/// stands in the shipped script as a word must stand in `doctor.check_command`
/// as well, so a run that reaches for a utility the machine has not got reports
/// the tool missing rather than failing halfway through a file.
///
/// `printf` and `test` are absent on purpose: every POSIX shell carries both as
/// built-ins, so neither one is a binary `which` can answer for.
///
/// The reading below finds a WORD, and it cannot tell a command from a name, so
/// no `awk` variable and no shell variable of the script may carry one of these
/// names. That costs the script one word and it keeps the reading whole.
const SWIFT_IDIOMS_SHELL_UTILITIES: &[&str] = &[
    "awk", "basename", "cat", "cut", "dirname", "find", "grep", "head", "mktemp", "paste", "rm",
    "sed", "sort", "swift", "tail", "tr", "uniq", "wc", "xargs",
];

/// The path a probe stages its one file at.
///
/// A distinct type, so a path and the source staged at it cannot trade places.
#[derive(Clone, Copy)]
struct ProbePath<'a>(&'a str);

/// The name of a shell utility a script may run.
///
/// A distinct type, so the utility and the script that may run it cannot trade
/// places.
#[derive(Clone, Copy)]
struct Utility<'a>(&'a str);

/// What a failure message calls the text a helper reads, such as `body`.
///
/// A distinct type, so the label and the text it names cannot trade places.
#[derive(Clone, Copy)]
struct Subject<'a>(&'a str);

/// A mark whose position in a text a helper looks up.
///
/// A distinct type, so the mark and the text it stands in cannot trade places.
#[derive(Clone, Copy)]
struct Marker<'a>(&'a str);

/// The two ends of a span of text: what opens it and what closes it.
///
/// Named fields, so the opening and the closing cannot trade places.
#[derive(Clone, Copy)]
struct Between<'a> {
    head: &'a str,
    end: &'a str,
}

/// The heading line a body table stands under.
///
/// A distinct type, so the heading and the body it stands in cannot trade
/// places.
#[derive(Clone, Copy)]
struct Heading<'a>(&'a str);

/// Whether `script` runs `utility` — the name standing in it as a whole word.
///
/// A word ends at any character that is neither a letter, a digit, an
/// underscore nor a dash. So `format` never reads as `rm`, `trap` never reads
/// as `tr`, and `sah-probe.swift` reads as `swift`, which the script does run.
fn script_runs_utility(script: &str, utility: Utility<'_>) -> bool {
    let utility = utility.0;
    let word = |letter: char| letter.is_alphanumeric() || letter == '_' || letter == '-';

    script.match_indices(utility).any(|(at, _)| {
        !script[..at].chars().next_back().is_some_and(word)
            && !script[at + utility.len()..]
                .chars()
                .next()
                .is_some_and(word)
    })
}

/// Acceptance: `doctor.check_command` names every utility the shipped script
/// runs, and names no other.
///
/// The check is what stands between a missing utility and a run that judges
/// half a work list. It is all-or-nothing over the whole command, so a utility
/// the script runs and the check does not ask for is a utility whose absence
/// the doctor reports as healthy.
///
/// Both directions are load-bearing. A utility run and not named leaves that
/// hole open; a utility named and not run makes the gate ask a machine for a
/// tool it has no use for, which reports a healthy rule as broken.
#[test]
fn the_shipped_swift_idioms_rule_doctor_names_every_utility_its_script_runs() {
    let loader = builtin_loader();
    let shipped = required_shipped_tool_rule(&loader, SWIFT_IDIOMS_RULE);
    let check = shipped
        .check_command
        .as_deref()
        .unwrap_or_else(|| panic!("`{SWIFT_IDIOMS_RULE}` must carry a `doctor.check_command`"));

    let asked: std::collections::BTreeSet<&str> = checked_binaries(check).into_iter().collect();
    let run: std::collections::BTreeSet<&str> = SWIFT_IDIOMS_SHELL_UTILITIES
        .iter()
        .copied()
        .filter(|utility| script_runs_utility(&shipped.script, Utility(utility)))
        .collect();

    let unasked: Vec<&str> = run.difference(&asked).copied().collect();
    assert!(
        unasked.is_empty(),
        "`doctor.check_command` must ask for every utility the script runs; it reads \
         `{check}`, and the script also runs {unasked:?}"
    );

    let unrun: Vec<&str> = asked.difference(&run).copied().collect();
    assert!(
        unrun.is_empty(),
        "`doctor.check_command` must ask for nothing the script does not run; it reads \
         `{check}`, and the script runs none of {unrun:?}"
    );
}

/// Where one command of a shell line ends: at a pipe, a semicolon, an
/// ampersand or a newline.
///
/// A backslash before a newline CONTINUES the command rather than ending it,
/// so the reading below joins those pairs before it splits.
const SWIFT_IDIOMS_COMMAND_END: &[char] = &['|', ';', '&', '\n'];

/// What opens an option word rather than an operand.
///
/// The lone `-` the run block and the check each hand the tool is an OPERAND,
/// the path that names standing input, so this head leaves it out.
const SWIFT_IDIOMS_OPTION_HEAD: &str = "--";

/// A shell line continued onto the next: a backslash, then the newline.
const SWIFT_IDIOMS_LINE_CONTINUATION: &str = "\\\n";

/// Every option the `swift format lint` invocation inside `command` gives the
/// tool.
///
/// The reading opens at the lint verb and closes where that one command
/// closes, so a `which` list standing before it and a `|| status=$?` standing
/// after it reach the answer neither.
fn swift_format_lint_options(
    command: &str,
    what: Subject<'_>,
) -> std::collections::BTreeSet<String> {
    let head = format!("{SWIFT_TOOLCHAIN_TOOL} {SWIFT_FORMAT_SUBCOMMAND} {SWIFT_FORMAT_LINT_VERB}");
    let joined = command.replace(SWIFT_IDIOMS_LINE_CONTINUATION, " ");
    let what = what.0;
    let at = joined
        .find(&head)
        .unwrap_or_else(|| panic!("the {what} of `{SWIFT_IDIOMS_RULE}` must run `{head}`"));

    joined[at + head.len()..]
        .split(SWIFT_IDIOMS_COMMAND_END)
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .filter(|word| word.len() > SWIFT_IDIOMS_OPTION_HEAD.len())
        .filter(|word| word.starts_with(SWIFT_IDIOMS_OPTION_HEAD))
        .map(str::to_string)
        .collect()
}

/// Acceptance: `doctor.check_command` gives `swift format lint` every option
/// the run block gives it, and gives it no other.
///
/// The check is the only thing that stands between a toolchain under the floor
/// and a run that declines every file. It reports one answer for the whole
/// rule, so an option the run block depends on and the check does not exercise
/// is an option whose refusal the doctor reports as HEALTHY.
///
/// `--assume-filename` beside the operand `-` is that option pair: Swift 6.0
/// and Swift 6.1 each raise a `ValidationError` for it and exit 64, and the
/// guard of the run block reads 64 as trouble, so the gate then judges
/// nothing while the doctor says the rule is well.
///
/// Both directions are load-bearing. An option run and not checked leaves that
/// hole open; an option checked and not run makes the doctor measure a
/// toolchain answer the gate has no use for.
#[test]
fn the_shipped_swift_idioms_rule_doctor_measures_every_option_its_script_gives_the_tool() {
    let loader = builtin_loader();
    let shipped = required_shipped_tool_rule(&loader, SWIFT_IDIOMS_RULE);
    let check = shipped
        .check_command
        .as_deref()
        .unwrap_or_else(|| panic!("`{SWIFT_IDIOMS_RULE}` must carry a `doctor.check_command`"));

    let run = swift_format_lint_options(&shipped.script, Subject("run block"));
    let asked = swift_format_lint_options(check, Subject("`doctor.check_command`"));

    assert_eq!(
        run, asked,
        "`doctor.check_command` of `{SWIFT_IDIOMS_RULE}` must give `swift format lint` the same \
         options the run block gives it, or the check reports healthy over an option the run \
         needs; the run block gives {run:?} and the check gives {asked:?}"
    );
}

/// The Swift version this gate needs.
///
/// The `swift format` SUBCOMMAND arrived at Swift 6.0, but this gate hands the
/// tool `--assume-filename` beside the operand `-`, and only Swift 6.2 and
/// above accept that pair. `release/6.0` and `release/6.1` of
/// `swiftlang/swift-format` each guard the option with
/// `if assumeFilename != nil && !paths.isEmpty`, and `-` IS a path, so each
/// raises a `ValidationError` and exits 64; `release/6.2` and `main` each read
/// `!(paths.isEmpty || paths == ["-"])` instead.
const SWIFT_IDIOMS_VERSION_FLOOR: &str = "6.2";

/// The Xcode release that ships [`SWIFT_IDIOMS_VERSION_FLOOR`].
///
/// Read from `https://www.swift.org/api/v1/install/releases.json`, which names
/// `Xcode 26` for Swift 6.2, released 2025-09-15.
const SWIFT_IDIOMS_FLOOR_XCODE: &str = "26";

/// What opens the sentence of the rule body that states the floor of the gate.
const SWIFT_IDIOMS_BODY_FLOOR_HEAD: &str = "The floor of this GATE is **Swift ";

/// What closes that sentence's version.
const SWIFT_IDIOMS_BODY_FLOOR_END: &str = "**";

/// What follows the Swift version `doctor.fix_hint` names.
const SWIFT_IDIOMS_HINT_FLOOR_MARK: &str = " or newer";

/// What follows the Xcode release the body and `doctor.fix_hint` each name.
const SWIFT_IDIOMS_XCODE_MARK: &str = " and above ship it";

/// The word standing immediately before the first `mark` of `text`.
fn word_before(text: &str, mark: Marker<'_>, what: Subject<'_>) -> String {
    let (mark, what) = (mark.0, what.0);
    let at = text
        .find(mark)
        .unwrap_or_else(|| panic!("the {what} of `{SWIFT_IDIOMS_RULE}` must state `{mark}`"));

    text[..at]
        .split_whitespace()
        .next_back()
        .unwrap_or_else(|| {
            panic!("the {what} of `{SWIFT_IDIOMS_RULE}` must name a version before `{mark}`")
        })
        .to_string()
}

/// The text standing between the first `head` of `text` and the next `end`
/// after it.
fn text_between(text: &str, span: Between<'_>, what: Subject<'_>) -> String {
    let (head, end, what) = (span.head, span.end, what.0);
    let at = text
        .find(head)
        .unwrap_or_else(|| panic!("the {what} of `{SWIFT_IDIOMS_RULE}` must state `{head}`"));
    let rest = &text[at + head.len()..];
    let shut = rest
        .find(end)
        .unwrap_or_else(|| panic!("the {what} of `{SWIFT_IDIOMS_RULE}` must close `{head}`"));

    rest[..shut].to_string()
}

/// Acceptance: the rule states ONE Swift version floor, and it is the version
/// that first accepts the option pair the run block gives the tool.
///
/// A person who reads an unhealthy doctor reads `doctor.fix_hint` and installs
/// what it names. A hint that names a version under the floor sends that
/// person to a toolchain that cannot run the gate, so the hint and the body
/// must not be able to disagree, and neither may drift under
/// [`SWIFT_IDIOMS_VERSION_FLOOR`].
#[test]
fn the_shipped_swift_idioms_rule_states_one_swift_version_floor() {
    let loader = builtin_loader();
    let source = shipped_swift_idioms_source(&loader);
    let hint = shipped_swift_idioms_fix_hint(&loader);

    let stated = text_between(
        &source,
        Between {
            head: SWIFT_IDIOMS_BODY_FLOOR_HEAD,
            end: SWIFT_IDIOMS_BODY_FLOOR_END,
        },
        Subject("body"),
    );
    let hinted = word_before(
        &hint,
        Marker(SWIFT_IDIOMS_HINT_FLOOR_MARK),
        Subject("`doctor.fix_hint`"),
    );

    assert_eq!(
        stated, hinted,
        "the body of `{SWIFT_IDIOMS_RULE}.md` and its `doctor.fix_hint` must state ONE Swift \
         floor; the body states {stated:?} and the hint states {hinted:?}"
    );
    assert_eq!(
        stated, SWIFT_IDIOMS_VERSION_FLOOR,
        "`{SWIFT_IDIOMS_RULE}` must state the floor `--assume-filename` beside `-` first \
         accepts; it states {stated:?}"
    );

    let body_xcode = word_before(&source, Marker(SWIFT_IDIOMS_XCODE_MARK), Subject("body"));
    let hint_xcode = word_before(
        &hint,
        Marker(SWIFT_IDIOMS_XCODE_MARK),
        Subject("`doctor.fix_hint`"),
    );

    assert_eq!(
        body_xcode, hint_xcode,
        "the body of `{SWIFT_IDIOMS_RULE}.md` and its `doctor.fix_hint` must name ONE Xcode \
         release for that floor; the body names {body_xcode:?} and the hint names \
         {hint_xcode:?}"
    );
    assert_eq!(
        body_xcode, SWIFT_IDIOMS_FLOOR_XCODE,
        "`{SWIFT_IDIOMS_RULE}` must name the Xcode release that ships Swift \
         {SWIFT_IDIOMS_VERSION_FLOOR}; it names {body_xcode:?}"
    );
}

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

/// Each form the probe above is written in, with the prompt rule that states
/// it.
///
/// The pairs are what make [`SWIFT_PROMPT_RULE_ANSWER`] the PROMPT RULES' own
/// answer rather than a shape this file invented: every form must stand, word
/// for word, in the body of the rule that names it.
const SWIFT_PROMPT_RULE_FORMS: &[(&str, &str)] = &[
    (SWIFT_IDIOMS_PROMPT_RULE, "var items: [Int] = []"),
    (SWIFT_IDIOMS_PROMPT_RULE, "var ids: Set<String> = []"),
    (SWIFT_OPTIONALS_PROMPT_RULE, "try #require("),
];

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
///
/// Each form the probe is written in is held to standing in the body of the
/// prompt rule that states it. Without that, an edit to either prompt rule
/// would leave this test measuring a house style nothing ships.
#[test]
fn the_shipped_swift_idioms_tool_rule_agrees_with_the_swift_prompt_rules() {
    let loader = builtin_loader();

    for (rule, form) in SWIFT_PROMPT_RULE_FORMS {
        let body = swift_prompt_rule_body(&loader, rule);
        assert!(
            body.contains(form),
            "`{rule}.md` must state `{form}`, or the probe below measures a form no \
             prompt rule asks for"
        );
        assert!(
            SWIFT_PROMPT_RULE_ANSWER.contains(form),
            "`{form}` stands in `{rule}.md` and in no probe, so nothing measures it"
        );
    }

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
/// The line stands, character for character, in the `idioms.md` bullet that
/// states the `where` half.
const SWIFT_IDIOMS_FILTER_CHAIN: &str = concat!(
    "public enum Filtering {\n",
    "    public static func walk(_ things: [Int]) {\n",
    "        things.filter { $0 > 2 }.forEach { thing in print(thing) }\n",
    "    }\n",
    "}\n",
);

/// The DON'T of the `idioms.md` bullet that states the `where` half, word for
/// word as [`SWIFT_IDIOMS_FILTER_CHAIN`] writes it.
const SWIFT_IDIOMS_FILTER_CHAIN_FORM: &str =
    "things.filter { $0 > 2 }.forEach { thing in print(thing) }";

/// Acceptance: `ReplaceForEachWithForLoop` reports the `filter` chain, and its
/// message asks for a for-in loop rather than for the `where` clause the
/// prompt bullet asks for.
///
/// This is the row the body of `idioms-swift.md` records under "Which rule
/// reports a filter chain", and it answers a question a person has to see: an
/// author who takes this finding is told to write a for-in loop, and the
/// `where` clause the bullet wants is a further edit the tool never names.
///
/// The tool CORRECTS nothing here. Measured with Apple Swift 6.4,
/// `swift format --in-place` with this rule ON leaves the file byte for byte
/// as it was, so the message is the whole of what the author receives.
///
/// Both halves are load-bearing. The gate must report the chain, or the row
/// the body records is not true; and `idioms.md` must state the chain word for
/// word, or the probe measures a shape no prompt rule asks for.
#[test]
fn the_shipped_swift_idioms_tool_rule_reports_a_filter_chain() {
    let loader = builtin_loader();
    let body = swift_prompt_rule_body(&loader, SWIFT_IDIOMS_PROMPT_RULE);

    assert!(
        body.contains(SWIFT_IDIOMS_FILTER_CHAIN_FORM),
        "`{SWIFT_IDIOMS_PROMPT_RULE}.md` must state `{SWIFT_IDIOMS_FILTER_CHAIN_FORM}`, or \
         the probe below measures a shape no prompt rule asks for"
    );
    assert!(
        SWIFT_IDIOMS_FILTER_CHAIN.contains(SWIFT_IDIOMS_FILTER_CHAIN_FORM),
        "the probe must hold the form `{SWIFT_IDIOMS_PROMPT_RULE}.md` states, or the two \
         measure different shapes"
    );

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
/// The `NoVoidReturnOnFunctionSignature` row carries the `()` half of the
/// bullet `idioms.md` split, and the rule decides the omit-the-clause half as
/// well. `idioms.md` still states that second half, so it is not a row here:
/// the bullet has two owners until a person moves it, and the task that moves
/// it is the one that rebalances the Swift prompt rules.
///
/// `ReplaceForEachWithForLoop` carries the `forEach` + `if` half the same way,
/// and it reports the `filter` chain of the surviving `where` bullet as well —
/// `the_shipped_swift_idioms_tool_rule_reports_a_filter_chain` measures that.
///
/// One bullet this gate USED to own is not here at all. `value-semantics.md`
/// "Mark classes not designed for subclassing" was `preferFinalClasses`, and
/// the toolchain carries no rule like it, so that requirement waits for the
/// prompt rule to state it again.
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
    let loader = builtin_loader();

    for bullet in SWIFT_IDIOMS_SUPERSEDED_BULLETS {
        let reported = swift_idioms_reporting_rules(bullet.defect, NO_SUPPORT_FILES);
        verify_superseded_swift_bullet(&loader, bullet, &reported);
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

/// The command that reaches the Swift toolchain's own formatter.
///
/// `swift format` is a SUBCOMMAND of the toolchain, spelled with a space. It
/// is a different program from `swiftformat`, which is what the shipped gate
/// runs, and the rule body carries the measurements that separate the two.
const SWIFT_TOOLCHAIN_TOOL: &str = "swift";

/// The subcommand that reaches the toolchain's formatter.
const SWIFT_FORMAT_SUBCOMMAND: &str = "format";

/// The `swift format` verb that reports a finding and rewrites nothing.
const SWIFT_FORMAT_LINT_VERB: &str = "lint";

/// The flag that makes a `swift format lint` finding answer a nonzero status.
const SWIFT_FORMAT_STRICT_FLAG: &str = "--strict";

/// The flag that hands `swift format` a configuration, as a path or as the
/// JSON itself.
const SWIFT_FORMAT_CONFIGURATION_FLAG: &str = "--configuration";

/// The `swift format` verb that writes the configuration it would read.
const SWIFT_FORMAT_DUMP_VERB: &str = "dump-configuration";

/// The configuration key that carries one switch for each rule.
const SWIFT_FORMAT_RULES_KEY: &str = "rules";

/// How many rules the configuration of the measured toolchain carries.
///
/// Measured with Apple Swift 6.4: `swift format dump-configuration` writes 43
/// keys under `rules`. The count is the assertion that a release that adds or
/// drops a rule reaches this guard, because the tables below state that every
/// one of the 43 is switched OFF.
const SWIFT_FORMAT_RULE_COUNT: usize = 43;

/// The markdown source of the shipped `idioms-swift` rule, front matter and
/// body alike.
fn shipped_swift_idioms_source(loader: &ValidatorLoader) -> String {
    std::fs::read_to_string(shipped_asset(loader, &RULE_SOURCE_ASSET, SWIFT_IDIOMS_RULE))
        .expect("read the shipped Swift idioms rule source")
}

/// The `doctor.fix_hint` the set ships for the `idioms-swift` rule.
///
/// The hint is the one sentence a person reads when the doctor reports the
/// rule unhealthy, so a guard over what it PROMISES reads the shipped bytes.
fn shipped_swift_idioms_fix_hint(loader: &ValidatorLoader) -> String {
    required_shipped_tool_rule(loader, SWIFT_IDIOMS_RULE)
        .fix_hint
        .unwrap_or_else(|| panic!("`{SWIFT_IDIOMS_RULE}` must carry a `doctor.fix_hint`"))
}

/// The `swift format` configuration of the installed toolchain, with every
/// rule switched OFF.
///
/// The switches come from the toolchain's own dump rather than from a list
/// this file writes, so a release that adds a rule turns that rule off here as
/// well. A tag the run still writes under this configuration is a tag no
/// configuration can stop.
fn swift_format_configuration_with_every_rule_off() -> String {
    let mut configuration = swift_format_dumped_configuration();

    let rules = configuration
        .get_mut(SWIFT_FORMAT_RULES_KEY)
        .and_then(serde_json::Value::as_object_mut)
        .expect("the swift-format configuration must carry a `rules` table");

    for switch in rules.values_mut() {
        *switch = serde_json::Value::Bool(false);
    }

    configuration.to_string()
}

/// The whole configuration `swift format dump-configuration` writes.
fn swift_format_dumped_configuration() -> serde_json::Value {
    let dumped = std::process::Command::new(SWIFT_TOOLCHAIN_TOOL)
        .arg(SWIFT_FORMAT_SUBCOMMAND)
        .arg(SWIFT_FORMAT_DUMP_VERB)
        .output()
        .expect("the installed Swift toolchain must write its swift-format configuration");

    assert!(
        dumped.status.success(),
        "`{SWIFT_TOOLCHAIN_TOOL} {SWIFT_FORMAT_SUBCOMMAND} {SWIFT_FORMAT_DUMP_VERB}` must write \
         the configuration every measurement of this rule reads; it exited {} and wrote {:?} on \
         stderr. A run that read the stdout alone would land on empty JSON and name the wrong \
         cause",
        dumped.status,
        String::from_utf8_lossy(&dumped.stderr)
    );

    serde_json::from_slice(&dumped.stdout)
        .expect("`swift format dump-configuration` must write JSON")
}

/// The `rules` table of that configuration, one switch for each rule the
/// installed toolchain carries.
fn swift_format_dumped_rules() -> serde_json::Map<String, serde_json::Value> {
    let mut configuration = swift_format_dumped_configuration();

    let rules = configuration
        .get_mut(SWIFT_FORMAT_RULES_KEY)
        .and_then(serde_json::Value::as_object_mut)
        .expect("the swift-format configuration must carry a `rules` table");

    rules.clone()
}

/// What `swift format lint` answered: its exit status, its stdout, its stderr.
type SwiftFormatLintRun = (i32, String, String);

/// Runs `swift format lint --strict` over `path`, under `configuration` when
/// the caller states one.
fn swift_format_lint(path: &Path, configuration: Option<&str>) -> SwiftFormatLintRun {
    let mut command = std::process::Command::new(SWIFT_TOOLCHAIN_TOOL);
    command
        .arg(SWIFT_FORMAT_SUBCOMMAND)
        .arg(SWIFT_FORMAT_LINT_VERB)
        .arg(SWIFT_FORMAT_STRICT_FLAG);

    if let Some(configuration) = configuration {
        command
            .arg(SWIFT_FORMAT_CONFIGURATION_FLAG)
            .arg(configuration);
    }

    let run = command
        .arg(path)
        .output()
        .expect("the installed Swift toolchain must run `swift format lint`");

    (
        run.status
            .code()
            .expect("`swift format lint` must exit rather than be stopped by a signal"),
        String::from_utf8_lossy(&run.stdout).into_owned(),
        String::from_utf8_lossy(&run.stderr).into_owned(),
    )
}

/// A throwaway directory to stage the probe files of one `swift format` run
/// in.
fn swift_format_probe_directory() -> tempfile::TempDir {
    tempfile::tempdir().expect("stage a probe directory")
}

/// What opens and closes every cell of a markdown table row.
const RULE_BODY_TABLE_EDGE: char = '|';

/// What the rule under a table head is drawn with.
const RULE_BODY_TABLE_RULE: char = '-';

/// What a rule body wraps a name in.
const RULE_BODY_CODE_MARK: char = '`';

/// Every measurement row of the markdown table that stands under `heading` in
/// `body`, as its cells.
///
/// The head row and the rule under it are dropped, so what comes back is the
/// rows the table measures and nothing else. A heading the body does not state
/// answers NO row, which is what makes a test that reads a table it expects
/// fail rather than pass over a table nobody wrote.
fn rule_body_table(body: &str, heading: Heading<'_>) -> Vec<Vec<String>> {
    let heading = heading.0;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut under_heading = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed == heading {
            under_heading = true;
            continue;
        }
        if !under_heading {
            continue;
        }
        if !trimmed.starts_with(RULE_BODY_TABLE_EDGE) {
            if rows.is_empty() {
                continue;
            }
            break;
        }

        let cells: Vec<String> = trimmed
            .trim_matches(RULE_BODY_TABLE_EDGE)
            .split(RULE_BODY_TABLE_EDGE)
            .map(|cell| cell.trim().to_string())
            .collect();

        let is_rule = cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|mark| mark == RULE_BODY_TABLE_RULE));
        if !is_rule {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return rows;
    }
    rows.split_off(1)
}

/// Reads the case names of a Swift type out of the reflection metadata a
/// Mach-O image carries.
///
/// `swift format` writes a `[<Name>]` tag for every finding, and the tags the
/// 43 configurable rules do NOT own are the cases of two enumerations inside
/// the tool. Neither the command line nor `dump-configuration` names those
/// cases, and `strings` cannot answer the question either, for a reason that
/// has nothing to do with which names stand in the file. Measured on the Apple
/// Swift 6.4 binary, `strings -a <binary> | grep -cx <case>` writes 1 or more
/// for every one of the nine CASE names, each of them in `__swift5_reflstr`.
/// What `strings` never writes is the OWNER of a name: `indentation` alone
/// stands five times, in `__objc_methname` as well, and nothing in the output
/// says which of the five is a case of `WhitespaceFindingCategory`. Only the
/// reflection metadata pairs a name with the type that holds it, and that
/// pairing is the whole question, so that is what this module reads, out of
/// the `__swift5_fieldmd` section of the image.
///
/// It reads a Mach-O image, so it stands under macOS alone. That is the image
/// format of the toolchain the self-hosted CI runner ships, and it is the
/// toolchain every measurement of `idioms-swift.md` was made with.
#[cfg(target_os = "macos")]
mod swift_reflection {
    use std::collections::{BTreeMap, BTreeSet};

    /// The magic of a 64-bit little-endian Mach-O image.
    const MACH_O_MAGIC_64: u32 = 0xfeed_facf;

    /// The load command that describes one 64-bit segment.
    const MACH_O_SEGMENT_COMMAND_64: u32 = 0x19;

    /// Where the count of load commands stands in a 64-bit Mach-O header.
    const MACH_O_COMMAND_COUNT_OFFSET: usize = 16;

    /// Where the load commands themselves start.
    const MACH_O_HEADER_SIZE: usize = 32;

    /// Where the byte count of a load command stands inside it.
    const MACH_O_COMMAND_SIZE_OFFSET: usize = 4;

    /// Where the count of sections stands in a `segment_command_64`.
    const MACH_O_SEGMENT_SECTION_COUNT_OFFSET: usize = 64;

    /// Where the section records of a `segment_command_64` start.
    const MACH_O_SEGMENT_SECTIONS_OFFSET: usize = 72;

    /// How many bytes one `section_64` record holds.
    const MACH_O_SECTION_SIZE: usize = 80;

    /// How many bytes the name of a `section_64` holds.
    const MACH_O_SECTION_NAME_SIZE: usize = 16;

    /// Where the mapped address of a `section_64` stands inside its record.
    const MACH_O_SECTION_ADDRESS_OFFSET: usize = 32;

    /// Where the byte count of a `section_64` stands inside its record.
    const MACH_O_SECTION_BYTES_OFFSET: usize = 40;

    /// Where the file offset of a `section_64` stands inside its record.
    const MACH_O_SECTION_FILE_OFFSET_OFFSET: usize = 48;

    /// The section that holds one field descriptor for each type of the image.
    const SWIFT_FIELD_METADATA_SECTION: &str = "__swift5_fieldmd";

    /// How many bytes the head of a field descriptor holds, ahead of the
    /// records of its own fields.
    const SWIFT_FIELD_DESCRIPTOR_HEAD_SIZE: usize = 16;

    /// Where the byte count of one field record stands in that head.
    const SWIFT_FIELD_RECORD_SIZE_OFFSET: usize = 10;

    /// Where the count of field records stands in that head.
    const SWIFT_FIELD_RECORD_COUNT_OFFSET: usize = 12;

    /// Where the name of a field record stands inside the record.
    const SWIFT_FIELD_RECORD_NAME_OFFSET: u64 = 8;

    /// Where the name of a nominal type descriptor stands inside it.
    const SWIFT_TYPE_DESCRIPTOR_NAME_OFFSET: u64 = 8;

    /// The byte that opens a DIRECT symbolic reference to a type descriptor.
    ///
    /// A mangled type name that opens with it is one byte of tag and a relative
    /// pointer rather than text, and the name the descriptor carries is the one
    /// a person reads.
    const SWIFT_DIRECT_SYMBOLIC_REFERENCE: u8 = 0x01;

    /// How many bytes a 16-bit word holds.
    const HALF_WORD_SIZE: usize = 2;

    /// How many bytes a 32-bit word holds, which is also the width of every
    /// relative pointer of Swift's reflection metadata.
    const WORD_SIZE: usize = 4;

    /// How many bytes a 64-bit word holds.
    const LONG_WORD_SIZE: usize = 8;

    /// The suffix the name of every `swift format` finding-category type ends
    /// in.
    const CATEGORY_SUFFIX: &str = "FindingCategory";

    /// The finding-category type whose tag is the name of the RULE that
    /// reported.
    ///
    /// Its tags are the 43 rule names, and a configuration key stops each one,
    /// so it carries no tag the allowlist has to name.
    const RULE_CATEGORY: &str = "RuleBasedFindingCategory";

    /// The two finding-category types whose cases no configuration key can
    /// stop.
    ///
    /// The pretty-printer writes the cases of the first, and the whitespace
    /// linter writes the cases of the second. Both parts run whatever the
    /// configuration says.
    pub const UNSTOPPABLE_CATEGORIES: &[&str] =
        &["PrettyPrintFindingCategory", "WhitespaceFindingCategory"];

    /// The tool that answers where a toolchain command stands.
    const XCRUN_TOOL: &str = "xcrun";

    /// The flag that makes it write that path alone.
    const XCRUN_FIND_FLAG: &str = "--find";

    /// The binary the `swift format` subcommand runs.
    const SWIFT_FORMAT_BINARY: &str = "swift-format";

    /// Why this walk could not answer which tags the toolchain owns.
    ///
    /// Every arm is an EXPECTED answer rather than a defect of the walk: a
    /// machine with no Swift toolchain, a release that rebuilt the binary in
    /// another shape, and a path that names a file this walk cannot read all
    /// reach one of them. So the walk answers each of them, and the caller
    /// states the one it met. Nothing here panics, because a panic would put
    /// the wrong cause in front of the person reading the run.
    #[derive(Debug, thiserror::Error)]
    pub enum ReflectionFailure {
        /// The locator command could not be started at all.
        #[error("`{command}` could not be started: {source}")]
        LocatorFailed {
            /// The command, written the way a person would run it.
            command: String,

            /// What the operating system answered.
            source: std::io::Error,
        },

        /// The locator ran and named no path.
        ///
        /// This is the answer a machine with no Swift toolchain gives, and it
        /// is the one shape whose own message names the cause. Measured with
        /// Apple Swift 6.4, `xcrun --find swift-format-does-not-exist` exits
        /// 72, writes NOTHING on stdout, and writes `unable to find utility
        /// ...` on stderr. A caller that read the status alone would carry an
        /// empty path forward and report a missing file, which names the wrong
        /// cause, so the status and the stderr both stand here.
        #[error("`{command}` exited {status} and named no path; it wrote {stderr:?} on stderr")]
        NotLocated {
            /// The command, written the way a person would run it.
            command: String,

            /// How the run ended.
            status: String,

            /// What the run wrote on stderr, which names the cause.
            stderr: String,
        },

        /// The path the locator named could not be read.
        #[error("read the toolchain binary at `{path}`: {source}")]
        Unreadable {
            /// The path the locator named.
            path: String,

            /// What the operating system answered.
            source: std::io::Error,
        },

        /// The file the locator named is not the image shape this walk reads.
        #[error(
            "`{path}` is not a 64-bit little-endian Mach-O image; it opens with {magic:#010x}"
        )]
        NotMachO {
            /// The path the locator named.
            path: String,

            /// The word the file opens with.
            magic: u32,
        },

        /// A window this walk asked for runs past the end of the image.
        #[error(
            "the toolchain binary holds {held} bytes, and this walk asked for {wanted} at {offset}"
        )]
        Truncated {
            /// Where the window opens.
            offset: usize,

            /// How many bytes the window holds.
            wanted: usize,

            /// How many bytes the image holds.
            held: usize,
        },

        /// A load command of zero bytes, which no walk can step over.
        #[error("the load command at {offset} states a length of zero bytes")]
        EmptyLoadCommand {
            /// Where that load command stands.
            offset: usize,
        },

        /// A reflection address that falls inside no section of the image.
        #[error(
            "the reflection address {address:#x} falls inside no section of the toolchain binary"
        )]
        Unmapped {
            /// The address that falls outside every section.
            address: u64,
        },

        /// A reflection string that runs to the end of the image with no NUL.
        #[error("the reflection string at {address:#x} runs to the end of the toolchain binary")]
        Unterminated {
            /// Where that string opens.
            address: u64,
        },

        /// An image that carries no field metadata for this walk to read.
        #[error("the toolchain binary carries no `{section}` section")]
        NoFieldMetadata {
            /// The section the walk reads.
            section: &'static str,
        },

        /// An image whose finding-category types are not the ones measured.
        ///
        /// A release that adds a category type, renames one, or drops one
        /// reaches this arm, rather than answering a short list that the two
        /// hand-written lists of the rule body still match.
        #[error(
            "the reflection metadata of `{path}` must carry the finding-category types the \
             `{rule}.md` measurement names, which are {expected:?}; it carries {named:?}"
        )]
        UnexpectedCategories {
            /// The path the locator named.
            path: String,

            /// The shipped rule whose measurement names the types.
            rule: &'static str,

            /// The type names the measurement records.
            expected: Vec<String>,

            /// The type names the binary carries.
            named: Vec<String>,
        },
    }

    /// One section of a Mach-O image.
    struct Section {
        /// The section's own name, as `__swift5_fieldmd`.
        name: String,

        /// The address the image maps the section at.
        address: u64,

        /// How many bytes the section holds.
        bytes: u64,

        /// Where those bytes stand in the file.
        file_offset: usize,
    }

    /// The locator command, written the way a person would run it.
    fn locator_command() -> String {
        format!("{XCRUN_TOOL} {XCRUN_FIND_FLAG} {SWIFT_FORMAT_BINARY}")
    }

    /// The `N` bytes of `image` at `offset`.
    ///
    /// A window that runs past the end answers a failure rather than panicking,
    /// because a file that is not the image this walk reads is one of the
    /// answers the locator can give.
    fn window<const N: usize>(image: &[u8], offset: usize) -> Result<[u8; N], ReflectionFailure> {
        image
            .get(offset..offset.saturating_add(N))
            .and_then(|bytes| <[u8; N]>::try_from(bytes).ok())
            .ok_or(ReflectionFailure::Truncated {
                offset,
                wanted: N,
                held: image.len(),
            })
    }

    /// The unsigned 32-bit word at `offset`.
    fn word(image: &[u8], offset: usize) -> Result<u32, ReflectionFailure> {
        Ok(u32::from_le_bytes(window::<WORD_SIZE>(image, offset)?))
    }

    /// The signed 32-bit word at `offset`.
    fn signed_word(image: &[u8], offset: usize) -> Result<i32, ReflectionFailure> {
        Ok(i32::from_le_bytes(window::<WORD_SIZE>(image, offset)?))
    }

    /// The unsigned 16-bit word at `offset`.
    fn half_word(image: &[u8], offset: usize) -> Result<u16, ReflectionFailure> {
        Ok(u16::from_le_bytes(window::<HALF_WORD_SIZE>(image, offset)?))
    }

    /// The unsigned 64-bit word at `offset`.
    fn long_word(image: &[u8], offset: usize) -> Result<u64, ReflectionFailure> {
        Ok(u64::from_le_bytes(window::<LONG_WORD_SIZE>(image, offset)?))
    }

    /// The name a `section_64` record at `record` carries.
    fn section_name(image: &[u8], record: usize) -> Result<String, ReflectionFailure> {
        let raw = window::<MACH_O_SECTION_NAME_SIZE>(image, record)?;
        let named = raw.split(|byte| *byte == 0).next().unwrap_or_default();
        Ok(String::from_utf8_lossy(named).into_owned())
    }

    /// Every section of `image`, in the order the load commands name them.
    fn sections(image: &[u8], path: &str) -> Result<Vec<Section>, ReflectionFailure> {
        let magic = word(image, 0)?;
        if magic != MACH_O_MAGIC_64 {
            return Err(ReflectionFailure::NotMachO {
                path: path.to_string(),
                magic,
            });
        }

        let mut found = Vec::new();
        let mut command = MACH_O_HEADER_SIZE;

        for _ in 0..word(image, MACH_O_COMMAND_COUNT_OFFSET)? {
            if word(image, command)? == MACH_O_SEGMENT_COMMAND_64 {
                let count = word(
                    image,
                    command.saturating_add(MACH_O_SEGMENT_SECTION_COUNT_OFFSET),
                )?;
                for index in 0..count as usize {
                    let record = command
                        .saturating_add(MACH_O_SEGMENT_SECTIONS_OFFSET)
                        .saturating_add(index.saturating_mul(MACH_O_SECTION_SIZE));
                    found.push(Section {
                        name: section_name(image, record)?,
                        address: long_word(
                            image,
                            record.saturating_add(MACH_O_SECTION_ADDRESS_OFFSET),
                        )?,
                        bytes: long_word(
                            image,
                            record.saturating_add(MACH_O_SECTION_BYTES_OFFSET),
                        )?,
                        file_offset: word(
                            image,
                            record.saturating_add(MACH_O_SECTION_FILE_OFFSET_OFFSET),
                        )? as usize,
                    });
                }
            }

            let length = word(image, command.saturating_add(MACH_O_COMMAND_SIZE_OFFSET))? as usize;
            if length == 0 {
                return Err(ReflectionFailure::EmptyLoadCommand { offset: command });
            }
            command = command.saturating_add(length);
        }

        Ok(found)
    }

    /// Where the byte mapped at `address` stands in the file.
    fn file_offset(sections: &[Section], address: u64) -> Result<usize, ReflectionFailure> {
        sections
            .iter()
            .find(|section| {
                section.address <= address
                    && address < section.address.saturating_add(section.bytes)
            })
            .map(|section| {
                section
                    .file_offset
                    .saturating_add((address - section.address) as usize)
            })
            .ok_or(ReflectionFailure::Unmapped { address })
    }

    /// The NUL-terminated bytes that start at `address`.
    fn text(
        image: &[u8],
        sections: &[Section],
        address: u64,
    ) -> Result<Vec<u8>, ReflectionFailure> {
        let start = file_offset(sections, address)?;
        let rest = image.get(start..).ok_or(ReflectionFailure::Truncated {
            offset: start,
            wanted: 0,
            held: image.len(),
        })?;
        let length = rest
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(ReflectionFailure::Unterminated { address })?;
        Ok(rest[..length].to_vec())
    }

    /// The address the Swift relative pointer at `address` names.
    ///
    /// Swift writes every pointer of its reflection metadata as a signed 32-bit
    /// offset from the field that HOLDS it, so the image runs wherever it is
    /// mapped.
    fn relative_target(
        image: &[u8],
        sections: &[Section],
        address: u64,
    ) -> Result<u64, ReflectionFailure> {
        let offset = signed_word(image, file_offset(sections, address)?)?;
        Ok(address.wrapping_add(offset as i64 as u64))
    }

    /// The name of the type a field descriptor at `descriptor` describes.
    ///
    /// A descriptor whose mangled name is not a direct symbolic reference to a
    /// nominal type descriptor answers NOTHING. This reads the shape the
    /// measured toolchain writes, so a release that wrote another shape drops
    /// the type out of the answer rather than making a name up for it.
    fn type_name(
        image: &[u8],
        sections: &[Section],
        descriptor: u64,
    ) -> Result<Option<String>, ReflectionFailure> {
        if signed_word(image, file_offset(sections, descriptor)?)? == 0 {
            return Ok(None);
        }

        let mangled = relative_target(image, sections, descriptor)?;
        if text(image, sections, mangled)?.first() != Some(&SWIFT_DIRECT_SYMBOLIC_REFERENCE) {
            return Ok(None);
        }

        let nominal = relative_target(image, sections, mangled.saturating_add(1))?;
        let named = relative_target(
            image,
            sections,
            nominal.saturating_add(SWIFT_TYPE_DESCRIPTOR_NAME_OFFSET),
        )?;
        Ok(Some(
            String::from_utf8_lossy(&text(image, sections, named)?).into_owned(),
        ))
    }

    /// The names of the `count` field records of the descriptor at
    /// `descriptor`, each `record_size` bytes wide.
    fn record_names(
        image: &[u8],
        sections: &[Section],
        descriptor: u64,
        record_size: u64,
        count: u64,
    ) -> Result<Vec<String>, ReflectionFailure> {
        (0..count)
            .map(|index| {
                let record = descriptor
                    .saturating_add(SWIFT_FIELD_DESCRIPTOR_HEAD_SIZE as u64)
                    .saturating_add(index.saturating_mul(record_size));
                let named = relative_target(
                    image,
                    sections,
                    record.saturating_add(SWIFT_FIELD_RECORD_NAME_OFFSET),
                )?;
                Ok(String::from_utf8_lossy(&text(image, sections, named)?).into_owned())
            })
            .collect()
    }

    /// The suffix a type name must end in to be read.
    ///
    /// A distinct type, so the suffix and the path of the image cannot trade
    /// places.
    #[derive(Clone, Copy)]
    struct TypeSuffix<'a>(&'a str);

    /// The case names of every type of `image` whose name ends in `suffix`,
    /// keyed by the type's own name.
    ///
    /// `path` names the file the bytes came from, so a failure states which
    /// image it read rather than leaving a person to find that out.
    fn cases_of_types_named(
        image: &[u8],
        path: &str,
        suffix: TypeSuffix<'_>,
    ) -> Result<BTreeMap<String, Vec<String>>, ReflectionFailure> {
        let sections = sections(image, path)?;
        let metadata = sections
            .iter()
            .find(|section| section.name == SWIFT_FIELD_METADATA_SECTION)
            .ok_or(ReflectionFailure::NoFieldMetadata {
                section: SWIFT_FIELD_METADATA_SECTION,
            })?;

        let mut found = BTreeMap::new();
        let mut walked: u64 = 0;

        while walked.saturating_add(SWIFT_FIELD_DESCRIPTOR_HEAD_SIZE as u64) <= metadata.bytes {
            let descriptor = metadata.address.saturating_add(walked);
            let head = metadata.file_offset.saturating_add(walked as usize);
            let record_size = u64::from(half_word(
                image,
                head.saturating_add(SWIFT_FIELD_RECORD_SIZE_OFFSET),
            )?);
            let count = u64::from(word(
                image,
                head.saturating_add(SWIFT_FIELD_RECORD_COUNT_OFFSET),
            )?);

            if let Some(name) = type_name(image, &sections, descriptor)? {
                if name.ends_with(suffix.0) {
                    found.insert(
                        name,
                        record_names(image, &sections, descriptor, record_size, count)?,
                    );
                }
            }

            walked = walked
                .saturating_add(SWIFT_FIELD_DESCRIPTOR_HEAD_SIZE as u64)
                .saturating_add(count.saturating_mul(record_size));
        }

        Ok(found)
    }

    /// The `[<Name>]` tag a finding-category case reaches the output as.
    ///
    /// `swift format` writes the case name with its first letter in upper
    /// case, so `spacingCharacter` reaches the output as `[SpacingCharacter]`.
    fn tag_of_case(case: &str) -> String {
        let mut letters = case.chars();
        match letters.next() {
            Some(first) => first.to_uppercase().chain(letters).collect(),
            None => String::new(),
        }
    }

    /// Every tag `swift format` writes that no key of the configuration stops,
    /// read out of the toolchain binary itself.
    ///
    /// This is a THIRD source, beside the tag column of the rule body and
    /// `SWIFT_FORMAT_TAG_PROBES`. One person wrote those two lists, so a tag
    /// that NEITHER list named failed nothing, which is how `SpacingCharacter`
    /// stood outside both of them. The tool wrote this list.
    ///
    /// The guard on the type names is what keeps the answer whole. A release
    /// that adds a finding-category type, renames one, or drops one answers
    /// [`ReflectionFailure::UnexpectedCategories`] here, rather than answering
    /// a short list that the two hand-written lists still match.
    pub fn tags_no_configuration_stops() -> Result<BTreeSet<String>, ReflectionFailure> {
        let command = locator_command();
        let located = std::process::Command::new(XCRUN_TOOL)
            .arg(XCRUN_FIND_FLAG)
            .arg(SWIFT_FORMAT_BINARY)
            .output()
            .map_err(|source| ReflectionFailure::LocatorFailed {
                command: command.clone(),
                source,
            })?;

        if !located.status.success() {
            return Err(ReflectionFailure::NotLocated {
                command,
                status: located.status.to_string(),
                stderr: String::from_utf8_lossy(&located.stderr).trim().to_string(),
            });
        }

        let path = String::from_utf8_lossy(&located.stdout).trim().to_string();
        let image = std::fs::read(&path).map_err(|source| ReflectionFailure::Unreadable {
            path: path.clone(),
            source,
        })?;

        let categories = cases_of_types_named(&image, &path, TypeSuffix(CATEGORY_SUFFIX))?;

        let mut expected: Vec<&str> = UNSTOPPABLE_CATEGORIES.to_vec();
        expected.push(RULE_CATEGORY);
        expected.sort_unstable();
        let named: Vec<&str> = categories.keys().map(String::as_str).collect();

        if named != expected {
            return Err(ReflectionFailure::UnexpectedCategories {
                rule: super::SWIFT_IDIOMS_RULE,
                expected: expected.iter().map(|name| (*name).to_string()).collect(),
                named: named.iter().map(|name| (*name).to_string()).collect(),
                path,
            });
        }

        Ok(categories
            .into_iter()
            .filter(|(name, _)| UNSTOPPABLE_CATEGORIES.contains(&name.as_str()))
            .flat_map(|(_, cases)| cases.into_iter().map(|case| tag_of_case(&case)))
            .collect())
    }
}

/// The heading of the rule-body table that names every tag the pretty-printer
/// and the whitespace linter write.
const SWIFT_FORMAT_TAG_TABLE_HEADING: &str = "## The tags no configuration can stop";

/// How many cells each row of that table holds.
const SWIFT_FORMAT_TAG_TABLE_WIDTH: usize = 3;

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

/// Acceptance: every tag the rule body records as one no configuration can
/// stop is a tag the installed toolchain still writes.
///
/// `swift format lint` writes a `[<Name>]` tag for each finding, and the tags
/// this table names come from the PRETTY-PRINTER and the WHITESPACE LINTER
/// rather than from the 43 rules. No key of the configuration reaches them:
/// each probe below runs with every one of the 43 rules switched OFF and still
/// draws its own tag.
///
/// That is why a gate built on `swift format` has to read the tag off each
/// output line and keep only the tags an allowlist states. This test is what
/// holds the allowlist the rule body states to the tags the tool really
/// writes: a toolchain release that stops writing one fails here BY NAME,
/// rather than leaving the gate filtering for a tag nothing sends.
///
/// THREE sources meet here, and the third is what makes the test whole. The
/// body's tag column and [`SWIFT_FORMAT_TAG_PROBES`] were both written by a
/// person, so holding them to EACH OTHER lets a tag that neither one names
/// pass unseen — which is how `SpacingCharacter`, the ninth tag, stood outside
/// both lists. So the SET comes from the tool: the cases of
/// `PrettyPrintFindingCategory` and `WhitespaceFindingCategory`, read out of
/// the reflection metadata of the toolchain binary by
/// `swift_reflection::tags_no_configuration_stops`. A tag the tool owns and
/// the two lists miss now fails here.
///
/// Each of the three is load-bearing. The body must name the same tags this
/// test probes, or the allowlist and the measurement drift apart; each probe
/// must draw its tag from the live tool, or the row records a fact no run
/// makes; and the two lists together must hold the whole case set, or the
/// allowlist is short.
#[test]
fn the_shipped_swift_idioms_rule_body_names_every_tag_swift_format_writes() {
    let loader = builtin_loader();
    let source = shipped_swift_idioms_source(&loader);
    let recorded = rule_body_table(&source, Heading(SWIFT_FORMAT_TAG_TABLE_HEADING));

    let named: Vec<String> = recorded
        .iter()
        .map(|row| {
            assert_eq!(
                row.len(),
                SWIFT_FORMAT_TAG_TABLE_WIDTH,
                "every row of the `{SWIFT_FORMAT_TAG_TABLE_HEADING}` table must hold \
                 {SWIFT_FORMAT_TAG_TABLE_WIDTH} cells; this one holds {row:?}"
            );
            row[0].trim_matches(RULE_BODY_CODE_MARK).to_string()
        })
        .collect();
    let probed: Vec<String> = SWIFT_FORMAT_TAG_PROBES
        .iter()
        .map(|(tag, _)| (*tag).to_string())
        .collect();

    assert_eq!(
        named, probed,
        "the `{SWIFT_FORMAT_TAG_TABLE_HEADING}` table of `{SWIFT_IDIOMS_RULE}.md` must name the \
         tags this test probes, in the order it probes them; it names {named:?}"
    );

    #[cfg(target_os = "macos")]
    {
        let owned = swift_reflection::tags_no_configuration_stops().unwrap_or_else(|failure| {
            panic!(
                "the tag set of `{SWIFT_IDIOMS_RULE}.md` comes from the toolchain binary itself, \
                 and this run could not read it: {failure}"
            )
        });
        let listed: std::collections::BTreeSet<String> = probed.iter().cloned().collect();

        assert_eq!(
            listed,
            owned,
            "the tag column of `{SWIFT_IDIOMS_RULE}.md` and `SWIFT_FORMAT_TAG_PROBES` must \
             together name every case of {:?}, because those cases ARE the tags no configuration \
             key stops; the two lists name {listed:?} and the toolchain owns {owned:?}",
            swift_reflection::UNSTOPPABLE_CATEGORIES
        );
    }

    let configuration = swift_format_configuration_with_every_rule_off();
    let probe = swift_format_probe_directory();

    for (tag, source) in SWIFT_FORMAT_TAG_PROBES {
        let path = probe.path().join(format!("{tag}.swift"));
        std::fs::write(&path, source).expect("stage a pretty-printer probe file");

        let (status, wrote_out, wrote_err) = swift_format_lint(&path, Some(&configuration));

        assert!(
            wrote_err.contains(&format!("[{tag}]")),
            "`{SWIFT_TOOLCHAIN_TOOL} {SWIFT_FORMAT_SUBCOMMAND} {SWIFT_FORMAT_LINT_VERB}` must \
             write `[{tag}]` for its own probe with every one of the \
             {SWIFT_FORMAT_RULE_COUNT} rules switched OFF, or the allowlist \
             `{SWIFT_IDIOMS_RULE}.md` states filters for a tag nothing sends; the run exited \
             {status} and wrote {wrote_err:?} on stderr and {wrote_out:?} on stdout"
        );
    }
}

/// The heading of the rule-body table that records every status the lint run
/// writes.
const SWIFT_FORMAT_STATUS_TABLE_HEADING: &str = "## Every status the lint run writes";

/// How many cells each row of that table holds.
const SWIFT_FORMAT_STATUS_TABLE_WIDTH: usize = 4;

/// What the rule body writes for a channel a run left empty.
const SWIFT_FORMAT_EMPTY_CHANNEL: &str = "0 bytes";

/// Where a status probe stages the path the run carries.
const SWIFT_FORMAT_STATUS_PROBE_PATH: &str = "Probe.swift";

/// The same declaration written the way the pretty-printer asks for.
const SWIFT_FORMAT_CLEAN_SOURCE: &str = "let alpha = 1 + 2\n";

/// Swift the parser cannot read, because `@@@` opens an attribute that names
/// nothing.
const SWIFT_FORMAT_UNPARSABLE_SOURCE: &str =
    concat!("public struct Broken {\n", "  @@@ let alpha = 1\n", "}\n");

/// Swift written in Latin-1 rather than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence. The
/// declaration under it holds a defect, so a run that DID decode the file
/// would report one.
const SWIFT_FORMAT_UNDECODABLE_SOURCE: &[u8] = b"let name = \"caf\xe9\"\nlet alpha = 1+2\n";

/// How one row of [`SWIFT_FORMAT_STATUS_PROBES`] is staged.
enum SwiftFormatProbeShape {
    /// Swift source the run can read, written at the probe path.
    Readable(&'static str),

    /// A path the run cannot read, staged the way [`ShippedUnreadableFile`]
    /// states.
    Refusing(ShippedUnreadableFile),

    /// A directory, at the probe path.
    Directory,
}

/// One shape `swift format lint --strict` answers, beside the rule-body row
/// that records the status it answers with.
struct SwiftFormatStatusProbe {
    /// The first cell of the rule-body row this probe measures.
    run: &'static str,

    /// What the probe directory holds when the run starts.
    shape: SwiftFormatProbeShape,
}

/// Every shape the rule-body status table records, in the order it records
/// them.
///
/// The four refusing shapes are the ones the rule body records for
/// `swiftformat` today, and this gate's replacement has to answer each of them
/// again, because `swift format` is a different program with statuses of its
/// own. The directory row stands beside them because a path that names one is
/// the shape a work list reaches by naming a package.
const SWIFT_FORMAT_STATUS_PROBES: &[SwiftFormatStatusProbe] = &[
    SwiftFormatStatusProbe {
        run: "a file with findings",
        shape: SwiftFormatProbeShape::Readable(SWIFT_FORMAT_DIRTY_SOURCE),
    },
    SwiftFormatStatusProbe {
        run: "a file with no finding",
        shape: SwiftFormatProbeShape::Readable(SWIFT_FORMAT_CLEAN_SOURCE),
    },
    SwiftFormatStatusProbe {
        run: "a path that holds no file",
        shape: SwiftFormatProbeShape::Refusing(ShippedUnreadableFile::Absent),
    },
    SwiftFormatStatusProbe {
        run: "a file with no read permission",
        shape: SwiftFormatProbeShape::Refusing(ShippedUnreadableFile::Forbidden(
            SWIFT_FORMAT_DIRTY_SOURCE,
        )),
    },
    SwiftFormatStatusProbe {
        run: "a file whose bytes are not UTF-8",
        shape: SwiftFormatProbeShape::Refusing(ShippedUnreadableFile::Undecodable(
            SWIFT_FORMAT_UNDECODABLE_SOURCE,
        )),
    },
    SwiftFormatStatusProbe {
        run: "a file the parser cannot read",
        shape: SwiftFormatProbeShape::Readable(SWIFT_FORMAT_UNPARSABLE_SOURCE),
    },
    SwiftFormatStatusProbe {
        run: "a path that names a directory",
        shape: SwiftFormatProbeShape::Directory,
    },
];

/// Stages `shape` inside `repo`, and answers the path the run carries.
fn stage_swift_format_probe(repo: &Path, shape: &SwiftFormatProbeShape) -> PathBuf {
    match shape {
        SwiftFormatProbeShape::Readable(source) => {
            stage_probe_bytes(repo, SWIFT_FORMAT_STATUS_PROBE_PATH, source.as_bytes());
        }
        SwiftFormatProbeShape::Refusing(unreadable) => {
            stage_probe_unreadable(repo, SWIFT_FORMAT_STATUS_PROBE_PATH, unreadable);
        }
        SwiftFormatProbeShape::Directory => {
            std::fs::create_dir_all(repo.join(SWIFT_FORMAT_STATUS_PROBE_PATH))
                .expect("stage a probe directory at the path the run carries");
        }
    }

    repo.join(SWIFT_FORMAT_STATUS_PROBE_PATH)
}

/// Acceptance: the rule body records the status `swift format lint --strict`
/// really writes for each shape a work list reaches.
///
/// A gate reads a status before it reads a finding, and one of these rows is
/// the shape that costs a gate everything: measured with Apple Swift 6.4, a
/// path that holds no file exits 0 and writes NOTHING, which reads exactly
/// like a clean pass over a file the run never opened. A gate that trusted the
/// status alone would answer clean for a work list of paths that are all gone.
///
/// The stdout column is load-bearing beside the status. `swift format` writes
/// every finding and every error on STDERR, so a gate that read stdout would
/// read an empty channel for a file holding findings.
///
/// The rows come from the body and the statuses come from the live tool, so a
/// release that moves one fails here rather than moving the gate's behaviour
/// without a word.
#[test]
fn the_shipped_swift_idioms_rule_body_records_every_status_the_lint_run_writes() {
    let loader = builtin_loader();
    let source = shipped_swift_idioms_source(&loader);
    let recorded = rule_body_table(&source, Heading(SWIFT_FORMAT_STATUS_TABLE_HEADING));

    assert_eq!(
        recorded.len(),
        SWIFT_FORMAT_STATUS_PROBES.len(),
        "the `{SWIFT_FORMAT_STATUS_TABLE_HEADING}` table of `{SWIFT_IDIOMS_RULE}.md` must record \
         the {} shapes this test probes; it records {recorded:?}",
        SWIFT_FORMAT_STATUS_PROBES.len()
    );

    for (row, probe) in recorded.iter().zip(SWIFT_FORMAT_STATUS_PROBES) {
        assert_eq!(
            row.len(),
            SWIFT_FORMAT_STATUS_TABLE_WIDTH,
            "every row of the `{SWIFT_FORMAT_STATUS_TABLE_HEADING}` table must hold \
             {SWIFT_FORMAT_STATUS_TABLE_WIDTH} cells; this one holds {row:?}"
        );
        assert_eq!(
            row[0], probe.run,
            "the rows must stand in the order this test probes them; the body reads `{}` where \
             the probe reads `{}`",
            row[0], probe.run
        );

        let staged = swift_format_probe_directory();
        let path = stage_swift_format_probe(staged.path(), &probe.shape);
        let (status, wrote_out, wrote_err) = swift_format_lint(&path, None);

        assert_eq!(
            row[1],
            status.to_string(),
            "the `{}` row must record the status the run writes; it records `{}` and the run \
             exited {status}, writing {wrote_err:?} on stderr",
            probe.run,
            row[1]
        );
        assert_eq!(
            row[2], SWIFT_FORMAT_EMPTY_CHANNEL,
            "`{SWIFT_TOOLCHAIN_TOOL} {SWIFT_FORMAT_SUBCOMMAND} {SWIFT_FORMAT_LINT_VERB}` writes \
             every finding and every error on stderr, so the stdout cell of the `{}` row must \
             read `{SWIFT_FORMAT_EMPTY_CHANNEL}`; it reads `{}`",
            probe.run, row[2]
        );
        assert!(
            wrote_out.is_empty(),
            "the `{}` row records an empty stdout, and the run wrote {wrote_out:?} on it",
            probe.run
        );
    }
}
