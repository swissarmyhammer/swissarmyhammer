# Generic checks — third-party CI and native providers

Use this when CI is **not** GitHub Actions or GitLab — e.g. CircleCI, Travis, Buildkite, Azure Pipelines, or Jenkins. There are two cases.

## Case A: the provider reports back to GitHub (most common)

CircleCI, Travis, Buildkite, and Azure Pipelines (via their GitHub Apps) post their results as **commit statuses** and **check runs** on GitHub. You don't need the provider's own CLI — `gh` reads them all uniformly. This is the preferred path whenever the repo has a GitHub remote.

```
git rev-parse HEAD
```

Aggregate state (the legacy commit-status rollup — `success` / `pending` / `failure`):

```
gh api repos/{owner}/{repo}/commits/<sha>/status --jq '.state, (.statuses[] | "\(.state)\t\(.context)\t\(.target_url)")'
```

Per-check detail (the Checks API — richer, includes Actions and third-party apps):

```
gh api repos/{owner}/{repo}/commits/<sha>/check-runs \
  --jq '.check_runs[] | "\(.status)\t\(.conclusion // "-")\t\(.name)\t\(.details_url)"'
```

For a PR, `gh pr checks <number>` already merges Actions and third-party checks into one view — start there.

**Reading the results:**
- `status` is `queued` / `in_progress` / `completed`; `conclusion` is `success` / `failure` / `neutral` / `cancelled` / `skipped` / `timed_out`. Apply the same contract as everywhere: not green until `completed` + `success` for the current sha.
- The `details_url` / `target_url` points at the provider's own run page. To read failure logs, follow that URL (open it / fetch it) or drop to the provider's API (Case B) — the check-runs API gives status, not the build log.

## Case B: a native provider with no GitHub integration

When results aren't on GitHub (e.g. internal Jenkins), use the provider directly:

- **CircleCI** — `circleci` CLI is mostly for local config; for run status use the API:
  `curl -H "Circle-Token: $CIRCLE_TOKEN" https://circleci.com/api/v2/project/gh/<owner>/<repo>/pipeline?branch=<branch>` then drill into `/pipeline/<id>/workflow` and `/workflow/<id>/job`.
- **Azure Pipelines** — `az pipelines runs list --branch <branch>` and `az pipelines runs show --id <id>` (requires the `azure-devops` extension and a configured org/project).
- **Jenkins** — the JSON API: `curl <jenkins>/job/<name>/lastBuild/api/json` for status, `.../lastBuild/consoleText` for the log. Match the build whose `actions[].lastBuiltRevision.SHA1` is `HEAD`.
- **Buildkite** — `bk build view` (the `bk` CLI), or the REST API `https://api.buildkite.com/v2/organizations/<org>/pipelines/<pipe>/builds?commit=<sha>`.
- **Travis** — `travis status`/`travis show` (the legacy `travis` gem), or the API at `https://api.travis-ci.com`.

## Applying the contract here

The judgment is identical to the root skill regardless of provider:

- Match the **current `HEAD` sha** — don't trust a status from an older commit.
- **pending/running ≠ passing** — wait it out.
- Read the **actual failure log** (follow `details_url` / the provider API) before naming a cause.
- Classify **transient infra vs real failure**, and re-trigger only transient ones (each provider has a "re-run"/"retry" on its run page or API).
- Watch for **skipped/manual/allow-failure** stages that make a run look green without the meaningful work having run.
