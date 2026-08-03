---
assignees:
- claude-code
position_column: todo
position_ordinal: f580
title: session/resume also drops system_prompt and extra_args on re-spawn
---
## What

While fixing `^j9rwjtx` (review fleet forks reporting 100% cold prompt-cache
reuse), the root cause found was: `session_fork.rs::spawn_forked_process`
built the forked child's `SpawnConfig` WITHOUT `.system_prompt(...)` or
`.extra_args(...)`, so the forked `claude` CLI process launched with a
DIFFERENT system prompt (Claude's stock default, not the parent's persona)
and no `--model` override — a different request prefix Anthropic's prompt
cache never matches. Fixed by adding a `Session.system_prompt` field
(persisted at `session/new` from `request.meta.system_prompt`) and reading it
plus `self.config.claude.extra_args` in a new `build_fork_spawn_config`.

`crates/claude-agent/src/session_resume.rs::restore` (the `ResumeStrategy`
impl backing `session/resume` and `session/load`) has the IDENTICAL omission:
its `SpawnConfig::builder()` call sets `session_id`, `acp_session_id`, `cwd`,
`mcp_servers`, `ephemeral`, `tools_override`, and
`.attachment(ConversationAttachment::Resume)` — but no `.system_prompt(...)`
and no `.extra_args(...)`. A resumed session (after a process restart, or an
explicit `session/load`) therefore silently reverts to Claude's stock system
prompt and loses any `--model` override, exactly like the fork bug did.

This is a correctness bug beyond caching: a resumed SAH-mode session
(reviewer, planner, implementer, etc.) loses its persona and possibly its
model tier on resume, not just its cache warmth.

## Fix shape (mirrors ^j9rwjtx's fork fix)

- In `session_resume.rs::restore`, add
  `.system_prompt(rehydrated_session.system_prompt.clone())` and
  `.extra_args(self.config.claude.extra_args.clone())` to the `SpawnConfig`
  builder, reading the live in-memory `Session` after
  `rehydrate_in_memory_session` restores it from the durable record.
- Confirm `Session.system_prompt` round-trips through
  `rehydrate_in_memory_session` (check whether `SessionRecord` — the durable,
  serialized form — carries `system_prompt` at all, since resume can run
  after a process restart with no live `Session` to read from; if not, it
  needs adding there too so a genuine restart-then-resume still gets it).
- Add a unit test mirroring `session_fork.rs`'s
  `test_fork_spawn_config_carries_parent_system_prompt` /
  `test_fork_spawn_config_carries_extra_args`, for the resume path.

## Evidence

See `^j9rwjtx`'s task comments for the empirical CLI transcript proving a
system-prompt/model mismatch between a session's original spawn and its
resume/fork breaks Anthropic's prompt cache (and, worse, can produce a
nonsensical warm hit against an unrelated cached prefix).

#bug #claude-agent