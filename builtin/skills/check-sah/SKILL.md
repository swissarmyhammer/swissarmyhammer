---
name: check-sah
description: Monitor the performance of a sah /finish batch (the kanban implement→test→review→commit pipeline) running in a given project folder — locate the active finish session, report progress, scan for errors, quantify token usage by source, and surface sah tooling errors and behavior changes. Takes a project folder as its argument. Use when the user says "check sah", "watch the run", "monitor the finish run", "fire up the timer", "is the run healthy", "analyze token usage", or wants periodic status on a finish batch in a folder. Drives a self-paced 10-minute watch loop until the run completes.
license: MIT OR Apache-2.0
compatibility: Read-only observability skill. Requires the `shell` and `kanban` MCP tools (to read the board and run analysis over transcripts); reads Claude Code session transcripts under `~/.claude/projects/<slug>` derived from the given project folder.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# check-sah

Watch a running sah `/finish` batch from the *outside* — read its transcripts and kanban board, not its conversation — and report progress, errors, token cost, and the health of the review engine. Runs as a 10-minute self-paced loop (via `ScheduleWakeup`) until the board is clear or finish goes idle, then emits a final summary.

**You are an observer.** Never edit the project's code, never touch the finish run's board/ralph, never spawn agents. Read-only: `mcp__sah__shell` (analysis scripts), `mcp__sah__kanban get board`, file reads.

## Input: the project folder

This skill takes **one argument — the absolute path of the project folder** sah is running in (e.g. `/Users/me/src/myrepo`). Everything else is derived from it. If no folder is given, ask for one (or default to the current working directory).

```bash
REPO="${ARG:?pass the project folder sah is running in}"        # the folder argument
# Claude Code stores transcripts under ~/.claude/projects/<slug>, where <slug> is
# the absolute repo path with '/' (and '.'/space) replaced by '-'. Leading '/' -> leading '-'.
SLUG=$(echo "$REPO" | sed 's#[/. ]#-#g')
TRANSCRIPTS="$HOME/.claude/projects/$SLUG"
# Fallback if the slug doesn't resolve: list and match by basename
[ -d "$TRANSCRIPTS" ] || TRANSCRIPTS="$HOME/.claude/projects/$(ls "$HOME/.claude/projects/" | grep -i "$(basename "$REPO")" | head -1)"
```

Pin `REPO` and `TRANSCRIPTS` for the whole run and pass them forward in every scheduled wake-up.

## Layout of what you're reading

- `$TRANSCRIPTS/<uuid>.jsonl` — a session (the orchestrator for `/finish`, `/plan`, etc.).
- `$TRANSCRIPTS/<uuid>/subagents/agent-*.jsonl` — that session's delegated subagents (implementer/tester/reviewer/committer/double-check).
- `$TRANSCRIPTS/019*-*.jsonl` (top-level) — the **review engine** fans out into *separate* top-level sessions (content contains `Files under review` / `current contents`). These are NOT under the finish session's `subagents/` dir and are easy to miss when summing tokens.
- Each `/finish` review is scoped to that checkpoint's commit delta (`sha HEAD~1..HEAD`) and content-batched by `batch_size` (default 32 KB). There is **no per-file hash store / skip-hash cache** to inspect — the old `.validators/.hashes/` incremental-tracking subsystem was removed, so there is nothing on disk that records what was reviewed.
- The project repo is typically reset between runs; per-task `/finish` commits are **local, never pushed**.

## Locate the active finish session

A run usually starts as `/plan` (builds kanban tasks) → then `/finish` (drives them). Identify a finish session by the **finish skill loading** into it — not by a `/finish` slash-command marker (that only matches an explicit slash invocation, and misses skill-tool / programmatic loads). When any skill loads, the harness injects a preamble line:

```
Base directory for this skill: <...>/skills/finish
```

That line appears exactly once, when the skill is loaded as the session's active skill, regardless of how it was invoked. Match on it — and anchor on the **skill path** so a mere prose mention of "finish" (e.g. in a `/plan` session) doesn't false-positive:

```bash
cd "$TRANSCRIPTS"
for f in $(find . -maxdepth 1 -name '*.jsonl' -mmin -20); do
  grep -q 'Base directory for this skill: .*/skills/finish' "$f" 2>/dev/null \
    && echo "$f $(wc -l <$f)L $(stat -f %Sm -t %H:%M:%S $f)"
done
```

(The same pattern with `/skills/<name>` locates any other skill — e.g. `/skills/plan` for the planning session.) EXCLUDE your own monitoring session and any prior completed runs (track their ids across cycles). If no finish session exists yet, report the `/plan` (`skills/plan`) + board progress and reschedule. Pin the finish session id. Record the **run start time** — you need it as the mtime cutoff to scope review sessions to this run.

## Per-cycle checks

Run every wake-up (keep each shell command **< 4000 chars**; split into multiple calls; avoid escaped quotes inside `python3 -c`):

1. **Liveness & progress** — `date +%H:%M:%S`; finish lines + mtime; **subagent mtimes** (the orchestrator file looks *stale* while blocked waiting on a subagent — judge liveness from `subagents/*.jsonl` mtimes, not just the orchestrator). `mcp__sah__kanban get board` for done/in-flight/blocked. Agent count + roles + tail → current task, review-fix loops, scope violations. **Confirm finish stays SEQUENTIAL** (flag parallel Agents or inline Edit/Write/test/commit by the orchestrator).
2. **Error scan** — count `"is_error":true` across finish + subagents; count `security check failed` blocks. Classify new ones (see Benign patterns). Flag NEW/fatal.
3. **Token quant** (below).
4. **sah tool errors & behavior** (below).
5. **Review-rule health** (below) — contradictions, invalid-code findings, declined/force-closed findings, churn, stuck tasks.

## Token quant (cache_creation + cache_read = the cost driver)

Sum **scoped to this run** (mtime-filter the review sessions — a naive `019*` glob counts every prior run in the folder):
- **Review engine**: mtime-filtered top-level `019*-*.jsonl` whose content has `Files under review`/`current contents`.
- **Subagent roles**: classify each `<finish>/subagents/*.jsonl` by its first line — `implement`/`address`→implementer, `test suite`→tester, `review`→reviewer, `commit`→committer, `adversari`/`verify a`→double-check.
- **Orchestrator**: the finish session file itself (surprisingly large — re-reads its growing transcript each turn).
- **GRAND** = review + subagents + orch. Report per-task.

**Calibration** (reference, from an 8–9 task greenfield build — adjust per project): healthy ≈ **9–10.5M tokens/task**; flag a run drifting toward **15M+/task** as a regression to investigate. Three "big rocks" ≈ 80% of spend: **review engine** (~35–39%), **implementer** (~22–28%), **orchestrator** (often ~11–18M, roughly fixed regardless of task count). Treat the first run you observe as the baseline; flag later runs that deviate.

## sah tool errors & behavior

The point of this skill is to surface how the **sah tooling itself** behaves under a real run. Each cycle, tabulate **every `mcp__sah__*` tool call by tool + op**, and surface **any that errored** — across finish + its subagents (and, if relevant, the review-engine sessions). This catches regressions and behavior changes after a tooling merge, not just the run's progress.

```python
import json,glob
from collections import Counter
TOOLPREFIX='mcp__sah__'
ops=Counter(); errs=[]
for fn in glob.glob('<FINISH_ID>/subagents/*.jsonl')+['<FINISH_ID>.jsonl']:
    pend={}                        # tool_use_id -> (tool, op, input)
    for line in open(fn):
        try:o=json.loads(line)
        except:continue
        c=o.get('message',{}).get('content')
        if not isinstance(c,list): continue
        for x in c:
            if x.get('type')=='tool_use' and x['name'].startswith(TOOLPREFIX):
                op=x['input'].get('op','?'); ops[(x['name'],op)]+=1
                pend[x['id']]=(x['name'],op,x['input'])
            if x.get('type')=='tool_result' and x.get('is_error'):
                t=pend.get(x.get('tool_use_id'))
                if t:
                    r=x.get('content'); r=' '.join(z.get('text','') for z in r if isinstance(z,dict)) if isinstance(r,list) else str(r)
                    errs.append((t[0],t[1],json.dumps(t[2])[:120],str(r)[:160]))
for k,v in ops.most_common(): print(k,v)
print('--- sah tool errors ---')
for tool,op,inp,err in errs: print(f'{tool} {op} | {inp} -> {err}')
```

For each error report the **exact tool + op + input + result**, and decide: a **known/benign** pattern (see below), or a **new/real** tooling defect worth flagging. Compare op usage to the prior run's profile to catch behavior shifts (e.g. an op suddenly used heavily, or one that used to work now erroring).

Known tooling issues to recognize (examples, not exhaustive):
- **`code_context`** — `get callgraph`/`get blastradius`/`get symbol` failures surface as a misleading `-32603: invalid regex pattern`: `get callgraph {symbol:X}` on an unindexed symbol → `invalid regex pattern: symbol not found: X` (a plain not-found, mislabeled); `get blastradius {file_path:F}` on a file with no indexed symbols → `invalid regex pattern: no symbols found in file 'F' matching '*'` (real bug — the "all symbols" path compiles glob `*` as a regex).
- **`kanban`** — `delete column: missing id` / `get task: missing id` (parse retries); `init board: already exists`.
- **`shell`** — `security check failed` false-positives (see Benign patterns); `Unknown operation 'detect projects'` when `detect projects` is mis-routed to `shell` instead of `code_context`.

## Review-rule health (the agent should obey findings, not fight them)

The finish agent must **obey** every review finding (fix the code) or — only for a genuine contradiction / impossibility — **report it and park the task stuck** for a human to fix the rule. It must **never** dismiss a finding, rewrite a validator to silence one, or force a task to `done` with findings open. Each cycle, watch for these and surface each with the offending finding quoted verbatim and the likely validator named:

1. **Force-closed / declined findings — cardinal violation, flag hard.** The orchestrator moved a task to `done` (`complete task`, or `move task … done`) while its latest `/review` was non-clean, or prior `- [ ]` items were still unchecked. Tell-tale prose around the close: "decline", "exercise (orchestrator) judgment", "review-churn", "pedantic", "no bonus refactoring", "I'll close/exercise judgment". This is the behavior the updated finish/review skills forbid; report the close **and** the open findings it skipped.
2. **Genuine bad rule — a finding that demands invalid code or fights a deliberate contract.** e.g. `null`→`undefined` where `tsc` needs `T | null`; `snake_case`→camelCase on a parameter mirroring a backend/IPC payload; any suggestion that wouldn't compile. Here the agent is *right* to refuse to hand-apply it — but the correct resolution is a **human-made builtin validator fix**, not a silent close. Surface as a "candidate validator bug" naming the rule (`js-ts/api-design` undefined-over-null, `naming/naming-consistency`, etc.).
3. **Contradictory findings.** Two findings on the same file/lines whose fixes are mutually exclusive. NB: `data-driven`/reuse (consolidate) vs `function-length` (split) is **not** a true contradiction — a spec-table + generator satisfies both; if the agent declined citing that "conflict," that is an agent failure, not a rule bug. A true contradiction is two rules that genuinely cannot both hold.
4. **Review-churn / non-convergence.** Re-review of byte-identical (or trivially-changed) content surfaces *new* findings each round instead of converging to zero — rising finding `counts` across dated `## Review Findings` sections, or fresh findings on unchanged lines. This is review-engine instability (non-deterministic fan-out, no suppression of already-declined items) and is what pressures the agent into force-closing.
5. **Stuck tasks.** A task that hit the step-7 guardrail (same finding ×3) or has sat in `review` many iterations on the same finding. Under the current skill this should be **parked stuck with the contradiction reported** — confirm that happened and surface it as a human-actionable rule-fix candidate; flag hard if the agent force-closed it instead.

Detection: diff the dated `## Review Findings (...)` sections on each in-`review` task round-over-round (from `get task` and the orchestrator's `update task` calls); pull each `review` tool_result's `counts`; and check every `complete task` / `move task→done` against the immediately-preceding review verdict and the task's unchecked boxes. Classify each into one of the five above; (1) and a force-closed (5) are agent disobedience, (2)–(3) are validator bugs to escalate, (4) is engine instability.

## Stop & final summary

STOP (do not reschedule) when the board is fully clear (all tasks `done`) **or** finish did `clear ralph` / went idle (subagents *and* orchestrator not advancing across two checks). Final summary:
- **GRAND tokens by source** + per-task, vs the run baseline.
- **sah tool** op breakdown (every `mcp__sah__*` tool/op) + any errors (exact tool + op + input + result).
- All commits (`git -C "$REPO" log --oneline`).
- Full error list (classified), review-fix loops, committer slowness, scope adherence.
- **Review-rule health**: every force-closed/declined finding (the close + the open findings it skipped, quoted), candidate validator bugs (invalid-code / contract-fighting findings, naming the rule to fix in `builtin/validators/…`), true contradictions, churn/non-convergence, and stuck tasks correctly parked for a human rule-fix — classified as agent-disobedience vs validator-bug vs engine-instability.
- **Standing token-saving recs**: (1) review-engine — cut per-round fan-out and tune `batch_size` so each per-commit `HEAD~1..HEAD` review packs files efficiently (the review is already commit-delta-scoped — there is no hash cache left to skip unchanged files, so the levers are fan-out width and batch packing, not re-review suppression); (2) orchestrator slimming (large fixed overhead); (3) committer — stop re-running clippy/nextest/fmt on an already-verified tree (commit inline); (4) test skill — empty suite ≠ failure, never author tests to force green; (5) `get-lines` — shell `execute` should inline small output instead of a mandatory 2nd call (median output ~13 lines; ~46% of shell calls were paging); (6) fix the security-guard false-positive (see below) and the `code_context` "invalid regex pattern" mislabel.

## Benign / known error patterns

Recovered, not fatal: kanban `init board: already exists`; kanban `delete column: missing id` (column-reorder retry); shell `security check failed` on an **`eval`** substring when the project under test *is* an evaluator (e.g. `cargo test eval`) — the same guard also legitimately blocks real `rm -rf /tmp/...`; `Edit` "File has been modified since read" / "not been read yet" (re-read + retry); `detect projects` mis-routed as a `shell` op; `code_context` "invalid regex pattern" (above).

## Scheduling

Self-pace with `ScheduleWakeup` at **600s** (10 min), passing this skill's monitoring instructions + the run state (REPO, TRANSCRIPTS, finish id, run start time, prior-run baselines, last cycle's numbers) back as the prompt each turn. Don't reschedule once the run is done.

## Shell gotchas

- **Never** put the literal `Dynamic code eval`+`uation` phrase in a command — the shell security guard false-positive-blocks any command containing it (it will block your own analysis command). Use `grep 'security check failed'` to count blocks.
- Keep commands **< 4096 chars**; split long analyses; avoid escaped `\"` inside `python3 -c`.
- Transcript timestamps are **UTC** (= local + 5h).
- The board often resets to empty (`todo→doing→done`) between runs; the `review` column gets appended later and can momentarily land terminal — finish fixes the column order autonomously (benign).
