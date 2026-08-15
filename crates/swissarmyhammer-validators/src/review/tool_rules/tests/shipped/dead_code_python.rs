//! Acceptance tests for the shipped `dead-code-python` tool rule.
//!
//! Each test drives the SHIPPED script over a probe repository and reads what
//! the real vulture reported.
//!
//! The module stands beside `dead_code`, which holds the whole family to its
//! fixture pair. The pair holds the two carve-outs a manifest and a base class
//! answer, because both take a finding off the fixture under test. What it
//! cannot hold is the SHAPE of the manifest — the pair ships one
//! `pyproject.toml` and a directory holds one manifest of each name — and the
//! facts vulture reads on its own, which stand in a package the fixture
//! directory has no room for.

use super::*;

/// Where the library module of an entry-point probe stands, as the work-list
/// holds it.
const PYTHON_PROBE_LIB_PATH: &str = "src/lib.py";

/// A module holding one name a manifest publishes and one name nothing names.
///
/// `published_entry` stands on row 4 and `truly_dead` on row 9, so a probe
/// that stages this module expects `src/lib.py:9` when the manifest is read
/// and both rows when it is not.
const PYTHON_PROBE_LIB: &str = concat!(
    "\"\"\"A probe module for the shipped Python dead-code tool rule.\"\"\"\n",
    "\n",
    "\n",
    "def published_entry():\n",
    "    \"\"\"The object the manifest of this probe names.\"\"\"\n",
    "    return 1\n",
    "\n",
    "\n",
    "def truly_dead():\n",
    "    \"\"\"A function nothing calls and no manifest names.\"\"\"\n",
    "    return 2\n",
);

/// Where a `pyproject.toml` stands, as the work-list holds it.
const PYTHON_PROBE_PYPROJECT_PATH: &str = "pyproject.toml";

/// Where a `setup.cfg` stands, as the work-list holds it.
const PYTHON_PROBE_SETUP_CFG_PATH: &str = "setup.cfg";

/// Where a `setup.py` stands, as the work-list holds it.
const PYTHON_PROBE_SETUP_PY_PATH: &str = "setup.py";

/// A manifest naming the probe's published object under `[project.scripts]`.
const PYTHON_PROBE_PYPROJECT: &str = concat!(
    "[project]\n",
    "name = \"entry-probe\"\n",
    "version = \"0.0.0\"\n",
    "\n",
    "[project.scripts]\n",
    "entry-probe = \"lib:published_entry\"\n",
);

/// A manifest naming the probe's published object under a plugin group.
///
/// A plugin group is a nested table, so the run has to walk the whole
/// `entry-points` tree rather than read one level of it.
const PYTHON_PROBE_PLUGIN_PYPROJECT: &str = concat!(
    "[project]\n",
    "name = \"entry-probe\"\n",
    "version = \"0.0.0\"\n",
    "\n",
    "[project.entry-points.\"probe.plugins\"]\n",
    "entry-probe = \"lib:published_entry\"\n",
);

/// A `setup.cfg` naming the probe's published object as a console script.
const PYTHON_PROBE_SETUP_CFG: &str = concat!(
    "[metadata]\n",
    "name = entry-probe\n",
    "\n",
    "[options.entry_points]\n",
    "console_scripts =\n",
    "    entry-probe = lib:published_entry\n",
);

/// A `setup.py` naming the probe's published object as a console script.
///
/// The value is a dictionary of LISTS, which is the shape setuptools has
/// always taken, so the run has to walk a list as well as a table.
const PYTHON_PROBE_SETUP_PY: &str = concat!(
    "\"\"\"The build script of the probe.\"\"\"\n",
    "\n",
    "from setuptools import setup\n",
    "\n",
    "setup(\n",
    "    name=\"entry-probe\",\n",
    "    entry_points={\"console_scripts\": [\"entry-probe = lib:published_entry\"]},\n",
    ")\n",
);

/// A `pyproject.toml` no TOML reader can parse.
const PYTHON_PROBE_BROKEN_PYPROJECT: &str = "[project\nname = \"entry-probe\"\n";

/// What a probe expects when the manifest states the published object.
const PYTHON_ONLY_THE_DEAD_NAME: &[&str] = &["src/lib.py:9"];

/// A probe whose `pyproject.toml` states the published object as a script.
const PYTHON_PYPROJECT_SCRIPT_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: PYTHON_ONLY_THE_DEAD_NAME,
    },
    staged: &[
        (PYTHON_PROBE_PYPROJECT_PATH, PYTHON_PROBE_PYPROJECT),
        (PYTHON_PROBE_LIB_PATH, PYTHON_PROBE_LIB),
    ],
    reason: "an object `[project.scripts]` names is called from outside the tree, and the \
             function no manifest names is the one finding",
};

/// A probe whose `pyproject.toml` states the published object as a plugin.
const PYTHON_PYPROJECT_PLUGIN_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: PYTHON_ONLY_THE_DEAD_NAME,
    },
    staged: &[
        (PYTHON_PROBE_PYPROJECT_PATH, PYTHON_PROBE_PLUGIN_PYPROJECT),
        (PYTHON_PROBE_LIB_PATH, PYTHON_PROBE_LIB),
    ],
    reason: "a `[project.entry-points]` group is a nested table, and the object it names is \
             called from outside the tree just as a script is",
};

/// A probe whose `setup.cfg` states the published object.
const PYTHON_SETUP_CFG_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: PYTHON_ONLY_THE_DEAD_NAME,
    },
    staged: &[
        (PYTHON_PROBE_SETUP_CFG_PATH, PYTHON_PROBE_SETUP_CFG),
        (PYTHON_PROBE_LIB_PATH, PYTHON_PROBE_LIB),
    ],
    reason: "a `setup.cfg` states entry points in its own format, and a project that has not \
             moved to `pyproject.toml` states its surface there",
};

/// A probe whose `setup.py` states the published object.
const PYTHON_SETUP_PY_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: PYTHON_ONLY_THE_DEAD_NAME,
    },
    staged: &[
        (PYTHON_PROBE_SETUP_PY_PATH, PYTHON_PROBE_SETUP_PY),
        (PYTHON_PROBE_LIB_PATH, PYTHON_PROBE_LIB),
    ],
    reason: "a `setup.py` states entry points as a literal argument, and the run reads that \
             literal rather than running the build script",
};

/// A probe whose only manifest cannot be parsed.
const PYTHON_BROKEN_MANIFEST_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: &["src/lib.py:4", "src/lib.py:9"],
    },
    staged: &[
        (PYTHON_PROBE_PYPROJECT_PATH, PYTHON_PROBE_BROKEN_PYPROJECT),
        (PYTHON_PROBE_LIB_PATH, PYTHON_PROBE_LIB),
    ],
    reason: "a manifest the run cannot read states nothing, so every name stays under the gate \
             and the failure adds a finding rather than taking one away",
};

/// A manifest whose own `[tool.vulture]` would silence the whole gate, beside
/// the entry point the run must still read out of the same file.
///
/// `ignore_names = ["*"]` matches every name vulture defines, and vulture
/// merges what it finds here UNDER the command line, so an option the run does
/// not state is the project's to set.
const PYTHON_PROBE_GATE_PYPROJECT: &str = concat!(
    "[project]\n",
    "name = \"entry-probe\"\n",
    "version = \"0.0.0\"\n",
    "\n",
    "[project.scripts]\n",
    "entry-probe = \"lib:published_entry\"\n",
    "\n",
    "[tool.vulture]\n",
    "ignore_names = [\"*\"]\n",
    "min_confidence = 100\n",
    "make_whitelist = true\n",
);

/// A probe whose own vulture configuration would turn the gate off.
const PYTHON_PROJECT_CONFIG_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: PYTHON_ONLY_THE_DEAD_NAME,
    },
    staged: &[
        (PYTHON_PROBE_PYPROJECT_PATH, PYTHON_PROBE_GATE_PYPROJECT),
        (PYTHON_PROBE_LIB_PATH, PYTHON_PROBE_LIB),
    ],
    reason: "the run passes a `--config` of its own, so a project cannot turn the gate off from \
             its `[tool.vulture]` table, and the run still reads that file for entry points",
};

/// Where the support module of the test-runner probe stands.
const PYTHON_PROBE_SUPPORT_PATH: &str = "src/support.py";

/// A module holding one class a test runner loads and one class nothing loads.
///
/// Neither the directory nor the file name matches vulture's own test-file
/// patterns, so vulture's native handling of a `Test`-named class never fires
/// here and only the base class states which of the two has an outside caller.
/// `NothingLoads` stands on row 14.
const PYTHON_PROBE_SUPPORT: &str = concat!(
    "\"\"\"A probe module for the shipped Python dead-code tool rule.\"\"\"\n",
    "\n",
    "import unittest\n",
    "\n",
    "\n",
    "class RunnerLoads(unittest.TestCase):\n",
    "    \"\"\"A class the test runner loads, outside any test file.\"\"\"\n",
    "\n",
    "    def test_nothing(self):\n",
    "        \"\"\"A test the runner calls by name.\"\"\"\n",
    "        self.assertIsNone(None)\n",
    "\n",
    "\n",
    "class NothingLoads:\n",
    "    \"\"\"A class nothing loads and nothing names.\"\"\"\n",
);

/// A probe holding a `TestCase` subclass outside every test file.
const PYTHON_TEST_RUNNER_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: &["src/support.py:14"],
    },
    staged: &[(PYTHON_PROBE_SUPPORT_PATH, PYTHON_PROBE_SUPPORT)],
    reason: "a class that inherits from `TestCase` is loaded by the test runner wherever it \
             stands, and the class that inherits from nothing is the one finding",
};

/// Where the handler module of the framework probe stands.
const PYTHON_PROBE_HANDLERS_PATH: &str = "src/handlers.py";

/// A module holding three framework registrations and one dead function.
///
/// The three cover what the fixed decorator roster missed: FastAPI and
/// Starlette routing, a Django signal receiver, and a pytest fixture imported
/// by its bare name. `truly_dead` stands on row 25.
const PYTHON_PROBE_HANDLERS: &str = concat!(
    "\"\"\"A probe module for the shipped Python dead-code tool rule.\"\"\"\n",
    "\n",
    "from probe_framework import app, receiver, started\n",
    "from pytest import fixture\n",
    "\n",
    "\n",
    "@app.get(\"/items\")\n",
    "def read_items():\n",
    "    \"\"\"A route the web framework calls.\"\"\"\n",
    "    return 1\n",
    "\n",
    "\n",
    "@receiver(started)\n",
    "def on_started():\n",
    "    \"\"\"A signal handler the framework calls.\"\"\"\n",
    "    return 2\n",
    "\n",
    "\n",
    "@fixture\n",
    "def probe_client():\n",
    "    \"\"\"A fixture the test runner calls.\"\"\"\n",
    "    return 3\n",
    "\n",
    "\n",
    "def truly_dead():\n",
    "    \"\"\"A function nothing calls.\"\"\"\n",
    "    return 4\n",
);

/// A probe holding the framework registrations the fixed roster missed.
const PYTHON_FRAMEWORK_DECORATOR_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: &["src/handlers.py:25"],
    },
    staged: &[(PYTHON_PROBE_HANDLERS_PATH, PYTHON_PROBE_HANDLERS)],
    reason: "a route, a signal receiver and a bare pytest fixture are each called by a \
             framework, and the undecorated function is the one finding",
};

/// Where the package root of the stated-surface probe stands.
const PYTHON_PROBE_PACKAGE_INIT_PATH: &str = "src/pkg/__init__.py";

/// Where the package module of the stated-surface probe stands.
const PYTHON_PROBE_PACKAGE_API_PATH: &str = "src/pkg/api.py";

/// A package root that explicitly re-exports one name of its own module.
///
/// The redundant alias is the form the typing specification reserves for "this
/// name is this package's own", and flask and fastapi both state their whole
/// surface that way.
const PYTHON_PROBE_PACKAGE_INIT: &str = concat!(
    "\"\"\"A probe package for the shipped Python dead-code tool rule.\"\"\"\n",
    "\n",
    "from .api import aliased as aliased\n",
);

/// A module stating one name in a tuple `__all__`, one behind the package's
/// re-export, and one nowhere.
///
/// `truly_dead` stands on row 16.
const PYTHON_PROBE_PACKAGE_API: &str = concat!(
    "\"\"\"A probe module for the shipped Python dead-code tool rule.\"\"\"\n",
    "\n",
    "__all__ = (\"listed\",)\n",
    "\n",
    "\n",
    "def listed():\n",
    "    \"\"\"A name the module's own `__all__` states.\"\"\"\n",
    "    return 1\n",
    "\n",
    "\n",
    "def aliased():\n",
    "    \"\"\"A name the package re-exports explicitly.\"\"\"\n",
    "    return 2\n",
    "\n",
    "\n",
    "def truly_dead():\n",
    "    \"\"\"A name the package states nowhere.\"\"\"\n",
    "    return 3\n",
);

/// A probe holding the two surface facts vulture reads for itself.
const PYTHON_STATED_SURFACE_PROBE: ShippedStagedTree = ShippedStagedTree {
    run: ShippedRun {
        project_types: PYTHON_PROJECT_TYPES,
        rule: PYTHON_DEAD_CODE_RULE,
        expected: &["src/pkg/api.py:16"],
    },
    staged: &[
        (PYTHON_PROBE_PACKAGE_INIT_PATH, PYTHON_PROBE_PACKAGE_INIT),
        (PYTHON_PROBE_PACKAGE_API_PATH, PYTHON_PROBE_PACKAGE_API),
    ],
    reason: "a tuple `__all__` and an explicit re-export each state the package's surface, and \
             the name stated nowhere is the one finding",
};

/// Acceptance: an object a package manifest names as an entry point is not
/// dead, whichever of the three manifests states it.
///
/// This is the `dead-code` carve-out for the exported public API, for the one
/// place vulture cannot read it. Vulture reads `__all__` and the explicit
/// re-export itself; an entry point names an object a packaging tool imports
/// from outside every module of the tree, and no Python source of the project
/// mentions it.
///
/// Measured over the corpus: flask states `flask.cli:main`, fastapi states
/// `fastapi.cli:main` and django states
/// `django.core.management:execute_from_command_line`, each in
/// `pyproject.toml` and none in `setup.py` or `setup.cfg`.
#[test]
fn the_shipped_python_dead_code_tool_rule_reports_no_published_entry_point() {
    verify_shipped_tree_reports(&PYTHON_PYPROJECT_SCRIPT_PROBE);
    verify_shipped_tree_reports(&PYTHON_PYPROJECT_PLUGIN_PROBE);
    verify_shipped_tree_reports(&PYTHON_SETUP_CFG_PROBE);
    verify_shipped_tree_reports(&PYTHON_SETUP_PY_PROBE);
}

/// Acceptance: a manifest the run cannot read leaves every name under the
/// gate.
///
/// Reading the surface fails OPEN. A failure adds findings and never takes one
/// away, so a broken manifest cannot answer clean for code the tool judged
/// dirty.
#[test]
fn the_shipped_python_dead_code_tool_rule_reads_a_broken_manifest_as_no_surface() {
    verify_shipped_tree_reports(&PYTHON_BROKEN_MANIFEST_PROBE);
}

/// Acceptance: the rule owns its own gate beside a project vulture
/// configuration.
///
/// Vulture reads `[tool.vulture]` out of the `pyproject.toml` of its working
/// directory and merges it UNDER the command line, so an option the run does
/// not state is the project's. Measured over a probe holding one dead
/// function: `ignore_names = ["*"]` reported 0 at exit 0, `min_confidence =
/// 100` beside `make_whitelist = true` reported 0 at exit 0, and a
/// `pyproject.toml` that is not TOML at all exited 1 on a traceback while the
/// pipe read it as a clean tree.
///
/// The run therefore passes a `--config` of its own, and the same probe reads
/// the project's file for entry points and finds one.
#[test]
fn the_shipped_python_dead_code_tool_rule_keeps_its_own_gate_beside_a_project_config() {
    verify_shipped_tree_reports(&PYTHON_PROJECT_CONFIG_PROBE);
}

/// Acceptance: a class that inherits from a `TestCase` is loaded by the test
/// runner, wherever the class stands.
///
/// `dead-code`, the prompt rule this one supersedes, exempts "test functions
/// and test-only helpers". Vulture answers that on its own INSIDE a test file:
/// `_ignore_class` drops a class with `Test` in its name, and
/// `_ignore_function` drops a `test_*` function, when the path matches
/// `*/test/*`, `*/tests/*`, `*/test*.py` or `*[-_]test.py`. Measured over the
/// corpus: 0 `Test`-named classes reported over 4184 `.py` files. Outside
/// those paths the name states nothing and the BASE CLASS does.
#[test]
fn the_shipped_python_dead_code_tool_rule_reports_no_test_runner_class() {
    verify_shipped_tree_reports(&PYTHON_TEST_RUNNER_PROBE);
}

/// Acceptance: the decorator roster covers the framework registrations the
/// fixed list missed.
///
/// `dead-code` also exempts "framework-invoked handlers, CLI command
/// callbacks, registered hooks/callbacks". The roster before this change
/// covered Flask `@*.route`, click `@*.command`, celery `@*.task` and
/// `@*.fixture`, and it covered neither FastAPI nor Starlette routing, nor
/// Django's `@receiver`, nor a `fixture` imported by its bare name.
///
/// Measured over `fastapi` at `a1fa70d`: 685 findings stood under `@*.get`
/// and 277 under `@*.post`. Measured over `django` at `3436cf9`: 23 stood
/// under `@receiver`.
#[test]
fn the_shipped_python_dead_code_tool_rule_reports_no_framework_registration() {
    verify_shipped_tree_reports(&PYTHON_FRAMEWORK_DECORATOR_PROBE);
}

/// Acceptance: the two surface facts vulture reads for itself still hold.
///
/// The rule body states that a tuple `__all__` and an explicit re-export each
/// answer the exported-surface carve-out with no work from the run. Both are
/// vulture's behaviour rather than the script's, so a vulture upgrade could
/// take either away and the run would keep exiting 0. This test is the gate on
/// that.
#[test]
fn the_shipped_python_dead_code_tool_rule_reads_the_surface_the_package_states() {
    verify_shipped_tree_reports(&PYTHON_STATED_SURFACE_PROBE);
}
