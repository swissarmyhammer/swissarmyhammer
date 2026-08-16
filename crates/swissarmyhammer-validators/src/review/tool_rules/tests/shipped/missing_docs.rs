//! Acceptance tests for the shipped missing-docs tool rules.
//!
//! One test holds the whole roster to its fixture pair and to the prompt rule
//! it supersedes. The tests under it drive one language each through its real
//! tool, so each measures the shipped script rather than a copy.
//!
//! The Rust rule stands in `missing_docs_rust`, because the shapes `cargo
//! clippy` answers a broken run with are cargo's own.

use super::*;

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

/// The binary both runs of the shipped Dart script call.
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
/// class on row 1 and the method on row 2 of [`DART_BROKEN_RUN_SOURCE`].
const DART_BROKEN_RUN_ROWS: &[&str] = &[
    concat!(dart_broken_run_path!(), ":1"),
    concat!(dart_broken_run_path!(), ":2"),
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
/// `.dart_tool`. An SDK outside the `>=3.0.0 <5.0.0` constraint the script
/// declares reaches that state — `dart pub get` then exits 1 and writes
/// nothing.
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

/// Where the Dart file that is never written stands inside the probe
/// repository.
const DART_ABSENT_PATH: &str = "lib/absent.dart";

/// What the script writes for a file it cannot read.
const DART_CANNOT_READ_MESSAGE: &str = concat!(missing_docs_rule!(dart), " cannot read");

/// What the one error of an absent file must name.
const DART_ABSENT_ERROR: &[&str] = &[DART_CANNOT_READ_MESSAGE, DART_ABSENT_PATH];

/// The `missing-docs-dart` probe over a path that holds no file.
const DART_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: FLUTTER_PROJECT_TYPES,
        rule: DART_MISSING_DOCS_RULE,
        expected: DART_ABSENT_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Dart file that is not there",
    path: DART_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Dart missing-docs tool rule BREAKS on a file it
/// cannot read, through the real `dart analyze` pipeline.
///
/// The script copies each file it is given into the probe package, so a file
/// that is not there is a file the analyzer never sees, and the run would
/// report no member of it while reporting for every other file of the group.
/// The script therefore tests each file it is given before it builds anything,
/// and exits nonzero with the name of the file it cannot read.
#[test]
fn the_shipped_dart_missing_docs_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&DART_ABSENT_PROBE);
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

/// What revive puts at the front of the failure it writes for a file it could
/// not parse. The run's error detail must carry it, so the agent reading the
/// error learns which file broke.
const GO_INVALID_FILE_PREFIX: &str = "invalid file";

/// What the one error of an unparsable Go file must name.
const GO_UNPARSABLE_ERROR: &[&str] = &[GO_INVALID_FILE_PREFIX, GO_UNPARSABLE_PATH];

/// The `missing-docs-go` probe over a Go file revive cannot parse.
const GO_UNPARSABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: GO_PROJECT_TYPES,
        rule: GO_MISSING_DOCS_RULE,
        expected: GO_UNPARSABLE_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Go file the parser cannot read",
    path: GO_UNPARSABLE_PATH,
    source: Some(GO_UNPARSABLE_SOURCE.as_bytes()),
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Go missing-docs tool rule BREAKS on a Go file it
/// cannot parse, through the real revive pipeline.
///
/// revive exits 0 for such a file and reports the failure with an empty
/// `RuleName`, under the `validity` category rather than under `exported`. A
/// pipe that selected the `exported` findings alone therefore dropped it, and
/// the file read as clean — a run answering zero for a reason other than a
/// clean file. The script counts the failures that belong to no rule, writes
/// each one to stderr, and exits nonzero.
#[test]
fn the_shipped_go_missing_docs_tool_rule_breaks_on_a_file_it_cannot_parse() {
    verify_shipped_run_breaks(&GO_UNPARSABLE_PROBE);
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

/// Holds the shipped Python missing-docs run to judging `judged.py` and to
/// stating the one path it could not read, through the real ruff pipeline.
///
/// The judged file carries one undocumented function, so the run has a finding
/// to lose. Losing it is what a nonzero exit over a declined item costs, and
/// staying silent about the path is what reads that path as a clean file.
fn verify_python_missing_docs_declines(unreadable: &ShippedUnreadableFile) {
    let expected = python_missing_docs_judged_row();

    verify_unreadable_file_is_declined(
        PYTHON_PROJECT_TYPES,
        PYTHON_MISSING_DOCS_RULE,
        &[(PYTHON_JUDGED_PATH, PYTHON_JUDGED_SOURCE)],
        PYTHON_UNREADABLE_PATH,
        unreadable,
        &[&expected],
    );
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
    verify_python_missing_docs_declines(&ShippedUnreadableFile::Absent);
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
    verify_python_missing_docs_declines(&ShippedUnreadableFile::Undecodable(
        PYTHON_UNDECODABLE_SOURCE,
    ));
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
    verify_python_missing_docs_declines(&ShippedUnreadableFile::Forbidden(PYTHON_FORBIDDEN_SOURCE));
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
const PYTHON_READ_FINDINGS: &[&str] = &[
    "deep/nested/other.py:1",
    "deep/nested/other.py:1",
    "deep/nested/other.py:2",
    "top.py:1",
    "top.py:1",
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

/// Where the `missing-docs-swift` fail fixture stands inside the probe
/// repository, as the work-list holds it.
const SWIFT_MISSING_DOCS_FIXTURE_PATH: &str = "Sources/MissingDocsSwiftFail.swift";

/// Every declaration the `missing-docs-swift` fail fixture leaves
/// undocumented, trimmed as the fixture writes it.
///
/// A line, and not a claim, because `missing_docs` writes one message —
/// `public declarations should be documented` — for every declaration, so the
/// claim never spells which one it read.
const SWIFT_MISSING_DOCS_FAIL_LINES: &[&str] = &[
    "public struct UndocumentedStructure {",
    "public func undocumentedMethod() {}",
    "public func undocumentedFunction() {}",
];

/// The `missing-docs-swift` fail fixture, and every undocumented declaration
/// the real swiftlint pipeline must report inside it.
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
/// undocumented declaration its fail fixture holds, through the real swiftlint
/// pipeline.
///
/// A declaration is held to the SOURCE LINE its finding stands on, because
/// `missing_docs` writes one message for every declaration and never spells
/// the declaration it read.
///
/// The count is the other half, and it is what a silent run cannot fake. The
/// script writes its own configuration and names the project's own
/// `.swiftlint.yml` beside it, and a run that reached neither would report
/// zero findings. Holding this run to exactly these three lines states that
/// swiftlint read the configuration the script wrote and reported each kind
/// the fixture holds.
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

/// One undocumented public structure, and one undocumented stored property
/// inside it.
///
/// Every staged position holds these same bytes, so the POSITION is the only
/// thing that can tell one file of the run from another.
const SWIFT_STAGED_DECLARATIONS: &str = concat!(
    "public struct StagedThing {\n",
    "    public var value: Int = 0\n",
    "}\n",
);

/// The file of each finding the Swift run must report: the ordinary file, once
/// for its structure and once for its stored property.
const SWIFT_STAGED_REPORTED: &[&str] =
    &[SWIFT_ORDINARY_POSITION.path, SWIFT_ORDINARY_POSITION.path];

/// The staged Swift positions, and the one of them the real swiftlint pipeline
/// must report.
const SWIFT_MISSING_DOCS_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure, staged in two positions",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_EXCLUDE_POSITIONS,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the ordinary file reports its structure and its stored property, and the \
             file under the project's excluded directory reports nothing",
};

/// Acceptance: the shipped Swift missing-docs tool rule honours the project's
/// own `excluded:` list, through the real swiftlint pipeline.
///
/// This is the half the fixture pair cannot reach. The doctor materializes one
/// fixture as a loose file with no directory, so no fixture can carry a
/// position and no fixture can stand beside a project configuration.
///
/// The two files hold the same bytes on purpose. The project's `excluded:`
/// list is the only difference between the file that reports and the file that
/// stays silent. swiftlint applies that list to a file named as a command-line
/// argument only under `--force-exclude`, so this test fails the moment the
/// script drops the flag or stops naming the project configuration.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_reads_the_project_exclude_list() {
    verify_shipped_staged_positions_report(&SWIFT_MISSING_DOCS_POSITIONS_PROBE);
}

/// The `missing-docs-swift` probe over a run whose every file the project's
/// `excluded:` list names.
const SWIFT_EVERY_FILE_EXCLUDED_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure under the project's excluded directory",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project excludes every file of the run, so the run reports nothing and \
             breaks nothing",
};

/// Acceptance: the shipped Swift missing-docs tool rule reports nothing, and
/// breaks nothing, when the project excludes every file of the run, through
/// the real swiftlint pipeline.
///
/// swiftlint exits 1 with `Error: No lintable files found at paths` when
/// `--force-exclude` leaves it no file to read. That status reads as a broken
/// tool, so a run over a change that touched generated code alone would report
/// a tool error rather than a clean answer. The script tests each file it is
/// given for readability first, so the message can mean one thing only, and it
/// then exits 0 with no finding.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_answers_zero_when_the_project_excludes_every_file() {
    verify_shipped_staged_positions_report(&SWIFT_EVERY_FILE_EXCLUDED_PROBE);
}

/// The file of each finding the `child_config:` probe must report: the file
/// under the project's excluded directory, once for its structure and once for
/// its stored property.
///
/// The project excludes that directory, and the run drops that exclude list,
/// so the file reports.
const SWIFT_CHILD_CONFIG_REPORTED: &[&str] =
    &[SWIFT_GENERATED_POSITION.path, SWIFT_GENERATED_POSITION.path];

/// The `missing-docs-swift` probe beside a project that names a child
/// configuration of its own.
const SWIFT_CHILD_CONFIG_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_CHILD_CONFIG_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure beside a project child configuration",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_EXCLUDED_POSITION_ONLY,
    support: SWIFT_CHILD_CONFIG_SUPPORT_FILES,
    reason: "swiftlint cannot read that project configuration beside the rule's own, so the run \
             measures with the rule's configuration alone and reports the staged declarations",
};

/// Acceptance: the shipped Swift missing-docs tool rule measures beside a
/// project that names a child configuration of its own, through the real
/// swiftlint pipeline.
///
/// swiftlint reads a list of `--config` paths as one parent-child hierarchy. A
/// parent that names a child of its own makes that hierarchy ambiguous, and
/// swiftlint aborts with exit 134. The script read that as a broken tool, so a
/// project switched the gate off with a configuration swiftlint reads on its
/// own.
///
/// The script now runs a second time with its own configuration alone. The
/// project's `excluded:` list is dropped for that run, so the staged file under
/// the excluded directory reports.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_measures_beside_a_project_child_config() {
    verify_shipped_staged_positions_report(&SWIFT_CHILD_CONFIG_PROBE);
}

/// The `missing-docs-swift` probe beside a project that states a warning
/// threshold of one finding.
const SWIFT_WARNING_THRESHOLD_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_STAGED_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented public structure beside a project warning threshold",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_ORDINARY_POSITION_ONLY,
    support: SWIFT_WARNING_THRESHOLD_SUPPORT_FILES,
    reason: "the threshold makes swiftlint exit 2 with the whole report on stdout, and the \
             script reads that status as a measured run, so the staged declarations report",
};

/// Acceptance: the shipped Swift missing-docs tool rule measures beside a
/// project that states `warning_threshold:`, through the real swiftlint
/// pipeline.
///
/// Measured with swiftlint 0.65.0 over the staged declarations, with
/// `warning_threshold: 1` in the project configuration: swiftlint writes 3
/// entries to stdout — the 2 `missing_docs` findings and one
/// `warning_threshold` entry of error severity — and exits 2. The script read
/// each nonzero status as a broken tool and reported 0 findings, so one line
/// in the project file switched the gate off.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_measures_beside_a_project_warning_threshold() {
    verify_shipped_staged_positions_report(&SWIFT_WARNING_THRESHOLD_PROBE);
}

/// The `missing-docs-swift` probe beside a project that names a swiftlint
/// version that is not installed.
const SWIFT_VERSION_MISMATCH_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_VERSION_MISMATCH_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "undocumented public declarations beside a project version mismatch",
    path: SWIFT_ORDINARY_POSITION.path,
    source: Some(SWIFT_STAGED_DECLARATIONS.as_bytes()),
    support: SWIFT_VERSION_MISMATCH_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift missing-docs tool rule BREAKS beside a
/// project that names a swiftlint version that is not installed, through the
/// real swiftlint pipeline.
///
/// swiftlint compares `swiftlint_version:` with the version it is. At a
/// difference it writes one warning line to stderr, writes 0 bytes to stdout,
/// runs no lint, and exits 2. Measured with swiftlint 0.65.0 over the staged
/// declarations: a run with no project configuration reports 2 findings, and a
/// run beside `swiftlint_version: 99.0.0` reports 0. A script that reads every
/// status 2 as a measured run hands `jq` an empty report, reports 0 findings
/// and exits 0, so the engine reads a dirty file as clean. The script accepts
/// status 2 only when the report holds a JSON array of one entry or more.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_breaks_beside_a_project_version_mismatch() {
    verify_shipped_run_breaks(&SWIFT_VERSION_MISMATCH_PROBE);
}

/// One undocumented type that declares an inherited type, and one that
/// declares none, each holding one undocumented stored property.
///
/// `excludes_inherited_types: true` is what keeps `Wide` and its property
/// silent, and `warning: [open, public]` is what makes `Plain` and its
/// property report. The two shapes stand together so one run measures both.
const SWIFT_OPTION_DECLARATIONS: &str = concat!(
    "public struct Wide: Equatable {\n",
    "    public var name: String = \"\"\n",
    "}\n",
    "\n",
    "public struct Plain {\n",
    "    public var value: Int = 0\n",
    "}\n",
);

/// A project `.swiftlint.yml` that switches the rule off and states other
/// options for it.
///
/// Each of the three settings changes the answer on its own: `disabled_rules`
/// switches `missing_docs` off, `warning: [open]` drops every `public`
/// declaration, and `excludes_inherited_types: false` adds the two rows of
/// `Wide`.
const SWIFT_OVERRIDING_PROJECT_CONFIG: &str = concat!(
    "disabled_rules:\n",
    "  - missing_docs\n",
    "missing_docs:\n",
    "  warning: [open]\n",
    "  excludes_inherited_types: false\n",
);

/// The overriding project configuration staged beside the two shapes, which
/// the work-list does NOT name.
const SWIFT_OVERRIDING_SUPPORT_FILES: &[(&str, &str)] =
    &[(SWIFT_PROJECT_CONFIG_PATH, SWIFT_OVERRIDING_PROJECT_CONFIG)];

/// The file of each finding the run must report: the staged file, once for
/// `Plain` and once for its stored property.
const SWIFT_OPTION_REPORTED: &[&str] =
    &[SWIFT_ORDINARY_POSITION.path, SWIFT_ORDINARY_POSITION.path];

/// The `missing-docs-swift` probe over a project that states other options for
/// the rule.
const SWIFT_RULE_OPTIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_OPTION_REPORTED,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "one undocumented type with an inherited type and one without",
    declarations: SWIFT_OPTION_DECLARATIONS,
    staged: SWIFT_ORDINARY_POSITION_ONLY,
    support: SWIFT_OVERRIDING_SUPPORT_FILES,
    reason: "the rule's own options decide: the type with no inherited type reports its \
             declaration and its property, and the type with one reports nothing",
};

/// Acceptance: the shipped Swift missing-docs tool rule keeps its own rule
/// options against a project that states other ones, through the real
/// swiftlint pipeline.
///
/// The script names the project's `.swiftlint.yml` as the PARENT of its own
/// configuration, so the project decides which files are read. It must not
/// decide what the rule measures. The script's own configuration states every
/// `missing_docs` option, and a child block replaces the parent's block whole.
///
/// Each setting in the staged project configuration moves the count on its
/// own, so this run tells the three apart: 0 findings if the project switched
/// the rule off or dropped `public`, and 4 if it widened the rule to a type
/// that declares an inherited type.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_keeps_its_own_rule_options() {
    verify_shipped_staged_positions_report(&SWIFT_RULE_OPTIONS_PROBE);
}

/// Where the Swift file that is never written stands inside the probe
/// repository.
const SWIFT_ABSENT_PATH: &str = "Sources/Absent.swift";

/// What the script writes for a file it cannot read.
const SWIFT_CANNOT_READ_MESSAGE: &str = concat!(missing_docs_rule!(swift), " cannot read");

/// What the one error of an absent file must name.
const SWIFT_ABSENT_ERROR: &[&str] = &[SWIFT_CANNOT_READ_MESSAGE, SWIFT_ABSENT_PATH];

/// The `missing-docs-swift` probe over a path that holds no file.
const SWIFT_ABSENT_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_ABSENT_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Swift file that is not there",
    path: SWIFT_ABSENT_PATH,
    source: None,
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift missing-docs tool rule BREAKS on a file it
/// cannot read, through the real swiftlint pipeline.
///
/// swiftlint exits 1 for a path that is not there and writes nothing to
/// stdout. A pipeline takes the exit status of its LAST command, and that
/// command was `jq`, so the earlier pipe exited 0 and reported nothing — a run
/// answering zero for a reason other than a clean file. The script tests each
/// file it is given before it starts, and exits 1 with the name of the file it
/// cannot read.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_breaks_on_a_file_it_cannot_read() {
    verify_shipped_run_breaks(&SWIFT_ABSENT_PROBE);
}

/// Where the Swift file swiftlint cannot decode stands inside the probe
/// repository.
const SWIFT_UNDECODABLE_PATH: &str = "Sources/Latin1.swift";

/// A Swift file written in Latin-1 rather than in UTF-8.
///
/// The byte `0xE9` is `é` in Latin-1, and it is not a UTF-8 sequence.
/// swiftlint reads a file as UTF-8 and nothing else, so it cannot decode this
/// one. The staged declarations stand under the string, so a run that DID read
/// the file reports them.
const SWIFT_UNDECODABLE_SOURCE: &[u8] = b"let name = \"caf\xe9\"\n\
public struct StagedThing {\n\
public var value: Int = 0\n\
}\n";

/// What the one error of a file swiftlint cannot decode must name: the rule's
/// own line, and swiftlint's own message, which carries the path.
const SWIFT_UNDECODABLE_ERROR: &[&str] = &[
    concat!(
        missing_docs_rule!(swift),
        ": swiftlint could not read the contents of a file this run names"
    ),
    "Could not read contents of",
    "Latin1.swift",
];

/// The `missing-docs-swift` probe over a Swift file swiftlint cannot decode.
const SWIFT_UNDECODABLE_PROBE: ShippedNamedPath = ShippedNamedPath {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: SWIFT_UNDECODABLE_ERROR,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a Swift file that is not UTF-8",
    path: SWIFT_UNDECODABLE_PATH,
    source: Some(SWIFT_UNDECODABLE_SOURCE),
    support: NO_SUPPORT_FILES,
};

/// Acceptance: the shipped Swift missing-docs tool rule BREAKS on a Swift file
/// swiftlint cannot decode, through the real swiftlint pipeline.
///
/// The file is readable, so the `[ ! -r "$file" ]` guard admits it and
/// swiftlint reads it. Measured with swiftlint 0.65.0 over this file:
/// swiftlint writes ``Could not read contents of `<path>` `` to stderr, writes
/// an empty JSON array to stdout, and exits 0 — the status and the report of a
/// clean file. So the script read a file swiftlint never read as a clean file,
/// and the undocumented public declarations reached the engine as a clean tree.
///
/// Measured over the same file beside one file that holds a finding: swiftlint
/// writes the same stderr line, writes 2 entries, and exits 0 as
/// well. The child states `warning: [open, public]` and no `error:` list, so no
/// finding of this rule reaches error severity and swiftlint never exits 2.
/// Every row of the measurement is therefore the status and the report of a
/// healthy run, and only stderr tells the two apart.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_breaks_on_a_file_it_cannot_decode() {
    verify_shipped_run_breaks(&SWIFT_UNDECODABLE_PROBE);
}

/// The `missing-docs-swift` probe over a file whose name holds the words of
/// swiftlint's decode message.
const SWIFT_DECODE_NAME_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a file whose name holds the words of swiftlint's decode message",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_DECODE_NAME_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project excludes the file, so the run reports nothing and breaks nothing, \
             whatever the file is named",
};

/// Acceptance: the shipped Swift missing-docs tool rule MEASURES a run over a
/// file whose name holds the words of swiftlint's decode message, through the
/// real swiftlint pipeline.
///
/// The script tests stderr for the message swiftlint writes when it cannot
/// decode a file. swiftlint writes the PATH of a file into stderr as well, so a
/// test that read all of stderr answered the file NAME.
///
/// Measured with swiftlint 0.65.0 over this probe: swiftlint writes
/// `Error: No lintable files found at paths: 'Generated/Could not read contents
/// of.swift'` to stderr, writes 0 bytes to stdout, and exits 1. A test spelled
/// `grep -qF 'Could not read contents of'` matched that path echo, and the
/// script then wrote its own tool-error line and exited 1 over a run that
/// measured correctly. The same run over `Generated/Plain.swift`, with the same
/// exclude list, reports no finding and exits 0.
///
/// swiftlint writes its own decode message at the START of a line, and it
/// writes the path echo after `Error: `. Measured, a pattern anchored on the
/// start of the line matches the decode message and does not match the path
/// echo, so the script anchors the test that way.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_measures_a_file_named_for_the_decode_message() {
    verify_shipped_staged_positions_report(&SWIFT_DECODE_NAME_PROBE);
}

/// The `missing-docs-swift` probe over a file whose name holds the words of
/// swiftlint's configuration message.
const SWIFT_CONFIG_NAME_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: SWIFT_PROJECT_TYPES,
        rule: SWIFT_MISSING_DOCS_RULE,
        expected: NO_STAGED_REPORTS,
    },
    prompt_rule: MISSING_DOCS_PROMPT_RULE,
    change_purpose: "a file whose name holds the words of swiftlint's configuration message",
    declarations: SWIFT_STAGED_DECLARATIONS,
    staged: SWIFT_CONFIG_NAME_POSITION_ONLY,
    support: SWIFT_EXCLUDING_SUPPORT_FILES,
    reason: "the project configuration is readable, so the run keeps the project exclude list \
             and reports nothing, whatever the file is named",
};

/// Acceptance: the shipped Swift missing-docs tool rule MEASURES a run over a
/// file whose name holds the words of swiftlint's configuration message,
/// through the real swiftlint pipeline.
///
/// The same cause reaches the configuration test, and there it makes a WRONG
/// FINDING rather than a break. Measured with swiftlint 0.65.0 over this probe:
/// a test spelled `grep -qF 'Could not read configuration'` matched the path
/// echo, so the script wrote `swiftlint cannot read .swiftlint.yml beside this
/// rule`, ran swiftlint a second time with no project configuration, and
/// reported 2 findings on a file the project excludes.
///
/// The project configuration of this probe is the one every Swift probe of this
/// module stages, and swiftlint reads it without trouble, so the run must keep
/// the project's `excluded:` list and report nothing.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_measures_a_file_named_for_the_configuration_message() {
    verify_shipped_staged_positions_report(&SWIFT_CONFIG_NAME_PROBE);
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
/// directory that holds no Swift file, through the real swiftlint pipeline.
///
/// The `[ ! -r "$file" ]` guard tests each path for reading, and a directory
/// is readable, so the guard admits it and swiftlint reads it. Measured with
/// swiftlint 0.65.0 over such a directory: swiftlint writes 0 bytes to stdout,
/// writes `Error: No lintable files found at paths: ...` to stderr, and
/// exits 1. The script reads that stderr, reports no finding, and exits 0. A
/// guard that tested for a FILE would stop the directory instead, and the run
/// would answer one tool error over a path swiftlint reads without trouble.
#[test]
fn the_shipped_swift_missing_docs_tool_rule_stays_clean_over_a_hollow_directory() {
    verify_shipped_hollow_directory_answers_clean(&SWIFT_HOLLOW_PROBE);
}

/// An undocumented Swift file at the root of the probe repository.
const SWIFT_UNREAD_TOP_SOURCE: &str = "public struct Top {\n    public var value: Int = 0\n}\n";

/// The same, nested three directories deep. swiftlint walks a whole tree, so a
/// default target reaches this file as readily as the one at the root.
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
const SWIFT_READ_FINDINGS: &[&str] = &[
    "deep/nested/Other.swift:1",
    "deep/nested/Other.swift:2",
    "Top.swift:1",
    "Top.swift:2",
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
/// it is given, through the real swiftlint pipeline.
///
/// `swiftlint lint` with no path argument falls back to a default target of
/// the working directory, and it walks that whole tree. A script that hands
/// `"$@"` straight to swiftlint therefore answers for every Swift file under
/// the repository root when the run carries no file, and it exits 0, so the
/// answer reads as a measured result rather than a mistake.
///
/// The script therefore answers an empty argument list at once, with no
/// finding and an exit status of 0. The same script over the two staged files
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
const TYPESCRIPT_MISSING_DOCS_READ_FINDINGS: &[&str] = &["deep/nested/other.ts:1", "top.ts:1"];

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
const GO_MISSING_DOCS_READ_FINDINGS: &[&str] = &["top.go:3", "deep/nested/other.go:3"];

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
const DART_MISSING_DOCS_READ_FINDINGS: &[&str] = &[
    "deep/nested/other.dart:1",
    "deep/nested/other.dart:2",
    "top.dart:1",
    "top.dart:2",
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
