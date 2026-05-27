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

Determine whether CI is **green for the current commit**, and when it isn't, find out *why* and whether it's a real failure or transient infra.

This skill owns the provider-agnostic judgment — what "passing" means, how to wait, how to classify. Exact commands live in a per-provider reference file.

## Modes

Default to **report** unless the argument asks to fix.

- **report** (default) — check, wait if needed, diagnose, report with evidence. Never edits code. Steps 1–6.
- **fix** — everything report does, then *repair a real failure*: apply, re-verify with the same gate that failed, commit, push so a fresh run starts. Triggered when the argument is or contains `fix` (e.g. `/ci fix`, "fix ci"). Adds step 7.

Fix mode still **diagnoses and classifies first** (steps 1–6) — never edit code to "fix" transient infra; re-run that instead.

{% if arguments %}
## Target

> {{arguments}}

Interpret as: PR number, branch, run/pipeline id, or `wait`/`fix`. May combine target + mode (e.g. `123 fix`). Unmatched → treat as branch name.
{% endif %}

## The contract

CI is **passing** only when the run *for the current commit* is finished **and** successful. Anything else is not:

- **queued / in progress** → not yet. Wait, or report "still running" — never imply green while it runs.
- **stale green run from an older commit** → doesn't count. Always match current `HEAD`.
- **green overall but a gating job was skipped** → suspicious. A run can succeed while the publishing/matrix-leg job was `skipped` by an `if:`. Confirm jobs that *matter* actually ran.

Read failed logs before diagnosing. Never guess.

## Process

### 1. Detect provider, open reference

| Signal | Provider | Reference |
|--------|----------|-----------|
| `.github/workflows/*.yml` + `gh` | GitHub Actions | [GITHUB_ACTIONS.md](./references/GITHUB_ACTIONS.md) |
| `.gitlab-ci.yml` + `glab` | GitLab CI | [GITLAB.md](./references/GITLAB.md) |
| GitHub remote, checks from third party (CircleCI, Travis, Buildkite, Azure…) | external via GitHub checks | [GENERIC_CHECKS.md](./references/GENERIC_CHECKS.md) |
| Native provider with no GitHub integration (Jenkins, standalone CircleCI/Azure) | that provider | [GENERIC_CHECKS.md](./references/GENERIC_CHECKS.md) |

Multiple match → prefer the provider with both a usable CLI AND recent runs. No CI found → say so plainly and stop.

### 2. Find the run for the current commit

```
git rev-parse HEAD
git branch --show-current
```

Use the reference's commands to list recent runs and select those whose head sha equals `HEAD`. Multiple workflows may run per commit (CI, Release, docs) — check each that matters.

### 3. Compact status — never dump full logs first

Use the reference's compact query (overall + per-job, a few lines). Pull logs only for failing jobs (step 5).

### 4. Still running? Wait

Queued/in-progress is normal — a single self-hosted runner serializes jobs. **Not** a failure. Poll on sensible cadence (short builds ~30s; long native/release runs a few minutes). Report progress; no verdict until finished. On most providers step/job logs are only retrievable after the run completes.

### 5. Failed? Diagnose before reporting

Read only the failing job's failing steps. Filter noise (deprecation warnings, unrelated annotations). Then **classify**:

- **Transient infrastructure** — network timeout, runner offline/stuck queued, rate limit, missing signing identity/secret, flaky external service. Fix is usually re-run, not code. Don't blame code.
- **Real failure** — compile error, failing test, lint/format violation, type error. Quote file:line / test name from the log. Hand off: `/test` for test/lint, `/implement` or direct fix for code. Don't excuse a real failure as "flaky" without evidence of non-determinism.

### 6. Report with evidence

Per relevant run:
- provider + workflow/pipeline name + run id (URL)
- verdict: **green**, **failed**, or **still running**
- per-job results (compact view)
- failures: failing step, root-cause log line, classification (transient/real), recommended next step

Only call CI "green" with a finished, successful run for the current commit and the jobs that matter actually ran (not skipped).

### 7. Fix mode — repair and re-verify

Only in **fix** mode, only after steps 1–6:

- **Already green** — nothing to fix. Report and stop.
- **Still running** — no failure yet. Wait (step 4), re-evaluate.
- **Transient infra** — do NOT edit code. Re-run the failed job; report it was infra. Re-running *is* the fix.
- **Real failure** — repair:
  1. **Apply fix.** Small unambiguous (lint, format, obvious one-liner) → change directly. Larger → hand off (`/test`, `/implement`, or direct change), then return.
  2. **Re-verify locally** with the *exact* gate that failed (not a weaker proxy). Never push a fix you haven't reproduced as green locally. Fix every issue the gate reports — never silence with `#[allow(...)]`, `// eslint-disable`, skips, or weakening checks.
  3. **Commit + push** via the project's conventions (use `commit` skill / conventional commits). Push starts a fresh run.
  4. **Re-check** the run for the new HEAD (back to step 2). New failures → repair. Stop when green, when only transient infra remains, or when a failure needs a human decision — then report and ask. No infinite loops.

Report: what you changed, local re-verification output, new commit sha, fresh run underway (or result if you waited).

## Rules

- **Match current commit.** Green on older sha proves nothing now.
- **Running ≠ passing.** Wait or say so.
- **Read log before naming cause.** No guessing.
- **Separate infra from code.** Re-run fixes a network blip, not a failing test.
- **Skipped gating job is a red flag, not a pass.** Confirm meaningful jobs ran.
- **Only edit code in fix mode.** Report mode reports + classifies; fixing real failures is a deliberate next step. Re-running transient failures is fair in either mode. Fix mode: repair after diagnosis, re-verify locally before pushing.
- **Never fix transient by editing code.** Even in fix mode.
- **Never push a fix you haven't reproduced green locally**, never make a check pass by weakening or suppressing it.

## Examples

**Green:** `/ci`. Detect GHA, open reference. `git rev-parse HEAD`, list runs, select matching sha. Compact status → finished + success, all jobs green. Report: "CI green for `<sha>` — run `<id>`, all jobs passed."

**Infra failure:** "did the build pass?" Run for HEAD shows one job failed. Read failing step → network timeout installing the toolchain; real work never ran. Classify as transient. Re-run the failed job. Report it was infra and the re-run is in progress.

**Green but nothing published:** `/ci` after cutting a release tag. Release run reports success — per-job view shows build/host/announce were `skipped` while only `plan` ran. Inspect the `if:` gate. Report: "Release run is green but published nothing — publishing jobs skipped. This is a release-config bug, not a passing release." Hand off.

**Fix real failure end-to-end:** `/ci fix`. Steps 1–6: GHA, find run for HEAD, compact status → `Clippy` failed; read step → `clone_on_copy` lint at `src/foo.rs:38`. Real failure. Step 7: small unambiguous lint, change directly (deref instead of clone). Re-verify with `cargo clippy --all-targets -- -D warnings` → clean. Commit (`fix(foo): avoid clone on Copy type`) and push. Report change, clean clippy output, new sha, fresh run underway.

**Fix asked, infra failure:** "make ci pass". Steps 1–6: failed job is a toolchain download timeout — transient. Step 7: do NOT edit code; re-run. Report: "Failure was infrastructure (toolchain download timeout), not code — re-running the job. Nothing to fix in the code."
