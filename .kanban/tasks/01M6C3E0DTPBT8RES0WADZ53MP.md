---
assignees:
- claude-code
position_column: todo
position_ordinal: ffff9380
title: Run every release job on the self-hosted runner
---
Goal: no job in this repo runs on a GitHub-hosted runner. This matches the other Foundation packages and the shared `swissarmyhammer/workflows` workflows, which use `self-hosted`.

User decision: change the CI to run on the local runners.

Current state, measured:
- `.github/workflows/ci.yml` and `docs.yml` already use `runs-on: self-hosted`.
- `dist-workspace.toml` already maps `aarch64-apple-darwin` to `self-hosted`.
- `.github/workflows/release.yml` still has five jobs on `runs-on: "ubuntu-22.04"`: `plan`, `build-global-artifacts`, `host`, `publish-homebrew-formula`, `announce`.

Steps:
- Change those five jobs to `runs-on: self-hosted`. Use the same label as `ci.yml`.
- The runner is a Mac. Check each job for Linux-only steps: the `linuxbrew` PATH line in `publish-homebrew-formula`, `apt` or other Linux package installs, Linux paths, and the dist installer script (it must pick the macOS build). Fix each one so it works on macOS. Do not remove any job and do not change a publish gate.
- `release.yml` is hand-maintained (`allow-dirty = ["ci"]`), so edit it by hand. Keep `dist-workspace.toml` and `release.yml` consistent.
- Check `docs.yml` and any other workflow for `ubuntu-*`, `macos-*` or `windows-*` runners. Fix each.
- Do not add tests that assert on workflow text.

Verification: run `actionlint .github/workflows/*.yml`; no new errors. The release workflow has a `dry-run` tag input. Name it in the report as the real check, to run after the user pushes.

Depends on the card "Drop Linux release targets and llama.cpp leftovers from release config".

Done when: `grep -rn "runs-on" .github/workflows` shows only `self-hosted` and `${{ matrix.runner }}`, and no Linux-only step is left in a job that now runs on the Mac. #remove-llama-cpp