//! Acceptance test for the shipped `dead-code-swift` tool rule.
//!
//! The test drives the SHIPPED script over a probe SwiftPM package and reads
//! what the real periphery reported.
//!
//! The module stands beside `dead_code`, which holds the whole family to its
//! fixture pair, because this rule builds the package's TEST targets before it
//! judges it. That one flag decides two answers at once — which declarations
//! have a caller, and which declarations are judged at all — and the fixture
//! package declares no test target, so neither answer is measured there.
//!
//! The test stands under `#[serial_test::serial(cwd)]`. The rule's
//! `doctor.check_command` ends in `test -f Package.swift`, so the check reads
//! the process working directory, and the working directory is one value every
//! thread of the test binary shares. `swift_package_root` states that contract
//! for the roster tests; this module takes the guard over its own probe package
//! instead, so the directory the check reads is the directory the script runs
//! in.

use super::*;

/// Where the SwiftPM manifest of the dead-code probe stands, as the work-list
/// holds it.
const SWIFT_DEAD_CODE_MANIFEST_PATH: &str = "Package.swift";

/// A SwiftPM package of one product target and one test target, each at the
/// path SwiftPM gives it by convention.
///
/// The manifest names no path of its own, so `swift package describe` answers
/// `Sources/Probe` and `Tests/ProbeTests` — the convention the script must read
/// off the manifest rather than assume.
const SWIFT_DEAD_CODE_MANIFEST: &str = concat!(
    "// swift-tools-version:5.9\n",
    "import PackageDescription\n",
    "\n",
    "let package = Package(\n",
    "    name: \"Probe\",\n",
    "    targets: [\n",
    "        .target(name: \"Probe\"),\n",
    "        .testTarget(name: \"ProbeTests\", dependencies: [\"Probe\"]),\n",
    "    ]\n",
    ")\n",
);

/// Where the product source of the dead-code probe stands, as the work-list
/// holds it.
const SWIFT_DEAD_CODE_PRODUCT_PATH: &str = "Sources/Probe/Product.swift";

/// The product target: one internal declaration a test calls, and one internal
/// declaration nothing calls.
///
/// Both are `internal`, so `--retain-public` reaches neither and the run judges
/// both.
const SWIFT_DEAD_CODE_PRODUCT: &str = concat!(
    "/// An internal declaration whose only caller is a test.\n",
    "func onlyATestCalls() -> Int {\n",
    "    1\n",
    "}\n",
    "\n",
    "/// An internal declaration nothing calls at all.\n",
    "func nothingCalls() -> Int {\n",
    "    2\n",
    "}\n",
);

/// Where the test source of the dead-code probe stands, as the work-list holds
/// it.
const SWIFT_DEAD_CODE_TEST_PATH: &str = "Tests/ProbeTests/ProbeTests.swift";

/// The test target: one test-only helper nothing calls, and one test method
/// that calls the product declaration.
///
/// `@testable import` is what lets a test reach an `internal` declaration, and
/// it is the reference the index records.
const SWIFT_DEAD_CODE_TEST: &str = concat!(
    "import XCTest\n",
    "@testable import Probe\n",
    "\n",
    "/// A test-only helper nothing calls.\n",
    "func unusedTestSupport() -> Int {\n",
    "    3\n",
    "}\n",
    "\n",
    "final class ProbeTests: XCTestCase {\n",
    "    func testProduct() {\n",
    "        XCTAssertEqual(onlyATestCalls(), 1)\n",
    "    }\n",
    "}\n",
);

/// Every file of the probe package.
const SWIFT_DEAD_CODE_PACKAGE: &[(&str, &str)] = &[
    (SWIFT_DEAD_CODE_MANIFEST_PATH, SWIFT_DEAD_CODE_MANIFEST),
    (SWIFT_DEAD_CODE_PRODUCT_PATH, SWIFT_DEAD_CODE_PRODUCT),
    (SWIFT_DEAD_CODE_TEST_PATH, SWIFT_DEAD_CODE_TEST),
];

/// The row `nothingCalls()` stands on inside [`SWIFT_DEAD_CODE_PRODUCT`].
///
/// The first declaration takes rows 1 to 4, a blank line takes row 5, and the
/// documentation line of the second takes row 6.
const SWIFT_NOTHING_CALLS_ROW: usize = 7;

/// Stages `staged` in a temporary repository, drives the shipped `dead-code-swift`
/// script there, and answers each finding it reported as `path:line`, sorted.
///
/// The probe repository is the process working directory for the whole run, so
/// the rule's `test -f Package.swift` check reads the package the script judges.
/// The guard is taken after the staging, because the check needs the manifest on
/// disk, and it drops before the temporary directory, because a process standing
/// in a removed directory fails every later `getcwd`.
///
/// The findings are the SCRIPT's own, before the engine keeps only the ones in
/// the changed files.
fn swift_dead_code_findings(staged: &[(&str, &str)]) -> Vec<String> {
    let loader = builtin_loader();
    let repo = tempfile::tempdir().expect("temp dir");
    stage_probe_files(repo.path(), staged.iter().copied());
    let repo_root = probe_repository_root(repo.path());
    let _cwd = CurrentDirGuard::new(&repo_root).expect("cwd guard");

    require_tool_installed(&loader, SWIFT_PROJECT_TYPES, SWIFT_DEAD_CODE_RULE);
    let shipped = required_shipped_tool_rule(&loader, SWIFT_DEAD_CODE_RULE);
    let args = script_args(shipped.scope, NO_SCRIPT_FILES);

    let reported = run_script(&shipped.script, &repo_root, &args)
        .expect("the shipped Swift dead-code script must judge the probe package and exit 0");

    sorted_names(&finding_rows(&reported, &repo_root))
}

/// Acceptance: the shipped Swift dead-code tool rule keeps the test targets in
/// the index and out of the report, through the real periphery pipeline.
///
/// `dead-code`, the prompt rule this one supersedes, exempts "test functions and
/// test-only helpers". The script builds the test targets, so it must answer
/// both halves of that carve-out at once, and the probe holds one declaration
/// for each half beside the control that proves the gate still fires:
///
/// - `unusedTestSupport()` is a test-only helper nothing calls. It must stay out
///   of the report.
/// - `onlyATestCalls()` is a product declaration a test calls. It must stay out
///   of the report, which it can only do while the test target is INDEXED.
/// - `nothingCalls()` is a product declaration nothing calls. It must report.
///
/// Measured with periphery 3.8.0 over this probe: the script as it shipped
/// before `--report-exclude` reported `unusedTestSupport()` beside
/// `nothingCalls()`, and the same script with `--exclude-tests` in place of the
/// report filter reported `onlyATestCalls()` beside `nothingCalls()`. Only the
/// shipped shape reports one finding.
///
/// The same three shapes over `Alamofire` at `0455bfb` answer 74, 25 and 22
/// findings, and the rule body states that table.
#[test]
#[serial_test::serial(cwd)]
fn the_shipped_swift_dead_code_tool_rule_reports_no_test_support_declaration() {
    let reported = swift_dead_code_findings(SWIFT_DEAD_CODE_PACKAGE);

    assert_eq!(
        reported,
        vec![format!(
            "{SWIFT_DEAD_CODE_PRODUCT_PATH}:{SWIFT_NOTHING_CALLS_ROW}"
        )],
        "the test target must count as a caller and never as a subject: the product \
         declaration a test calls stays silent, the test-only helper stays silent, and \
         the product declaration nothing calls reports"
    );
}
