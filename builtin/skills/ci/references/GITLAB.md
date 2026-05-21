# GitLab CI — `glab ci`

Commands for checking CI when the repo uses GitLab CI (`.gitlab-ci.yml`) and `glab` is on PATH and authenticated. Confirm auth with `glab auth status` if commands fail.

## Find the pipeline for the current commit

```
git rev-parse HEAD
git branch --show-current
```

The current branch's latest pipeline:

```
glab ci status            # status of the pipeline for the current branch
glab ci list              # recent pipelines, with ids and sha
```

Match the pipeline whose sha equals `HEAD`. For a merge request, `glab mr view <id>` shows the associated pipeline status.

## Compact status

```
glab ci status            # live, per-job summary for the current branch
glab ci view <pipeline-id>   # interactive job grid for a specific pipeline
```

`glab ci status` already gives the per-job pass/fail breakdown — it is the equivalent of the compact view. Use it before pulling logs.

## Wait while it runs

```
glab ci status --live     # refresh until the pipeline finishes
```

A queued pipeline (no available runner) is not a failure — wait it out.

## Diagnose a failure

Trace the failing job's log:

```
glab ci trace <job-id>    # full log for a job; pick the failed job from `glab ci view`
```

Read the tail, find the root-cause line, then classify transient infra vs real failure (see the root skill).

## Retry a transient failure

```
glab ci retry <job-id>    # retry a single failed job
glab ci retry             # retry the current branch's pipeline
```

## API fallback (no `glab`, or scripting)

When `glab` is unavailable, use the GitLab REST API via `glab api` (or `curl` with a token):

```
glab api projects/:id/pipelines?ref=<branch>           # list pipelines
glab api projects/:id/pipelines/<pipeline-id>          # one pipeline's status
glab api projects/:id/pipelines/<pipeline-id>/jobs     # per-job results
glab api projects/:id/jobs/<job-id>/trace              # a job's log
```

## Gotchas specific to GitLab

- **`manual` jobs show as not-run, not failed.** A pipeline can be green with manual jobs (deploys) deliberately not triggered — confirm whether a skipped manual job mattered.
- **`allow_failure: true` jobs** report failure without failing the pipeline. The pipeline is "passed (with warnings)" — surface the warning rather than calling it clean.
- **Stages run sequentially.** A later stage shows `created`/`pending` until earlier stages pass; that is waiting, not failure.
