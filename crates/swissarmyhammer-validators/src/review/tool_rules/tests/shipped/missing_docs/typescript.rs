//! Acceptance tests for the shipped TypeScript missing-docs tool rule.

use super::*;

/// A TypeScript function that carries no JSDoc comment.
/// `jsdoc/require-jsdoc` reports the declaration, so each file holds one
/// finding.
const TYPESCRIPT_MISSING_DOCS_UNREAD_SOURCE: &str = r#"export function undocumented(value: number): number {
  return value;
}
"#;

/// Every TypeScript file staged in the probe repository the missing-docs
/// script is given none of.
const TYPESCRIPT_MISSING_DOCS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.ts", TYPESCRIPT_MISSING_DOCS_UNREAD_SOURCE),
    (
        "deep/nested/other.ts",
        TYPESCRIPT_MISSING_DOCS_UNREAD_SOURCE,
    ),
];

/// Each finding the TypeScript missing-docs script reports over the two files
/// it is given, as `path:line`.
///
/// Both files hold the same bytes, and the exported function that opens them
/// is the one declaration `jsdoc/require-jsdoc` reports.
const TYPESCRIPT_MISSING_DOCS_READ_FINDINGS: &[&str] = &[
    concat!("deep/nested/other.ts:", opening_declaration_row!()),
    concat!("top.ts:", opening_declaration_row!()),
];

/// The `missing-docs-typescript` probe over a run that is given no file.
const TYPESCRIPT_MISSING_DOCS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: TYPESCRIPT_MISSING_DOCS_UNREAD_FILES,
    with_files: TYPESCRIPT_MISSING_DOCS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped TypeScript missing-docs tool rule reads only the
/// files it is given, through the real eslint pipeline.
///
/// eslint with no path argument reads the working directory, and the config
/// this rule writes names `**/*.{js,jsx,mjs,cjs,ts,tsx}`. Measured over this
/// probe with no argument: without the guard the script reported 2 findings
/// and exited 0; with the guard it reports none and exits 0. The same script
/// over the two staged files reports 2.
#[test]
fn the_shipped_typescript_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&TYPESCRIPT_MISSING_DOCS_EMPTY_RUN_PROBE);
}

/// The materialized name of the `missing-docs-typescript` fail fixture.
const TYPESCRIPT_MISSING_DOCS_FAIL_FIXTURE: &str =
    concat!(missing_docs_rule!(typescript), ".fail.ts");

/// Where the `missing-docs-typescript` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const TYPESCRIPT_MISSING_DOCS_FIXTURE_PATH: &str =
    concat!("src/", missing_docs_rule!(typescript), "-fail.ts");

/// Every item the `missing-docs-typescript` fail fixture leaves undocumented,
/// trimmed as the fixture writes it.
///
/// A line, and not a claim, because `jsdoc/require-jsdoc` writes one message —
/// `Missing JSDoc comment.` — for every finding, so the claim never spells
/// which item it read.
///
/// The first five hold the five kinds the rule body claims: an interface, a
/// type alias, an enumeration, a class and a method, with a function under
/// them.
///
/// The getter and the setter are the load-bearing pair. Each one holds two
/// statements, so each stands OUTSIDE the accessor carve-out the passing
/// fixture holds the tool to, and the two fixtures together state where the
/// carve-out ends.
const TYPESCRIPT_MISSING_DOCS_FAIL_ITEMS: &[&str] = &[
    "export interface UndocumentedInterface {",
    "export type UndocumentedAlias = string;",
    "export enum UndocumentedEnum {",
    "export class UndocumentedClass {",
    "undocumentedMethod(): void {}",
    "get busy(): string {",
    "set busy(next: string) {",
    "export function undocumentedFunction(): void {}",
];

/// The `missing-docs-typescript` fail fixture, and every undocumented exported
/// item the real eslint pipeline must report inside it.
const TYPESCRIPT_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_MISSING_DOCS_RULE,
        expected: TYPESCRIPT_MISSING_DOCS_FAIL_ITEMS,
    },
    fixture: TYPESCRIPT_MISSING_DOCS_FAIL_FIXTURE,
    path: TYPESCRIPT_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "undocumented exported item",
};

/// Acceptance: the shipped TypeScript missing-docs tool rule reports every
/// undocumented exported item its fail fixture holds, through the real eslint
/// pipeline.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// passing fixture holds an undocumented simple getter, an undocumented simple
/// setter, four undocumented object-protocol methods and an undocumented
/// `[Symbol.iterator]`, so a run that reported one of them would fail the
/// pair; holding this run to exactly these eight states the same silence from
/// the other side.
#[test]
fn the_shipped_typescript_missing_docs_tool_rule_reports_every_fail_fixture_item() {
    verify_shipped_fail_fixture_reports_each(
        &TYPESCRIPT_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented interface, type alias, enumeration, class, method, getter, \
                 setter and function",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    TYPESCRIPT_MISSING_DOCS_RULE.to_string(),
                ],
                [(TYPESCRIPT_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, expected| reported == expected,
    );
}

/// One undocumented exported interface, which is a declaration a `.d.ts` file
/// and a `.ts` file may each hold.
///
/// Every staged position holds these same bytes, so the POSITION is the only
/// thing that can tell one file of the run from another.
const TYPESCRIPT_STAGED_DECLARATIONS: &str = concat!(
    "export interface StagedShape {\n",
    "  member: string;\n",
    "}\n"
);

/// The source position: an ordinary TypeScript file, which the rule reads.
const TYPESCRIPT_STAGED_SOURCE_PATH: &str = "src/staged.ts";

/// The declaration position. A `.d.ts` file is generated or ambient, and the
/// sibling `dead-code-typescript` drops it for that reason, so only the filter
/// in the pipe keeps this file silent.
const TYPESCRIPT_STAGED_DECLARATION_PATH: &str = "src/staged.d.ts";

/// The head a TypeScript staged file carries: none. The filter decides on the
/// path alone, so both files hold the same bytes.
const TYPESCRIPT_NO_HEAD: &[&str] = &[];

/// Each position the staged interface is written to, in the order the
/// work-list holds them.
const TYPESCRIPT_STAGED_POSITIONS: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: TYPESCRIPT_STAGED_SOURCE_PATH,
        head: TYPESCRIPT_NO_HEAD,
    },
    ShippedStagedFile {
        path: TYPESCRIPT_STAGED_DECLARATION_PATH,
        head: TYPESCRIPT_NO_HEAD,
    },
];

/// The file of each finding the TypeScript run must report: the source file,
/// once for its interface.
const TYPESCRIPT_STAGED_REPORTED: &[&str] = &[TYPESCRIPT_STAGED_SOURCE_PATH];

/// The staged TypeScript positions, and the one of them the real eslint
/// pipeline must report.
const TYPESCRIPT_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_MISSING_DOCS_RULE,
        expected: TYPESCRIPT_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented exported interface, staged in two positions",
    declarations: TYPESCRIPT_STAGED_DECLARATIONS,
    staged: TYPESCRIPT_STAGED_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "the `.ts` file reports its interface, and the `.d.ts` file beside it \
             reports nothing",
};

/// Acceptance: the shipped TypeScript missing-docs tool rule reads the `.ts`
/// file and stays silent on the `.d.ts` file, through the real eslint
/// pipeline.
///
/// This is the half the fixture pair cannot reach. The doctor materializes one
/// fixture as a loose file with no directory, so no fixture can carry a name
/// the filter reads.
///
/// The two files hold the same bytes on purpose. eslint reads a `.d.ts` file
/// as an ordinary TypeScript file and reports its interface, so without the
/// filter both would report the same declaration. The difference between one
/// file reporting and two reporting is therefore the filter and nothing else.
#[test]
fn the_shipped_typescript_missing_docs_tool_rule_reads_no_declaration_file() {
    verify_shipped_staged_positions_report(&TYPESCRIPT_MISSING_DOCS_POSITIONS_PROBE);
}
