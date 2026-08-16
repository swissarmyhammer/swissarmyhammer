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
    declined_head="warning: Failed to lint "
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    status=0
    ruff check --isolated --no-cache --select "$codes" --output-format json "$@" > "$work/ruff.json" 2> "$work/ruff.err" || status=$?
    if [ "$status" -gt 1 ]; then
      cat "$work/ruff.err" "$work/ruff.json" >&2
      printf 'missing-docs-python: ruff exited %s and judged no code\n' "$status" >&2
      exit 1
    fi
    while IFS= read -r line; do
      case "$line" in
        "$declined_head"*)
          printf 'sah-diagnostic: ruff could not read %s\n' "${line#"$declined_head"}" >&2
          ;;
      esac
    done < "$work/ruff.err"
    jq -r --arg codes "$codes" '($codes | split(",")) as $selected | .[]
           | .code as $code | select($selected | index($code) | not)
           | "sah-diagnostic: ruff could not measure \(.filename): \(.code // "no code") \(.message)"' "$work/ruff.json" >&2
    jq -r --arg codes "$codes" '($codes | split(",")) as $selected | .[]
           | .code as $code | select($selected | index($code))
           | [.filename, .location.row, .code, .message] | @tsv' "$work/ruff.json" > "$work/reported.tsv"
    awk -F'\t' '
      function scan(file,   text, count, result) {
        count = 0
        result = (getline text < file)
        if (result < 0) {
          printf "sah-diagnostic: missing-docs-python could not read %s, so every finding of that file stands\n", file > "/dev/stderr"
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
    check_command: "which ruff jq awk mktemp"
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

**ruff exits 2 for a selector it cannot read.** A pipeline takes the exit
status of its LAST command, and that command was `jq`, so the run exited 0
with no output. The script holds no pipe. It writes each report to a file, and
`set -e` makes each step's own failure the exit status of the script. A ruff
status over 1 exits the script 1. Measured: `--select ZZ999` exits 2. A status
of 1 is not a failure — ruff exits 0 for a clean file and 1 for a file with
findings.

ruff writes its own diagnostic to stderr, and the script forwards that file
before it exits, so the agent gets ruff's words. Measured on `--select ZZ999`:
ruff writes 0 bytes to stdout and 93 bytes to stderr, the text `error: invalid
value 'ZZ999' for '--select <RULE_CODE>'`. The `cat` of the stdout file adds
nothing in that case. It stands to forward a partial report ruff wrote before
it stopped. The script names the status beside it, so a reader learns the run
broke rather than reading ruff's line alone. Measured against a stub ruff:
status 2 and status 101 each exit the script 1, with the stub's own stderr and
`missing-docs-python: ruff exited <status> and judged no code`.

ruff exits 2 for a configuration file it cannot read as well, and this script
never meets that. `--isolated` makes ruff read no configuration file at all.
Measured beside a `pyproject.toml` that holds `[[[ not toml`: with
`--isolated` ruff exits 1 and judges the Python file; without `--isolated`
ruff exits 2 and writes `Failed to parse ... pyproject.toml`.

## A file ruff could not measure

The run reads two statements ruff makes about a file it could not measure: a
row of another code on the report, and a `Failed to lint` line on stderr. Each
one is ONE item of a run that judged every other file it was handed, so the run
answers both the same way — a line opening `sah-diagnostic:` at exit 0 — and the
two sections below state what was measured for each. `builtin/validators/README.md`
states the reason: "Do not exit nonzero for a declined item. A nonzero exit
fails the WHOLE run, so one unjudged path throws away every finding the run did
make."

The probe every measurement below was taken over holds `judged.py`, a module
with a docstring whose one function carries none, so ruff reports exactly one
`D103` on it. That finding is what a nonzero exit costs.

### A file ruff cannot parse

ruff writes a file it cannot parse onto the SAME report as a finding, under
`"code": "invalid-syntax"`, and it judges every other file of the same run
beside it. Measured with ruff 0.14.5 against the shipped command line, over
`judged.py` and a file holding `def broken(`:

| the run | the report | stderr | exit |
|---|---|---|---|
| the unparsable file alone | one `invalid-syntax` row | nothing | 1 |
| `judged.py` alone | one `D103` row | nothing | 1 |
| both together | one `D103` row AND one `invalid-syntax` row | nothing | 1 |

The third row is the whole reason. ruff read the docstrings of the file it
could parse while refusing the file it could not, so the parse failure is one
item of a run that stayed sound.

A filter that selects the seven codes drops the `invalid-syntax` row, and the
file then reads as clean. The earlier shape of this script wrote each row of
another code to stderr and exited 1 instead, which is the answer the README
refuses. Measured with that shape over the two files: nothing on stdout, one
unmarked line on stderr, exit 1 — the `D103` finding lost.

So the filter keeps the seven codes as findings and writes each row of another
code under the marker at exit 0, naming the file and the parser's own message.
ruff writes an absolute path on its report, so the line carries one:

    sah-diagnostic: ruff could not measure <repo>/broken.py: invalid-syntax unexpected EOF while parsing

Measured with the shipped script over the two files: one finding on stdout, one
marked line on stderr, exit 0. The acceptance test
`the_shipped_python_missing_docs_tool_rule_declines_a_file_it_cannot_parse`
holds both halves, and a run that lost either one fails it.

### A path ruff cannot read

ruff states a path it could not read on stderr, and it exits as it would
without that path. Measured with ruff 0.14.5, one path for each run beside
`judged.py`, against the shipped command line:

| the path | the report | stderr | exit |
|---|---|---|---|
| a path that holds no file | the `D103` row alone | `warning: Failed to lint absent.py: No such file or directory (os error 2)` | 1 |
| a file whose bytes are not UTF-8 | the `D103` row alone | `warning: Failed to lint notutf8.py: stream did not contain valid UTF-8` | 1 |
| a file with no read permission | the `D103` row alone | `warning: Failed to lint noread.py: Permission denied (os error 13)` | 1 |

Each row is a path the tool declined. ruff judges every other file the run was
handed, so neither the report nor the status carries the decline, and a script
that read the report alone let the engine read a path ruff never opened as a
clean file.

The script therefore reads ruff's stderr for a line opening
`warning: Failed to lint `, and writes what stands after that head under the
marker `builtin/validators/README.md` states, at exit 0:

    sah-diagnostic: ruff could not read absent.py: No such file or directory (os error 2)

The head is stripped as a quoted value, so the reason keeps every `: ` it
holds and a path that carries one is never cut short. The engine renders each
marked line in the report, and no file filter drops it, because a diagnostic is
about the RUN and has no path to be kept by.

A pre-flight test of the PATH is the answer this rule used to give, and it was
wrong twice over. It exited 1, which costs the run every finding it did make;
and `[ ! -r "$file" ]` cannot answer all three rows. Measured against the three
staged paths: the test is true for the path that holds no file and for the file
with no read permission, and FALSE for the file whose bytes are not UTF-8 — the
mode lets a reader open that one. Measured with the shipped script before the
fix: `judged.py` beside the absent path exited 1 with no finding, `judged.py`
beside the non-UTF-8 file exited 0 with the finding and ruff's own UNMARKED
`Failed to lint` line, which the engine drops as tool chatter, so that file read
as CLEAN.

The guard hid the other decline as well. Measured with that shape over the
absent path beside the unparsable file: exit 1, `missing-docs-python cannot read
absent.py` on stderr, and NOTHING about the parse failure — the guard ran before
ruff, so the run never learned the second file does not parse.

Three acceptance tests hold the three rows, one for each —
`the_shipped_python_missing_docs_tool_rule_declines_a_path_that_holds_no_file`,
`..._declines_a_file_it_cannot_decode` and `..._declines_a_file_it_may_not_read`.
Each stages `judged.py` beside the path, and holds the run to reporting that
finding AND to stating one diagnostic that names the path.

### The scan of the definition line, which fails open

The scan in the filter re-reads each file ruff reported, to read the definition
line the test carve-out needs. That read can fail where ruff's own read did not,
and a failed scan is one more declined item: the finding is real, and only the
carve-out is unanswerable.

The scan therefore states the file under the marker and reads no line for it. A
row the scan does not hold keeps its finding, so a short read can add a finding
and can drop none — which is what this rule's prose always claimed and its code
did not do.

The failure is reachable through the shipped pipeline, and it was measured
there. `jq @tsv` escapes a backslash, so a Python file named `back\slash.py`
reaches awk as `back\\slash.py`, which awk cannot open. Measured with the shipped
script before the fix, over that file beside `judged.py`: nothing on stdout,
`missing-docs-python cannot read <repo>/back\\slash.py` on stderr, exit 1 — the
`D103` of `judged.py` lost with it. Measured after the fix over the same two
files: three findings on stdout, one marked line on stderr, and exit 0.

    sah-diagnostic: missing-docs-python could not read <repo>/back\\slash.py, so every finding of that file stands

`@tsv` naming that file with a doubled backslash on the FINDING row as well is a
defect of its own, and `^b2kq9hy` covers it.

### Every answer in one run

Measured with the shipped script over `judged.py` handed to the run beside the
unparsable file AND all three refusing paths: one finding on stdout, four marked
lines on stderr — three reads and one measure — and exit 0. No decline costs
another, and none costs the finding, because none exits nonzero.

The two kinds of line name the file differently, and each names it the way ruff
did. ruff's stderr writes the path the command line gave it, so a read line
carries `absent.py`; ruff's report writes an absolute path, so a measure line
carries the whole path.

The broken-run gate stands beside all of it, so a ruff that refuses its command
line still never reads as a clean tree.

## A run answers for the files it is given, and for no other

`ruff check` with no path argument falls back to a default target of `.`, and it
walks that whole tree. A `files`-scope script that hands `"$@"` straight to ruff
therefore answers for every Python file under the workspace root when the run
carries no file. That answer exits 0, so it reads as a measured result.

The script counts its arguments first. A count of zero exits 0 with no finding.
Measured over a probe tree of `top.py` and `deep/nested/other.py`, with no
argument: before the guard the script reported 5 findings over those two files
and exited 0; after the guard it reports none and exits 0. The same script over
the two files reports 5. The acceptance test
`the_shipped_python_missing_docs_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two files.

`mktemp -d` makes the working directory the script writes ruff's report and
ruff's stderr into, and `trap 'rm -rf "$work"' EXIT` removes it. The trap covers
every way the script leaves: a clean run, a finding, a declined item, and a
broken tool. Measured by counting the directories directly under `TMPDIR`: five
runs over one file leave the count unchanged, a run that declines four items at
exit 0 leaves it unchanged, and a run that exits 1 on a ruff that refused its
command line leaves it unchanged.

## How to exempt one item

Selection in the filter is attribution, not exemption: to exempt one item,
write `# noqa: D103` on its definition line. Measured: the marker silences that
finding, and the function under it still reports.

A whole file takes `# ruff: noqa: D100, D103` at the top. The file marker needs
each exact code. Measured: `# ruff: noqa: D1` silences nothing, because `D1` is
a prefix and not a code.
