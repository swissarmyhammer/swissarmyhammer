//! Acceptance tests for the shipped `dead-code-typescript` tool rule.
//!
//! Each test drives the SHIPPED script over a probe repository and reads what
//! the real ts-prune reported.
//!
//! The module stands beside `dead_code`, which holds the whole family to its
//! fixture pair, because the carve-outs measured here cannot be held by that
//! pair. Doctor counts only the findings a run reports ABOUT the fixture under
//! test, and every carve-out here takes findings off a DIFFERENT file — the
//! package's entry module — so a pair of one fail fixture and one pass fixture
//! cannot tell the carve-out working from the carve-out absent.

use super::*;

/// The tsconfig of a probe project whose program holds every file under `src`.
///
/// `include` names the source directory alone, which is what a project states
/// when its tests stand beside its sources.
const TYPESCRIPT_PROBE_TSCONFIG: &str = concat!(
    "{\n",
    "  \"compilerOptions\": {\n",
    "    \"target\": \"ES2021\",\n",
    "    \"module\": \"ESNext\",\n",
    "    \"moduleResolution\": \"bundler\",\n",
    "    \"noEmit\": true,\n",
    "    \"strict\": true,\n",
    "    \"skipLibCheck\": true\n",
    "  },\n",
    "  \"include\": [\"src\"]\n",
    "}\n",
);

/// Where the tsconfig of every probe stands, as the work-list holds it.
const TYPESCRIPT_PROBE_TSCONFIG_PATH: &str = "tsconfig.json";

/// Where the package manifest of every probe stands, as the work-list holds it.
const TYPESCRIPT_PROBE_MANIFEST_PATH: &str = "package.json";

/// Where the entry module of a probe stands, as the work-list holds it.
const TYPESCRIPT_PROBE_ENTRY_PATH: &str = "src/index.ts";

/// Where the ordinary module of a probe stands, as the work-list holds it.
const TYPESCRIPT_PROBE_LIB_PATH: &str = "src/lib.ts";

/// An entry module: one export nothing else in the project imports.
///
/// The export is the package's surface, so the callers the module graph cannot
/// see stand outside the repository.
const TYPESCRIPT_PROBE_ENTRY: &str = concat!(
    "/** The one name the package publishes. */\n",
    "export const surface = 1;\n",
);

/// An ordinary module: one export nothing imports and nothing publishes.
///
/// `trulyDead` stands on row 2, under its documentation line, so every probe
/// that stages this module expects `src/lib.ts:2`.
const TYPESCRIPT_PROBE_LIB: &str = concat!(
    "/** An export nothing in the project imports. */\n",
    "export const trulyDead = 1;\n",
);

/// A manifest that publishes its entry module under a `source` condition.
///
/// The `default` condition names build output that no source file matches, so
/// the run has to read the whole `exports` map rather than its first leaf.
const TYPESCRIPT_SOURCE_EXPORTS_MANIFEST: &str = concat!(
    "{\n",
    "  \"name\": \"entry-probe\",\n",
    "  \"version\": \"0.0.0\",\n",
    "  \"exports\": {\n",
    "    \".\": {\n",
    "      \"source\": \"./src/index.ts\",\n",
    "      \"default\": \"./dist/index.js\"\n",
    "    }\n",
    "  }\n",
    "}\n",
);

/// A probe whose manifest publishes the entry module as source.
const TYPESCRIPT_PUBLISHED_ENTRY_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["src/lib.ts:2"],
    },
    staged: &[
        (
            TYPESCRIPT_PROBE_MANIFEST_PATH,
            TYPESCRIPT_SOURCE_EXPORTS_MANIFEST,
        ),
        (TYPESCRIPT_PROBE_TSCONFIG_PATH, TYPESCRIPT_PROBE_TSCONFIG),
        (TYPESCRIPT_PROBE_ENTRY_PATH, TYPESCRIPT_PROBE_ENTRY),
        (TYPESCRIPT_PROBE_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "an export the package manifest publishes is the package surface, and the module \
             nothing publishes and nothing imports is the one finding",
};

/// A manifest that publishes build output alone, and names the package.
///
/// `main` names a bundle whose name matches no source file, so the manifest
/// resolves to nothing and the tsconfig `paths` table is the only place the
/// source entry is stated.
const TYPESCRIPT_BUILD_OUTPUT_MANIFEST: &str = concat!(
    "{\n",
    "  \"name\": \"paths-probe\",\n",
    "  \"version\": \"0.0.0\",\n",
    "  \"main\": \"./dist/paths-probe.cjs\"\n",
    "}\n",
);

/// A tsconfig that maps the package's own name onto its source entry, beside
/// an internal alias that maps every source file.
///
/// The internal alias is the control: its key is no package name of this
/// workspace, so the run must leave every file it names under the gate.
const TYPESCRIPT_SELF_PATHS_TSCONFIG: &str = concat!(
    "{\n",
    "  \"compilerOptions\": {\n",
    "    \"target\": \"ES2021\",\n",
    "    \"module\": \"ESNext\",\n",
    "    \"moduleResolution\": \"bundler\",\n",
    "    \"noEmit\": true,\n",
    "    \"strict\": true,\n",
    "    \"skipLibCheck\": true,\n",
    "    \"paths\": {\n",
    "      \"paths-probe\": [\"./src/index.ts\"],\n",
    "      \"@internal/*\": [\"./src/*\"]\n",
    "    }\n",
    "  },\n",
    "  \"include\": [\"src\"]\n",
    "}\n",
);

/// A probe whose source entry is stated by the tsconfig `paths` table alone.
const TYPESCRIPT_SELF_PATHS_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["src/lib.ts:2"],
    },
    staged: &[
        (
            TYPESCRIPT_PROBE_MANIFEST_PATH,
            TYPESCRIPT_BUILD_OUTPUT_MANIFEST,
        ),
        (
            TYPESCRIPT_PROBE_TSCONFIG_PATH,
            TYPESCRIPT_SELF_PATHS_TSCONFIG,
        ),
        (TYPESCRIPT_PROBE_ENTRY_PATH, TYPESCRIPT_PROBE_ENTRY),
        (TYPESCRIPT_PROBE_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "a `paths` key equal to the package's own name states the source entry, and a key \
             that names no package of the workspace states nothing",
};

/// A manifest carrying a ts-prune configuration of the project's own.
///
/// ts-prune reads its configuration through cosmiconfig, and `package.json` is
/// one of the places it searches. `ignore` and `skip` each turn the gate off
/// for every file under `src`.
const TYPESCRIPT_TS_PRUNE_CONFIG_MANIFEST: &str = concat!(
    "{\n",
    "  \"name\": \"config-probe\",\n",
    "  \"version\": \"0.0.0\",\n",
    "  \"ts-prune\": {\n",
    "    \"ignore\": \"src\",\n",
    "    \"skip\": \"src\"\n",
    "  }\n",
    "}\n",
);

/// A probe whose own ts-prune configuration would silence the whole gate.
const TYPESCRIPT_PROJECT_CONFIG_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["src/lib.ts:2"],
    },
    staged: &[
        (
            TYPESCRIPT_PROBE_MANIFEST_PATH,
            TYPESCRIPT_TS_PRUNE_CONFIG_MANIFEST,
        ),
        (TYPESCRIPT_PROBE_TSCONFIG_PATH, TYPESCRIPT_PROBE_TSCONFIG),
        (TYPESCRIPT_PROBE_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "the rule states its own `--ignore` and `--skip` on the command line, so a project \
             cannot turn the gate off from its own ts-prune configuration",
};

/// Where the staged module of the marker probe stands.
const TYPESCRIPT_PROBE_STAGED_PATH: &str = "src/staged.ts";

/// A module whose export carries the staging marker with its reason.
const TYPESCRIPT_PROBE_STAGED: &str = concat!(
    "/** An export the importer lands with. */\n",
    "// ts-prune-ignore-next  the importer lands in the next change\n",
    "export const stagedHelper = 1;\n",
);

/// A probe holding one marked export beside one bare export.
const TYPESCRIPT_STAGING_MARKER_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["src/lib.ts:2"],
    },
    staged: &[
        (TYPESCRIPT_PROBE_TSCONFIG_PATH, TYPESCRIPT_PROBE_TSCONFIG),
        (TYPESCRIPT_PROBE_STAGED_PATH, TYPESCRIPT_PROBE_STAGED),
        (TYPESCRIPT_PROBE_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "a staged export carries the marker and stays silent, and an export with no marker \
             is dead",
};

/// A module holding one export only a test imports, beside one export nothing
/// imports at all.
///
/// `helperForTests` stands on row 2 and `trulyDead` on row 5, so a probe that
/// stages this module expects `src/lib.ts:2` for the first and `src/lib.ts:5`
/// for the second.
const TYPESCRIPT_PROBE_TEST_SUPPORT: &str = concat!(
    "/** An export the test module imports. */\n",
    "export const helperForTests = 1;\n",
    "\n",
    "/** An export nothing in the project imports. */\n",
    "export const trulyDead = 2;\n",
);

/// Where the test module of the program probe stands.
const TYPESCRIPT_PROBE_TEST_PATH: &str = "src/lib.test.ts";

/// A test module: the one importer of the test-support export.
const TYPESCRIPT_PROBE_TEST: &str = concat!(
    "import { helperForTests } from \"./lib\";\n",
    "\n",
    "if (helperForTests !== 1) {\n",
    "  throw new Error(\"the probe helper changed\");\n",
    "}\n",
);

/// A tsconfig whose program leaves the test module out.
const TYPESCRIPT_NO_TESTS_TSCONFIG: &str = concat!(
    "{\n",
    "  \"compilerOptions\": {\n",
    "    \"target\": \"ES2021\",\n",
    "    \"module\": \"ESNext\",\n",
    "    \"moduleResolution\": \"bundler\",\n",
    "    \"noEmit\": true,\n",
    "    \"strict\": true,\n",
    "    \"skipLibCheck\": true\n",
    "  },\n",
    "  \"include\": [\"src\"],\n",
    "  \"exclude\": [\"src/**/*.test.ts\"]\n",
    "}\n",
);

/// A probe whose tsconfig holds the test module in the program.
const TYPESCRIPT_TESTS_IN_PROGRAM_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["src/lib.ts:5"],
    },
    staged: &[
        (TYPESCRIPT_PROBE_TSCONFIG_PATH, TYPESCRIPT_PROBE_TSCONFIG),
        (TYPESCRIPT_PROBE_LIB_PATH, TYPESCRIPT_PROBE_TEST_SUPPORT),
        (TYPESCRIPT_PROBE_TEST_PATH, TYPESCRIPT_PROBE_TEST),
    ],
    reason: "a test in the program is a caller, so the export only the test imports is not dead",
};

/// A probe whose tsconfig takes the test module out of the program.
const TYPESCRIPT_TESTS_OUT_OF_PROGRAM_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["src/lib.ts:2", "src/lib.ts:5"],
    },
    staged: &[
        (TYPESCRIPT_PROBE_TSCONFIG_PATH, TYPESCRIPT_NO_TESTS_TSCONFIG),
        (TYPESCRIPT_PROBE_LIB_PATH, TYPESCRIPT_PROBE_TEST_SUPPORT),
        (TYPESCRIPT_PROBE_TEST_PATH, TYPESCRIPT_PROBE_TEST),
    ],
    reason: "a project that takes its tests out of its own program takes the callers with them, \
             and the export only a test imports then reports",
};

/// Acceptance: an export the package manifest publishes is not dead.
///
/// This is the `dead-code` carve-out for the exported public API. The manifest
/// states the surface, the same way `--retain-public` states it for Swift, and
/// the run reads the `exports` map whole so a `source` condition beside build
/// output is found.
///
/// Measured over `zod` at `4e1720c`: the run reported 1946 findings before the
/// carve-out and 78 after it, and the rule body states that table.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_reports_no_published_entry_module() {
    verify_shipped_tree_reports(&TYPESCRIPT_PUBLISHED_ENTRY_PROBE);
}

/// Acceptance: a `paths` key equal to a workspace package's own name states
/// that package's source entry, and no other key states anything.
///
/// Every published library of the corpus names build output in `main` and in
/// `exports`, so the manifest resolves to no source file. The self-reference
/// mapping a repository writes so that its own tests can import the package by
/// its published name is the second place the source entry is stated.
///
/// Measured: `zustand` at `2115efb` and `redux` at `3084fc3` each carry one,
/// and each answers its entry findings through this path alone.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_reads_the_package_name_out_of_the_tsconfig_paths() {
    verify_shipped_tree_reports(&TYPESCRIPT_SELF_PATHS_PROBE);
}

/// Acceptance: the rule owns its own gate beside a project ts-prune
/// configuration.
///
/// ts-prune merges a configuration it finds through cosmiconfig under the
/// command line, so a `package.json` holding `"ts-prune": {"ignore": "src"}`
/// silenced the whole gate before the rule stated the two options itself.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_keeps_its_own_gate_beside_a_project_config() {
    verify_shipped_tree_reports(&TYPESCRIPT_PROJECT_CONFIG_PROBE);
}

/// Acceptance: the staging marker silences one export, and a bare export is
/// dead.
///
/// The fixture pair holds the same contract for five kinds of export. This
/// test holds it through the shipped script rather than through doctor, so the
/// marker survives a change to the run itself.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_answers_the_staging_marker() {
    verify_shipped_tree_reports(&TYPESCRIPT_STAGING_MARKER_PROBE);
}

/// Acceptance: the program the project's own tsconfig states is the whole
/// module graph the rule reads.
///
/// `dead-code`, the prompt rule this one supersedes, exempts "test functions
/// and test-only helpers". A test in the program is a caller, so the exemption
/// needs no marker. A project that excludes its tests from its own tsconfig
/// takes those callers out of the graph, and the export only a test imports
/// then reports.
///
/// Measured over 12 `tsconfig.json` projects of the four corpus workspaces:
/// each one holds every test file that stands beside its sources.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_reads_the_program_the_project_states() {
    verify_shipped_tree_reports(&TYPESCRIPT_TESTS_IN_PROGRAM_PROBE);
    verify_shipped_tree_reports(&TYPESCRIPT_TESTS_OUT_OF_PROGRAM_PROBE);
}
