---
name: missing-docs-python
description: Public Python items need docstrings — checked by ruff, not by prompt.
match:
  files:
    - "**/*.py"
  project_types:
    - python
supersedes: missing-docs
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    codes="D100,D101,D102,D103,D104,D106,D107"
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    for file in "$@"; do
      if [ ! -r "$file" ]; then
        printf 'missing-docs-python cannot read %s\n' "$file" >&2
        exit 1
      fi
    done
    status=0
    ruff check --isolated --no-cache --select "$codes" --output-format json "$@" > "$work/ruff.json" || status=$?
    if [ "$status" -gt 1 ]; then
      cat "$work/ruff.json" >&2
      exit 1
    fi
    jq -r --arg codes "$codes" '($codes | split(",")) as $selected | .[]
           | .code as $code | select($selected | index($code) | not)
           | "\(.filename):\(.location.row): \(.code // "no code") \(.message)"' "$work/ruff.json" > "$work/unread.txt"
    if [ -s "$work/unread.txt" ]; then
      cat "$work/unread.txt" >&2
      exit 1
    fi
    jq -r --arg codes "$codes" '($codes | split(",")) as $selected | .[]
           | .code as $code | select($selected | index($code))
           | [.filename, .location.row, .code, .message] | @tsv' "$work/ruff.json" > "$work/reported.tsv"
    awk -F'\t' '
      function scan(file,   text, count, result) {
        count = 0
        result = (getline text < file)
        if (result < 0) {
          printf "missing-docs-python cannot read %s\n", file > "/dev/stderr"
          exit 1
        }
        while (result > 0) {
          source[file, ++count] = text
          result = (getline text < file)
        }
        close(file)
        scanned[file] = 1
      }
      {
        if (!($1 in scanned)) { scan($1) }
        head = (($1 SUBSEP $2) in source) ? source[$1, $2] : ""
        if (($3 == "D102" || $3 == "D103") && head ~ /^[ \t]*(async[ \t]+)?def[ \t]+test/) { next }
        if (($3 == "D101" || $3 == "D106") && head ~ /^[ \t]*class[ \t]+Test/) { next }
        printf "%s:%s: %s %s\n", $1, $2, $3, $4
      }' "$work/reported.tsv"
  doctor:
    check_command: "which ruff jq awk"
    check_version_command: "ruff --version"
  install:
    commands:
      - "uv tool install ruff==0.14.5"
      - "pipx install ruff==0.14.5"
---

# Missing Documentation — Python

`ruff` reports a public item without a docstring. `D1` is the group of eight
codes that make those reports. The rule names seven of them:

| code | the item it reports |
|---|---|
| `D100` | a module |
| `D101` | a class |
| `D102` | a method |
| `D103` | a function |
| `D104` | a package, which is an `__init__.py` |
| `D106` | a nested class |
| `D107` | an `__init__` |

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration.
`--no-cache` keeps ruff from writing a cache directory into the workspace.

The scope is `files` because ruff reads the files it is given.

Every measurement below was made on ruff 0.14.5.

## The eighth code, `D105`, is left out

`D105` reports a magic method. The `missing-docs` prompt rule carves out
"Obvious implementations (Display, Debug, ToString, etc.)", and a Python magic
method is that carve-out: the language gives the name, the language gives the
parameters, and the language documents what the method must do.

ruff draws the same line, and it draws it by ITSELF. Measured on one class that
holds 13 undocumented magic methods: `__init__` reports `D107`, `__new__` and
`__call__` each report `D102`, and `__str__`, `__repr__`, `__eq__`, `__hash__`,
`__len__`, `__iter__`, `__enter__`, `__exit__`, `__getattr__` and `__add__`
each report `D105`. The three methods that keep a code of their own are the
three that take the author's own parameters. Leaving `D105` out therefore
exempts a magic method whose whole signature the language fixes, and no other.

The passing fixture holds an undocumented `__str__`, `__repr__` and `__eq__`,
so a ruff release that moves one of them to a selected code fails the fixture
pair.

## Tests, which the filter carves out by the item's own NAME

The prompt rule asks for one test in particular: "Identify test items from the
structural marker on the item itself ... not from the file name or path." In
Python that marker is the NAME, and two tools define it.

- pytest collects a class whose name starts with `Test`, and a function or
  method whose name starts with `test`. Read from pytest 9.1.1:
  `python_classes = ["Test"]` and `python_functions = ["test"]`.
- unittest collects a method whose name starts with `test`. Read from the
  standard library: `unittest.TestLoader.testMethodPrefix` is `test`.

ruff has no filter on a name, and `--isolated` discards the
`per-file-ignores` entry a project holds for its own test tree. The filter in
the script therefore reads the DEFINITION LINE each finding stands on. It drops
a `D102` or `D103` whose line reads `def test...` or `async def test...`, and a
`D101` or `D106` whose line reads `class Test...`.

An asynchronous test is a test. Measured over one documented module that holds
`async def test_async`, `async def helper` and `def test_plain`: the rule reports
`D103` on `helper` alone, and it drops both test functions.

Measured over one file that holds no module docstring, `def test_foo()`,
`def helper_for_tests()`, `class TestThing` and `def test_method` inside it:
ruff reports 5 findings and the rule reports 2 — `D100` on the module, and
`D103` on `helper_for_tests`.

The filter reads no path. A helper in a test file keeps its requirement, which
is what the prompt rule asks for word for word. The same read costs a
`test_connection` function in an ordinary module its requirement, because the
name is the whole marker.

The filter needs the row of a finding to be the row of the definition, and it
is. Measured: `D101`, `D102`, `D103`, `D106` and `D107` each report at the row
of the `def` or `class` line, and at the column of the name. A decorator above
the definition does not move the row — `@property` on row 7 and `def paired` on
row 8 report row 8. `D100` and `D104` report at row 1, column 1, because a
module and a package have no definition line of their own.

A test module keeps its `D100`. A module carries no name of its own on the
line the finding stands on, and its only test marker is the FILE NAME, which
the prompt rule refuses.

## A module and a package docstring, which the prompt rule does not ask for

The prompt rule reports a function, a type, a constant and an interface. It
names no module. `D100` and `D104` therefore ask for more than the rule they
supersede, and the prompt rule sanctions that: "These exemptions yield to
stricter language-specific documentation rules." PEP 257 asks for a docstring
on every module and every package.

Measured: an empty file reports `D100`, and a `pkg/__init__.py` with no
docstring reports `D104`.

## What the tool does not carve out

The prompt rule carves out "Simple getters/setters with self-explanatory
names". Measured: an undocumented `@property def paired` reports `D102`.

`lint.pydocstyle.ignore-decorators` is not the setting to reach for. It takes
a whole decorator name, and it silences every item that carries it. Measured
with `ignore-decorators=['property']` over one class: the `@property` getter
goes silent, and the `@functools.cached_property` getter beside it still
reports. The carve-out asks for a SIMPLE getter, and the setting has no form
for "simple".

ruff reports the getter and never the accessor under it. Measured:
`@paired.setter def paired` and `@gone.deleter def gone` each report nothing,
while `@other.setter def renamed` and `@a.b.setter def deep` each report. The
decorator must name the function it stands above.

So a public getter needs a docstring, and the setter under it needs none. The
fail fixture carries the getter for that reason, and the acceptance test
`the_shipped_python_missing_docs_tool_rule_reports_every_fail_fixture_item`
holds ruff to reporting it, so the gap stays measured. The recourse is the
inline suppression at the end of this file.

## Private items, which Python carves out by the `_` prefix

`D1` reads a public name and no other. Measured: an undocumented
`_private_method` and an undocumented `_private_function` each report nothing.
This is the prompt rule's private carve-out, reproduced by the language.

## A run cannot answer zero for a broken tool

ruff states a failure in three ways, and the earlier pipe read none of them.

- **ruff exits 2 for a selector it cannot read.** A pipeline takes the exit
  status of its LAST command, and that command was `jq`, so the run exited 0
  with no output. The script holds no pipe. It writes each report to a file, and
  `set -e` makes each step's own failure the exit status of the script. A ruff
  status over 1 exits the script 1. Measured: `--select ZZ999` exits 2. A status
  of 1 is not a failure — ruff exits 0 for a clean file and 1 for a file with
  findings.

  ruff writes its own diagnostic to stderr, and the engine reads the script's
  stderr, so the agent gets ruff's words. Measured on `--select ZZ999`: ruff
  writes 0 bytes to stdout and 93 bytes to stderr, the text `error: invalid
  value 'ZZ999' for '--select <RULE_CODE>'`. The `cat` of the stdout file adds
  nothing in that case. It stands to forward a partial report ruff wrote before
  it stopped.

  ruff exits 2 for a configuration file it cannot read as well, and this script
  never meets that. `--isolated` makes ruff read no configuration file at all.
  Measured beside a `pyproject.toml` that holds `[[[ not toml`: with
  `--isolated` ruff exits 1 and judges the Python file; without `--isolated`
  ruff exits 2 and writes `Failed to parse ... pyproject.toml`.
- **ruff exits 0 for a file it cannot read.** Measured: a path that is not
  there prints `[]`, exits 0, and writes `warning: Failed to lint ...` to
  stderr. The empty report reads as a clean file. The script therefore tests
  each file it is given before it starts, and exits 1 with the name of the file
  it cannot read. The acceptance test
  `the_shipped_python_missing_docs_tool_rule_breaks_on_a_file_it_cannot_read`
  holds that behaviour.
- **ruff reports a Python file it cannot PARSE under the code
  `invalid-syntax`, and exits 1.** A filter that selects the seven codes drops
  that record, and the file then reads as clean. The script counts each record
  outside the seven codes, writes each one to stderr, and exits 1. Measured
  over a file that holds `def broken(`: the script reports no finding and exits
  1, with `invalid-syntax unexpected EOF while parsing` on stderr. The
  acceptance test
  `the_shipped_python_missing_docs_tool_rule_breaks_on_a_file_it_cannot_parse`
  holds that behaviour.

The scan in the filter reads each file ruff reports. Measured with a path that
is not there: the scan writes `missing-docs-python cannot read ...` to stderr
and exits 1. A row the scan does not hold keeps its finding, so a short read
can add a finding and can drop none.

## A run answers for the files it is given, and for no other

`ruff check` with no path argument falls back to a default target of `.`, and it
walks that whole tree. A `files`-scope script that hands `"$@"` straight to ruff
therefore answers for every Python file under the workspace root when the run
carries no file. That answer exits 0, so it reads as a measured result.

The script counts its arguments first. A count of zero exits 0 with no finding.
Measured over a probe tree of `top.py` and `deep/nested/other.py`, with no
argument: before the guard the script reported 5 findings over those two files
and exited 0; after the guard it reports none and exits 0. The acceptance test
`the_shipped_python_missing_docs_tool_rule_reads_only_the_files_it_is_given`
holds that behaviour.

`mktemp -d` makes the working directory the script writes each report into, and
`trap 'rm -rf "$work"' EXIT` removes it. The trap covers every way the script
leaves: a clean run, a finding, and a failure. Measured: five runs over one file
leave the count of directories under `TMPDIR` unchanged, and a run that exits 1
on an unparsable file leaves it unchanged as well.

## How to exempt one item

Selection in the filter is attribution, not exemption: to exempt one item,
write `# noqa: D103` on its definition line. Measured: the marker silences that
finding, and the function under it still reports.

A whole file takes `# ruff: noqa: D100, D103` at the top. The file marker needs
each exact code. Measured: `# ruff: noqa: D1` silences nothing, because `D1` is
a prefix and not a code.
