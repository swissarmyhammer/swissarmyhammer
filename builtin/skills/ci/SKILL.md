---
name: ci
description: Check that CI is green for the current work and diagnose it when it is not, and optionally fix it. Detects the CI provider in use (GitHub Actions, GitLab, or any provider that reports commit checks back to GitHub) and reads the matching reference for the exact commands. Use when the user says "check ci", "is ci passing", "did the build pass", "are the checks green", "ci status", "did my push pass", or before claiming a branch/PR is ready to merge. Pass `fix` (e.g. "fix ci", "make ci pass", "fix the build") to also repair a real failure, re-verify locally, and commit and push.
license: MIT OR Apache-2.0
compatibility: Requires a CI provider CLI on PATH — `gh` for GitHub Actions (and for third-party checks via the check-runs API), `glab` for GitLab. Falls back to the provider's web API through `gh api` when a native CLI is missing.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Check CI

Determine whether CI is **green for the current commit**, and when it is not, find out *why* and whether it's a real failure or transient infrastructure.

This skill owns the provider-agnostic judgment — what "passing" means, how to wait, how to classify a failure. The exact commands live in a per-provider reference file you read once you know which CI is in use.

## Modes

This skill has two modes. Default to **report** unless the argument asks to fix.

- **report** (default) — check, wait if needed, diagnose, and report with evidence. Never edits code. This is steps 1–6.
- **fix** — everything report does, then *repair a real failure*: apply the fix, re-verify locally with the same gate that failed, commit, and push so a fresh run starts. Triggered when the argument is (or contains) `fix` — e.g. `/ci fix`, "fix ci", "make ci pass", "fix the build". This adds step 7.

Fix mode still **diagnoses and classifies first** (steps 1–6) — never edit code to "fix" a transient infrastructure failure; re-run that instead.

{% if arguments %}
## Target

> {{arguments}}

Interpret the argument as a PR number, a branch, a run/pipeline id, or one of the mode words `wait` (poll until the run finishes) or `fix` (repair a real failure — see Modes and step 7). The argument may combine a target with a mode (e.g. `123 fix`). If it doesn't match any of those, treat it as a branch name.
{% endif %}

## The contract

CI is **passing** only when the run *for the current commit* is finished **and** successful. Anything else is not passing:

- **queued / in progress** → not passing yet. Wait, or report "still running" — never report green while it runs.
- **a stale green run from an older commit** → does not count. Always match the current `HEAD` sha.
- **green overall but a gating job was skipped** → suspicious. A run can succeed while the job that does the real work (publish, deploy, a matrix leg) was `skipped` by an `if:` condition. Confirm the jobs that *matter* actually ran.

Read the actual failed logs before diagnosing. Never guess the cause of a failure.

## Process

### 1. Detect the provider and open its reference

Look at the repo and the available CLIs, then read the matching reference file for the exact commands:

| Signal | Provider | Reference |
|--------|----------|-----------|
| `.github/workflows/*.yml` + `gh` on PATH | GitHub Actions | [GITHUB_ACTIONS.md](./references/GITHUB_ACTIONS.md) |
| `.gitlab-ci.yml` + `glab` on PATH | GitLab CI | [GITLAB.md](./references/GITLAB.md) |
| A GitHub remote, but checks come from a third party (CircleCI, Travis, Buildkite, Azure…) | external, reported as GitHub checks | [GENERIC_CHECKS.md](./references/GENERIC_CHECKS.md) |
| A native provider with no GitHub integration (Jenkins, standalone CircleCI/Azure) | that provider | [GENERIC_CHECKS.md](./references/GENERIC_CHECKS.md) |

If several apply, prefer the provider that has a usable CLI **and** recent runs for this repo. If you can't find any CI, say so plainly and stop — don't pretend to check.

### 2. Find the run for the current commit

```
git rev-parse HEAD
git branch --show-current
```

Use the reference's commands to list recent runs and select the one(s) whose head sha equals `HEAD`. There may be several workflows per commit (CI, Release, docs) — check each that matters.

### 3. Get status compactly — never dump full logs first

Use the reference's compact status query (overall state plus per-job results, a few lines). Pull logs only for the jobs that actually failed (step 5).

### 4. If it's still running, wait

Queued/in-progress is normal — a single self-hosted runner runs jobs serially, so jobs sit `queued` for a long time. That is **not** a failure. Poll on a sensible cadence (short builds: ~30s; long native/release builds: a few minutes) until every relevant run is finished. Report progress, don't declare a verdict until it finishes. On most providers, step/job logs are only retrievable **after** the run completes.

### 5. If it failed, diagnose before reporting

Read only the failing job's failing steps (commands in the reference), filter out noise (deprecation warnings, unrelated annotations), then **classify** the failure — this is the most useful thing you produce:

- **Transient infrastructure** — network timeout pulling a toolchain/dependency, runner offline or stuck `queued`, rate limit, a missing signing identity/secret on the runner, a flaky external service. The fix is usually a re-run, not a code change. Don't blame the code for these.
- **Real failure** — compile error, failing test, lint/format violation, type error, a genuinely broken step. Quote the file:line / failing test name from the log. Hand off: `/test` to fix test/lint failures, `/implement` or a direct fix for code. Don't excuse a real failure as "flaky" without evidence of non-determinism.

### 6. Report with evidence

State, for each relevant run:

- provider + workflow/pipeline name + run id (and URL)
- overall verdict: **green**, **failed**, or **still running**
- per-job results (the compact view from step 3)
- for failures: the failing step, the root-cause line from the log, and the classification (transient vs real) with the recommended next step

Only call CI "green" when you have seen a finished, successful run for the current commit, with the jobs that matter actually run (not skipped).

### 7. If in fix mode, repair and re-verify

Only when invoked in **fix** mode (see Modes), and only after steps 1–6 have produced a verdict:

- **Green already** — nothing to fix. Report green and stop.
- **Still running** — there is no failure to fix yet. Wait it out (step 4), then re-evaluate.
- **Transient infrastructure** — do not edit code. Re-run the failed job (reference command) and report it was infra. Re-running *is* the fix.
- **Real failure** — repair it:
  1. **Apply the fix.** For a small, unambiguous failure (a lint, a format violation, an obvious one-line bug) make the change directly. For anything larger, hand off — `/test` for test/lint failures, `/implement` or a direct change for code — then come back here.
  2. **Re-verify locally** by running the *same gate that failed* (the exact lint/format/test command from the log), not a weaker proxy. Never push a fix you have not reproduced as green locally. Fix every issue the gate reports — do not silence it with `#[allow(...)]`, `// eslint-disable`, skips, or by weakening the check.
  3. **Commit and push** using the project's commit conventions (use the `commit` skill / a conventional-commit message). The push starts a fresh run for the new commit.
  4. **Re-check** the run for the *new* HEAD (back to step 2). If new failures surface, repair them too. Stop when the run is green, when only transient infra remains (re-run and report), or when a failure needs a decision you can't make alone — then report and ask. Don't loop indefinitely.

Report what you changed, the local re-verification output, the new commit sha, and that a fresh run is underway (or its result if you waited).

## Rules

- **Match the current commit.** A green run on an older sha proves nothing about now.
- **Running ≠ passing.** Wait it out or say it's still running. Never imply success while jobs are queued or in progress.
- **Read the log before naming a cause.** No guessing why something failed.
- **Separate infra from code.** A re-run fixes a network blip; it will not fix a failing test. Say which one it is.
- **A skipped gating job is a red flag, not a pass.** Confirm the meaningful jobs ran.
- **Only edit code in fix mode.** In the default report mode, checking CI reports and classifies — fixing real failures is a deliberate next step (`/test`, `/implement`, or a direct change). Re-running a transient failure is fair game in either mode. In fix mode you may repair real failures, but only after diagnosing, and only after re-verifying the fix locally before you push (step 7).
- **Never fix a transient failure by editing code.** Infra blips get a re-run, not a code change — in fix mode too.
- **Never push a fix you haven't reproduced as green locally**, and never make a check pass by weakening or suppressing it.

## Examples

### Example 1: green run on the current commit

User says: `/ci`

Actions:
1. Detect GitHub Actions; open `references/GITHUB_ACTIONS.md`.
2. `git rev-parse HEAD`, list recent runs, select the CI run whose head sha matches.
3. Compact status query → run finished and successful, all jobs green.
4. Report: "CI green for `<sha>` — run `<id>`, all jobs passed."

### Example 2: a failure that is just infrastructure

User says: "did the build pass?"

Actions:
1. The run for the current commit shows one job failed.
2. Read the failing step's log per the reference → a network timeout while installing the toolchain; the real work never ran.
3. Classify as transient infrastructure, not a code problem.
4. Re-run the failed job (reference command). Report it was infra, not code, and the re-run is in progress.

### Example 3: green overall, but nothing was published

User says: `/ci` after cutting a release tag.

Actions:
1. The release run reports success — but the per-job view shows the build/host/announce jobs were `skipped` while only `plan` ran.
2. A success with the publishing jobs skipped means no artifacts were produced. Inspect the `if:` gate on those jobs to see why they skipped.
3. Report: "Release run is green but published nothing — the publishing jobs skipped. This is a release-config bug, not a passing release." Hand off to fix the gating condition.

### Example 4: fix a real failure end to end

User says: `/ci fix`

Actions:
1. Steps 1–6: detect GitHub Actions, find the run for `HEAD`, compact status → the `Clippy` job failed; read the failing step → a `clone_on_copy` lint at `src/foo.rs:38`. Classify as a real failure.
2. Fix mode, step 7: it's a small, unambiguous lint — apply the change directly (dereference instead of clone).
3. Re-verify locally with the exact gate that failed (`cargo clippy --all-targets -- -D warnings`) → clean.
4. Commit (`fix(foo): avoid clone on Copy type`) and push.
5. Report the change, the clean local clippy output, the new commit sha, and that a fresh run is underway for the new HEAD.

### Example 5: fix asked, but the failure is infrastructure

User says: "make ci pass"

Actions:
1. Steps 1–6: the failed job is a network timeout pulling the toolchain — transient infra.
2. Fix mode, step 7: do **not** edit code. Re-run the failed job (reference command).
3. Report: "Failure was infrastructure (toolchain download timeout), not code — re-running the job. Nothing to fix in the code."
