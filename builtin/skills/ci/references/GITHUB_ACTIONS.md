# GitHub Actions — `gh run`

Commands for checking CI when the repo uses GitHub Actions (`.github/workflows/`) and `gh` is on PATH and authenticated. Confirm auth with `gh auth status` if commands fail with a 401/403.

## Find the run for the current commit

```
git rev-parse HEAD
git branch --show-current
```

List recent runs with the head sha so you can match the current commit (there may be several workflows per commit — CI, Release, docs):

```
gh run list --branch <branch> --limit 10 --json databaseId,headSha,status,conclusion,workflowName,event \
  --jq '.[] | "\(.status)\t\(.conclusion // "-")\t\(.workflowName)\t\(.databaseId)\t\(.headSha[0:9])"'
```

Select the run(s) whose `headSha` equals `HEAD`. A green run on a different sha does not count.

For a pull request, the consolidated check view is simplest:

```
gh pr checks <number>          # all checks for the PR's head, incl. third-party
gh pr checks <number> --watch  # block until they finish
```

## Compact status — do this before touching logs

```
gh run view <id> --json status,conclusion,jobs \
  --jq '"RUN: \(.status) \(.conclusion // "-")", (.jobs[] | "\(.status)\t\(.conclusion // "-")\t\(.name)")'
```

This is the working view: overall state plus per-job results in a few lines.

## Wait while it runs

```
gh run watch <id> --exit-status   # streams until done; non-zero exit if it failed
```

Or poll the compact status query above. Note: `gh run view --log` / `--log-failed` only return content **after** the run completes ("logs will be available when it is complete"). Until then, rely on the `--json` status view.

A single self-hosted runner runs jobs serially, so jobs sit `queued` for a long time — that is normal, not a failure.

## Diagnose a failure

Find the failed job(s) and their failing steps:

```
gh run view <id> --json jobs \
  --jq '.jobs[] | select(.conclusion=="failure") | .name, (.steps[] | select(.conclusion=="failure") | "  FAILED: \(.name)")'
```

Read only the failed step output, filtering noise (e.g. the recurring "Node.js NN actions are deprecated" annotation):

```
gh run view --job <jobId> --log-failed 2>&1 | grep -vE "Node.js [0-9]+ actions are deprecated" | tail -60
```

(`gh run view --job <id> --log` gives the full job log; prefer `--log-failed` to jump to the failure. Both require the run to be complete.)

## Re-run a transient failure

When the failure is infrastructure (network timeout, runner blip, missing secret on the runner), re-run rather than changing code:

```
gh run rerun <id> --failed     # only the failed jobs
gh run rerun <id>              # the whole run
gh run rerun --job <jobId>     # a single job
```

## Gotchas specific to Actions

- **Skipped ≠ failed, but skipped can mean "nothing ran".** A run is `success` even when jobs were `skipped` by their `if:` condition. If a publish/deploy/matrix job that should have run was skipped, that is a config bug, not a pass — inspect the job's `if:` in the workflow YAML.
- **`conclusion` is null until done.** Use `status` (`queued`/`in_progress`/`completed`) to decide whether to wait; use `conclusion` (`success`/`failure`/`cancelled`/`skipped`) only once `status == completed`.
- **`workflow_run`-triggered workflows** (e.g. a release-app step that fires after a Release workflow) only run if the upstream run's `conclusion` was `success`. If the downstream never started, check the upstream first.
- **Logs of a re-run** attach to the same run id; re-fetch status after re-running.
