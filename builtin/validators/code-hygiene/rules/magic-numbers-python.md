---
name: magic-numbers-python
description: Unnamed Python literals need constants — checked by ruff, not by prompt.
match:
  files:
    - "**/*.py"
  project_types:
    - python
supersedes: magic-numbers
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    code="PLR2004"
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    status=0
    ruff check --isolated --no-cache --select "$code" --output-format json "$@" > "$work/ruff.json" 2> "$work/ruff.err" || status=$?
    if [ "$status" -gt 1 ]; then
      cat "$work/ruff.err" "$work/ruff.json" >&2
      printf 'magic-numbers-python: ruff exited %s and judged no code\n' "$status" >&2
      exit 1
    fi
    filtered=0
    jq -r --arg code "$code" '.[] | select(.code != $code)
           | "sah-diagnostic: ruff could not measure \(.filename): \(.code // "no code") \(.message)"' "$work/ruff.json" \
      > "$work/unmeasured.txt" || filtered=$?
    jq -c --arg code "$code" '.[] | select(.code == $code)
           | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}' "$work/ruff.json" \
      > "$work/reported.json" || filtered=$?
    if [ "$filtered" -ne 0 ]; then
      printf 'magic-numbers-python: jq could not read the ruff report\n' >&2
      exit 1
    fi
    while IFS= read -r line || [ -n "$line" ]; do
      printf 'sah-diagnostic: ruff declined an item and said: %s\n' "$line" >&2
    done < "$work/ruff.err"
    cat "$work/unmeasured.txt" >&2
    cat "$work/reported.json"
  doctor:
    check_command: "which ruff jq mktemp"
    check_version_command: "ruff --version"
  install:
    commands:
      - "uv tool install ruff==0.14.5"
      - "pipx install ruff==0.14.5"
---

# Magic Numbers — Python

`ruff` reports every unnamed numeric literal a comparison reads. `PLR2004` is the
one rule that names that check, and its own name — `magic-value-comparison` —
states the narrow scope: a literal in a comparison, and nothing else.

That scope is why this rule needs no threshold of its own. `PLR2004` has no
threshold to set: it carries a fixed value list of its own, and `ruff` gives no
option that adds a value to it.

Every measurement below was made on `ruff` 0.14.5.

## Which carve-outs the tool reproduces, and which it does not

Measured against a probe module on `ruff` 0.14.5:

- **The value list is `0`, `1` and `-1`.** `x == 0`, `x == 1`, `x == -1`,
  `x == 0.0` and `x == 1.0` are all silent. That is the first half of the
  `magic-numbers` prompt carve-out, word for word.
- **The declaration carve-out is reproduced.** `PLR2004` reads a comparison and
  nothing else, so it never reads a literal a declaration names, a default
  parameter, or an index.
- **`100` REPORTS.** The prompt rule carves out "conventional values (a `<< 8`,
  `100` for percent)", and `x == 100` reports:

      PLR2004 Magic value used in comparison, consider replacing `100` with a
      constant variable

  `x == 3600`, `x == -2` and `x > 100.0` report the same way.
- **`a << 8` is silent, and not for the prompt rule's reason.** It is silent
  because a shift is an operation and not a comparison. `x == 8` reports. So the
  shift form of the conventional carve-out survives by accident, and the percent
  form does not survive at all.

`ruff` cannot restore the `100` carve-out. `lint.pylint.allow-magic-value-types`
selects TYPES, never values — it takes `str`, `bytes`, `int`, `float` and
`complex`, and naming `int` there silences EVERY integer, which turns the rule
off rather than carving one value out. There is no `allow-magic-values` key;
`ruff` answers a config that names one with `unknown field
'allow-magic-values'`.

## The exemption is a `# noqa` on the comparison

A conventional value the review must not report carries `# noqa: PLR2004` on the
line the tool reports, which is the line the comparison stands on. Write the
reason after the code:

    if usage == 100:  # noqa: PLR2004 — a whole ratio, in percent

Measured on the same probe: that line is silent, `# noqa: PLR2004` with no
reason is silent, and a bare `# noqa` is silent. The reason is not decoration.
It says which conventional value the number is, which is the one thing `ruff`
cannot read.

That is the exemption, and it is the only one. `builtin/validators/README.md`
states the contract this rule keeps: "Selection in the pipe is attribution, not
exemption ... To exempt one code item, use an inline suppression in the code —
never the pipe." A filter step that dropped the `100` findings would be
exemption in the filter, and it would drop a genuine `status == 100` along with
the percent one, because the filter reads the value and never the meaning. The
`# noqa` reads the meaning, because the author writes it at the one site that
has one.

## This rule and `magic-numbers-dart` are the two of five that cannot allow `100`

Three of the five tools take a usable value allow-list, and the allow-list is
where the percent carve-out goes:

- `magic-numbers-swift` states `allowed_numbers: [0, 1, -1, 100]`.
- `magic-numbers-typescript` states `ignore: [0, 1, -1, 100]`.
- `magic-numbers-go` states `ignored-numbers: ["0", "1", "-1", "100"]`. The
  `mnd` key takes strings, and the values are the same four.

`ruff` and `solid_lints` are the two tools of the five that give no usable value
allow-list, and each fails in its own way. `ruff` states no allow-list key at
all, as the paragraph above measures. `solid_lints` 0.3.3 states an `allowed`
key that its own parameter parser cannot read, so `magic-numbers-dart` keeps the
built-in default `[-1, 0, 1]` and that file records the measurement.

So `x == 100` reports here, `part * 100` reports in Dart, and the divergence
belongs to the tool in each case. The `# noqa: PLR2004` above is the recourse
here: a percent comparison carries the marker and the reason, and the review
then stays silent on it. `magic-numbers-dart` states its own marker for the same
purpose.

## Where this rule is NARROWER than the rule it supersedes

`supersedes: magic-numbers` is a claim, so state its limit. `PLR2004` reads a
comparison and nothing else. A repeated literal in a call argument, in an
operation, or in a `return` is never reported — measured: `a * 100`,
`a + 3600`, `g(3600)` and `return 3600` are all silent. Repetition is the prompt
rule's primary target, so for Python this tool answers the position question and
leaves the repetition question unanswered.

That gap is real and it is the price of the trade. `mnd` reads six positions,
`no-magic-numbers` reads three, and `no_magic_number` reads every position its
own carve-out list does not exempt; `PLR2004` reads one, which makes this the
narrowest of the five `magic-numbers-*` rules. A Python reviewer gets one
comparison verdict every reviewer gets the same, in place of a repetition count
an agent makes by eye.

## How the run is shaped

`--isolated` makes ruff ignore every configuration file, so the rule owns its
whole invocation and never reads the project's own lint configuration.
`--no-cache` keeps ruff from writing a cache directory into the workspace.

The scope is `files` because ruff reads the files it is given.

`mktemp -d` makes the working directory the script writes ruff's report and
ruff's stderr into, and `trap 'rm -rf "$work"' EXIT` removes it. The trap covers
every way the script leaves: a clean run, a finding, a declined item, and a
broken tool. Measured by counting the entries directly under `TMPDIR`: five runs
over one file leave the count unchanged, a run that declines three items at exit
0 leaves it unchanged, and a run that exits 1 on a ruff that refused its command
line leaves it unchanged.

The fixture pair holds every statement above that a run can check. The failing
fixture carries `404`, `4096`, `10`, `90` and `100`, and the acceptance test
`the_shipped_python_magic_numbers_tool_rule_reports_every_fail_fixture_value`
holds the run to exactly those five: `100` proves the carve-out is absent, and
the count proves no other position reports.

## The run answers for its own arguments

A run with no path leaves ruff its default target of `.`, and `PLR2004`
then reports a comparison literal in each Python file of the tree. The
script counts its arguments first, and a count of zero exits 0 with no
finding.

Measured over two Python files, each comparing against one unnamed
literal, with no argument: 2 findings before the guard, 0 after it. The
same script over the two files reports 2. The acceptance test
`the_shipped_python_magic_numbers_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.

## A run cannot answer zero for a broken tool

**ruff exits 2 for a command line it refuses.** The shape this rule replaced was
one pipe, and a pipeline takes the exit status of its LAST command. That command
was `jq`, so the run exited 0 with no finding, and the engine read a broken tool
as a clean tree. Measured against a stub ruff that writes ruff's own refusal and
exits 2: the pipe exited 0 and wrote 0 bytes to stdout.

The script holds no pipe. It writes each channel to a file of its own, reads
ruff's status, and gates on it. A status over 1 exits the script 1. Measured
with ruff 0.14.5: `--select ZZ999` writes 0 bytes to stdout and 93 bytes to
stderr, the text `error: invalid value 'ZZ999' for '--select <RULE_CODE>'`, and
exits 2; `--output-format zzz` writes 0 bytes to stdout and 217 bytes to stderr
and exits 2 as well. A status of 1 is not a failure — measured, ruff exits 0 for
a file with no finding and 1 for a file with one.

The script forwards ruff's own stderr and ruff's own report before it exits, so
the agent gets ruff's words, and it names the status beside them. The `cat` of
the report adds nothing for the two shapes above, because each writes 0 bytes
there; it stands to forward a partial report ruff wrote before it stopped.
Measured against a stub ruff: status 2 and status 101 each exit the script 1,
with the stub's own stderr and
`magic-numbers-python: ruff exited <status> and judged no code`. The acceptance
test `the_shipped_python_magic_numbers_tool_rule_breaks_on_a_status_it_cannot_read`
holds that shape.

ruff exits 2 for a configuration file it cannot read as well, and this script
never meets that. `--isolated` makes ruff read no configuration file at all.

**A report `jq` cannot read is a broken run too, and the status gate misses
it.** The gate reads ruff's number, and ruff keeps status 1 for a file that HAS
findings, so a report ruff wrote malformed at status 0 or 1 goes straight
through. A filter that then runs bare under `set -e` takes the whole script down
with jq's own status, and the run then states no rule name at all.

So each filter step reads its own status into `filtered`, and one gate states
the break in the rule's own words. Measured with a stub ruff that exits 1 and
writes a report stopping inside its first entry: exit 1, 0 bytes on stdout, jq's
own `jq: parse error: Unfinished JSON term at EOF`, and
`magic-numbers-python: jq could not read the ruff report`. A stub ruff that
writes `{ not json` at status 0 reads the same way. `missing-docs-python`,
`missing-docs-rust` and `function-length-rust` carry this shape as well. The
acceptance test
`the_shipped_python_magic_numbers_tool_rule_breaks_on_a_report_the_filter_cannot_read`
holds it.

## A file ruff could not measure

The run reads two statements ruff makes about a file it could not measure: a row
of another code on the report, and a line on stderr. Each one is ONE item of a
run that judged every other file it was handed, so the run answers both the same
way — a line opening `sah-diagnostic:` at exit 0 — and the two sections below
state what was measured for each. `builtin/validators/README.md` states the
reason: "Do not exit nonzero for a declined item. A nonzero exit fails the WHOLE
run, so one unjudged path throws away every finding the run did make."

The probe every measurement below was taken over holds `judged.py`, a function
that compares against the unnamed literal `42`, so ruff reports exactly one
`PLR2004` on it. That finding is what a nonzero exit costs.

### A file ruff cannot parse

ruff writes a file it cannot parse onto the SAME report as a finding, under
`"code": "invalid-syntax"`, and it judges every other file of the same run
beside it. Measured with ruff 0.14.5 against the shipped command line, over
`judged.py` and a file holding `def broken(`:

| the run | the report | stderr | exit |
|---|---|---|---|
| the unparsable file alone | one `invalid-syntax` row | nothing | 1 |
| `judged.py` alone | one `PLR2004` row | nothing | 1 |
| both together | one `PLR2004` row AND one `invalid-syntax` row | nothing | 1 |

The third row is the whole reason. ruff read the comparisons of the file it
could parse while refusing the file it could not, so the parse failure is one
item of a run that stayed sound.

The `.[]` of the pipe this rule replaced carried no `select`, so it made a
FINDING of that row. Measured with that pipe over the two files: 2 findings at
exit 0, and one of them reads

    {"file":"<repo>/broken.py","line":2,"message":"invalid-syntax unexpected EOF while parsing"}

which states a magic-numbers defect ruff never reported. A filter that selects
`PLR2004` and drops the rest instead is no better: the unparsable file then
reads as clean.

So the filter keeps the `PLR2004` rows as findings and writes each row of
another code under the marker at exit 0, naming the file and the parser's own
message. ruff writes an absolute path on its report, so the line carries one:

    sah-diagnostic: ruff could not measure <repo>/broken.py: invalid-syntax unexpected EOF while parsing

Measured with the shipped script over the two files: one finding on stdout, one
marked line on stderr, exit 0. Over the unparsable file alone: nothing on
stdout, the same marked line, exit 0. The acceptance test
`the_shipped_python_magic_numbers_tool_rule_declines_a_file_it_cannot_parse`
holds both halves of the first run, and a run that lost either one fails it.

### A path ruff cannot read

ruff states a path it could not read on stderr, and it exits as it would without
that path. Measured with ruff 0.14.5, one path for each run beside `judged.py`,
against the shipped command line:

| the path | the report | stderr | exit |
|---|---|---|---|
| a path that holds no file | the `PLR2004` row alone | `warning: Failed to lint absent.py: No such file or directory (os error 2)` | 1 |
| a file whose bytes are not UTF-8 | the `PLR2004` row alone | `warning: Failed to lint notutf8.py: stream did not contain valid UTF-8` | 1 |
| a file with no read permission | the `PLR2004` row alone | `warning: Failed to lint noread.py: Permission denied (os error 13)` | 1 |
| a broken symbolic link | the `PLR2004` row alone | `warning: Failed to lint brokenlink.py: No such file or directory (os error 2)` | 1 |
| a symbolic link that points at itself | the `PLR2004` row alone | `warning: Failed to lint looplink.py: Too many levels of symbolic links (os error 62)` | 1 |
| a directory with no read permission | the `PLR2004` row alone | `warning: Encountered error: Permission denied (os error 13)` | 1 |
| a directory that holds no Python file | the `PLR2004` row alone | nothing | 1 |

Each row is a path the tool declined. ruff judges every other file the run was
handed, so neither the report nor the status carries the decline. The pipe this
rule replaced read the report alone, and the engine then read a path ruff never
opened as a clean file. Measured with that pipe over `judged.py` beside the
absent path: one finding at exit 0, and ruff's own UNMARKED
`warning: Failed to lint absent.py: No such file or directory (os error 2)` on
stderr, which the engine drops as tool chatter. The absent path read as CLEAN.

ruff writes those lines under more than one HEAD, and one head names no path at
all. `Failed to lint ` opens a line about a file ruff reached and could not
read. `Encountered error: ` opens a line about a WALK that stopped before it
reached a file to name, which is what a directory nobody may read does.
`No Python files found under the given path(s)` says the walk reached no file at
all, and a run given such a directory ALONE writes it at exit 0. So a scan
written for one head answers for the rows of that head and stays silent for
every other row.

The script therefore reads EVERY line ruff writes to stderr, and states each one
whole under the marker `builtin/validators/README.md` asks for, at exit 0:

    sah-diagnostic: ruff declined an item and said: warning: Failed to lint absent.py: No such file or directory (os error 2)
    sah-diagnostic: ruff declined an item and said: warning: Encountered error: Permission denied (os error 13)

The line is forwarded, and no head is stripped or enumerated. ruff's own words
carry the path where ruff knows one, and name what stopped the run where it does
not, so a reason that holds a `: ` of its own is never cut short and a ruff
release that writes a head this rule never met still says its piece.

A sound run writes nothing on that channel, which is what makes the whole of it
readable this way. Measured against the shipped command line, each of these
writes 0 bytes to stderr: `judged.py` alone, `judged.py` named two times,
`judged.py` beside a module that names its own limit, `judged.py` beside a text
file that is not Python, `judged.py` beside a directory that holds no Python
file, a module that names its own limit, a file carrying
`# noqa: PLR2004`, a file opening with a byte-order mark, a file holding JSON,
and a file whose one comparison stands in an `async def`. Only a run that
declined an item writes any.

The engine renders each marked line in the report, and no file filter drops it,
because a diagnostic is about the RUN and has no path to be kept by.

`while IFS= read -r line` alone loses the LAST line where no newline closes it:
`read` answers nonzero for a partial line, so a loop that reads the status alone
never runs its body for that line. Measured over two decline lines with no
closing newline: 1 line read without the partial-line arm, and 2 with it. The
loop therefore runs for a partial last line as well.

Four acceptance tests hold four rows, one for each —
`the_shipped_python_magic_numbers_tool_rule_declines_a_path_that_holds_no_file`,
`..._declines_a_file_it_cannot_decode`, `..._declines_a_file_it_may_not_read`
and `..._declines_a_directory_it_may_not_read`. Each stages `judged.py` beside
the path, and holds the run to reporting that finding AND to stating one
diagnostic. The first three hold the diagnostic to naming the path, and the
fourth to carrying ruff's own words, because the line for a directory names no
path.

### Every answer in one run

Measured with the shipped script over `judged.py` handed to the run beside the
unparsable file, a path that holds no file, a file whose bytes are not UTF-8, a
file with no read permission AND the directory nobody may read: one finding on
stdout, five marked lines on stderr — four declines and one measure — and exit
0. No decline costs another, and none costs the finding, because none exits
nonzero.

The two kinds of line name the file differently, and each names it the way ruff
did. ruff's stderr writes the path the command line gave it, so a decline line
carries `absent.py`; ruff's report writes an absolute path, so a measure line
carries the whole path.

Two gates stand beside all of it, and each one breaks the run rather than
answering zero: a ruff status over 1, and a report `jq` could not read. Neither
one reaches a ruff that REFUSED one argument and judged the rest, because that
run keeps status 1 and writes a readable report. The stderr channel is what
answers that shape, and it answers every head ruff writes there.
