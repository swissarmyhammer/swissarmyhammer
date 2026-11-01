//! Acceptance tests for the shipped missing-docs tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! it supersedes. The tests under it drive one language each through its real
//! tool, so each measures the shipped script rather than a copy.
//!
//! The Rust rule stands in `missing_docs_rust`, because the shapes `cargo
//! clippy` answers a broken run with are cargo's own.

use super::*;

/// The row the declaration that OPENS a probe source stands on: the first line
/// of the file.
///
/// Every probe source of this file that its tool reports at the head writes
/// that declaration on the first line of the staged bytes, so the number is a
/// fact of the fixture and not a value the probe chose.
///
/// The row stands in a macro beside the probes that hold it because each probe
/// keeps its rows in a `&'static [&'static str]` built with `concat!`, and
/// `concat!` takes literals alone.
macro_rules! opening_declaration_row {
    () => {
        "1"
    };
}

/// The row the declaration NESTED under the opening declaration stands on: the
/// second line of the file.
///
/// Each nested probe source writes its member on the line directly under the
/// type head that opens the file, so the number is a fact of the staged bytes.
///
/// It stands in a macro for the reason [`opening_declaration_row`] states.
macro_rules! nested_declaration_row {
    () => {
        "2"
    };
}

/// The row ruff stands a whole-module finding on: the first line of the file.
///
/// `D100` names the module itself, which carries no declaration line of its
/// own, so ruff reports it at the head of the file rather than under a
/// definition. That is a different reason from
/// [`opening_declaration_row`], although both name the same line.
///
/// It stands in a macro for the reason [`opening_declaration_row`] states.
macro_rules! python_module_row {
    () => {
        "1"
    };
}

/// The row the first declaration of a Go probe source stands on: the third
/// line of the file.
///
/// A Go file opens with its `package` clause and carries a blank line under
/// it, so the `func Exported` head revive reports takes the third line.
///
/// It stands in a macro for the reason [`opening_declaration_row`] states.
macro_rules! go_declaration_row {
    () => {
        "3"
    };
}

/// Acceptance: every shipped missing-docs tool rule passes its fixture pair
/// in doctor, and supersedes the `missing-docs` prompt rule.
///
/// A tool that reads the whole public surface answers the documentation
/// question the prompt rule asks, so it replaces it for the files it covers.
/// [`verify_shipped_tool_rules_pass_fixtures`] carries the rest of the
/// contract, including what a machine without the tool proves.
#[test]
#[serial_test::serial(cwd)]
fn every_shipped_missing_docs_tool_rule_passes_its_fixtures() {
    verify_shipped_tool_rules_pass_fixtures(SHIPPED_MISSING_DOCS_RULES, MISSING_DOCS_PROMPT_RULE);
}

/// The materialized name of the `missing-docs-dart` fail fixture.
const DART_MISSING_DOCS_FAIL_FIXTURE: &str = concat!(missing_docs_rule!(dart), ".fail.dart");

/// Where the `missing-docs-dart` fail fixture stands inside the probe
/// repository, as the work-list holds it.
///
/// It stands under `lib/`, because that is the one position
/// `public_member_api_docs` reads.
const DART_MISSING_DOCS_FIXTURE_PATH: &str = "lib/missing_docs_dart_fail.dart";

/// Every member the `missing-docs-dart` fail fixture leaves undocumented,
/// trimmed as the fixture writes it.
///
/// A line, and not a claim, because `public_member_api_docs` writes one
/// message — `Missing documentation for a public member.` — for every member,
/// so the claim never spells which one it read.
///
/// The getter and the setter are load-bearing. The `missing-docs` prompt rule
/// carves out "Simple getters/setters with self-explanatory names", and this
/// rule restores nothing, because the lint takes no option at all. The rule
/// body states that, and these two entries hold the tool to the statement.
const DART_MISSING_DOCS_FAIL_LINES: &[&str] = &[
    "class UndocumentedClass {",
    "void undocumentedMethod() {}",
    "int get undocumentedProperty => _value;",
    "set undocumentedProperty(int next) => _value = next;",
    "void undocumentedFunction() {}",
];

/// The `missing-docs-dart` fail fixture, and every undocumented public member
/// the real `dart analyze` pipeline must report inside it.
const DART_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_MISSING_DOCS_FAIL_LINES,
    },
    fixture: DART_MISSING_DOCS_FAIL_FIXTURE,
    path: DART_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "line holding an undocumented public member",
};

/// Acceptance: the shipped Dart missing-docs tool rule reports every
/// undocumented public member its fail fixture holds, through the real
/// `dart analyze` pipeline.
///
/// A member is held to the SOURCE LINE its finding stands on, because
/// `public_member_api_docs` writes one message for every member and never
/// spells the member it read.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// script builds a probe package and runs `dart pub get` inside it, and a run
/// that reached neither the lint nor the package would report zero findings
/// and exit `0`. Holding this run to exactly these five lines states that the
/// analyzer recognized the package, read the configuration the script wrote,
/// and read each kind of member.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reports_every_fail_fixture_line() {
    verify_shipped_fail_fixture_reports_each(
        &DART_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented class, method, getter, setter and function",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    DART_MISSING_DOCS_RULE.to_string(),
                ],
                [(DART_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, expected| reported == expected,
    );
}

/// One undocumented public class, and one undocumented method inside it.
///
/// Every staged position holds these same bytes, so the POSITION is the only
/// thing that can tell one file of the run from another.
const DART_STAGED_LIBRARY: &str =
    concat!("class StagedClass {\n", "  void stagedMethod() {}\n", "}\n");

/// The library position: a file under a package `lib/`.
///
/// This is the one position `public_member_api_docs` reads in the project
/// itself, so this is the one file of the three the run may report.
const DART_STAGED_LIBRARY_PATH: &str = "lib/staged.dart";

/// The test position. A Dart test lives under `test/`, never under `lib/`, so
/// the project's own analyzer never reads it. The probe stages every changed
/// file under a `lib/` of its own, so only the exclude list keeps this file
/// silent.
const DART_STAGED_TEST_PATH: &str = "test/staged_test.dart";

/// The generator position. `.g.dart` is the fixed output name of
/// `build_runner`, and the `missing-docs` prompt rule this tool rule replaces
/// carves generated code out, so only the exclude list keeps this file silent.
const DART_STAGED_GENERATED_PATH: &str = "lib/staged.g.dart";

/// The head a Dart staged file carries: none. `dart analyze` decides on the
/// path alone, so all three files hold the same bytes.
const DART_NO_HEAD: &[&str] = &[];

/// Each position the staged class is written to, in the order the work-list
/// holds them.
const DART_STAGED_POSITIONS: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: DART_STAGED_LIBRARY_PATH,
        head: DART_NO_HEAD,
    },
    ShippedStagedFile {
        path: DART_STAGED_TEST_PATH,
        head: DART_NO_HEAD,
    },
    ShippedStagedFile {
        path: DART_STAGED_GENERATED_PATH,
        head: DART_NO_HEAD,
    },
];

/// The file of each finding the Dart run must report: the library file, once
/// for its class and once for its method.
const DART_STAGED_REPORTED: &[&str] = &[DART_STAGED_LIBRARY_PATH, DART_STAGED_LIBRARY_PATH];

/// The staged Dart positions, and the one of them the real `dart analyze`
/// pipeline must report.
const DART_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public class, staged in three positions",
    declarations: DART_STAGED_LIBRARY,
    staged: DART_STAGED_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "the file under `lib/` reports its class and its method, and the test \
             file and the generated file report nothing",
};

/// Acceptance: the shipped Dart missing-docs tool rule reports the file under
/// `lib/` and stays silent on the test file and the generated file, through
/// the real `dart analyze` pipeline.
///
/// This is the half the fixture pair cannot reach. The doctor materializes one
/// fixture as a loose file with no directory, so no fixture can carry a
/// position, and the probe's `analyzer: exclude:` list decides by position
/// alone.
///
/// The three files hold the same bytes on purpose. `public_member_api_docs`
/// reports a member only inside a package's `lib/`, and the probe stages every
/// changed file under a `lib/` of its own, so without the exclude list all
/// three would report the same two members. The difference between one file
/// reporting and three reporting is therefore the list and nothing else.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reads_only_the_package_library() {
    verify_shipped_staged_positions_report(&DART_MISSING_DOCS_POSITIONS_PROBE);
}

/// The head of the `@override` every Dart package-config probe stages, as the
/// source writes it.
///
/// `public_member_api_docs` carves out a member that overrides a member the
/// analyzer CAN resolve, so this line reports exactly when the run failed to
/// resolve the framework the package depends on.
const DART_OVERRIDE_HEAD: &str = "String build() =>";

/// The head of the undocumented method every Dart package-config probe stages.
///
/// It overrides nothing, so it reports whether or not the run resolved the
/// package. It is the control that tells a resolved run from a run that never
/// reached the lint.
const DART_UNDOCUMENTED_HEAD: &str = "void undocumented()";

/// One application package of the Dart package-config probe, with the
/// framework package it path-depends on.
///
/// The application library overrides a documented framework method and holds
/// one undocumented method of its own, so the project's own analyzer reports
/// the undocumented method alone.
struct DartPackageConfigProbe {
    /// The library file the work-list names, and its source.
    changed: (String, String),

    /// The framework package and the two manifests: every file the package
    /// needs that the change did not touch.
    support: Vec<(String, String)>,

    /// The `.dart_tool/package_config.json` `dart pub get` writes, which is
    /// the one file that resolves the framework import.
    package_config: (String, String),
}

/// The package-config probe for the application package `name`.
///
/// Both packages stand under `packages/`, which is the layout of a Dart
/// monorepo: `packages/<name>` is the application, `packages/<name>_framework`
/// is the framework it path-depends on, and the package config of the
/// application names the framework by a relative root.
///
/// Two probes built with different names share no package, no library and no
/// package config, so a run that read one probe's config for the other
/// probe's file could not resolve the import and would report the override.
fn dart_package_config_probe(name: &str) -> DartPackageConfigProbe {
    let framework = format!("{name}_framework");
    DartPackageConfigProbe {
        changed: (
            format!("packages/{name}/lib/screen.dart"),
            format!(
                "import 'package:{framework}/{framework}.dart';\n\
                 \n\
                 /// A documented screen.\n\
                 class Screen extends Widget {{\n\
                 \x20 @override\n\
                 \x20 String build() => 'screen';\n\
                 \n\
                 \x20 void undocumented() {{}}\n\
                 }}\n"
            ),
        ),
        support: vec![
            (
                format!("packages/{framework}/pubspec.yaml"),
                format!("name: {framework}\n{DART_PROBE_ENVIRONMENT}"),
            ),
            (
                format!("packages/{framework}/lib/{framework}.dart"),
                DART_PROBE_FRAMEWORK_LIBRARY.to_string(),
            ),
            (
                format!("packages/{name}/pubspec.yaml"),
                format!(
                    "name: {name}\n\
                     {DART_PROBE_ENVIRONMENT}\
                     dependencies:\n\
                     \x20 {framework}:\n\
                     \x20   path: ../{framework}\n"
                ),
            ),
        ],
        package_config: (
            format!("packages/{name}/.dart_tool/package_config.json"),
            format!(
                "{{\n\
                 \x20 \"configVersion\": 2,\n\
                 \x20 \"packages\": [\n\
                 \x20   {{ \"name\": \"{framework}\", \"rootUri\": \"../../{framework}\", \
                 \"packageUri\": \"lib/\", \"languageVersion\": \"3.0\" }},\n\
                 \x20   {{ \"name\": \"{name}\", \"rootUri\": \"../\", \
                 \"packageUri\": \"lib/\", \"languageVersion\": \"3.0\" }}\n\
                 \x20 ]\n\
                 }}\n"
            ),
        ),
    }
}

/// The SDK constraint every package of a Dart package-config probe declares.
const DART_PROBE_ENVIRONMENT: &str = "environment:\n  sdk: '>=3.0.0 <5.0.0'\n";

/// The framework library every Dart package-config probe stages. Every member
/// of it is documented, so it contributes no finding of its own and the
/// documentation of `build` is what carves out the override that answers it.
const DART_PROBE_FRAMEWORK_LIBRARY: &str = concat!(
    "/// A documented base class.\n",
    "abstract class Widget {\n",
    "  /// A documented base method.\n",
    "  String build();\n",
    "}\n",
);

/// Each `(path, source)` pair of `files` as the borrowed pair the shipped
/// probe helpers take.
fn borrowed_files(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect()
}

/// The name of the one application package the two single-package Dart
/// package-config acceptance tests stage.
const DART_SINGLE_PACKAGE: &str = "app";

/// Acceptance: the shipped Dart missing-docs tool rule reads the package
/// config of the file it is given, so an `@override` of a resolved member
/// stays silent, through the real `dart analyze` pipeline.
///
/// This is the half no fixture can reach. The doctor materializes a fixture as
/// a loose file with no project around it, so no fixture can carry a
/// dependency, and the override carve-out needs the analyzer to RESOLVE the
/// overridden member.
///
/// The probe package holds two undocumented-looking members and they differ in
/// one way: `build` overrides a documented framework method, and
/// `undocumented` overrides nothing. Reporting the second alone states that
/// the analyzer resolved the framework — the answer the project's own
/// `dart analyze` gives. Reporting both is the answer a probe that declares no
/// dependency gives, and it is the defect this test holds shut.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reads_the_package_config_of_the_file() {
    let probe = dart_package_config_probe(DART_SINGLE_PACKAGE);
    let (path, source) = &probe.changed;
    let mut support = probe.support.clone();
    support.push(probe.package_config.clone());
    let expected = expected_row(path, source, DART_UNDOCUMENTED_HEAD);

    verify_supported_rows_report(
        FLUTTER_PROJECT_TYPES,
        DART_MISSING_DOCS_RULE,
        &[(path.as_str(), source.as_str())],
        &borrowed_files(&support),
        &[&expected],
        "the package config resolves the framework, so the override carries the \
         documentation of the member it overrides and the undocumented method \
         stands alone",
    );
}

/// Acceptance: the shipped Dart missing-docs tool rule still reports when the
/// package holds no package config, through the real `dart analyze` pipeline.
///
/// A project that has never run `dart pub get` or `flutter pub get` has no
/// `.dart_tool/package_config.json` for the run to name. The run must not
/// break there, and it must not fall silent either: a run that answered no
/// finding would read exactly like a documented package.
///
/// So the run falls back to the probe package alone, which resolves nothing
/// outside the file. The answer is a SUPERSET — the override reports beside
/// the undocumented method — and this test holds both rows, so the fallback
/// can never be mistaken for a clean file.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reports_the_override_with_no_package_config() {
    let probe = dart_package_config_probe(DART_SINGLE_PACKAGE);
    let (path, source) = &probe.changed;
    let override_row = expected_row(path, source, DART_OVERRIDE_HEAD);
    let undocumented_row = expected_row(path, source, DART_UNDOCUMENTED_HEAD);

    verify_supported_rows_report(
        FLUTTER_PROJECT_TYPES,
        DART_MISSING_DOCS_RULE,
        &[(path.as_str(), source.as_str())],
        &borrowed_files(&probe.support),
        &[&override_row, &undocumented_row],
        "with no package config to name, the framework does not resolve, the analyzer \
         sees no override, and the run reports a superset rather than nothing",
    );
}

/// The two application packages the Dart monorepo acceptance test stages.
const DART_MONOREPO_PACKAGES: &[&str] = &["alpha", "beta"];

/// Acceptance: the shipped Dart missing-docs tool rule reads one package
/// config for each package of a monorepo, through the real `dart analyze`
/// pipeline.
///
/// A monorepo holds one `.dart_tool/package_config.json` for each package, and
/// `dart analyze` takes one `--packages` for one run. So the run groups the
/// files it is given by the config that resolves them, and makes one probe
/// package for each group.
///
/// Neither package's config names the other's framework. A run that read one
/// config for both files would leave one framework unresolved and report that
/// package's override, so the two rows this test names are what states that
/// each file was read against its own package.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reads_one_package_config_for_each_package() {
    let probes: Vec<DartPackageConfigProbe> = DART_MONOREPO_PACKAGES
        .iter()
        .map(|name| dart_package_config_probe(name))
        .collect();
    let named: Vec<(String, String)> = probes.iter().map(|probe| probe.changed.clone()).collect();
    let support: Vec<(String, String)> = probes
        .iter()
        .flat_map(|probe| {
            probe
                .support
                .iter()
                .cloned()
                .chain(std::iter::once(probe.package_config.clone()))
        })
        .collect();
    let expected: Vec<String> = named
        .iter()
        .map(|(path, source)| expected_row(path, source, DART_UNDOCUMENTED_HEAD))
        .collect();
    let expected: Vec<&str> = expected.iter().map(String::as_str).collect();

    verify_supported_rows_report(
        FLUTTER_PROJECT_TYPES,
        DART_MISSING_DOCS_RULE,
        &borrowed_files(&named),
        &borrowed_files(&support),
        &expected,
        "each file is read against the package config of its own package, so neither \
         override reports",
    );
}

/// The binary every run of the shipped Dart script calls.
const DART_BINARY_NAME: &str = "dart";

/// The word `dart pub get` takes as its first argument.
///
/// It is what tells the run that BUILDS the probe package from the run that
/// judges it, so a stub can break one of the two and leave the other standing.
const DART_PUB_SUBCOMMAND: &str = "pub";

/// The word `dart analyze` takes as its first argument.
const DART_ANALYZE_SUBCOMMAND: &str = "analyze";

/// Where the library of the broken-`dart` probes stands, as the work-list
/// holds it.
///
/// It stands under `lib/`, because that is the one position
/// `public_member_api_docs` reads.
///
/// The path stands in a macro beside the constant that holds it because
/// `concat!` takes literals, and each row of `DART_BROKEN_RUN_ROWS` is this
/// path with a row number after it.
macro_rules! dart_broken_run_path {
    () => {
        "lib/broken_run.dart"
    };
}

/// The path `dart_broken_run_path` names, as one constant the probes hold.
const DART_BROKEN_RUN_PATH: &str = dart_broken_run_path!();

/// One undocumented public class holding one undocumented method.
///
/// Two members, so a run that reached the lint reports two rows. That is what
/// makes silence readable as a defect here: no answer of this file is empty.
const DART_BROKEN_RUN_SOURCE: &str = concat!("class BrokenRun {\n", "  void member() {}\n", "}\n");

/// The one file the broken-`dart` probes stage. The work-list names it.
const DART_BROKEN_RUN_STAGED: &[(&str, &str)] = &[(DART_BROKEN_RUN_PATH, DART_BROKEN_RUN_SOURCE)];

/// Each `path:line` entry the probe reports when both `dart` runs stand: the
/// class head that opens [`DART_BROKEN_RUN_SOURCE`], and the method head on the
/// line under it.
const DART_BROKEN_RUN_ROWS: &[&str] = &[
    concat!(dart_broken_run_path!(), ":", opening_declaration_row!()),
    concat!(dart_broken_run_path!(), ":", nested_declaration_row!()),
];

/// The words the error of a `dart pub get` that could not run must carry.
const DART_PUB_GET_BROKEN_ERROR: &[&str] = &[DART_MISSING_DOCS_RULE, "dart pub get exited"];

/// The probe of a `dart pub get` that cannot run, and the words its error must
/// carry.
const DART_PUB_GET_BROKEN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_PUB_GET_BROKEN_ERROR,
    },
    staged: DART_BROKEN_RUN_STAGED,
    reason: "a `dart pub get` that did not run leaves the probe package with no package \
             config, and the lint then reads no member of it at all",
};

/// The words the error of a `dart analyze` that could not run must carry.
const DART_ANALYZE_BROKEN_ERROR: &[&str] = &[DART_MISSING_DOCS_RULE, "dart analyze exited"];

/// The probe of a `dart analyze` that cannot run, and the words its error must
/// carry.
const DART_ANALYZE_BROKEN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_ANALYZE_BROKEN_ERROR,
    },
    staged: DART_BROKEN_RUN_STAGED,
    reason: "a `dart analyze` that did not run judged no code, so the run has nothing to \
             report and must not answer as though it had",
};

/// The same probe with no `dart` run stubbed, and each row it must report.
const DART_WHOLE_RUN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_BROKEN_RUN_ROWS,
    },
    staged: DART_BROKEN_RUN_STAGED,
    reason: "both `dart` runs stand, so the probe package is built, the analyzer \
             recognizes it, and the lint reports the class and the method",
};

/// Acceptance: the shipped Dart missing-docs tool rule BREAKS when
/// `dart pub get` cannot run.
///
/// The script builds a probe package and runs `dart pub get` inside it, because
/// the analyzer reads `public_member_api_docs` only for a package it recognizes,
/// and only `dart pub get` writes the `.dart_tool/package_config.json` that
/// makes it one. So a `dart pub get` that failed takes the whole rule down —
/// the fallback path and the `--packages` path alike — and it takes it down
/// SILENTLY: the analyzer still runs, still exits 0, and reports no member.
///
/// Measured on Dart SDK 3.11.0 over a probe package holding one undocumented
/// class and one undocumented method: 2 rows after a `dart pub get` that
/// succeeds, and 0 rows at exit 0 after one that fails and writes no
/// `.dart_tool`. A probe whose `environment: sdk:` window leaves the installed
/// SDK outside it reaches that state — measured with `sdk: '>=3.5.0 <3.6.0'`
/// on Dart SDK 3.11.0, `dart pub get` exits 1 offline and online alike, and
/// writes nothing. The script derives its constraint from `dart --version`, so
/// no installed SDK stands outside it.
///
/// The stub breaks the `pub` run and hands the `analyze` run through, so this
/// probe measures the pub-get status alone.
/// [`the_shipped_dart_missing_docs_tool_rule_reports_both_members_when_dart_runs`]
/// is the control that keeps the gate from simply breaking every run.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_dart_missing_docs_tool_rule_breaks_when_pub_get_cannot_run() {
    verify_shipped_tree_breaks_without_run_of(
        &DART_PUB_GET_BROKEN_PROBE,
        DART_BINARY_NAME,
        Some(DART_PUB_SUBCOMMAND),
    );
}

/// Acceptance: the shipped Dart missing-docs tool rule BREAKS when
/// `dart analyze` cannot run.
///
/// `dart analyze` keeps one status for issues and another for a failure.
/// Measured on Dart SDK 3.11.0: 0 with infos alone, 1 under `--fatal-infos`,
/// 2 with a warning, 3 with an error — and each of those four writes its rows
/// to stdout. 64 is the usage error, which writes 0 bytes and judges nothing;
/// `--packages=<file that does not exist>` and an unknown subcommand each take
/// it. The script accepts 0 through 3 and breaks above them.
///
/// The earlier shape of this step was one pipe that ended in `awk`, so the
/// script took awk's status and answered exit 0 for every failure of the
/// analyzer. Measured over this probe with `dart analyze` replaced by a command
/// that exits 127: the pipe wrote 0 rows and exited 0, which the engine reads
/// as a clean file; the shipped shape writes no row, that line, and exit 1.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_dart_missing_docs_tool_rule_breaks_when_the_analyzer_cannot_run() {
    verify_shipped_tree_breaks_without_run_of(
        &DART_ANALYZE_BROKEN_PROBE,
        DART_BINARY_NAME,
        Some(DART_ANALYZE_SUBCOMMAND),
    );
}

/// Acceptance: the shipped Dart missing-docs tool rule reports both members of
/// the probe when every `dart` run stands, through the real `dart analyze`
/// pipeline.
///
/// This is the control half of the two tests above it. A gate that broke every
/// run it could not read at a glance would pass both of them and throw away the
/// findings of a run the tool DID make. Holding the same staged file to two
/// rows states that the two status tests break a broken run and no other.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reports_both_members_when_dart_runs() {
    verify_shipped_tree_reports(&DART_WHOLE_RUN_PROBE);
}

/// Where the path the Dart missing-docs run cannot judge stands inside the
/// probe repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const DART_MISSING_DOCS_UNREADABLE_PATH: &str = "lib/unreadable.dart";

/// A Dart library written in Latin-1 rather than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence. The
/// members stand under the string, so a run that DID read the file reports
/// them.
const DART_MISSING_DOCS_UNDECODABLE_SOURCE: &[u8] = b"final name = 'caf\xe9';\n\
class Undecodable {\n\
  void member() {}\n\
}\n";

/// A Dart library the analyzer could read if the mode let it.
///
/// Dart states privacy with the `_` prefix, so a run that DID read this file
/// would report no member — which is the clean answer this rule must not give
/// for a file it never read.
const DART_MISSING_DOCS_FORBIDDEN_SOURCE: &str =
    concat!("class _Forbidden {\n", "  void _member() {}\n", "}\n");

/// The `missing-docs-dart` probe over a refusing path beside the staged
/// library.
///
/// The staged library is the one [`DART_BROKEN_RUN_STAGED`] holds: one
/// undocumented public class and one undocumented method, so the run has two
/// rows to lose. Losing them is what a nonzero exit over a declined item costs,
/// and staying silent about the path is what reads that path as a clean file.
fn dart_missing_docs_decline_probe() -> ShippedDeclineProbe {
    ShippedDeclineProbe {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        judged: DART_BROKEN_RUN_STAGED
            .iter()
            .map(|(path, source)| (*path, (*source).to_string()))
            .collect(),
        path: DART_MISSING_DOCS_UNREADABLE_PATH,
        expected: DART_BROKEN_RUN_ROWS
            .iter()
            .map(|row| (*row).to_string())
            .collect(),
    }
}

/// Acceptance: the shipped Dart missing-docs tool rule DECLINES a path that
/// holds no file, through the real `dart analyze` pipeline.
///
/// The script copies each file it is given into the probe package, and `cp`
/// answers a path that holds no file with a nonzero status. `set -e` then took
/// the whole run down, and the two rows of the library the run DID judge went
/// away with it. Measured with the two tests taken out: `cp: lib/absent.dart:
/// No such file or directory` and exit 1.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_declines_a_path_that_holds_no_file() {
    verify_unreadable_file_is_declined(
        &dart_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Absent,
    );
}

/// Acceptance: the shipped Dart missing-docs tool rule DECLINES a Dart file it
/// cannot decode, through the real `dart analyze` pipeline.
///
/// `[ ! -r "$file" ]` admits this file — the mode lets a reader open it — so
/// `cp` copies it into the probe package and `dart analyze` reads it. Measured
/// on Dart SDK 3.11.0 over a probe package that holds this file beside the
/// judged library: the analyzer reports the two rows of the judged library, 0
/// bytes on stderr, and exit 0, and it says NOTHING about the file it could not
/// decode. So the engine read a file the analyzer never read as a clean file,
/// and the `iconv` test is what answers it.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_declines_a_file_it_cannot_decode() {
    verify_unreadable_file_is_declined(
        &dart_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Undecodable(DART_MISSING_DOCS_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Dart missing-docs tool rule DECLINES a Dart file it
/// may not read, through the real `dart analyze` pipeline.
///
/// `cp` answers a file whose mode refuses a read with a nonzero status, and
/// `set -e` then took the whole run down. Measured with the two tests taken
/// out: `cp: lib/forbidden.dart: Permission denied` and exit 1.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_dart_missing_docs_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &dart_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Forbidden(DART_MISSING_DOCS_FORBIDDEN_SOURCE),
    );
}

/// Where the Dart library that uses a declaration newer than the earlier probe
/// floor stands inside the probe repository.
///
/// It stands under `lib/`, which is the one position `public_member_api_docs`
/// reads.
const DART_MISSING_DOCS_LANGUAGE_VERSION_PATH: &str = "lib/language_version.dart";

/// A Dart library whose first declaration is an `extension type`, beside a
/// plain class holding the same two undocumented member kinds.
///
/// `extension type` arrived in Dart 3.3. A probe package whose
/// `environment: sdk:` states a LOWER floor gives the analyzer a language
/// version that refuses the declaration, and every member inside it goes off
/// the report.
///
/// The plain class is what makes the loss readable. A run that lost the
/// extension type still answers three rows, so silence is never the signal —
/// the missing rows are.
const DART_MISSING_DOCS_LANGUAGE_VERSION_SOURCE: &str = concat!(
    "extension type Meters(int value) {\n",
    "  int get doubled => value + value;\n",
    "\n",
    "  void report() {}\n",
    "}\n",
    "\n",
    "class OtherClass {\n",
    "  String otherField = 'plain';\n",
    "\n",
    "  void otherMethod() {}\n",
    "}\n",
);

/// The head of each member [`DART_MISSING_DOCS_LANGUAGE_VERSION_SOURCE`]
/// leaves
/// undocumented.
///
/// The first three stand inside the `extension type` and the last three inside
/// the plain class. The lint reports the extension type itself, its getter and
/// its method, and reports nothing for the representation field `value`.
const DART_MISSING_DOCS_LANGUAGE_VERSION_HEADS: &[&str] = &[
    "extension type Meters",
    "int get doubled",
    "void report()",
    "class OtherClass",
    "String otherField",
    "void otherMethod()",
];

/// Acceptance: the shipped Dart missing-docs tool rule reports every member of
/// a library that uses a declaration newer than the earlier probe floor,
/// through the real `dart analyze` pipeline.
///
/// A Dart package's LANGUAGE VERSION is the lower bound of its
/// `environment: sdk:` constraint, and the analyzer refuses syntax newer than
/// that version. The script therefore reads the version out of `dart --version`
/// and writes `sdk: '^<version>'`, so the probe parses with the language
/// version of the installed SDK.
///
/// Measured on Dart SDK 3.11.0 over this source, with `public_member_api_docs`
/// on:
///
/// | the probe constraint | what the run reports |
/// |---|---|
/// | `>=3.0.0 <5.0.0`, the earlier fixed floor | rows 7, 8 and 10, beside one `EXPERIMENT_NOT_ENABLED` SYNTACTIC_ERROR naming `inline-class` |
/// | `^3.11.0`, derived from `dart --version` | rows 1, 2, 4, 7, 8 and 10 |
///
/// The earlier floor loses the three members of the extension type and exits 0,
/// which reads exactly like a documented declaration. This test holds the run
/// to all six, so a fixed floor can never come back unmeasured.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reports_a_member_of_a_newer_declaration() {
    let expected: Vec<String> = DART_MISSING_DOCS_LANGUAGE_VERSION_HEADS
        .iter()
        .map(|head| {
            expected_row(
                DART_MISSING_DOCS_LANGUAGE_VERSION_PATH,
                DART_MISSING_DOCS_LANGUAGE_VERSION_SOURCE,
                head,
            )
        })
        .collect();
    let expected: Vec<&str> = expected.iter().map(String::as_str).collect();

    verify_staged_rows_report(
        FLUTTER_PROJECT_TYPES,
        DART_MISSING_DOCS_RULE,
        &[(
            DART_MISSING_DOCS_LANGUAGE_VERSION_PATH,
            DART_MISSING_DOCS_LANGUAGE_VERSION_SOURCE,
        )],
        &expected,
        "the probe package states the language version of the installed SDK, so the \
         analyzer parses the extension type and reports its three members beside the \
         three of the plain class",
    );
}

/// The word `dart --version` takes as its first argument.
///
/// It tells the run that reads the language version from the two runs that
/// build and judge the probe package, so a stub can break that one run and
/// leave the other two standing.
const DART_MISSING_DOCS_VERSION_SUBCOMMAND: &str = "--version";

/// Where the one file the no-version probe stages stands, as the work-list
/// holds it.
///
/// The script never reaches `dart analyze` over this file — the break happens
/// before the probe package is built — so its content answers for nothing
/// beyond giving the run one file to judge.
const DART_MISSING_DOCS_NO_VERSION_PATH: &str = "lib/no_version.dart";

/// [`DART_MISSING_DOCS_NO_VERSION_PATH`]'s source: one undocumented public
/// class holding one undocumented method.
const DART_MISSING_DOCS_NO_VERSION_SOURCE: &str =
    concat!("class NoVersion {\n", "  void member() {}\n", "}\n");

/// The one file the no-version probe stages.
const DART_MISSING_DOCS_NO_VERSION_STAGED: &[(&str, &str)] = &[(
    DART_MISSING_DOCS_NO_VERSION_PATH,
    DART_MISSING_DOCS_NO_VERSION_SOURCE,
)];

/// The words the error of a `dart --version` that names no version must carry.
const DART_MISSING_DOCS_NO_VERSION_ERROR: &[&str] =
    &[DART_MISSING_DOCS_RULE, "dart --version names no version"];

/// The probe of a `dart --version` that names no version, and the words its
/// error must carry.
const DART_MISSING_DOCS_NO_VERSION_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_MISSING_DOCS_NO_VERSION_ERROR,
    },
    staged: DART_MISSING_DOCS_NO_VERSION_STAGED,
    reason: "a `dart --version` that names no version leaves the probe package unable to \
             state the language version it parses with, and the run must not guess one",
};

/// Acceptance: the shipped Dart missing-docs tool rule BREAKS when
/// `dart --version` names no version.
///
/// The script reads the installed SDK's language version out of
/// `dart --version` because a fixed floor hides real code as the language
/// moves. A `dart --version` this script cannot read a version out of leaves
/// it with no constraint to derive, and the run must not guess one: it names
/// the failure and exits, rather than writing a probe package whose
/// `environment: sdk:` states a version nobody measured.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_dart_missing_docs_tool_rule_breaks_when_dart_version_names_no_version() {
    verify_shipped_tree_breaks_with_stub(
        &DART_MISSING_DOCS_NO_VERSION_PROBE,
        DART_BINARY_NAME,
        &format!(" && [ \"$1\" = \"{DART_MISSING_DOCS_VERSION_SUBCOMMAND}\" ]"),
        "  printf '%s\\n' 'Dart CLI has no version line here'\n  exit 0",
    );
}

/// The materialized name of the `missing-docs-go` fail fixture.
const GO_MISSING_DOCS_FAIL_FIXTURE: &str = concat!(missing_docs_rule!(go), ".fail.go");

/// Where the `missing-docs-go` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const GO_MISSING_DOCS_FIXTURE_PATH: &str = "src/missing_docs_go_fail.go";

/// Every item the `missing-docs-go` fail fixture leaves undocumented, as
/// revive's `exported` rule spells it inside the message it reports.
///
/// Each entry carries the KIND word beside the name, because the message
/// carries it, so an entry states which declaration revive read and not only
/// what it is called.
///
/// The first five hold the five kinds the rule body claims — a type, a method,
/// a function, a constant and a variable.
///
/// `WrongCommentForm` and `OnlyDeprecated` hold the two shapes revive reads as
/// undocumented although a comment stands above them: a doc comment that does
/// not open with the item's own name, and a `Deprecated:` note standing alone.
///
/// The getter and the setter are the load-bearing pair. The `missing-docs`
/// prompt rule carves out "Simple getters/setters with self-explanatory
/// names", and revive takes no option that restores the carve-out:
/// `disableChecksOnMethods` turns off EVERY method check, which is far wider.
/// The rule body states that a getter and a setter each report, and these two
/// entries hold revive to the statement.
const GO_MISSING_DOCS_FAIL_ITEMS: &[&str] = &[
    "type UndocumentedType",
    "method UndocumentedType.UndocumentedMethod",
    "function UndocumentedFunction",
    "const UndocumentedConst",
    "var UndocumentedVar",
    "function WrongCommentForm",
    "method Accessors.Value",
    "method Accessors.SetValue",
    "function OnlyDeprecated",
];

/// The `missing-docs-go` fail fixture, and every undocumented exported item
/// the real revive pipeline must report inside it.
const GO_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_MISSING_DOCS_RULE,
        expected: GO_MISSING_DOCS_FAIL_ITEMS,
    },
    fixture: GO_MISSING_DOCS_FAIL_FIXTURE,
    path: GO_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "undocumented exported item",
};

/// Acceptance: the shipped Go missing-docs tool rule reports every
/// undocumented exported item its fail fixture holds, through the real revive
/// pipeline.
///
/// An item is held to the CLAIM its finding carries, because revive spells the
/// kind and the name inside the message it reports.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// pass fixture holds six undocumented methods revive carves out by name, so a
/// run that reported one of them would fail the pair; holding this run to
/// exactly these nine states the same silence from the other side.
#[test]
fn the_shipped_go_missing_docs_tool_rule_reports_every_fail_fixture_item() {
    verify_shipped_fail_fixture_reports_each(
        &GO_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented type, method, function, constant, variable, getter and setter",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    GO_MISSING_DOCS_RULE.to_string(),
                ],
                [(GO_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        |verified, _source| verified.finding.claim.clone(),
        |reported, item| reported.contains(item),
    );
}

/// One undocumented exported type, and one undocumented method on it.
const GO_STAGED_DECLARATIONS: &str = concat!(
    "type StagedType struct{}\n",
    "\n",
    "func (s StagedType) StagedMethod() {}\n"
);

/// The package clause a library file carries.
const GO_STAGED_PACKAGE_CLAUSE: &str = "package staged\n\n";

/// The package clause a command file carries. revive's `exported` rule reads
/// no file of `package main`, because a command exports nothing to a caller
/// outside itself.
const GO_MAIN_PACKAGE_CLAUSE: &str = "package main\n\n";

/// The generated-code header the Go convention defines, with the blank line
/// that separates it from what follows.
///
/// This one line is the whole of what revive reads to know a file is
/// generated. The name of the file says nothing.
const GO_GENERATED_HEADER: &str = "// Code generated by the sah probe. DO NOT EDIT.\n\n";

/// The ordinary position: a library file with no generated header, and a name
/// that is not a test name. This is the one file of the four that must report.
const GO_STAGED_ORDINARY_PATH: &str = "staged.go";

/// The test position. revive's `exported` rule skips a file whose name ends in
/// `_test.go`, and it skips the whole file rather than the test functions in
/// it, so this file must stay silent.
const GO_STAGED_TEST_PATH: &str = "staged_test.go";

/// The generator position. The name carries the protobuf compiler's suffix and
/// the file carries the generated header, so this file must stay silent.
const GO_STAGED_GENERATED_PATH: &str = "staged.pb.go";

/// The command position. It stands in a directory of its own, which is where
/// a Go command stands, so one directory never holds two package names.
const GO_STAGED_MAIN_PATH: &str = "cmd/probe/main.go";

/// The head of the ordinary file and of the test file: the library package
/// clause and nothing else.
const GO_LIBRARY_HEAD: &[&str] = &[GO_STAGED_PACKAGE_CLAUSE];

/// The head of the generated file: the generated header, then the SAME library
/// package clause the two files above carry.
const GO_GENERATED_HEAD: &[&str] = &[GO_GENERATED_HEADER, GO_STAGED_PACKAGE_CLAUSE];

/// The head of the command file: the `main` package clause alone.
const GO_MAIN_HEAD: &[&str] = &[GO_MAIN_PACKAGE_CLAUSE];

/// Each position the staged type is written to, with the head that file
/// carries above the shared declarations.
///
/// The ordinary file and the test file hold the same bytes, so their NAMES are
/// the only difference. The generated file adds the header line and nothing
/// else, so that LINE is its only difference. The command file changes the
/// package clause and nothing else, so that CLAUSE is its only difference.
const GO_STAGED_POSITIONS: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: GO_STAGED_ORDINARY_PATH,
        head: GO_LIBRARY_HEAD,
    },
    ShippedStagedFile {
        path: GO_STAGED_TEST_PATH,
        head: GO_LIBRARY_HEAD,
    },
    ShippedStagedFile {
        path: GO_STAGED_GENERATED_PATH,
        head: GO_GENERATED_HEAD,
    },
    ShippedStagedFile {
        path: GO_STAGED_MAIN_PATH,
        head: GO_MAIN_HEAD,
    },
];

/// The file of each finding the Go run must report: the ordinary file, once
/// for its type and once for its method.
const GO_STAGED_REPORTED: &[&str] = &[GO_STAGED_ORDINARY_PATH, GO_STAGED_ORDINARY_PATH];

/// The staged Go positions, and the one of them the real revive pipeline must
/// report.
const GO_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_MISSING_DOCS_RULE,
        expected: GO_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented exported type, staged in four positions",
    declarations: GO_STAGED_DECLARATIONS,
    staged: GO_STAGED_POSITIONS,
    support: NO_SUPPORT_FILES,
    reason: "the ordinary library file reports its type and its method, and the \
             test file, the generated file and the command file report nothing",
};

/// Acceptance: the shipped Go missing-docs tool rule reports the ordinary
/// library file and stays silent on the test file, the generated file and the
/// command file, through the real revive pipeline.
///
/// This is the half the fixture pair cannot reach. The doctor materializes one
/// fixture as a loose file under a name of its own, in a package of its own, so
/// no fixture can carry a test name, a generated header, or a `main` package
/// clause.
///
/// Each of the three carve-outs is a DEFAULT of revive, and a default is what a
/// later edit can take away. `ignoreGeneratedHeader = true` makes revive ignore
/// the header rather than honour it, so a config that states it reports every
/// exported item of every generated file. This test fails the moment the config
/// states it.
#[test]
fn the_shipped_go_missing_docs_tool_rule_reads_neither_a_generated_a_test_nor_a_command_file() {
    verify_shipped_staged_positions_report(&GO_MISSING_DOCS_POSITIONS_PROBE);
}

/// The detected project type a Go workspace carries, as the match context
/// holds it.
const GO_PROJECT_TYPE: &str = "go";

/// Every shipped rule that reads a `.go` file, as `<set>/<rule>` and sorted.
///
/// The list is what the matcher SELECTS, before any rule supersedes another,
/// so `code-hygiene/missing-docs` stands here beside the tool rule that
/// replaces it. Half the list carries no file criteria at all, which is how a
/// set-wide rule reads every language.
///
/// The list is here to hold one sentence of the `missing-docs-go` rule body:
/// `code-hygiene/stuttering-name-go` owns a repetitive Go NAME, and it is the
/// one rule of this list that does. `missing-docs-go` turns revive's
/// repetitive-name check off with `disableStutteringCheck` and owns the
/// `comments` half of the same revive rule; `stuttering-name-go` runs the same
/// rule and selects the `naming` half.
///
/// A rule added to any set above fails this test. Read the new rule then: if
/// it owns a Go name as well, correct the `missing-docs-go` rule body and the
/// `stuttering-name-go` rule body with it. If it does not, add its name here.
const SHIPPED_RULES_THAT_READ_A_GO_FILE: &[&str] = &[
    "code-hygiene/data-driven",
    "code-hygiene/dead-code",
    "code-hygiene/dead-code-go",
    "code-hygiene/function-length",
    "code-hygiene/function-length-go",
    "code-hygiene/magic-numbers",
    "code-hygiene/magic-numbers-go",
    "code-hygiene/missing-docs",
    "code-hygiene/missing-docs-go",
    "code-hygiene/no-commented-code",
    "code-hygiene/stuttering-name-go",
    "code-security/command-safety",
    "code-security/injection",
    "code-security/no-secrets",
    "completeness/case-sensitivity-coverage",
    "completeness/invariant-propagation",
    "completeness/inverse-operation-coverage",
    "completeness/public-output-contract",
    "duplication/duplication",
    "duplication/rust",
    "duplication/swift",
    "reuse/reuse",
    "test-integrity/no-hard-code",
    "test-integrity/no-test-cheating",
    "test-integrity/test-partitioning",
];

/// Acceptance: the shipped rules that read a `.go` file are exactly the ones
/// [`SHIPPED_RULES_THAT_READ_A_GO_FILE`] names.
///
/// The `missing-docs-go` rule body states that `stuttering-name-go` owns a
/// repetitive Go name and that no other shipped rule reads one. That sentence
/// is about every rule and not about one, so only an enumeration can hold it.
/// The enumeration runs the real matcher over a `.go` path in a Go workspace,
/// which is the same question the review scope stage asks.
#[test]
fn the_shipped_rules_that_read_a_go_file_stay_the_stated_list() {
    let loader = builtin_loader();
    let context = MatchContext::new()
        .with_file(GO_STAGED_ORDINARY_PATH)
        .with_project_types([GO_PROJECT_TYPE.to_string()]);

    let mut reading: Vec<String> = Vec::new();
    for ruleset in loader.list_rulesets() {
        for rule in &ruleset.rules {
            if rule.matches(ruleset, &context) {
                reading.push(format!("{}/{}", ruleset.name(), rule.name));
            }
        }
    }
    reading.sort();

    assert_eq!(
        reading, SHIPPED_RULES_THAT_READ_A_GO_FILE,
        "the rules that read a Go file moved; a SECOND rule that owns a Go NAME \
         makes the `missing-docs-go` rule body wrong, because that body states \
         `stuttering-name-go` is the one owner of the repetitive name"
    );
}

/// A Go file that does not parse: the parameter list of `Broken` never closes.
const GO_UNPARSABLE_SOURCE: &str = concat!("package staged\n", "\n", "func Broken( {\n");

/// Where the unparsable file stands inside the probe repository.
const GO_UNPARSABLE_PATH: &str = "broken.go";

/// Where the Go file the run CAN judge stands, beside the file it cannot
/// judge.
const GO_JUDGED_PATH: &str = "judged.go";

/// A Go file the run judges: one exported type that carries no doc comment.
///
/// The name does not open with the package name, so the `exported` rule of
/// revive answers for it under the `comments` category alone and the count
/// stays at one finding.
const GO_JUDGED_SOURCE: &str = concat!("package staged\n", "\n", "type Plain struct{}\n");

/// The declaration line the one finding of the judged Go file stands on.
const GO_JUDGED_DECLARATION: &str = "type Plain struct{}";

/// The `path:line` row the run must report for the judged Go file.
fn go_missing_docs_judged_row() -> String {
    expected_row(GO_JUDGED_PATH, GO_JUDGED_SOURCE, GO_JUDGED_DECLARATION)
}

/// Acceptance: the shipped Go missing-docs tool rule DECLINES a Go file it
/// cannot parse, through the real revive pipeline.
///
/// revive exits 0 for such a file and writes the failure onto the SAME report
/// as a finding, with an empty `RuleName` and the `validity` category rather
/// than the rule name `exported`. A filter that selected the `exported`
/// findings alone dropped that record, and the file then read as clean.
///
/// An `exit 1` answers that no better. Measured with revive 1.15.0 under the
/// config this rule ships, over `func Broken( {` beside one undocumented
/// exported type: two records on the report — the unnamed record of the file
/// it could not parse AND the `exported` finding of the file it read — at
/// exit 0, with 0 bytes on stderr. revive judged the other file, so its
/// finding is there to lose, and the shipped script before this fix threw it
/// away: nothing on stdout, one unmarked line on stderr, exit 1.
///
/// The unparsable file is therefore one declined item of a sound run, and the
/// script states it under the `sah-diagnostic:` marker at exit 0.
#[test]
fn the_shipped_go_missing_docs_tool_rule_declines_a_file_it_cannot_parse() {
    let expected = go_missing_docs_judged_row();

    verify_unjudged_file_is_declined(
        GO_PROJECT_TYPES,
        GO_MISSING_DOCS_RULE,
        &[
            (GO_JUDGED_PATH, GO_JUDGED_SOURCE),
            (GO_UNPARSABLE_PATH, GO_UNPARSABLE_SOURCE),
        ],
        GO_UNPARSABLE_PATH,
        &[&expected],
    );
}

/// Acceptance: the shipped Go missing-docs tool rule DECLINES a Go file it may
/// not read, through the real revive pipeline.
///
/// revive STATS each path before it lints. A path it can stat and cannot open
/// is dropped in SILENCE: measured with revive 1.15.0, over a file at mode 000
/// beside one undocumented exported type, revive reported the finding of the
/// file it read, wrote NO record of the refusing path under any category, wrote
/// 0 bytes on stderr and exited 0. The same file alone answered `null` on
/// stdout at exit 0, which is the report and the status of a clean file.
///
/// So there is no line to forward and no record to select. The script tests
/// each path BEFORE revive starts, which is the shape
/// `builtin/validators/README.md` names for a tool that can exit 0 for a file
/// it could not open.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_go_missing_docs_tool_rule_declines_a_file_it_may_not_read() {
    verify_go_rule_declines_a_forbidden_path(
        GO_MISSING_DOCS_RULE,
        (GO_JUDGED_PATH, GO_JUDGED_SOURCE),
        go_missing_docs_judged_row(),
    );
}

/// The materialized name of the `missing-docs-python` fail fixture.
const PYTHON_MISSING_DOCS_FAIL_FIXTURE: &str = concat!(missing_docs_rule!(python), ".fail.py");

/// Where the fail fixture stands inside the probe repository, as the work-list
/// holds it.
const PYTHON_MISSING_DOCS_FIXTURE_PATH: &str = "src/missing_docs_python_fail.py";

/// The definition line of each item the `missing-docs-python` fail fixture
/// leaves undocumented.
///
/// One entry for each of the five codes a loose file can hold — `D101`, `D106`,
/// `D107`, `D102` and `D103` — and one more `D102` for the property getter.
/// `D100` and `D104` stand outside the fixture: the doctor materializes one
/// loose file that carries a docstring of its own, and it cannot take the name
/// `__init__.py`.
///
/// The getter is the entry the rule body owes a measurement. ruff carves out no
/// getter and the prompt rule does, so this entry holds ruff to reporting it.
const PYTHON_MISSING_DOCS_FAIL_ITEMS: &[&str] = &[
    "class UndocumentedClass:",
    "class UndocumentedNested:",
    "def __init__(self, name: str) -> None:",
    "def name(self) -> str:",
    "def undocumented_method(self) -> None:",
    "def undocumented_function() -> None:",
];

/// The `missing-docs-python` fail fixture, and every undocumented item the real
/// ruff pipeline must report inside it.
const PYTHON_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_MISSING_DOCS_FAIL_ITEMS,
    },
    fixture: PYTHON_MISSING_DOCS_FAIL_FIXTURE,
    path: PYTHON_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "undocumented item",
};

/// Acceptance: the shipped Python missing-docs tool rule reports every
/// undocumented item its fail fixture holds, through the real ruff pipeline.
///
/// An item is held to the SOURCE LINE its finding stands on, because ruff writes
/// one message for each code and never spells the name it read.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// pass fixture holds an undocumented `__str__`, `__repr__`, `__eq__`, property
/// setter, test class, test method and test function, so a run that reported one
/// of them would fail the pair; holding this run to exactly these six states the
/// same silence from the other side.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reports_every_fail_fixture_item() {
    verify_shipped_fail_fixture_reports_each(
        &PYTHON_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented class, nested class, constructor, getter, method and function",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    PYTHON_MISSING_DOCS_RULE.to_string(),
                ],
                [(PYTHON_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, item| reported == item,
    );
}

/// The declarations every staged Python position holds, each one undocumented.
///
/// `TestShared`, `test_method` and `test_shared` carry the name pytest and
/// unittest collect by, so the rule must drop each one at every position.
/// `helper_shared` carries no such name, so the rule must report it at every
/// position — the test file included.
const PYTHON_STAGED_DECLARATIONS: &str = concat!(
    "class TestShared:\n",
    "    def test_method(self) -> None:\n",
    "        assert True\n",
    "\n",
    "\n",
    "def test_shared() -> None:\n",
    "    assert True\n",
    "\n",
    "\n",
    "def helper_shared() -> None:\n",
    "    return None\n",
);

/// The module docstring a documented position carries above the shared
/// declarations.
const PYTHON_MODULE_DOCSTRING: &str = "\"\"\"A documented module.\"\"\"\n\n\n";

/// The head of a documented module: the docstring and nothing else.
const PYTHON_DOCUMENTED_HEAD: &[&str] = &[PYTHON_MODULE_DOCSTRING];

/// The head of an undocumented module: nothing at all.
const PYTHON_UNDOCUMENTED_HEAD: &[&str] = &[];

/// The ordinary position. It carries a module docstring, so the helper is its
/// one finding.
const PYTHON_STAGED_DOCUMENTED_PATH: &str = "documented.py";

/// The package position. An `__init__.py` with no docstring reports `D104`.
const PYTHON_STAGED_PACKAGE_PATH: &str = "pkg/__init__.py";

/// The test position. The directory and the file name are both what pytest
/// collects by, and the rule reads neither, so it reports the same finding the
/// ordinary position reports.
const PYTHON_STAGED_TEST_PATH: &str = "tests/test_documented.py";

/// The undocumented module position. It reports `D100` above the helper.
const PYTHON_STAGED_UNDOCUMENTED_PATH: &str = "undocumented.py";

/// Each position the shared declarations are staged at.
///
/// The ordinary position and the test position hold the same bytes, so their
/// PATHS are the only difference. The undocumented position drops the module
/// docstring, so that DOCSTRING is its only difference. The package position
/// drops the same docstring under the one file name Python reads as a package.
const PYTHON_STAGED_FILES: &[ShippedStagedFile] = &[
    ShippedStagedFile {
        path: PYTHON_STAGED_DOCUMENTED_PATH,
        head: PYTHON_DOCUMENTED_HEAD,
    },
    ShippedStagedFile {
        path: PYTHON_STAGED_PACKAGE_PATH,
        head: PYTHON_UNDOCUMENTED_HEAD,
    },
    ShippedStagedFile {
        path: PYTHON_STAGED_TEST_PATH,
        head: PYTHON_DOCUMENTED_HEAD,
    },
    ShippedStagedFile {
        path: PYTHON_STAGED_UNDOCUMENTED_PATH,
        head: PYTHON_UNDOCUMENTED_HEAD,
    },
];

/// The file of each finding the four staged positions must report, in the order
/// ruff writes them.
///
/// ruff sorts its report by path, and it holds one file's findings in row order,
/// so the package docstring stands above the helper of the same file. Measured:
/// the order does not move when the file arguments are shuffled.
const PYTHON_STAGED_REPORTS: &[&str] = &[
    PYTHON_STAGED_DOCUMENTED_PATH,
    PYTHON_STAGED_PACKAGE_PATH,
    PYTHON_STAGED_PACKAGE_PATH,
    PYTHON_STAGED_TEST_PATH,
    PYTHON_STAGED_UNDOCUMENTED_PATH,
    PYTHON_STAGED_UNDOCUMENTED_PATH,
];

/// The four staged Python positions, and what the real ruff pipeline must
/// report over them.
const PYTHON_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: PYTHON_STAGED_REPORTS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one test class, one test function and one helper at four positions",
    declarations: PYTHON_STAGED_DECLARATIONS,
    staged: PYTHON_STAGED_FILES,
    support: NO_SUPPORT_FILES,
    reason: "the rule reads the item's own name and never the path: the test class, the test \
             method and the test function are silent at every position, the helper reports at \
             every position, and a module or a package with no docstring reports one more",
};

/// Acceptance: the shipped Python missing-docs tool rule carves a test out by
/// the item's own NAME, through the real ruff pipeline.
///
/// The `missing-docs` prompt rule asks for exactly this test: "Identify test
/// items from the structural marker on the item itself ... not from the file
/// name or path." ruff has no filter on a name, and `--isolated` discards the
/// `per-file-ignores` entry a project holds for its own test tree, so the script
/// reads the definition line each finding stands on.
///
/// The four positions hold the same declarations, so the path and the module
/// docstring are the only things that differ. `tests/test_documented.py` carries
/// the directory and the file name pytest collects by, and it reports the same
/// helper the ordinary position reports — which is what a path-shaped carve-out
/// would lose in silence.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reads_the_item_name_and_not_the_path() {
    verify_shipped_staged_positions_report(&PYTHON_MISSING_DOCS_POSITIONS_PROBE);
}

/// A Python file that does not parse: the parameter list of `broken` never
/// closes.
const PYTHON_UNPARSABLE_SOURCE: &str = "def broken(\n";

/// Where the unparsable file stands inside the probe repository.
const PYTHON_UNPARSABLE_PATH: &str = "broken.py";

/// Where the file the run CAN judge stands, beside each item it cannot judge.
const PYTHON_JUDGED_PATH: &str = "judged.py";

/// A Python file the run judges. The module carries a docstring and the
/// function under it carries none, so `D103` is its one finding.
///
/// The module docstring is what holds the count at one: a module with no
/// docstring reports `D100` beside the function, and one row states as much
/// about a lost finding as two do.
const PYTHON_JUDGED_SOURCE: &str = concat!(
    "\"\"\"A documented module.\"\"\"\n",
    "\n",
    "\n",
    "def undocumented_function() -> None:\n",
    "    return None\n",
);

/// The definition line the one finding of the judged file stands on.
const PYTHON_JUDGED_DECLARATION: &str = "def undocumented_function() -> None:";

/// The `path:line` row the run must report for the judged file.
fn python_missing_docs_judged_row() -> String {
    expected_row(
        PYTHON_JUDGED_PATH,
        PYTHON_JUDGED_SOURCE,
        PYTHON_JUDGED_DECLARATION,
    )
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a Python
/// file it cannot parse, through the real ruff pipeline.
///
/// ruff writes a file it cannot parse onto the SAME report as a finding, under
/// `"code": "invalid-syntax"`. Measured with ruff 0.14.5 over `def broken(`
/// beside a module whose function carries no docstring: three rows on the
/// report — `D100` and `D103` of the file it read AND the parse failure — at
/// exit 1, and nothing on stderr. ruff judged the other file, so the finding is
/// there to lose.
///
/// A filter that selected the seven documentation codes alone dropped the parse
/// row, and the unparsable file then read as clean. An `exit 1` answers that no
/// better: it fails the WHOLE run, so the documentation findings of the file
/// ruff DID judge go away with the file it did not. Measured with the shipped
/// script before this fix over the same two files: nothing on stdout, one
/// unmarked line on stderr, exit 1. The parse failure is one declined item of a
/// sound run, so the script states it under the `sah-diagnostic:` marker at
/// exit 0.
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_file_it_cannot_parse() {
    let expected = python_missing_docs_judged_row();

    verify_unjudged_file_is_declined(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &[
            (PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE),
            (PYTHON_UNPARSABLE_PATH, PYTHON_UNPARSABLE_SOURCE),
        ],
        PYTHON_UNPARSABLE_PATH,
        &[&expected],
    );
}

/// Where the path the run cannot read stands inside the probe repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const PYTHON_UNREADABLE_PATH: &str = "unreadable.py";

/// A Python file whose bytes are not UTF-8.
///
/// The module carries a docstring, so a run that DID read it would report
/// nothing; the string literal holds two bytes that open no UTF-8 sequence, so
/// a reader opens the file and cannot decode it.
const PYTHON_UNDECODABLE_SOURCE: &[u8] =
    b"\"\"\"A documented module.\"\"\"\n\nVALUE = '\xff\xfe'\n";

/// A Python file the tool could read if the mode let it.
///
/// The module carries a docstring and holds no other item, so a run that DID
/// read it would report no finding — which is the clean answer this rule must
/// not give for a file it never read.
const PYTHON_FORBIDDEN_SOURCE: &str = "\"\"\"A documented module.\"\"\"\n";

/// The `missing-docs-python` probe over a refusing path beside `judged.py`.
///
/// The judged file carries one undocumented function, so the run has a finding
/// to lose. Losing it is what a nonzero exit over a declined item costs, and
/// staying silent about the path is what reads that path as a clean file.
fn python_missing_docs_decline_probe() -> ShippedDeclineProbe {
    ShippedDeclineProbe {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        judged: vec![(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE.to_string())],
        path: PYTHON_UNREADABLE_PATH,
        expected: vec![python_missing_docs_judged_row()],
    }
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a path that
/// holds no file, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a path, against the shipped command
/// line: the report holds the findings of the other file and nothing for this
/// path, `warning: Failed to lint absent.py: No such file or directory (os
/// error 2)` stands on stderr, and ruff exits as it would without the path. The
/// empty report reads exactly like a clean file, so the answer has to come from
/// what ruff itself said.
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_path_that_holds_no_file() {
    verify_unreadable_file_is_declined(
        &python_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Absent,
    );
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a file whose
/// bytes are not UTF-8, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a file, against the shipped command
/// line: the report holds the findings of the other file, `warning: Failed to
/// lint notutf8.py: stream did not contain valid UTF-8` stands on stderr, and
/// ruff exits as it would without the file.
///
/// This is the shape a readability test on the PATH admits — the mode lets the
/// tool open the file — so the run this replaced reported the other file,
/// exited 0, and said nothing an engine reads about this one. Measured with the
/// shipped script before this fix: two findings on stdout, ruff's own unmarked
/// `Failed to lint` line on stderr, exit 0, and the engine drops an unmarked
/// line as tool chatter. The file read as CLEAN.
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_file_it_cannot_decode() {
    verify_unreadable_file_is_declined(
        &python_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Undecodable(PYTHON_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a file it may
/// not read, through the real ruff pipeline.
///
/// Measured with ruff 0.14.5 over such a file, against the shipped command
/// line: the report holds the findings of the other file, `warning: Failed to
/// lint noread.py: Permission denied (os error 13)` stands on stderr, and ruff
/// exits as it would without the file.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &python_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Forbidden(PYTHON_FORBIDDEN_SOURCE),
    );
}

/// Where the directory nobody may read stands inside the probe repository.
///
/// The name carries the Python extension, the way `Sources/Hollow.swift` does
/// for Swift: the engine hands a `files`-scope run the paths its work-list
/// holds, and a path the rule's own file pattern refuses reaches no run at
/// all.
const PYTHON_UNREADABLE_DIRECTORY: &str = "unread.py";

/// What ruff says for a directory it may not read, with its `warning: ` head
/// taken off.
///
/// The line names NO path. ruff walks the path it is given, and a directory it
/// may not open stops the walk before it reaches a file of its own to name.
const PYTHON_DIRECTORY_REFUSAL: &str = "Encountered error: Permission denied (os error 13)";

/// Acceptance: the shipped Python missing-docs tool rule DECLINES a directory
/// it may not read, through the real ruff pipeline.
///
/// A directory refuses ruff under another head than a file does. Measured with
/// ruff 0.14.5 over `judged.py` beside a mode-000 directory: the report holds
/// the `D103` of the file it judged, ruff exits 1, and stderr carries
/// `warning: Encountered error: Permission denied (os error 13)` — a head that
/// is NOT `warning: Failed to lint `, and a line that carries no path.
///
/// A scan that read the one head therefore matched nothing. Measured with the
/// shipped script before this fix: exit 0, the `D103` reported, and 0 bytes on
/// stderr — the directory nobody read came back as a fully judged clean tree.
///
/// The probe takes every permission off the directory, which is a mode, so it
/// runs on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_python_missing_docs_tool_rule_declines_a_directory_it_may_not_read() {
    let expected = python_missing_docs_judged_row();
    let judged = [(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE)];
    let prepare = |repo: &Path| forbid_probe_directory(&repo.join(PYTHON_UNREADABLE_DIRECTORY));
    let restore = |repo: &Path| restore_probe_directory(&repo.join(PYTHON_UNREADABLE_DIRECTORY));
    let staging = ShippedStaging {
        prepare: &prepare,
        restore: &restore,
        ..ShippedStaging::of(&judged)
    };

    verify_declined_item_is_stated(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &staging,
        &[PYTHON_JUDGED_PATH, PYTHON_UNREADABLE_DIRECTORY],
        PYTHON_DIRECTORY_REFUSAL,
        &[&expected],
    );
}

/// Where the file whose name holds a backslash stands inside the probe
/// repository.
///
/// A backslash is the character the earlier hand-off spelled wrong. The script
/// flattened ruff's report to TSV with `jq ... | @tsv`, and `@tsv` escapes a
/// backslash to `\\`, so every row of this file named a path that stands on no
/// disk.
const PYTHON_BACKSLASH_PATH: &str = "back\\slash.py";

/// A Python file with no module docstring whose one function carries none
/// either, so ruff reports `D100` and `D103` on it.
const PYTHON_BACKSLASH_SOURCE: &str = "def in_a_named_file() -> None:\n    return None\n";

/// The definition line the `D103` of the backslash file stands on. It opens
/// [`PYTHON_BACKSLASH_SOURCE`], so the row is read out of the staged bytes.
const PYTHON_BACKSLASH_DECLARATION: &str = "def in_a_named_file() -> None:";

/// Each `path:line` row a run over the judged file and the backslash file must
/// report.
///
/// ruff sorts its report by path, so `back\slash.py` stands above `judged.py`.
/// Both of its rows stand on the first line, for two different reasons: `D100`
/// names the module, which carries no definition line of its own, and the
/// function `D103` names opens the file.
fn python_backslash_rows() -> Vec<String> {
    vec![
        format!("{PYTHON_BACKSLASH_PATH}:{}", python_module_row!()),
        expected_row(
            PYTHON_BACKSLASH_PATH,
            PYTHON_BACKSLASH_SOURCE,
            PYTHON_BACKSLASH_DECLARATION,
        ),
        python_missing_docs_judged_row(),
    ]
}

/// Acceptance: the shipped Python missing-docs tool rule reports a path that
/// holds a backslash as the path it really is, through the real ruff pipeline.
///
/// The script used to hand its rows from `jq` to `awk` as TSV. `@tsv` escapes
/// a backslash, a tab and a newline alike, so the text of every field reached
/// awk changed. Measured with ruff 0.14.5 and jq 1.8.2 over this probe before
/// the fix: three findings at exit 0, two of them naming
/// `back\\slash.py` — a path the engine can attribute to no file — and one
/// diagnostic saying the definition-line scan could not read that same doubled
/// name.
///
/// The filter now reads ruff's JSON report itself, so no text hand-off stands
/// between the report and the scan. The probe holds the whole row list rather
/// than a count, because the ROW is what was wrong.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reports_a_path_holding_a_backslash() {
    let loader = builtin_loader();
    require_tool_installed(&loader, PYTHON_PROJECT_TYPES, PYTHON_MISSING_DOCS_RULE);
    let staged = [
        (PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE),
        (PYTHON_BACKSLASH_PATH, PYTHON_BACKSLASH_SOURCE),
    ];
    let named = [PYTHON_JUDGED_PATH, PYTHON_BACKSLASH_PATH];
    let staging = ShippedStaging::of(&staged);
    let drive = |read: fn(&ScriptOutcome, &Path) -> Vec<String>| {
        drive_shipped_script(&loader, PYTHON_MISSING_DOCS_RULE, &staging, &named, read)
            .expect("a run over a path holding a backslash must judge both files and exit 0")
    };

    let reported = drive(finding_rows);
    let stated = drive(script_diagnostics);

    assert_eq!(
        reported,
        python_backslash_rows(),
        "every finding must name the path ruff read, with the backslash it really holds"
    );
    assert!(
        stated.is_empty(),
        "the run reads every file ruff reported, so it must decline no item; it stated {stated:?}"
    );
}

/// The path a stubbed ruff reports a finding of, and that the definition-line
/// scan then cannot open.
const PYTHON_VANISHED_PATH: &str = "vanished.py";

/// The row the stubbed ruff stands the finding of the vanished path on.
///
/// The path holds no file, so no staged source can carry the row. The stub
/// therefore writes the head of a file, and
/// [`the_shipped_python_missing_docs_tool_rule_keeps_the_findings_its_scan_cannot_carve`]
/// reads the same number back, so this constant is what holds the report the
/// stub wrote and the row the run must place to one value.
const PYTHON_VANISHED_FINDING_ROW: &str = opening_declaration_row!();

/// The code ruff writes for a public function that carries no docstring.
const PYTHON_UNDOCUMENTED_FUNCTION_CODE: &str = "D103";

/// The message ruff writes under [`PYTHON_UNDOCUMENTED_FUNCTION_CODE`].
const PYTHON_UNDOCUMENTED_FUNCTION_MESSAGE: &str = "Missing docstring in public function";

/// One entry of a stubbed ruff report: a `D103` of `path` on `row`.
fn python_stub_report_entry(path: &str, row: &str) -> String {
    format!(
        "{{\"filename\": \"{path}\", \"location\": {{\"row\": {row}}}, \
         \"code\": \"{PYTHON_UNDOCUMENTED_FUNCTION_CODE}\", \
         \"message\": \"{PYTHON_UNDOCUMENTED_FUNCTION_MESSAGE}\"}}"
    )
}

/// A stubbed ruff that exits 1 and reports one `D103` of a path that holds no
/// file, beside the `D103` of the judged file.
///
/// The shipped pipeline reaches no such row of its own any more: the filter
/// opens each path ruff wrote on its report, exactly as ruff spelled it, and
/// ruff declines on stderr every path it could not open itself. So the fail-
/// open needs a ruff that reports a file no reader can follow it to.
///
/// The row of the judged entry is read out of [`PYTHON_JUDGED_SOURCE`] rather
/// than written as a number, because the test holds the run to the row
/// [`python_missing_docs_judged_row`] reads out of those same bytes, and the
/// two must never drift apart.
fn python_vanished_report_answer() -> String {
    let judged_row = declaration_line(PYTHON_JUDGED_SOURCE, PYTHON_JUDGED_DECLARATION);
    let vanished = python_stub_report_entry(PYTHON_VANISHED_PATH, PYTHON_VANISHED_FINDING_ROW);
    let judged = python_stub_report_entry(PYTHON_JUDGED_PATH, &judged_row.to_string());

    format!("  printf '[{vanished}, {judged}]'\n  exit 1")
}

/// Acceptance: the shipped Python missing-docs tool rule keeps every finding
/// of a file its definition-line scan could not read.
///
/// The scan re-reads each file ruff reported, to read the definition line the
/// test carve-out needs. That read can fail where ruff's own read did not, and
/// a failed scan is one more declined item: the finding is real, and only the
/// carve-out is unanswerable. So the scan states the file under the marker and
/// reads no line for it, and every row of that file keeps its finding.
///
/// Measured with the shipped script before the fail-open, over a file the scan
/// could not open: nothing on stdout, one unmarked line on stderr, exit 1 —
/// the finding of the file the scan DID read lost with it.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_missing_docs_tool_rule_keeps_the_findings_its_scan_cannot_carve() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &[(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE)],
        &[PYTHON_JUDGED_PATH],
        PYTHON_TOOL_BINARY_NAME,
        &python_vanished_report_answer(),
    );
    let outcome = run
        .outcome
        .expect("a scan that cannot read one file must keep every finding and exit 0");

    assert_eq!(
        run.placed,
        vec![
            format!("{PYTHON_VANISHED_PATH}:{PYTHON_VANISHED_FINDING_ROW}"),
            python_missing_docs_judged_row(),
        ],
        "the finding of the file the scan could not read must stand beside the one it read"
    );
    assert_eq!(
        outcome.diagnostics,
        vec![format!(
            "missing-docs-python could not read {PYTHON_VANISHED_PATH}, so every finding of \
             that file stands"
        )],
        "the run must state the one file its scan could not read"
    );
}

/// The name ruff calls the linter of Python code by.
const PYTHON_TOOL_BINARY_NAME: &str = "ruff";

/// What the run must say when its filter could not read what ruff wrote.
///
/// A fragment of the line rather than the whole of it: the filter names the
/// reason Python gave beside these words, and that reason moves with the
/// Python release.
const PYTHON_FILTER_BROKEN_MESSAGE: &str =
    "missing-docs-python: the filter could not read the ruff report";

/// Why a report the filter cannot read must break the run rather than pass it.
const PYTHON_FILTER_BROKEN_REASON: &str =
    "a report the filter could not read leaves the run with no measurement at all, so it \
     must break in the rule's own words rather than exit on the filter's status";

/// A stubbed ruff that exits 1 and writes a report that stops in the middle of
/// its first entry.
///
/// Status 1 is the status ruff exits with for a file that HAS findings, so the
/// broken-run gate lets it through. The report is what the filter then cannot
/// read.
const PYTHON_TRUNCATED_REPORT_ANSWER: &str =
    "  printf '[\\n  {\\n    \"code\": \"D103\"'\n  exit 1";

/// Acceptance: the shipped Python missing-docs tool rule BREAKS when ruff
/// writes a report the filter cannot read.
///
/// The broken-run gate reads the STATUS, and ruff keeps status 1 for a file
/// with findings, so a malformed report at status 1 passes the gate. The
/// filter then fails, and a filter that reads the report with no guard takes
/// the whole script down with its own status. Measured with a stubbed ruff
/// over a filter with no guard: the script exited 1, wrote nothing to stdout,
/// and wrote a Python traceback naming the temporary copy of the filter — no
/// marker, and no word about which rule broke.
///
/// `missing-docs-rust.md` and `function-length-rust.md` hold the worked
/// answer, and this rule now holds it too: the filter catches the report it
/// cannot read, and breaks in the rule's own words.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_missing_docs_tool_rule_breaks_on_a_report_the_filter_cannot_read() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &[(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE)],
        &[PYTHON_JUDGED_PATH],
        PYTHON_TOOL_BINARY_NAME,
        PYTHON_TRUNCATED_REPORT_ANSWER,
    );
    let failure = run.outcome.expect_err(PYTHON_FILTER_BROKEN_REASON);

    assert_shipped_break(
        &failure,
        run.status,
        &[PYTHON_FILTER_BROKEN_MESSAGE],
        PYTHON_FILTER_BROKEN_REASON,
    );
    assert!(
        run.placed.is_empty(),
        "a run that breaks must place no finding; it placed {:?}: {PYTHON_FILTER_BROKEN_REASON}",
        run.placed
    );
}

/// The path a stubbed ruff declines on a line that carries no closing newline.
const PYTHON_UNTERMINATED_PATH: &str = "unterminated.py";

/// A stubbed ruff that judges nothing, and declines one path on a line with no
/// closing newline.
///
/// The report is an empty array, so the filter reads it and the run reports
/// nothing. The one decline stands on stderr, and the bytes stop where the
/// message stops.
const PYTHON_UNTERMINATED_ANSWER: &str = concat!(
    "  printf '[]'\n",
    "  printf 'warning: Failed to lint unterminated.py: No such file or directory (os error 2)' \
     >&2\n",
    "  exit 1"
);

/// Acceptance: the shipped Python missing-docs tool rule reads a decline line
/// that carries no closing newline.
///
/// `while IFS= read -r line` answers nonzero for a last line with no newline
/// under it, so a loop that reads the status alone never runs its body for
/// that line and the decline goes unsaid. Measured over two decline lines with
/// the shipped script before this fix: 2 diagnostics with the closing newline,
/// and 1 without it — the last decline lost, and that path then read as clean.
///
/// The probe leads `PATH` with a stub, which is process state, so it stands
/// under `#[serial_test::serial(env)]`.
#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn the_shipped_python_missing_docs_tool_rule_reads_a_decline_with_no_closing_newline() {
    let run = drive_shipped_script_with_stub(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        NO_SUPPORT_FILES,
        &[PYTHON_UNTERMINATED_PATH],
        PYTHON_TOOL_BINARY_NAME,
        PYTHON_UNTERMINATED_ANSWER,
    );
    let outcome = run
        .outcome
        .expect("a script handed an item it cannot judge must judge the rest and exit 0");

    assert_eq!(
        outcome.diagnostics,
        vec![format!(
            "ruff declined an item and said: warning: Failed to lint {PYTHON_UNTERMINATED_PATH}: \
             No such file or directory (os error 2)"
        )],
        "the run must state the decline that stands on the last line of ruff's stderr, \
         whether or not a newline closes it"
    );
}

/// An undocumented Python module at the root of the probe repository. ruff
/// reports `D100` on the module and `D103` on the function.
const PYTHON_UNREAD_TOP_SOURCE: &str = "def top():\n    return 1\n";

/// The same, nested three directories deep. ruff walks a whole tree, so a
/// default target reaches this file as readily as the one at the root.
const PYTHON_UNREAD_NESTED_SOURCE: &str = "class Other:\n    def method(self):\n        return 2\n";

/// Every Python file staged in the probe repository the script is given none
/// of.
const PYTHON_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.py", PYTHON_UNREAD_TOP_SOURCE),
    ("deep/nested/other.py", PYTHON_UNREAD_NESTED_SOURCE),
];

/// Each finding the Python missing-docs script reports over the two files it
/// is given, as `path:line`.
///
/// The nested file answers three: `D100` for the module, `D101` for the class
/// that opens it, and `D102` for the method on the line under the class head.
/// The file at the root answers two: `D100` for the module, and `D103` for the
/// function that opens it.
const PYTHON_READ_FINDINGS: &[&str] = &[
    concat!("deep/nested/other.py:", python_module_row!()),
    concat!("deep/nested/other.py:", opening_declaration_row!()),
    concat!("deep/nested/other.py:", nested_declaration_row!()),
    concat!("top.py:", python_module_row!()),
    concat!("top.py:", opening_declaration_row!()),
];

/// The `missing-docs-python` probe over a run that is given no file.
const PYTHON_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: PYTHON_UNREAD_FILES,
    with_files: PYTHON_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Python missing-docs tool rule reads only the files
/// it is given, through the real ruff pipeline.
///
/// `ruff check` with no path argument falls back to a default target of `.`,
/// and it walks that whole tree. A script that hands `"$@"` straight to ruff
/// therefore answers for every Python file under the repository root when the
/// run carries no file, and it exits 0, so the answer reads as a measured
/// result rather than a mistake. Measured over this probe before the guard:
/// 5 findings across `top.py` and `deep/nested/other.py`, neither of which the
/// script was given, and an exit status of 0.
///
/// The script therefore answers an empty argument list at once, with no
/// finding and an exit status of 0. The same script over the two staged files
/// reports 5.
#[test]
fn the_shipped_python_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&PYTHON_EMPTY_RUN_PROBE);
}

/// The materialized name of the `missing-docs-swift` fail fixture.
const SWIFT_MISSING_DOCS_FAIL_FIXTURE: &str = concat!(missing_docs_rule!(swift), ".fail.swift");

/// The materialized name of the `missing-docs-swift` pass fixture.
const SWIFT_MISSING_DOCS_PASS_FIXTURE: &str = concat!(missing_docs_rule!(swift), ".pass.swift");

/// Where the `missing-docs-swift` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const SWIFT_MISSING_DOCS_FIXTURE_PATH: &str = "Sources/MissingDocsSwiftFail.swift";

/// Where the `missing-docs-swift` pass fixture stands inside the probe
/// repository, as the work-list holds it.
const SWIFT_MISSING_DOCS_PASS_PATH: &str = "Sources/MissingDocsSwiftPass.swift";

/// Every declaration the `missing-docs-swift` fail fixture leaves
/// undocumented, trimmed as the fixture writes it.
///
/// A line, and not a claim, because the claim of a finding spells only the
/// NAME of the declaration, and a method and a function can share one.
const SWIFT_MISSING_DOCS_FAIL_LINES: &[&str] = &[
    "public struct UndocumentedStructure {",
    "public func undocumentedMethod() {}",
    "public func undocumentedFunction() {}",
];

/// The `missing-docs-swift` fail fixture, and every undocumented declaration
/// the real `swift format` pipeline must report inside it.
const SWIFT_MISSING_DOCS_FAIL_PROBE: ShippedFailFixture = ShippedFailFixture {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_MISSING_DOCS_FAIL_LINES,
    },
    fixture: SWIFT_MISSING_DOCS_FAIL_FIXTURE,
    path: SWIFT_MISSING_DOCS_FIXTURE_PATH,
    support: NO_SUPPORT_FIXTURES,
    noun: "line holding an undocumented public declaration",
};

/// Acceptance: the shipped Swift missing-docs tool rule reports every
/// undocumented declaration its fail fixture holds, through the real
/// `swift format` pipeline.
///
/// A declaration is held to the SOURCE LINE its finding stands on. The count
/// is the other half, and it is what a silent run cannot fake: the fixture
/// holds three undocumented declarations and one documented neighbor, and the
/// run reports exactly those three. A run that read no configuration of its
/// own reports none, because `AllPublicDeclarationsHaveDocumentation` is OFF
/// in the toolchain's default configuration.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_every_fail_fixture_line() {
    verify_shipped_fail_fixture_reports_each(
        &SWIFT_MISSING_DOCS_FAIL_PROBE,
        |content| {
            tool_rule_work(
                "an undocumented public structure, method and function",
                CODE_HYGIENE_SET,
                [
                    MISSING_DOCS_PROMPT_RULE.to_string(),
                    SWIFT_MISSING_DOCS_RULE.to_string(),
                ],
                [(SWIFT_MISSING_DOCS_FIXTURE_PATH, content)],
            )
        },
        fail_fixture_source_line,
        |reported, expected| reported == expected,
    );
}

/// The name of the one rule this gate keeps.
const SWIFT_MISSING_DOCS_TOOL_RULE: &str = "AllPublicDeclarationsHaveDocumentation";

/// Where a probe stages the Swift file it measures the rule over.
const SWIFT_MISSING_DOCS_PROBE_PATH: &str = "Sources/Probe.swift";

/// Where a probe stages a SECOND Swift file of the same run.
const SWIFT_MISSING_DOCS_SECOND_PATH: &str = "Sources/Second.swift";

/// Reads the shipped pass fixture, and answers the `path:line` rows the
/// shipped script reports over it.
fn swift_missing_docs_pass_fixture_findings() -> Vec<String> {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_MISSING_DOCS_RULE);

    let shipped = shipped_asset(
        &loader,
        &FIXTURE_TEMPLATE_ASSET,
        SWIFT_MISSING_DOCS_PASS_FIXTURE,
    );
    let content = std::fs::read_to_string(shipped).expect("read the shipped pass fixture");

    shipped_script_findings(
        &loader,
        SWIFT_MISSING_DOCS_RULE,
        &[(SWIFT_MISSING_DOCS_PASS_PATH, content.as_str())],
        &[SWIFT_MISSING_DOCS_PASS_PATH],
    )
    .expect("the shipped Swift missing-docs script must judge its pass fixture and exit 0")
}

/// Acceptance: the shipped Swift missing-docs tool rule reports NOTHING over
/// its own passing fixture.
///
/// The fixture is written with 4-space indentation, the house style of every
/// Swift fixture of this set, and `swift format` asks for 2. The raw tool run
/// over it writes one `[Indentation]` line for each member, and no key of the
/// configuration stops the pretty-printer that writes them. So the script must
/// read the tag off each output line and keep only
/// `AllPublicDeclarationsHaveDocumentation`. A script that drops that filter
/// reports layout here and fails this test by name.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_nothing_over_its_pass_fixture() {
    let rows = swift_missing_docs_pass_fixture_findings();

    assert!(
        rows.is_empty(),
        "the passing fixture documents every public declaration, so the gate must report \
         nothing over it, whatever the pretty-printer says about its layout; the run reported \
         {rows:?}"
    );
}

/// A Swift file that documents every public declaration and lays the code out
/// in ways the pretty-printer reports: a deep indent, a missing space around
/// an operator, and trailing whitespace.
const SWIFT_MISSING_DOCS_LAYOUT_SOURCE: &str = concat!(
    "/// A documented structure.\n",
    "public struct Laid {\n",
    "        /// A documented property.\n",
    "        public var total = 1+2   \n",
    "}\n",
);

/// Acceptance: the gate drops every layout tag the pretty-printer writes.
///
/// `swift format lint` always writes `[Indentation]`, `[Spacing]`,
/// `[TrailingWhitespace]` and the other layout tags, and no key of the
/// configuration turns them off. The probe documents every public declaration
/// and draws three of those tags, so the only honest answer is no finding.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_drops_every_layout_finding() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_LAYOUT_SOURCE,
        NO_SUPPORT_FILES,
    );

    assert!(
        reported.is_empty(),
        "a documented file with layout defects must draw no finding; the run reported \
         {reported:?}"
    );
}

/// A Swift file whose documentation comments draw the findings of the two
/// documentation-quality rules, beside one default-on rule and one real
/// missing comment.
///
/// - the summary runs to two sentences, which
///   `BeginDocumentationCommentWithOneLineSummary` reports once it is ON;
/// - the `Parameter` entry names no parameter, which
///   `ValidateDocumentationComments` reports once it is ON;
/// - `Bad_One` is a name `AlwaysUseLowerCamelCase` reports in the DEFAULT
///   configuration;
/// - the last property is the one missing comment.
const SWIFT_MISSING_DOCS_OTHER_RULES_SOURCE: &str = concat!(
    "/// Summary sentence one. Sentence two follows it.\n",
    "///\n",
    "/// - Parameter wrong: names no parameter.\n",
    "public func documented(right: Int) {}\n",
    "\n",
    "let Bad_One = 1\n",
    "\n",
    "/// A documented structure.\n",
    "public struct Bag {\n",
    "    public var items: Int\n",
    "}\n",
);

/// Acceptance: the gate keeps the one rule it owns and no other.
///
/// `ValidateDocumentationComments` and
/// `BeginDocumentationCommentWithOneLineSummary` decide the bullets of the
/// `doc-parameter-naming` prompt rule, and `AlwaysUseLowerCamelCase` is ON in
/// the toolchain's default configuration. Measured over this probe with those
/// three rules ON, the tool reports each of them. The gate must report the
/// one missing comment of `Bag.items` and nothing else, and every finding it
/// reports must carry `AllPublicDeclarationsHaveDocumentation`.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_names_only_its_own_rule() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_OTHER_RULES_SOURCE,
        NO_SUPPORT_FILES,
    );

    assert_eq!(
        reported,
        vec![SWIFT_MISSING_DOCS_TOOL_RULE.to_string()],
        "the probe holds one missing comment and three defects of other rules, so the gate \
         must report one finding carrying only `{SWIFT_MISSING_DOCS_TOOL_RULE}`"
    );
}

/// Where a project states its own `swift format` settings.
const SWIFT_MISSING_DOCS_PROJECT_CONFIG_PATH: &str = ".swift-format";

/// A project `.swift-format` that turns the rule this gate owns OFF.
const SWIFT_MISSING_DOCS_PROJECT_CONFIG: &str =
    "{\"rules\":{\"AllPublicDeclarationsHaveDocumentation\":false}}\n";

/// Acceptance: a project cannot switch the gate off with its own
/// `.swift-format`.
///
/// `swift format` reads a project `.swift-format` on its own, and the
/// `--configuration` the script gives the tool takes that file out of the run.
/// So a project that states the rule OFF still gets the finding. To exempt a
/// declaration the author writes the `swift-format-ignore` directive in the
/// code, where a reader sees it.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_keeps_its_own_configuration() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_STAGED_PATH,
        SWIFT_STAGED_DECLARATIONS,
        &[(
            SWIFT_MISSING_DOCS_PROJECT_CONFIG_PATH,
            SWIFT_MISSING_DOCS_PROJECT_CONFIG,
        )],
    );

    assert_eq!(
        reported.len(),
        2,
        "the project turns the rule OFF in its own `.swift-format`, and the gate must still \
         report the structure and the property; the run reported {reported:?}"
    );
}

/// Where the staged declarations stand for a probe of one file.
const SWIFT_STAGED_PATH: &str = "Sources/Staged.swift";

/// A public getter and a public protocol-conformance member inside types that
/// declare an inherited type.
///
/// The `missing-docs` prompt rule carves out "Simple getters/setters" and
/// "Obvious implementations". The toolchain rule keeps neither carve-out for
/// Swift: it reads the access level and the doc comment and nothing else.
const SWIFT_MISSING_DOCS_INHERITED_SOURCE: &str = concat!(
    "public struct Wide: Equatable {\n",
    "    public var name: String { \"\" }\n",
    "}\n",
    "\n",
    "extension Wide: CustomStringConvertible {\n",
    "    public var description: String { \"\" }\n",
    "}\n",
);

/// Acceptance: the gate reports an undocumented public getter and an
/// undocumented conformance member, inside a type that declares an inherited
/// type.
///
/// `missing-docs.md` states that Swift keeps neither the getter carve-out nor
/// the obvious-implementation carve-out. This probe is the measurement behind
/// that statement: the structure, the getter and the `description` member each
/// report.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_a_getter_in_a_type_with_an_inherited_type() {
    let reported = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_STAGED_PATH,
        SWIFT_MISSING_DOCS_INHERITED_SOURCE,
        NO_SUPPORT_FILES,
    );

    assert_eq!(
        reported.len(),
        3,
        "the structure, its getter and the conformance member are each public and each \
         undocumented, so the gate must report all three; the run reported {reported:?}"
    );
}

/// Two undocumented public functions, the first under the directive that
/// exempts one declaration.
const SWIFT_MISSING_DOCS_DIRECTIVE_SOURCE: &str = concat!(
    "// swift-format-ignore: AllPublicDeclarationsHaveDocumentation\n",
    "public func exempted() {}\n",
    "\n",
    "public func reported() {}\n",
);

/// The same two functions under the directive that exempts the whole file.
const SWIFT_MISSING_DOCS_FILE_DIRECTIVE_SOURCE: &str = concat!(
    "// swift-format-ignore-file\n",
    "public func exempted() {}\n",
    "\n",
    "public func alsoExempted() {}\n",
);

/// Acceptance: the directives an author writes in the code exempt what they
/// name, and the gate reads them.
///
/// The line form exempts ONE declaration, so the second function still
/// reports. The file form exempts the whole file, which is how generated code
/// leaves the gate: the `missing-docs` prompt rule carves generated code out,
/// and the tool carries no generated-code check of its own.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reads_the_exemption_directives() {
    let one = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_DIRECTIVE_SOURCE,
        NO_SUPPORT_FILES,
    );
    assert_eq!(
        one,
        vec![SWIFT_MISSING_DOCS_TOOL_RULE.to_string()],
        "the line directive exempts one declaration, so the second function alone must report"
    );

    let whole = swift_gate_reporting_rules(
        SWIFT_MISSING_DOCS_RULE,
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_FILE_DIRECTIVE_SOURCE,
        NO_SUPPORT_FILES,
    );
    assert!(
        whole.is_empty(),
        "the file directive exempts every declaration of the file; the run reported {whole:?}"
    );
}

/// Acceptance: a run over two files that both report gives the findings of
/// both.
///
/// The script hands `swift format` one path for each run, and a file that
/// reports makes the tool exit 1. A loop that stopped at the first file that
/// reported, or that let `set -e` end the run on that status, would report the
/// first file alone and the second would read as clean.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reports_every_file_that_reports() {
    let loader = builtin_loader();
    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_MISSING_DOCS_RULE);

    let staged = [
        (SWIFT_MISSING_DOCS_PROBE_PATH, SWIFT_STAGED_DECLARATIONS),
        (SWIFT_MISSING_DOCS_SECOND_PATH, SWIFT_STAGED_DECLARATIONS),
    ];
    let rows = shipped_script_findings(
        &loader,
        SWIFT_MISSING_DOCS_RULE,
        &staged,
        &[
            SWIFT_MISSING_DOCS_PROBE_PATH,
            SWIFT_MISSING_DOCS_SECOND_PATH,
        ],
    )
    .expect("the shipped Swift missing-docs script must judge both probe files and exit 0");

    for path in [
        SWIFT_MISSING_DOCS_PROBE_PATH,
        SWIFT_MISSING_DOCS_SECOND_PATH,
    ] {
        assert_eq!(
            rows.iter().filter(|row| row.starts_with(path)).count(),
            2,
            "a run over two files that both report must state the structure and the property \
             of `{path}`; the run reported {rows:?}"
        );
    }
}

/// One undocumented public structure, and one undocumented stored property
/// inside it.
const SWIFT_STAGED_DECLARATIONS: &str = concat!(
    "public struct StagedThing {\n",
    "    public var value: Int = 0\n",
    "}\n",
);

/// Where the path the missing-docs run cannot judge stands inside the probe
/// repository.
///
/// One name serves all three shapes: the same path holds no file, holds bytes
/// that are not UTF-8, or holds source nobody may read, so the way it refuses
/// is the one difference between the three probes.
const SWIFT_MISSING_DOCS_UNREADABLE_PATH: &str = "Sources/Unreadable.swift";

/// Where the file the run CAN judge stands.
const SWIFT_MISSING_DOCS_JUDGED_PATH: &str = "Sources/Judged.swift";

/// The head of the undocumented public structure
/// [`SWIFT_STAGED_DECLARATIONS`] declares.
const SWIFT_STAGED_STRUCTURE_HEAD: &str = "public struct StagedThing";

/// The head of the undocumented stored property
/// [`SWIFT_STAGED_DECLARATIONS`] declares.
const SWIFT_STAGED_PROPERTY_HEAD: &str = "public var value";

/// A Swift file written in Latin-1 rather than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence, so the
/// tool cannot decode this file. The staged declarations stand under the
/// string, so a run that DID read the file reports them.
const SWIFT_MISSING_DOCS_UNDECODABLE_SOURCE: &[u8] = b"let name = \"caf\xe9\"\n\
public struct StagedThing {\n\
public var value: Int = 0\n\
}\n";

/// A Swift file the tool could read if the mode let it.
///
/// Every declaration is internal, so a run that DID read this file would
/// report no finding, which is the clean answer this rule must not give for a
/// file it never read.
const SWIFT_MISSING_DOCS_FORBIDDEN_SOURCE: &str = concat!(
    "struct InternalThing {\n",
    "    var value: Int = 0\n",
    "}\n"
);

/// The `missing-docs-swift` probe over a refusing path beside
/// `Sources/Judged.swift`.
///
/// The judged file carries one undocumented public structure and one
/// undocumented stored property, so the run has two findings to lose. Losing
/// them is what a nonzero exit over a declined item costs, and staying silent
/// about the path is what reads that path as a clean file.
fn swift_missing_docs_decline_probe() -> ShippedDeclineProbe {
    let structure = expected_row(
        SWIFT_MISSING_DOCS_JUDGED_PATH,
        SWIFT_STAGED_DECLARATIONS,
        SWIFT_STAGED_STRUCTURE_HEAD,
    );
    let property = expected_row(
        SWIFT_MISSING_DOCS_JUDGED_PATH,
        SWIFT_STAGED_DECLARATIONS,
        SWIFT_STAGED_PROPERTY_HEAD,
    );

    ShippedDeclineProbe {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        judged: vec![(
            SWIFT_MISSING_DOCS_JUDGED_PATH,
            SWIFT_STAGED_DECLARATIONS.to_string(),
        )],
        path: SWIFT_MISSING_DOCS_UNREADABLE_PATH,
        expected: vec![structure, property],
    }
}

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a path that
/// holds no file, through the real `swift format` pipeline.
///
/// Measured with Apple Swift 6.4: `swift format lint --strict` over such a
/// path exits 0 and writes NOTHING, which reads exactly like a clean pass over
/// a file the run never opened. So the script tests the path itself, and
/// states it under the marker.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_path_that_holds_no_file() {
    verify_unreadable_file_is_declined(
        &swift_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Absent,
    );
}

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a Swift file
/// the tool cannot decode, through the real `swift format` pipeline.
///
/// The mode lets the script open this file, so a readability test on the path
/// admits it. The answer has to come from what the tool itself said.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_file_it_cannot_decode() {
    verify_unreadable_file_is_declined(
        &swift_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Undecodable(SWIFT_MISSING_DOCS_UNDECODABLE_SOURCE),
    );
}

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a Swift file
/// it may not read, through the real `swift format` pipeline.
///
/// The probe takes every permission off the file, which is a mode, so it runs
/// on unix alone.
#[cfg(unix)]
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_file_it_may_not_read() {
    verify_unreadable_file_is_declined(
        &swift_missing_docs_decline_probe(),
        &ShippedUnreadableFile::Forbidden(SWIFT_MISSING_DOCS_FORBIDDEN_SOURCE),
    );
}

/// A Swift file the parser cannot read.
const SWIFT_MISSING_DOCS_UNPARSABLE_SOURCE: &str = "public func oops( {\n";

/// Acceptance: the shipped Swift missing-docs tool rule DECLINES a file the
/// parser cannot read, and keeps the findings of the file beside it.
///
/// A file the parser cannot read is a file no rule judged. The tool writes
/// its parse errors untagged and exits 1, exactly as it exits for a finding,
/// so the script reads the SHAPE of each line: an untagged line is trouble,
/// and the file that wrote one is declined whole.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_declines_a_file_the_parser_cannot_read() {
    let structure = expected_row(
        SWIFT_MISSING_DOCS_JUDGED_PATH,
        SWIFT_STAGED_DECLARATIONS,
        SWIFT_STAGED_STRUCTURE_HEAD,
    );
    let property = expected_row(
        SWIFT_MISSING_DOCS_JUDGED_PATH,
        SWIFT_STAGED_DECLARATIONS,
        SWIFT_STAGED_PROPERTY_HEAD,
    );

    verify_unjudged_file_is_declined(
        SWIFT_PROJECT_TYPES,
        SWIFT_MISSING_DOCS_RULE,
        &[
            (SWIFT_MISSING_DOCS_JUDGED_PATH, SWIFT_STAGED_DECLARATIONS),
            (
                SWIFT_MISSING_DOCS_UNREADABLE_PATH,
                SWIFT_MISSING_DOCS_UNPARSABLE_SOURCE,
            ),
        ],
        SWIFT_MISSING_DOCS_UNREADABLE_PATH,
        &[structure.as_str(), property.as_str()],
    );
}

/// The `missing-docs-swift` probe over a directory that holds no Swift file.
///
/// The probe writes no file at the path, and the one staged file under that
/// path makes the directory.
const SWIFT_HOLLOW_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: SWIFT_HOLLOW_PURPOSE,
    path: SWIFT_HOLLOW_PATH,
    source: None,
    support: SWIFT_HOLLOW_FILES,
};

/// Acceptance: the shipped Swift missing-docs tool rule answers CLEAN over a
/// directory that holds no Swift file, through the real `swift format`
/// pipeline.
///
/// A path that names a directory makes `swift format` exit 64, and a shared
/// run would lose every finding beside it. The script tests that the path is a
/// file before the tool starts, declines the directory under the marker, and
/// exits 0.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_HOLLOW_PROBE);
}

/// An undocumented Swift file at the root of the probe repository.
const SWIFT_UNREAD_TOP_SOURCE: &str = "public struct Top {\n    public var value: Int = 0\n}\n";

/// The same, nested three directories deep. A tool that walks a whole tree
/// reaches this file as readily as the one at the root.
const SWIFT_UNREAD_NESTED_SOURCE: &str =
    "public enum Other {\n    public static let value = 2\n}\n";

/// Every Swift file staged in the probe repository the script is given none
/// of.
const SWIFT_UNREAD_FILES: &[(&str, &str)] = &[
    ("Top.swift", SWIFT_UNREAD_TOP_SOURCE),
    ("deep/nested/Other.swift", SWIFT_UNREAD_NESTED_SOURCE),
];

/// Each finding the Swift missing-docs script reports over the two files it
/// is given, as `path:line`.
///
/// Each file answers two: the type head that opens it, and the member on the
/// line under that head.
const SWIFT_READ_FINDINGS: &[&str] = &[
    concat!("deep/nested/Other.swift:", opening_declaration_row!()),
    concat!("deep/nested/Other.swift:", nested_declaration_row!()),
    concat!("Top.swift:", opening_declaration_row!()),
    concat!("Top.swift:", nested_declaration_row!()),
];

/// The `missing-docs-swift` probe over a run that is given no file.
const SWIFT_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: SWIFT_UNREAD_FILES,
    with_files: SWIFT_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Swift missing-docs tool rule reads only the files
/// it is given, through the real `swift format` pipeline.
///
/// The script answers an empty argument list at once, with no finding and an
/// exit status of 0, so a run that carries no file never answers for a file
/// the review did not give it. The same script over the two staged files
/// reports 4.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&SWIFT_EMPTY_RUN_PROBE);
}

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

/// An exported Go function that carries no doc comment. revive's `exported`
/// rule reports the declaration, so each file holds one finding.
const GO_MISSING_DOCS_UNREAD_SOURCE: &str = r#"package probe

func Exported() int {
    return 0
}
"#;

/// Every Go file staged in the probe repository the missing-docs script is
/// given none of.
///
/// revive reads the package in the working directory, so the file at the root
/// stands inside its default target and the nested file stands outside it.
const GO_MISSING_DOCS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.go", GO_MISSING_DOCS_UNREAD_SOURCE),
    ("deep/nested/other.go", GO_MISSING_DOCS_UNREAD_SOURCE),
];

/// Each finding the Go missing-docs script reports over the two files it is
/// given, as `path:line`.
///
/// Both files hold the same bytes, and the exported function under the
/// `package` clause is the one declaration revive reports.
const GO_MISSING_DOCS_READ_FINDINGS: &[&str] = &[
    concat!("top.go:", go_declaration_row!()),
    concat!("deep/nested/other.go:", go_declaration_row!()),
];

/// The `missing-docs-go` probe over a run that is given no file.
const GO_MISSING_DOCS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: GO_MISSING_DOCS_UNREAD_FILES,
    with_files: GO_MISSING_DOCS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Go missing-docs tool rule reads only the files it
/// is given, through the real revive pipeline.
///
/// revive with no path argument reads the package standing in the working
/// directory. Measured over this probe with no argument: without the guard
/// the script reported 1 finding, on `top.go`, and exited 0; with the guard
/// it reports none and exits 0. The same script over the two staged files
/// reports 2, so the guard is the whole difference and the nested file is
/// what the default target leaves out.
#[test]
fn the_shipped_go_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&GO_MISSING_DOCS_EMPTY_RUN_PROBE);
}

/// A Dart class and method that carry no documentation comment.
/// `public_member_api_docs` reports each of the two, so each file holds two
/// findings.
const DART_MISSING_DOCS_UNREAD_SOURCE: &str = r#"class Widget {
  int gate(int value) {
    return value;
  }
}
"#;

/// Every Dart file staged in the probe repository the missing-docs script is
/// given none of.
const DART_MISSING_DOCS_UNREAD_FILES: &[(&str, &str)] = &[
    ("top.dart", DART_MISSING_DOCS_UNREAD_SOURCE),
    ("deep/nested/other.dart", DART_MISSING_DOCS_UNREAD_SOURCE),
];

/// Each finding the Dart missing-docs script reports over the two files it is
/// given, as `path:line`.
///
/// Both files hold the same bytes, and each answers two: the class head that
/// opens the file, and the method on the line under it.
const DART_MISSING_DOCS_READ_FINDINGS: &[&str] = &[
    concat!("deep/nested/other.dart:", opening_declaration_row!()),
    concat!("deep/nested/other.dart:", nested_declaration_row!()),
    concat!("top.dart:", opening_declaration_row!()),
    concat!("top.dart:", nested_declaration_row!()),
];

/// The `missing-docs-dart` probe over a run that is given no file.
const DART_MISSING_DOCS_EMPTY_RUN_PROBE: ShippedEmptyRun = ShippedEmptyRun {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: NO_FINDINGS,
    },
    staged: DART_MISSING_DOCS_UNREAD_FILES,
    with_files: DART_MISSING_DOCS_READ_FINDINGS,
    reason: READS_ONLY_ITS_ARGUMENTS,
};

/// Acceptance: the shipped Dart missing-docs tool rule reads only the files
/// it is given, through the real `dart analyze` pipeline.
///
/// This script names the package it makes as the one path `dart analyze`
/// reads, and it copies each file it is given under that package. A run with
/// no argument therefore hands the tool a package holding no Dart file.
/// Measured over this probe with no argument: the script reported 0 findings
/// and exited 0 both without the guard and with it, and the same script over
/// the two staged files reports 4. The guard is what keeps the script from
/// making that package and running the analyzer over it.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_reads_only_the_files_it_is_given() {
    verify_shipped_run_reads_only_its_arguments(&DART_MISSING_DOCS_EMPTY_RUN_PROBE);
}
