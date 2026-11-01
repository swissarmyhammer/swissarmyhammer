//! Acceptance tests for the shipped Dart missing-docs tool rule.

use super::*;

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
