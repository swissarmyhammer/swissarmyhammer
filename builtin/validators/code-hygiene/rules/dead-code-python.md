---
name: dead-code-python
description: Python names nothing reads and statements behind a jump — checked by vulture, not by prompt.
match:
  files:
    - "**/*.py"
    - "**/*.pyw"
  project_types:
    - python
supersedes: dead-code
tool:
  scope: workspace
  run: |
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    cat > "$work/reached.py" <<'REACHED_NAMES'
    """Writes one name for each line: what a caller OUTSIDE this tree reaches.

    Vulture reads the two places a Python MODULE states its own surface — the
    `__all__` list or tuple, and the explicit re-export `from .m import N as N`.
    It reads neither of the two places the WORKSPACE states that something
    outside it reaches a name: an entry point of a package manifest, which
    names an object a packaging tool imports, and a class that inherits from a
    `TestCase`, which a test runner loads.

    The read fails OPEN. A file this script cannot read carries no name, so
    every name it would have carried stays under the gate. A failure adds
    findings and never takes one away.
    """

    import ast
    import configparser
    import os
    import sys

    # The directories a workspace walk never enters.
    UNWALKED = frozenset(
        {
            ".git",
            ".hg",
            ".mypy_cache",
            ".svn",
            ".tox",
            ".venv",
            "__pycache__",
            "build",
            "dist",
            "node_modules",
            "venv",
        }
    )

    # The `[project]` tables of a `pyproject.toml` that name an entry point.
    ENTRY_POINT_TABLES = ("scripts", "gui-scripts", "entry-points")

    # The `setup.cfg` section that names entry points.
    SETUP_CFG_SECTION = "options.entry_points"

    # The `setup()` keyword that names entry points.
    SETUP_PY_KEYWORD = "entry_points"

    # What the base of a class a test runner loads is spelled with. It covers
    # `unittest.TestCase` and `IsolatedAsyncioTestCase`, and Django's
    # `SimpleTestCase`, `TransactionTestCase` and `LiveServerTestCase`.
    TEST_CASE_SUFFIX = "TestCase"


    def toml_loader():
        """The TOML reader of this interpreter, or `None` when it has none."""
        for module in ("tomllib", "tomli"):
            try:
                return __import__(module).loads
            except ImportError:
                continue
        return None


    def walk(root):
        """Every file under `root`, skipping the unwalked directories."""
        for base, dirs, files in os.walk(root):
            dirs[:] = sorted(name for name in dirs if name not in UNWALKED)
            for name in sorted(files):
                yield os.path.join(base, name)


    def object_names(target):
        """The names the entry-point target `module:object.attr` publishes.

        Everything before the colon is the module, which is a path rather than
        a name vulture defines. Everything after it is the object, and each
        dotted part of it is a name.
        """
        _, _, obj = target.partition(":")
        return [part.strip() for part in obj.split(".") if part.strip()]


    def entry_point_targets(table, out):
        """Collects every string leaf of an entry-point table.

        A group of `[project.entry-points]` is a table of its own, and the
        `entry_points` of a `setup.py` is a table of LISTS, so the walk has to
        cross both shapes.
        """
        if isinstance(table, str):
            out.append(table)
        elif isinstance(table, dict):
            for inner in table.values():
                entry_point_targets(inner, out)
        elif isinstance(table, (list, tuple)):
            for inner in table:
                entry_point_targets(inner, out)


    def pyproject_names(path, loads, out):
        """The entry-point object names a `pyproject.toml` states."""
        with open(path, "rb") as handle:
            project = loads(handle.read().decode("utf-8")).get("project", {})
        targets = []
        for table in ENTRY_POINT_TABLES:
            entry_point_targets(project.get(table), targets)
        for target in targets:
            out.update(object_names(target))


    def setup_cfg_names(path, out):
        """The entry-point object names a `setup.cfg` states."""
        parser = configparser.ConfigParser()
        parser.read(path, encoding="utf-8")
        if not parser.has_section(SETUP_CFG_SECTION):
            return
        for _, block in parser.items(SETUP_CFG_SECTION):
            for line in block.splitlines():
                _, _, target = line.partition("=")
                if target.strip():
                    out.update(object_names(target.strip()))


    def setup_py_names(tree, out):
        """The entry-point object names a `setup.py` states.

        The value is read as a literal. A `setup.py` that computes its entry
        points states nothing this way, and its objects stay under the gate.
        """
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call):
                continue
            for keyword in node.keywords:
                if keyword.arg != SETUP_PY_KEYWORD:
                    continue
                try:
                    value = ast.literal_eval(keyword.value)
                except (TypeError, ValueError):
                    continue
                targets = []
                entry_point_targets(value, targets)
                for target in targets:
                    out.update(object_names(target))


    def base_name(node):
        """The trailing name of one base of a class statement."""
        if isinstance(node, ast.Attribute):
            return node.attr
        if isinstance(node, ast.Name):
            return node.id
        return ""


    def class_bases(tree, out):
        """Adds each class of `tree` to `out`, keyed on its own name.

        The value is the set of trailing base names. Two classes of one name
        share an entry, because vulture answers by name as well.
        """
        for node in ast.walk(tree):
            if not isinstance(node, ast.ClassDef):
                continue
            bases = {base_name(base) for base in node.bases if base_name(base)}
            out.setdefault(node.name, set()).update(bases)


    def test_runner_names(bases):
        """The classes a test runner loads, over the whole `bases` table.

        A class whose base is spelled `*TestCase` is loaded by name, and so is
        a class that inherits from such a class through any number of steps.
        The walk repeats until it adds nothing, which is what makes the base
        class of a suite carry the whole suite.
        """
        reached = {
            name
            for name, parents in bases.items()
            if any(parent.endswith(TEST_CASE_SUFFIX) for parent in parents)
        }
        growing = True
        while growing:
            growing = False
            for name, parents in bases.items():
                if name not in reached and parents & reached:
                    reached.add(name)
                    growing = True
        return reached


    def parsed(path):
        """The syntax tree of `path`.

        It raises for a file that does not parse and for a file that is not
        UTF-8. The caller names the file on stderr and reads no fact from it.
        """
        with open(path, "rb") as handle:
            return ast.parse(handle.read())


    def read_file(path, loads, published, bases):
        """Reads whichever fact the file at `path` states, if it states one."""
        name = os.path.basename(path)
        if name == "pyproject.toml":
            if loads is not None:
                pyproject_names(path, loads, published)
        elif name == "setup.cfg":
            setup_cfg_names(path, published)
        elif name.endswith(".py") or name.endswith(".pyw"):
            tree = parsed(path)
            if name == "setup.py":
                setup_py_names(tree, published)
            class_bases(tree, bases)


    def main():
        """Writes each reached name of this workspace on a line of its own."""
        loads = toml_loader()
        published = set()
        bases = {}
        for path in walk(os.getcwd()):
            try:
                read_file(path, loads, published, bases)
            except Exception as failure:
                sys.stderr.write(
                    f"dead-code-python: {path} states no reached name: {failure}\n"
                )
        for name in sorted(published | test_runner_names(bases)):
            if name.isidentifier():
                sys.stdout.write(name + "\n")


    try:
        main()
    except Exception as failure:
        sys.stderr.write(f"dead-code-python: no reached name read: {failure}\n")
    REACHED_NAMES
    printf '[tool.vulture]\n' > "$work/vulture.toml"
    python3 "$work/reached.py" > "$work/whitelist.py"
    vulture \
      --config "$work/vulture.toml" \
      --min-confidence 60 \
      --exclude '*/.venv/*,*/venv/*,*/node_modules/*,*/target/*,*/build/*,*/dist/*,*/__pycache__/*,*/.tox/*,*/.mypy_cache/*' \
      --ignore-names '__*__,test_*,setUp,setUpClass,setUpModule,setUpTestData,tearDown,tearDownClass,tearDownModule,asyncSetUp,asyncTearDown,runTest,visit_*,pytest_*,forward' \
      --ignore-decorators '@*.route,@fixture,@*.fixture,@*.command,@*.callback,@*.task,@*.errorhandler,@*.before_*,@*.after_*,@*.teardown_*,@*.setter,@*.getter,@*.deleter,@*.register,@*.validator,@*.event,@overload,@*.overload,@abstractmethod,@*.abstractmethod,@*.get,@*.post,@*.put,@*.patch,@*.delete,@*.head,@*.options,@*.api_route,@*.websocket,@*.websocket_route,@*.middleware,@*.exception_handler,@*.on_event,@receiver,@*.receiver,@*.tag,@*.simple_tag,@*.simple_block_tag,@*.filter,@*.register_lookup,@*.display,@*.action' \
      . "$work/whitelist.py" > "$work/report.txt" 2> "$work/vulture.err"
    status=$?
    if [ "$status" -ne 0 ] && [ "$status" -ne 3 ]; then
      cat "$work/vulture.err" "$work/report.txt" >&2
      printf 'dead-code-python: vulture exited %s and judged no code\n' "$status" >&2
      exit 1
    fi
    sed -n 's/^\(.*\) ([0-9]*% confidence)$/\1/p' "$work/report.txt"
  doctor:
    check_command: "which vulture python3 sed cat mktemp printf"
    check_version_command: "vulture --version"
  install:
    commands:
      - "uv tool install vulture==2.14"
      - "pipx install vulture==2.14"
---

# Dead Code — Python

`vulture` reports every name no other name reads — the import, the class, the
function, the method, the property, the attribute, the module variable — and
every statement behind a jump its branch always takes. This one rule owns all of
it. `unreachable-code-python` was folded in here so that one finding has one
owner.

The confidence floor is vulture's own default of 60, and the run states it
rather than letting it default, for the reason the gate section below gives. 100
admits unreachable code alone. 60 admits the whole set, which is the point:
dead code stops being a judgment.

## The corpus every number below was measured over

Four published Python packages, cloned at HEAD on 2026-08-14:

| repository | commit | `.py` files | states its surface with |
|---|---|---|---|
| psf/requests | `8068356` | 37 | a tuple `__all__` |
| pallets/flask | `2a8a38b` | 83 | 39 explicit re-exports, and one console script |
| fastapi/fastapi | `a1fa70d` | 1136 | 66 explicit re-exports, and one console script |
| django/django | `3436cf9` | 2928 | 660 `__all__` names, and one console script |

`requests` and `flask` are the two the first version of this rule measured.
`fastapi` and `django` stand beside them because the entry-point carve-out is
about the frameworks the first decorator roster missed.

## The exported public API, which the package already states

`dead-code`, the prompt rule this one supersedes, exempts "a `pub`/exported item
that is the crate's/library's surface for *external* callers", and it names how
to read that surface: "Where a language names its surface in one place — Python's
`__all__`, a module's export list — that list is the answer."

Python states that surface in three places, and vulture 2.14 already reads two
of them. Measured against `vulture/core.py`, and against a probe for each:

| the fact | where vulture reads it | measured |
|---|---|---|
| `__all__`, a list OR a tuple | `_assigns_special_variable__all__` | a name in a tuple `__all__` is silent; drop the name and it reports |
| `from .m import N as N` | `_add_aliases`, `if alias is not None` | a name behind the redundant alias is silent; the same name behind a plain import reports |
| an entry point of a manifest | nowhere | a function `[project.scripts]` names still reports |

The redundant alias is the form the typing specification reserves for "this
name is this package's own", and it is how the two libraries that write no
`__all__` state their whole surface: `flask/__init__.py` is 39 lines of
`from .app import Flask as Flask`, and `fastapi/__init__.py` is 20 more of the
same. The read costs the run nothing, because vulture treats ANY aliased import
as a use of the name it aliases.

So the run reads the third fact and only the third: **the entry point**. An
entry point names an object a packaging tool imports from outside every module
of the tree, and no Python source of the project mentions it. The run reads
`[project.scripts]`, `[project.gui-scripts]` and every group of
`[project.entry-points]` from each `pyproject.toml`, the `[options.entry_points]`
section of each `setup.cfg`, and the `entry_points=` literal of each `setup.py`;
it takes the object after the colon; and it writes the names into a vulture
whitelist module, which is vulture's own mechanism for "the callers are
elsewhere". All four corpus packages state theirs in `pyproject.toml`:
`flask.cli:main`, `fastapi.cli:main`,
`django.core.management:execute_from_command_line`, and `requests` states none.

This is `--retain-public` for Python, and it is the same trade `dead-code-typescript`
makes: the run reads what the package SAYS about itself instead of asking the
author to mark correct code.

### What no fact answers

A name a package publishes from a submodule, and states in no `__all__`, in no
re-export and in no entry point, is invisible to every reader and to every tool
alike. Measured over the two package directories `src/requests` and `src/flask`
side by side — which is the shape the first version of this rule measured, and
it reproduces its 100 exactly:

| run | findings |
|---|---|
| the two package directories, the roster before this change | 100 |
| the same, the shipped run, beside `flask`'s own `pyproject.toml` | 100 |

`MethodView`, `HTTPDigestAuth`, `get_namespace`, `iter_lines` and
`from_key_val_list` are all in that 100. None of them is in an `__all__`, none is
re-exported by `flask/__init__.py` or `requests/__init__.py`, and none is an
entry point. That is the finding, not a defect in the tool: a Python package
that never names its surface leaves a reader and a tool in exactly the same
position. The author answers with an `__all__` entry, a re-export, or the
staging marker below.

The same names are SILENT over the whole repositories, because each library's
own tests import them and a test in the tree is a caller. The scope is
`workspace`, so the whole repository is what the rule really reads.

### The test runner, which the class statement states

`dead-code` also exempts "test functions and test-only helpers". Vulture answers
that inside a test file on its own: `_ignore_class` drops a class with `Test` in
its name, `_ignore_function` drops a `test_*` function and the pytest module
hooks, and `_ignore_method` drops a `test_*` method — each when the path matches
`*/test/*`, `*/tests/*`, `*/test*.py` or `*[-_]test.py`, matched without case.
Measured over the corpus: **0** `Test`-named classes reported over 4184 `.py`
files, in four repositories that hold thousands of them.

Outside those paths the NAME states nothing and the BASE CLASS does. The run
therefore reads each class statement and treats a class whose base is spelled
`*TestCase` as reached, along with every class that inherits from such a class
through any number of steps. That covers `unittest.TestCase` and
`IsolatedAsyncioTestCase`, Django's `SimpleTestCase`, `TransactionTestCase` and
`LiveServerTestCase`, and the base class a suite writes for itself.

`--ignore-names` carries the unittest protocol names beside it, because a
runner calls those by name in any file: `setUp`, `setUpClass`, `setUpModule`,
`setUpTestData`, the three `tearDown` spellings, `asyncSetUp`, `asyncTearDown`
and `runTest`. Measured: `runTest` reports 7 times over the corpus without it.

## The entry points a framework registers

`dead-code` also exempts "framework-invoked handlers, CLI command callbacks,
registered hooks/callbacks — anything the runtime or a framework calls by
convention rather than by an in-repo call site". `--ignore-decorators` is that
roster, and every entry is a framework convention rather than one project's
exception.

The roster before this change covered Flask `@*.route`, click `@*.command`,
celery `@*.task` and `@*.fixture`, and it covered no other web framework at all.
Measured, by reading the decorator that stands on each item the shipped run
reported:

| decorator | `fastapi` | `django` | what registers it |
|---|---|---|---|
| `@*.get` | 685 | — | FastAPI and Starlette routing |
| `@*.post` | 277 | — | the same |
| `@*.put` | 37 | — | the same |
| `@*.websocket` | 27 | — | the same |
| `@*.exception_handler` | 11 | — | the same |
| `@*.delete` | 6 | — | the same |
| `@*.patch` | 4 | 26 | routing, and `unittest.mock.patch` |
| `@*.on_event`, `@*.middleware` | 6 | — | FastAPI lifespan and middleware |
| `@*.websocket_route`, `@*.api_route` | 3 | — | Starlette and FastAPI routing |
| `@*.register_lookup` | — | 57 | the Django ORM lookup registry |
| `@*.tag`, `@*.simple_tag`, `@*.simple_block_tag` | — | 70 | the Django template library |
| `@receiver` | — | 23 | Django signals |
| `@*.display` | — | 21 | the Django admin |
| `@*.filter` | — | 19 | a template filter registry |
| `@*.action` | — | 8 | the Django admin |

Each of those is now in the roster, together with `@*.head` and `@*.options` for
the two HTTP methods the corpus happens not to route, `@*.callback` for typer,
`@*.receiver` for a receiver reached through its module, and a bare `@fixture`
for the `from pytest import fixture` that `@*.fixture` cannot match.

`@*.get` is the broadest entry, and it is broad on purpose: a decorator named
`.get` registers a route. The whole roster matches on the decorator position
alone, so a name that is dead and undecorated is unaffected by any of it.

## What the whole corpus reports

Each repository is read as a workspace, which is how the rule runs:

| repository | before | after | dropped | time |
|---|---|---|---|---|
| requests | 89 | 89 | 0 | 0.2 s |
| flask | 105 | 98 | 7 | 0.3 s |
| fastapi | 1318 | 272 | 1046 | 1.0 s |
| django | 3255 | 2952 | 303 | 6.0 s |
| this workspace's own Python | 11 | 11 | 0 | 0.2 s |

`requests` does not move, and that is the answer a package with no web
framework, no signal receiver and no console script should get.

`flask` drops 7, and every one of them is a route handler defined INSIDE a
test: `@app.post("/")` above `def do_set`, `@app.get("/")` above `def do_get`.
Flask 3 ships an HTTP-method shortcut for each verb, so the old roster's
`@*.route` did not even cover Flask.

`fastapi` drops 1046 — 775 in `tests/` and 271 in `docs_src/`, its directory of
tutorial applications, each a handler under `@app.get` or `@app.post`.

`django` drops 303: 147 classes, 120 functions, 34 methods and 2 properties,
all of them under the Django registration roster. What remains there is a
different shape altogether: 2241 of the 2952 are unused module VARIABLES, most
of them in `django/conf/global_settings.py` and in the settings modules its test
applications carry, which Django reads by attribute at run time. Those take a
`# noqa: V107`.

This workspace's own Python is `crates/ane-embedding/convert`, and its 11
findings are unchanged: `new_ones`, a PyTorch tensor helper, twice, and nine
metadata attributes written onto a `coremltools` model. Each takes a
`# noqa: V101` or a `# noqa: V103`. PyTorch's `forward` is in `--ignore-names`;
it is the `nn.Module` protocol name, in the same class as `setUp` and `visit_*`.

## The staging contract

**Staged work** carries a `# noqa` with vulture's own code, on the line the tool
reports:

| Code | Kind |
|---|---|
| `V101` | unused attribute |
| `V102` | unused class |
| `V103` | unused function |
| `V104` | unused import |
| `V105` | unused method |
| `V106` | unused property |
| `V107` | unused variable |
| `V201` | unreachable code |

A bare `# noqa` suppresses every kind on its line. A wrong code suppresses
nothing — measured: `# noqa: V102` on an unused function still reports. Write
the reason beside the marker; the marker says the code is staged and the reason
says what lands the consumer. For a property the reported line is the
`@property` decorator, not the `def`, so the marker goes on the decorator.

Vulture reads `# noqa`. `# vulture: ignore` does nothing — measured, still
reported.

For a surface too large to annotate name by name, vulture reads a whitelist
module — a plain Python file that mentions each name — which the project owns
and passes on the command line. That is configuration the code carries, not
prose in a review.

## The rule owns its own gate

`vulture` reads `[tool.vulture]` out of the `pyproject.toml` of its working
directory, and it merges what it finds UNDER the command line. An option the run
does not state is therefore the project's to set, and one of those options turns
the whole gate off. Measured with vulture 2.14 over a probe holding one dead
function:

| the project's `pyproject.toml` | findings | status |
|---|---|---|
| none | 1 | 3 |
| `min_confidence = 100`, `make_whitelist = true` | 0 | 0 |
| `exclude = ["src"]` | 0 | 0 |
| `ignore_names = ["*"]` | 0 | 0 |
| `[project` — not TOML at all | 0 | 1, on a traceback |
| any of the four, beside `--config` | 1 | 3 |

Rows 2 to 4 are a project turning the gate off without saying so, and answering
0 findings at exit 0 while doing it. Row 5 is a project whose manifest is being
edited, and the pipe that used to end this run read that traceback as a clean
tree.

The run therefore writes a `[tool.vulture]` table of its own into its temporary
directory and passes it as `--config`. Vulture reads that file INSTEAD of the
project's, so every option is the rule's, and a `pyproject.toml` the project
cannot parse never reaches the tool. The acceptance test
`the_shipped_python_dead_code_tool_rule_reads_a_broken_manifest_as_no_surface`
holds row 5, and the run still reads that same broken file for entry points —
finding none, and reporting every name.

`--min-confidence 60` is stated for the same reason. It is vulture's own
default, so the number does not move; stating it is what stops a project from
moving it.

## How the run is shaped

The scope is `workspace` because "unused" is a whole-tree question. Passing only
the changed files makes vulture report every helper the unchanged files call.
The engine keeps only the findings in the changed files.

`--exclude` keeps the scan out of the directories a build writes — the
virtual environment, `node_modules`, `target`, `build`, `dist`, `__pycache__`,
`.tox` and `.mypy_cache`. Each pattern is written as an explicit glob because a
vulture pattern with no wildcard is matched as `*PATTERN*` against the absolute
path, and a bare `build` would then exclude any file whose path holds that word.

The run is a script rather than a pipe, because vulture has a failure status of
its own. Measured with vulture 2.14: 0 for a tree it judged clean, 3 for a tree
with findings, 2 for a command line it cannot parse, and 1 for a run that read
no file it was given — a path that is not there, a file that is not UTF-8, a
file that does not parse. A tree that holds one unreadable file BESIDE a finding
still exits 3, because the report status is written last, so status 1 means the
tool judged nothing at all. The script accepts 0 and 3, and for any other status
writes the report and the tool's stderr and exits 1, so a broken run never reads
as a clean tree.

`sed` then strips the confidence suffix vulture appends, so each line reads as
the `path:line: message` the engine parses.

Reading the reached names fails OPEN, in two steps. A file the script cannot
read is named on stderr and states nothing, and the walk carries on with the
rest; and a whole read that breaks leaves the whitelist empty. Either way the
names stay under the gate, so a failure adds findings and never takes one away.
An interpreter with no `tomllib` and no `tomli` reads no `pyproject.toml` and
reads `setup.cfg` and `setup.py` as before.

`mktemp -d` makes the directory the two generated files stand in, and
`trap 'rm -rf "$work"' EXIT` removes it. The scope is `workspace`, so this
script takes no file argument.

## The tool survey, and what vulture cannot do

The whole space was read again before this rule was changed, with each candidate
installed and driven over one probe: a package whose `__init__.py` re-exports
one name with the redundant alias, whose `pyproject.toml` names a second as a
console script, and which holds a third name nothing reaches.

| tool | latest | published | reads `__all__` | reads `X as X` | reads entry points |
|---|---|---|---|---|---|
| `vulture` | 2.16 | 2026-03-25 | yes, list or tuple | yes | no |
| `dead` | 2.1.0 | 2025-02-08 | yes | yes | `setup.py` and `setup.cfg` only |
| `deadcode` | 2.4.1 | 2024-08-09 | yes | **no** | no |
| `ruff` | 0.14.5 | — | — | — | no cross-module rule at all |
| `pylint` | 4.0.7 | 2026-08-09 | — | — | no cross-module rule at all |

`ruff` cannot answer the question. Its whole unused-* set was read from
`ruff rule --all`: `F401` unused import, `F841` and `F842` unused local,
`ARG001` to `ARG005` unused argument, `RUF059` unused unpacked variable, and
`PYI018`, `PYI046`, `PYI047`, `PYI049` for a private type that is never used.
Every one is scoped to a file or to a private name. There is no rule that asks
whether the whole project reads a name. `pylint` is in the same position, and
neither ships an option for a package's published surface.

`dead` is the one tool of the space that knows what an entry point is —
`parse_entry_points_setup_py` and `parse_entry_points_setup_cfg` — and it is
where the idea in this run comes from. It is not swapped in, for three measured
reasons. It reads no `pyproject.toml`, which is where all four corpus packages
state their entry points, so its one advantage does not fire on any of them. It
reads `git ls-files`, so it judges a tracked tree and nothing else. And it has
no decorator roster and no name roster, so every framework handler in the table
above would report.

`deadcode` reports a name behind the redundant alias — measured on the probe —
so it is behind vulture on the fact that answers flask and fastapi. It also
could not run at all under Python 3.14, raising from `find_unused_names`, and
its last release was 2024-08-09.

vulture is kept, and the run adds the one fact it does not read.

## What the fixture pair holds

The pair holds the whole contract except the SHAPE of a manifest. The fail
fixture carries one unannotated dead item of every kind, `unpublished_command`
that no manifest names, and `TestFailCase`, a class named like a test case that
inherits from nothing. The pass fixture carries the same kinds behind their
`# noqa` codes, `published_command` that the directory's `pyproject.toml` names
as an entry point, and `PassCase`, which inherits from `unittest.TestCase`.
Neither of the last two carries a marker, because the workspace already states
that each has a caller outside the tree.

What the pair cannot hold is the manifest each fact could stand in — a directory
holds one `pyproject.toml`, one `setup.cfg` and one `setup.py` — and the facts
vulture reads for itself, which need a package the flat fixture directory has no
room for. The six acceptance tests in `tests/shipped/dead_code_python.rs` drive
the shipped script over probe repositories for those, and each one names the
fact it holds.
