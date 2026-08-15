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

/// The opening of every probe tsconfig: the object, the `compilerOptions` it
/// holds, and the options every probe states.
///
/// The options say what the program IS — no emit, a bundler resolution, and
/// library checks skipped so a probe needs no `node_modules`. What each probe
/// varies is which files the program HOLDS, and that stands in the `include`,
/// `files` or `exclude` line after this opening.
///
/// The opening stands in a macro beside the constants that hold it because
/// `concat!` takes literals. It closes on the last option rather than on a
/// newline, so a probe that states an option of its own writes a comma after
/// it.
macro_rules! typescript_probe_tsconfig_head {
    () => {
        concat!(
            "{\n",
            "  \"compilerOptions\": {\n",
            "    \"target\": \"ES2021\",\n",
            "    \"module\": \"ESNext\",\n",
            "    \"moduleResolution\": \"bundler\",\n",
            "    \"noEmit\": true,\n",
            "    \"strict\": true,\n",
            "    \"skipLibCheck\": true",
        )
    };
}

/// The tsconfig of a probe project whose program holds every file under `src`.
///
/// `include` names the source directory alone, which is what a project states
/// when its tests stand beside its sources.
const TYPESCRIPT_PROBE_TSCONFIG: &str = concat!(
    typescript_probe_tsconfig_head!(),
    "\n",
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
    typescript_probe_tsconfig_head!(),
    ",\n",
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

/// Where the module nothing imports stands, as the work-list holds it.
const TYPESCRIPT_PROBE_ORPHAN_PATH: &str = "src/orphan.ts";

/// A module NO other module of the project imports, holding three kinds of
/// export.
///
/// Each export stands on a row of its own — the constant on row 2, the
/// function on row 5 and the type on row 10 — so the run has to name each
/// SYMBOL rather than the file that holds them.
const TYPESCRIPT_PROBE_ORPHAN: &str = concat!(
    "/** A constant nothing in the project imports. */\n",
    "export const ORPHAN_LIMIT = 1;\n",
    "\n",
    "/** A function nothing in the project imports. */\n",
    "export function orphanHelper(): number {\n",
    "  return 2;\n",
    "}\n",
    "\n",
    "/** A type nothing in the project imports. */\n",
    "export type OrphanOptions = { size: number };\n",
);

/// A probe whose entry module stands beside a module nothing imports at all.
const TYPESCRIPT_ORPHAN_MODULE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["src/orphan.ts:2", "src/orphan.ts:5", "src/orphan.ts:10"],
    },
    staged: &[
        (
            TYPESCRIPT_PROBE_MANIFEST_PATH,
            TYPESCRIPT_SOURCE_EXPORTS_MANIFEST,
        ),
        (TYPESCRIPT_PROBE_TSCONFIG_PATH, TYPESCRIPT_PROBE_TSCONFIG),
        (TYPESCRIPT_PROBE_ENTRY_PATH, TYPESCRIPT_PROBE_ENTRY),
        (TYPESCRIPT_PROBE_ORPHAN_PATH, TYPESCRIPT_PROBE_ORPHAN),
    ],
    reason: "a module no other module imports is dead export by export, so the run names each \
             of its three exports at the row that export stands on",
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
    typescript_probe_tsconfig_head!(),
    "\n",
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

/// Where the tsconfig of the consumer project stands, as the work-list holds
/// it.
const TYPESCRIPT_CONSUMER_TSCONFIG_PATH: &str = "packages/consumer/tsconfig.json";

/// Where the module INSIDE the consumer project stands, as the work-list holds
/// it.
const TYPESCRIPT_CONSUMER_LIB_PATH: &str = "packages/consumer/src/lib.ts";

/// Where the module OUTSIDE the consumer project stands, as the work-list
/// holds it.
const TYPESCRIPT_OUTSIDE_LIB_PATH: &str = "packages/shared/src/lib.ts";

/// A tsconfig whose program reaches a module OUTSIDE the project directory.
///
/// `include` names the project's own sources beside the source directory of a
/// package standing next to it. A monorepo writes that shape whenever one
/// package's program reads another package's source, and `zod` writes it:
/// `packages/bench` builds a program holding `packages/zod/src`.
const TYPESCRIPT_OUTSIDE_MODULE_TSCONFIG: &str = concat!(
    typescript_probe_tsconfig_head!(),
    "\n",
    "  },\n",
    "  \"include\": [\"src\", \"../shared/src\"]\n",
    "}\n",
);

/// A probe whose one project reaches a module inside itself and a module
/// outside itself.
const TYPESCRIPT_OUTSIDE_MODULE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &[
            "packages/consumer/src/lib.ts:2",
            "packages/shared/src/lib.ts:2",
        ],
    },
    staged: &[
        (
            TYPESCRIPT_CONSUMER_TSCONFIG_PATH,
            TYPESCRIPT_OUTSIDE_MODULE_TSCONFIG,
        ),
        (TYPESCRIPT_CONSUMER_LIB_PATH, TYPESCRIPT_PROBE_LIB),
        (TYPESCRIPT_OUTSIDE_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "the presenter's cut leaves a file inside the project under the project directory \
             and a file the cut never touched at its whole absolute path less the leading \
             separator, and the run reads both back off the file list of the program",
};

/// The head the staged module of the outside-module probe carries: none. The
/// POSITION is the whole of what this probe measures.
const TYPESCRIPT_NO_HEAD: &[&str] = &[];

/// The one position the work-list of the outside-module probe names: the module
/// the program reaches from outside the project.
const TYPESCRIPT_OUTSIDE_MODULE_POSITIONS: &[ShippedStagedFile] = &[ShippedStagedFile {
    path: TYPESCRIPT_OUTSIDE_LIB_PATH,
    head: TYPESCRIPT_NO_HEAD,
}];

/// The outside module, read through the whole engine rather than through the
/// script alone.
///
/// The engine keeps a workspace-scope finding only when its path meets a file
/// of the run, so a path that names no file is dropped without a word. This
/// probe is what states that the finding reaches the author.
///
/// One position is named — the module OUTSIDE the project, which is the whole
/// of what this probe measures. The module inside the project stands beside it
/// as support, so it shapes the run the tool makes and reaches no work-list
/// position of its own.
const TYPESCRIPT_OUTSIDE_MODULE_POSITIONS_PROBE: ShippedStagedPositions = ShippedStagedPositions {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &[TYPESCRIPT_OUTSIDE_LIB_PATH],
    },
    prompt_rule: DEAD_CODE_PROMPT_RULE,
    change_purpose: "one export nothing imports, in a module the project reaches from outside",
    declarations: TYPESCRIPT_PROBE_LIB,
    staged: TYPESCRIPT_OUTSIDE_MODULE_POSITIONS,
    support: &[
        (
            TYPESCRIPT_CONSUMER_TSCONFIG_PATH,
            TYPESCRIPT_OUTSIDE_MODULE_TSCONFIG,
        ),
        (TYPESCRIPT_CONSUMER_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "a finding whose path names no file is dropped by the engine, so the outside \
             module reports only when its path names the file it stands at",
};

/// Where the module of the sibling package whose directory name EXTENDS the
/// project's stands, as the work-list holds it.
///
/// `packages/consumer-bench` begins with `packages/consumer`, and the cut the
/// presenter makes needs no separator after the match, so ts-prune spells this
/// module `-bench/src/lib.ts`.
const TYPESCRIPT_SIBLING_BENCH_LIB_PATH: &str = "packages/consumer-bench/src/lib.ts";

/// Where the module of the sibling package whose directory name extends the
/// project's with the FIRST SEGMENT of a path inside it stands, as the
/// work-list holds it.
///
/// `packages/consumersrc` is `packages/consumer` followed by `src`, so the
/// presenter spells this module `src/lib.ts` — the spelling of a REAL file of
/// the project, standing at a row that file does not hold the same export on.
const TYPESCRIPT_SIBLING_CUT_LIB_PATH: &str = "packages/consumersrc/lib.ts";

/// Where the module that imports the project's live export stands, as the
/// work-list holds it.
const TYPESCRIPT_CONSUMER_OTHER_PATH: &str = "packages/consumer/src/other.ts";

/// A module whose one export another module of the same project imports.
///
/// The export is alive, so the run reports nothing at row 2 of this file. That
/// row is where a misread of the sibling's spelling lands.
const TYPESCRIPT_PROBE_IMPORTED_LIB: &str = concat!(
    "/** An export the project's own module imports. */\n",
    "export const usedHere = 1;\n",
);

/// A module that imports the live export and exports one name nothing imports.
///
/// `trulyDead` stands on row 4, under the import, the blank line and its
/// documentation line, so a probe that stages this module expects
/// `src/other.ts:4`.
const TYPESCRIPT_PROBE_IMPORTING_LIB: &str = concat!(
    "import { usedHere } from \"./lib\";\n",
    "\n",
    "/** An export nothing in the project imports. */\n",
    "export const trulyDead = usedHere;\n",
);

/// A tsconfig whose program reaches two sibling packages whose directory names
/// BEGIN with this project's own directory name.
///
/// One extends it with `-bench`, which stands nowhere inside the project. The
/// other extends it with `src`, which is the first segment of every source
/// path the project holds.
const TYPESCRIPT_SIBLING_PREFIX_TSCONFIG: &str = concat!(
    typescript_probe_tsconfig_head!(),
    "\n",
    "  },\n",
    "  \"include\": [\"src\", \"../consumer-bench/src\", \"../consumersrc\"]\n",
    "}\n",
);

/// The spelling two files of the sibling-prefix probe both carry, which is the
/// finding that run declines.
const TYPESCRIPT_DECLINED_SPELLING: &str = "src/lib.ts:2";

/// A probe whose project stands beside two packages whose names begin with its
/// own.
///
/// Measured with ts-prune 0.10.3 over this tree, the project's own directory
/// the working directory: `src/lib.ts:2`, `src/other.ts:4` and
/// `-bench/src/lib.ts:2`. The first row is the `packages/consumersrc` module
/// wearing the spelling of `packages/consumer/src/lib.ts`, whose row 2 holds
/// the LIVE export instead. Two files of the program carry that spelling, so
/// the run reports no finding for it and declines the item out loud.
const TYPESCRIPT_SIBLING_PREFIX_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &[
            "packages/consumer/src/other.ts:4",
            "packages/consumer-bench/src/lib.ts:2",
        ],
    },
    staged: &[
        (
            TYPESCRIPT_CONSUMER_TSCONFIG_PATH,
            TYPESCRIPT_SIBLING_PREFIX_TSCONFIG,
        ),
        (TYPESCRIPT_CONSUMER_LIB_PATH, TYPESCRIPT_PROBE_IMPORTED_LIB),
        (
            TYPESCRIPT_CONSUMER_OTHER_PATH,
            TYPESCRIPT_PROBE_IMPORTING_LIB,
        ),
        (TYPESCRIPT_SIBLING_BENCH_LIB_PATH, TYPESCRIPT_PROBE_LIB),
        (TYPESCRIPT_SIBLING_CUT_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "the run spells each file of the program the way ts-prune spells it, and reports the \
             finding at the one file that carries the spelling ts-prune wrote; it never names a \
             file that is not the file of the finding",
};

/// Where the link that carries a module into the program stands, as the
/// work-list holds it.
const TYPESCRIPT_LINK_PATH: &str = "src/link.ts";

/// Where the module the link stands for is held, relative to the directory it
/// is staged in.
///
/// The probe's tsconfig states its program with `files` and never names this
/// path, so the file reaches the program through the link alone. One probe
/// stages it INSIDE the repository, where the work-list holds it, and one
/// stages it BESIDE the repository, outside the workspace altogether.
const TYPESCRIPT_LINK_TARGET_PATH: &str = "outside/util.ts";

/// The link the two-readings probe stages, stated the way the repository holds
/// it — relative to the directory the link stands in.
const TYPESCRIPT_LINKED_FILE_LINKS: &[(&str, &str)] =
    &[(TYPESCRIPT_LINK_PATH, "../outside/util.ts")];

/// A tsconfig that names each root of its program one by one, one of them a
/// symbolic link.
///
/// `files` is what puts a link in the program ts-prune reads. ts-morph adds
/// each `files` entry by path, and the analyzer then reports
/// `fs.realpathSync` of it. Its `include` walk drops a symbolic link instead:
/// measured over the same tree stated with `include`, ts-morph's program held
/// `src/index.ts` alone, while `tsc` listed the link either way. So `files` is
/// the shape that lets the two readings be compared at all.
const TYPESCRIPT_LINKED_FILE_TSCONFIG: &str = concat!(
    typescript_probe_tsconfig_head!(),
    "\n",
    "  },\n",
    "  \"files\": [\"src/index.ts\", \"src/link.ts\"]\n",
    "}\n",
);

/// A probe whose program holds one file the two readings spell differently.
///
/// Measured with ts-prune 0.10.3 and tsc 5.9.3 over this tree, the project's
/// own directory the working directory: `tsc --listFilesOnly` prints
/// `src/index.ts` and `src/link.ts`, and ts-prune reports
/// `outside/util.ts:2 - trulyDead`. So the file the finding is about stands in
/// the list under a spelling the list never prints, and the run has to spell
/// each listed file BOTH ways to find it.
const TYPESCRIPT_LINKED_FILE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["outside/util.ts:2"],
    },
    staged: &[
        (
            TYPESCRIPT_PROBE_TSCONFIG_PATH,
            TYPESCRIPT_LINKED_FILE_TSCONFIG,
        ),
        (TYPESCRIPT_PROBE_ENTRY_PATH, TYPESCRIPT_PROBE_ENTRY),
        (TYPESCRIPT_LINK_TARGET_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "the run spells each file of the program the way `tsc` listed it AND the way its \
             real path spells it, so the file ts-prune reported stands among the candidates its \
             own spelling is matched against",
};

/// The one file the outside-workspace probe stages BESIDE its repository.
const TYPESCRIPT_OUTSIDE_WORKSPACE_STAGED: &[(&str, &str)] =
    &[(TYPESCRIPT_LINK_TARGET_PATH, TYPESCRIPT_PROBE_LIB)];

/// The link the outside-workspace probe stages, stated the way the repository
/// holds it — relative to the directory the link stands in.
///
/// Two segments up out of `src/` is one segment above the workspace root, so
/// the file the link stands for is a file the workspace root is no prefix of.
const TYPESCRIPT_OUTSIDE_WORKSPACE_LINKS: &[(&str, &str)] =
    &[(TYPESCRIPT_LINK_PATH, "../../outside/util.ts")];

/// A probe whose program holds one file standing outside the workspace.
///
/// The run reports each finding at the REAL path of the file it is about, and
/// the engine keeps a workspace-scope finding only when that path meets a file
/// of the run — repo-relative every one. A real path the workspace root is no
/// prefix of therefore meets nothing, and a row written there leaves the report
/// without a word. So the run declines the item instead, and says so.
const TYPESCRIPT_OUTSIDE_WORKSPACE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &[],
    },
    staged: &[
        (
            TYPESCRIPT_PROBE_TSCONFIG_PATH,
            TYPESCRIPT_LINKED_FILE_TSCONFIG,
        ),
        (TYPESCRIPT_PROBE_ENTRY_PATH, TYPESCRIPT_PROBE_ENTRY),
    ],
    reason: "a finding whose file stands outside the workspace is declined out loud, because a \
             row written at such a path is one the engine drops without a word",
};

/// Where the manifest of the package the probe's project publishes stands, as
/// the work-list holds it.
const TYPESCRIPT_APP_MANIFEST_PATH: &str = "packages/app/package.json";

/// Where the tsconfig of the probe's one project stands, as the work-list holds
/// it.
const TYPESCRIPT_APP_TSCONFIG_PATH: &str = "packages/app/tsconfig.json";

/// Where the entry module of the probe's own package stands, as the work-list
/// holds it.
const TYPESCRIPT_APP_ENTRY_PATH: &str = "packages/app/src/index.ts";

/// Where the ordinary module of the probe's own package stands, as the
/// work-list holds it.
const TYPESCRIPT_APP_LIB_PATH: &str = "packages/app/src/lib.ts";

/// Where the manifest that does not parse stands, as the work-list holds it.
///
/// It stands beside the project rather than above it, because ts-prune reads a
/// configuration out of every `package.json` on the way UP from its working
/// directory and dies on one it cannot parse.
const TYPESCRIPT_BROKEN_MANIFEST_PATH: &str = "packages/other/package.json";

/// Where the entry module of the package whose manifest does not parse stands,
/// as the work-list holds it.
const TYPESCRIPT_BROKEN_PACKAGE_ENTRY_PATH: &str = "packages/other/src/index.ts";

/// A manifest that stops halfway through its second field.
const TYPESCRIPT_BROKEN_MANIFEST: &str = concat!("{\n", "  \"name\": \"other-probe\",\n");

/// A manifest that publishes its entry module as source, and names the package.
const TYPESCRIPT_APP_MANIFEST: &str = concat!(
    "{\n",
    "  \"name\": \"app-probe\",\n",
    "  \"version\": \"0.0.0\",\n",
    "  \"exports\": {\n",
    "    \".\": {\n",
    "      \"source\": \"./src/index.ts\",\n",
    "      \"default\": \"./dist/index.js\"\n",
    "    }\n",
    "  }\n",
    "}\n",
);

/// A tsconfig whose program reaches the sources of the package standing beside
/// it.
const TYPESCRIPT_TWO_PACKAGE_TSCONFIG: &str = concat!(
    typescript_probe_tsconfig_head!(),
    "\n",
    "  },\n",
    "  \"include\": [\"src\", \"../other/src\"]\n",
    "}\n",
);

/// A probe holding one manifest that parses beside one that does not.
///
/// The entry module of the package whose manifest parses is spared, and the
/// entry module of the package whose manifest does not is reported like any
/// other dead export. That is the whole cost of the failure, and the author
/// cannot read it off the report unless the run states the manifest it could
/// not read.
const TYPESCRIPT_BROKEN_MANIFEST_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &["packages/app/src/lib.ts:2", "packages/other/src/index.ts:2"],
    },
    staged: &[
        (TYPESCRIPT_APP_MANIFEST_PATH, TYPESCRIPT_APP_MANIFEST),
        (
            TYPESCRIPT_APP_TSCONFIG_PATH,
            TYPESCRIPT_TWO_PACKAGE_TSCONFIG,
        ),
        (TYPESCRIPT_APP_ENTRY_PATH, TYPESCRIPT_PROBE_ENTRY),
        (TYPESCRIPT_APP_LIB_PATH, TYPESCRIPT_PROBE_LIB),
        (TYPESCRIPT_BROKEN_MANIFEST_PATH, TYPESCRIPT_BROKEN_MANIFEST),
        (TYPESCRIPT_BROKEN_PACKAGE_ENTRY_PATH, TYPESCRIPT_PROBE_ENTRY),
    ],
    reason: "a manifest that does not parse takes its own package's entry modules out of the \
             carve-out and leaves every other package's in, so the entry of the broken package \
             reports and the entry of the whole one does not",
};

/// The opening of the line the run writes when ts-prune judged no export of one
/// project.
///
/// The rule's own name opens it, the way every shipped script that breaks a run
/// opens the line it writes, so the reader of the error knows which gate
/// stopped and why.
const TYPESCRIPT_BROKEN_RUN_LINE: &str = "dead-code-typescript: ts-prune exited";

/// A `tsconfig.json` of bytes that are not JSON.
///
/// ts-prune builds its module graph through `@ts-morph/common`, which throws on
/// such a file, so the run judges no export of the project that states it.
const TYPESCRIPT_BROKEN_TSCONFIG: &str = "this is not a tsconfig\n";

/// A probe whose one project states a `tsconfig.json` ts-prune cannot read.
///
/// The module beside it holds one export nothing imports, so a run that judged
/// the project would report a finding. The run judges nothing instead, and the
/// probe holds it to saying so rather than to answering an empty list.
const TYPESCRIPT_BROKEN_TSCONFIG_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &[TYPESCRIPT_BROKEN_RUN_LINE, TYPESCRIPT_PROBE_TSCONFIG_PATH],
    },
    staged: &[
        (TYPESCRIPT_PROBE_TSCONFIG_PATH, TYPESCRIPT_BROKEN_TSCONFIG),
        (TYPESCRIPT_PROBE_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "ts-prune judged no export of a project whose tsconfig it cannot read, so the run \
             breaks and names that project rather than answering a clean workspace",
};

/// Where the tsconfig of the second project of the two-project probe stands, as
/// the work-list holds it.
const TYPESCRIPT_OTHER_TSCONFIG_PATH: &str = "packages/other/tsconfig.json";

/// Where the module of the second project of the two-project probe stands, as
/// the work-list holds it.
const TYPESCRIPT_OTHER_LIB_PATH: &str = "packages/other/src/lib.ts";

/// A probe holding one project ts-prune reads beside one it cannot.
///
/// `packages/app` states a whole tsconfig and holds one export nothing imports,
/// so the run places a finding for it. `packages/other` states a tsconfig
/// ts-prune cannot read, so the run judges no export of that package at all.
/// Every file of `packages/other` would then read as clean, and the probe holds
/// the run to breaking rather than to answering the findings of the package it
/// did judge.
const TYPESCRIPT_BROKEN_PROJECT_BESIDE_A_WHOLE_ONE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: NODEJS_PROJECT_TYPES,
        rule: TYPESCRIPT_DEAD_CODE_RULE,
        expected: &[TYPESCRIPT_BROKEN_RUN_LINE, TYPESCRIPT_OTHER_TSCONFIG_PATH],
    },
    staged: &[
        (TYPESCRIPT_APP_TSCONFIG_PATH, TYPESCRIPT_PROBE_TSCONFIG),
        (TYPESCRIPT_APP_LIB_PATH, TYPESCRIPT_PROBE_LIB),
        (TYPESCRIPT_OTHER_TSCONFIG_PATH, TYPESCRIPT_BROKEN_TSCONFIG),
        (TYPESCRIPT_OTHER_LIB_PATH, TYPESCRIPT_PROBE_LIB),
    ],
    reason: "one project ts-prune could not read leaves every export of that project unjudged, \
             so the run breaks and names it rather than answering the findings of the project \
             it did judge",
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

/// Acceptance: a module NO other module imports is dead export by export, and
/// the run names each export at its own row.
///
/// This is the property that decided the `knip` question, so it is held rather
/// than left to the survey. The rule claims "every exported symbol no other
/// module in the project imports", and the everyday shape of that claim is an
/// author adding a module nothing imports yet.
///
/// Measured on this probe with knip 6.32.2, tuned with the entry list this
/// rule's own node script computes: `--include exports,types,namespaceMembers,
/// enumMembers` answers `{"issues":[]}` at exit 0, and adding `files` answers
/// one entry carrying `"exports":[]` and no row. knip resolves reachability
/// first, so it reports such a module ONE time with no symbol and no row, and
/// no configuration makes it enumerate the exports. The shipped run names all
/// three.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_names_every_export_of_a_module_nothing_imports() {
    verify_shipped_tree_reports(&TYPESCRIPT_ORPHAN_MODULE_PROBE);
}

/// Acceptance: a module the program reaches from OUTSIDE the project directory
/// is named at the path it stands at.
///
/// ts-prune cuts the first occurrence of its own working directory out of each
/// path it reports, and cuts one leading separator after that. A file INSIDE
/// the project therefore comes back under the project directory, and a file the
/// cut never touched comes back as the whole absolute path less that separator.
/// The run reads both back off the file list of the program: it spells each
/// file of that list the way the presenter spells it, and the file whose
/// spelling meets the reported one is the file of the finding.
///
/// Measured over `zod` at `4e1720c` with the dependencies installed: 1 of the
/// 76 findings named `packages/bench/<the absolute path of the checkout>/
/// packages/zod/src/v4/core/standard-schema.ts`, a file that stands nowhere.
/// The engine keeps a workspace-scope finding only when its path meets a file
/// of the run, so that finding was dropped without a word — a silent miss
/// rather than a wrong finding.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_names_a_module_outside_the_project_directory() {
    verify_shipped_tree_reports(&TYPESCRIPT_OUTSIDE_MODULE_PROBE);
    verify_shipped_staged_positions_report(&TYPESCRIPT_OUTSIDE_MODULE_POSITIONS_PROBE);
}

/// Acceptance: the run never names a file that is not the file of the finding,
/// whatever the presenter's cut left of the path.
///
/// The presenter writes `result.file.replace(process.cwd(), "").replace(/^\//,
/// "")`. `String.replace` given a STRING cuts the first occurrence of that
/// text and needs no separator after it, so a sibling package whose directory
/// name BEGINS with the project's own comes back cut at a position no path
/// stands at: `packages/consumer-bench/src/lib.ts` reaches the pipe as
/// `-bench/src/lib.ts`, and `packages/consumersrc/lib.ts` reaches it as
/// `src/lib.ts` — the spelling of a real, LIVE file of the project.
///
/// The run never inverts that cut. It spells each file of the program the way
/// the presenter spells it, and reports the finding at the one file that
/// carries the spelling ts-prune wrote. A spelling two files of the program
/// carry names no one file, so the run reports no finding for it.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_names_no_file_that_is_not_the_file_of_the_finding() {
    verify_shipped_tree_reports(&TYPESCRIPT_SIBLING_PREFIX_PROBE);
}

/// Acceptance: a finding the run cannot place is stated out loud.
///
/// A run that reports no finding and exits 0 over an item it never judged reads
/// exactly like a clean tree. `builtin/validators/README.md` states the answer:
/// a script that judged the code and could not judge ONE item writes a line
/// opening `sah-diagnostic:` on stderr and still exits 0, and the report states
/// each marked line.
///
/// No file filter can drop such a line. The engine keeps a workspace-scope
/// FINDING only when its path meets a file of the run, and a diagnostic is
/// about the RUN rather than about a reviewed file, so it has no path to be
/// kept by.
///
/// The count alone is not the whole of it. This test reads the MESSAGE, so a
/// run that declined some other item cannot pass.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_says_the_finding_it_declines_out_loud() {
    let declined = drive_shipped_staged_tree_read(
        &TYPESCRIPT_SIBLING_PREFIX_PROBE,
        NO_PROBE_LINKS,
        script_diagnostics,
    );

    assert_eq!(
        declined.len(),
        1,
        "the run must state the one finding it cannot place; it stated {declined:?}"
    );
    assert!(
        declined[0].contains(TYPESCRIPT_DECLINED_SPELLING),
        "the diagnostic must name the finding it declined; it said '{}'",
        declined[0]
    );
}

/// Acceptance: the file ts-prune reported is among the candidates its own
/// spelling is matched against, whichever way the file list spells it.
///
/// The two readings of the program spell one file two ways. `tsc
/// --listFilesOnly` prints the path it globbed or was given, and ts-prune
/// reports `fs.realpathSync(result.file)` — `ts-prune/lib/analyzer.js`. A
/// count of the listed files that carry a spelling is therefore a count of what
/// EXISTS, not a count of what is right: with the reported file left out of the
/// list, one other file carrying its spelling would place the finding at a file
/// it is not about.
///
/// The run spells each listed file both ways, so the reported file stands among
/// its own candidates. One candidate then means that candidate is it, and a
/// second real file carrying the spelling declines the item rather than
/// choosing.
///
/// Two listed entries that resolve to ONE real file are one candidate here, so
/// a link listed beside its own target is not a collision.
#[cfg(unix)]
#[test]
fn the_shipped_typescript_dead_code_tool_rule_places_a_file_the_two_readings_spell_differently() {
    let reported = drive_shipped_staged_tree_read(
        &TYPESCRIPT_LINKED_FILE_PROBE,
        TYPESCRIPT_LINKED_FILE_LINKS,
        finding_rows,
    );
    let declined = drive_shipped_staged_tree_read(
        &TYPESCRIPT_LINKED_FILE_PROBE,
        TYPESCRIPT_LINKED_FILE_LINKS,
        script_diagnostics,
    );

    assert_shipped_tree_rows(&TYPESCRIPT_LINKED_FILE_PROBE, &reported);
    assert!(
        declined.is_empty(),
        "the run must place the finding rather than decline it; it stated {declined:?}"
    );
}

/// Acceptance: a manifest the run could not read is stated out loud.
///
/// Entry resolution fails OPEN, and the manifest half of that failure is
/// narrow: the other manifests still build the `--ignore` pattern, and the
/// entry modules of the one package fall out of it. Every export of those
/// modules then reports as dead, which reads on the report exactly like a
/// module nothing imports.
///
/// So the run has to say which manifest it could not read. Nothing else on any
/// channel tells the author why a published entry module reports.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_says_the_manifest_it_could_not_read_out_loud() {
    let declined = drive_shipped_staged_tree_read(
        &TYPESCRIPT_BROKEN_MANIFEST_PROBE,
        NO_PROBE_LINKS,
        script_diagnostics,
    );

    verify_shipped_tree_reports(&TYPESCRIPT_BROKEN_MANIFEST_PROBE);
    assert_eq!(
        declined.len(),
        1,
        "the run must state the one manifest it could not read; it stated {declined:?}"
    );
    assert!(
        declined[0].contains(TYPESCRIPT_BROKEN_MANIFEST_PATH),
        "the diagnostic must name the manifest it could not read; it said '{}'",
        declined[0]
    );
}

/// Acceptance: a finding whose file stands outside the workspace is stated out
/// loud rather than written at a path the report never carries.
///
/// The run reports each finding at the REAL path of the file it is about, which
/// for a `files` entry that is a symbolic link is the path behind the link. That
/// path can stand outside the workspace altogether.
///
/// A row written there reaches no reader. `normalize_tool_path` cannot strip a
/// root the path does not begin with, so the row keeps its absolute path, and
/// the engine keeps a workspace-scope finding only when its normalized path
/// meets a file of the run — repo-relative every one. The row is dropped without
/// a word.
///
/// So the run declines the item and says which file it was about. A lost finding
/// the author reads about is what this rule trades a silent one for.
#[cfg(unix)]
#[test]
fn the_shipped_typescript_dead_code_tool_rule_says_the_file_outside_the_workspace_out_loud() {
    let reported = drive_shipped_staged_tree_read_with(
        &TYPESCRIPT_OUTSIDE_WORKSPACE_PROBE,
        TYPESCRIPT_OUTSIDE_WORKSPACE_LINKS,
        TYPESCRIPT_OUTSIDE_WORKSPACE_STAGED,
        finding_rows,
    );
    let declined = drive_shipped_staged_tree_read_with(
        &TYPESCRIPT_OUTSIDE_WORKSPACE_PROBE,
        TYPESCRIPT_OUTSIDE_WORKSPACE_LINKS,
        TYPESCRIPT_OUTSIDE_WORKSPACE_STAGED,
        script_diagnostics,
    );

    assert_shipped_tree_rows(&TYPESCRIPT_OUTSIDE_WORKSPACE_PROBE, &reported);
    assert_eq!(
        declined.len(),
        1,
        "the run must state the one file it could not report inside the workspace; it stated \
         {declined:?}"
    );
    assert!(
        declined[0].contains(TYPESCRIPT_LINK_TARGET_PATH),
        "the diagnostic must name the file it declined; it said '{}'",
        declined[0]
    );
}

/// Acceptance: a project ts-prune judged no export of breaks the run.
///
/// ts-prune answers status 0 for a clean project and for a project holding
/// findings alike, and it answers a nonzero status for a project it could not
/// read. Measured with ts-prune 0.10.3: a `tsconfig.json` of bytes that are not
/// JSON exits 1 with 0 bytes on stdout and a node stack on stderr. The rule
/// body carries the whole table.
///
/// The earlier shape of this script threw that status away. Its per-project
/// pipe ended in the node placement and its loop ended in `sort -u`, and a
/// shell pipeline takes the status of its LAST command, so a project ts-prune
/// never read answered 0 findings at exit 0 — which the engine reads as "the
/// tool judged the code".
///
/// The second probe is the shape a monorepo makes reach this. One project the
/// run judged fills the row list while another project was never judged at all,
/// so a run that wrote those rows and exited 0 would answer clean for every file
/// of the broken package.
#[test]
fn the_shipped_typescript_dead_code_tool_rule_breaks_on_a_project_ts_prune_cannot_read() {
    verify_shipped_tree_breaks(&TYPESCRIPT_BROKEN_TSCONFIG_PROBE);
    verify_shipped_tree_breaks(&TYPESCRIPT_BROKEN_PROJECT_BESIDE_A_WHOLE_ONE_PROBE);
}
