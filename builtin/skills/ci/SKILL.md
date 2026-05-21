---
name: ci
description: Check that CI is green for the current work and diagnose it when it is not. Detects the CI provider in use (GitHub Actions via `gh`, GitLab via `glab`, or any provider that reports commit checks back to GitHub) and drives the right tool. Use when the user says "check ci", "is ci passing", "did the build pass", "are the checks green", "ci status", "did my push pass", or before claiming a branch/PR is ready to merge.
license: MIT OR Apache-2.0
compatibility: Requires a CI provider CLI on PATH — `gh` for GitHub Actions (and for third-party checks via the check-runs API), `glab` for GitLab. Falls back to the provider's web API through `gh api` when a native CLI is missing.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Check CI

Determine whether CI is **green for the current commit**, and when it is not, find out *why* and whether it's a real failure or transient infrastructure.

{% if arguments %}
## Target

> {{arguments}}

Interpret the argument as a PR number, a branch, a run/pipeline id, or the word `wait` (poll until the run finishes). If it doesn't match any of those, treat it as a branch name.
{% endif %}

## The contract

CI is **passing** only when the run *for the current commit* is `completed` **and** `success`. Anything else is not passing:

- **queued / in_progress** → not passing yet. Wait, or report "still running" — never report green while it runs.
- **a stale green run from an older commit** → does not count. Always match the current `HEAD` sha.
- **green overall but a gating job was skipped** → suspicious. A run can be `success` while the job that does the real work (publish, deploy, a matrix leg) was `skipped` by an `if:` condition. Confirm the jobs that *matter* actually ran.

Read the actual failed logs before diagnosing. Never guess the cause of a failure.

## Process

### 1. Detect the CI provider

Look at the repo and the available CLIs, in this order:

| Signal | Provider | Tool |
|--------|----------|------|
| `.github/workflows/*.yml` + `gh` on PATH | GitHub Actions | `gh run` |
| `.gitlab-ci.yml` + `glab` on PATH | GitLab CI | `glab ci` |
| A GitHub remote, but checks come from a third party (CircleCI, Travis, Buildkite, …) | external, reported as GitHub checks | `gh api .../commits/<sha>/check-runs` |
| `.circleci/`, `Jenkinsfile`, `azure-pipelines.yml`, `.travis.yml`, `.buildkite/` and no GitHub checks | that provider | its CLI, else its web API |

If several apply, prefer the provider that has a usable CLI **and** recent runs for this repo. If you can't find any CI, say so plainly and stop — don't pretend to check.

### 2. Find the run for the current commit

```
git rev-parse HEAD
git branch --show-current
```

**GitHub Actions** — list recent runs and match the `headSha`:

```
gh run list --branch <branch> --limit 10 --json databaseId,headSha,status,conclusion,workflowName,event
```

Pick the run(s) whose `headSha` equals `HEAD`. There may be several workflows per commit (CI, Release, docs) — check each that matters. For a PR, `gh pr checks <number>` gives the consolidated view.

**Third-party checks** (any provider reporting to GitHub):

```
gh api repos/{owner}/{repo}/commits/<sha>/check-runs --jq '.check_runs[] | "\(.status)\t\(.conclusion // "-")\t\(.name)"'
gh api repos/{owner}/{repo}/commits/<sha>/status --jq '.state'
```

**GitLab** — `glab ci status` (current branch) or `glab ci list`.

### 3. Get status compactly — never dump full logs first

```
gh run view <id> --json status,conclusion,jobs --jq '"RUN: \(.status) \(.conclusion // "-")", (.jobs[] | "\(.status)\t\(.conclusion // "-")\t\(.name)")'
```

This is the working view: overall state plus per-job results, a few lines. Pull logs only for the jobs that actually failed (step 5).

### 4. If it's still running, wait

Queued/in-progress is normal — a single self-hosted runner runs jobs serially, so jobs sit `queued` for a long time. That is **not** a failure. Poll on a sensible cadence (short builds: ~30s; long native/release builds: a few minutes) until every relevant run is `completed`. Report progress, don't declare a verdict until it finishes. Note: `gh run view --log` / `--log-failed` only return content **after** the run completes ("logs will be available when it is complete").

### 5. If it failed, diagnose before reporting

Find the failed job(s) and read only their failing steps:

```
gh run view <id> --json jobs --jq '.jobs[] | select(.conclusion=="failure") | .name, (.steps[] | select(.conclusion=="failure") | "  FAILED: \(.name)")'
gh run view --job <jobId> --log-failed
```

Filter out noise (deprecation warnings, "Node.js NN actions" annotations) so the real error stands out. Then **classify** the failure — this is the most useful thing you produce:

- **Transient infrastructure** — network timeout pulling a toolchain/dependency, runner offline or stuck `queued`, rate limit, a missing signing identity/secret on the runner, a flaky external service. The fix is usually a re-run, not a code change: `gh run rerun <id> --failed`. Don't blame the code for these.
- **Real failure** — compile error, failing test, lint/format violation, type error, a genuinely broken step. Quote the file:line / failing test name from the log. Hand off: `/test` to fix test/lint failures, `/implement` or a direct fix for code. Don't excuse a real failure as "flaky" without evidence of non-determinism.

### 6. Report with evidence

State, for each relevant run:

- provider + workflow name + run id (and URL)
- overall verdict: **green**, **failed**, or **still running**
- per-job results (the compact view from step 3)
- for failures: the failing step, the root-cause line from the log, and the classification (transient vs real) with the recommended next step

Only call CI "green" when you have seen `completed` + `success` for the current commit, with the jobs that matter actually run (not skipped).

## Rules

- **Match the current commit.** A green run on an older sha proves nothing about now.
- **Running ≠ passing.** Wait it out or say it's still running. Never imply success while jobs are queued or in progress.
- **Read the log before naming a cause.** No guessing why something failed.
- **Separate infra from code.** A re-run fixes a network blip; it will not fix a failing test. Say which one it is.
- **A skipped gating job is a red flag, not a pass.** Confirm the meaningful jobs ran.
- **Don't auto-fix code from this skill.** Checking CI reports and classifies; fixing real failures is a deliberate next step (`/test`, `/implement`, or a direct change). Re-running a transient failure is fair game.

## Examples

### Example 1: green run on the current commit

User says: `/ci`

Actions:
1. Detect GitHub Actions (`.github/workflows/` + `gh`).
2. `git rev-parse HEAD`, then `gh run list --branch main --limit 10 --json databaseId,headSha,status,conclusion,workflowName` — find the CI run whose `headSha` matches.
3. `gh run view <id> --json status,conclusion,jobs --jq ...` → `RUN: completed success`, all jobs ✓.
4. Report: "CI green for `<sha>` — run `<id>`, all 7 jobs passed."

### Example 2: a failure that is just infrastructure

User says: "did the build pass?"

Actions:
1. Run for the current commit shows `Rustfmt` failed.
2. `gh run view --job <id> --log-failed` → `error: failed to download ... channel-rust-stable.toml ... Operation timed out`.
3. Classify: transient network timeout during toolchain install — not a formatting problem. The "Enforce formatting" step never ran.
4. Re-run the job: `gh run rerun <id> --failed`. Report it was infra, not code, and that the re-run is in progress.

### Example 3: green overall, but nothing was published

User says: `/ci` after cutting a release tag.

Actions:
1. The `Release` run shows `RUN: completed success` — but `gh run view <id> --json jobs` shows `plan` succeeded while `build`, `host`, and `announce` are all `skipped`.
2. A success with the publishing jobs skipped means no artifacts were produced. Inspect the `if:` gate on those jobs (e.g. `needs.plan.outputs.publishing == 'true'`) to see why they skipped.
3. Report: "Release run is green but published nothing — the build/host/announce jobs skipped. This is a release-config bug, not a passing release." Hand off to fix the gating condition.
