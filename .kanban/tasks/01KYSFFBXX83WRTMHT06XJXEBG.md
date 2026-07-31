---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyvy3rmehewfd71xkaptvcnf
  text: |-
    Picked up by /finish ^6xjxebg. No prior attempts.

    Re-verified the defect still reproduces at HEAD: `create_tool_registry` in `apps/swissarmyhammer-cli/src/mcp_integration.rs` has 8 registrations, `register_all_tools` in `crates/swissarmyhammer-tools/src/mcp/server.rs:968-979` has 12. Missing: ralph, agent, diagnostics, skill.

    Sibling card ^t7ebyn8 is now DONE, so the Stop hook survives skill deployment and reaches `~/.skills/finish/SKILL.md`. This card is the other half — it makes the command the hook invokes actually exist. After this lands, the ralph loop should work end to end for the first time.

    Note the argument-shape difference the implementer must handle: `register_agent_tools` and `register_skill_tools` take library handles (`agent_library`, `skill_library`, `prompt_library`), so the CLI registry needs those too. The other two are plain `(&mut registry)`.

    The parity test is the real deliverable. A doc comment on `create_tool_registry` already said it "should mirror the registration in register_all_tools" and it drifted anyway. Prose did not hold; a test will.
  timestamp: 2026-07-31T11:14:21.966481+00:00
- actor: claude-code
  id: 01kyvzrdfepa8etekjzk3zdwd8
  text: |-
    Implemented. The CLI now registers the same tools as the MCP server, and `sah tool ralph ralph check --` works from the local build.

    ## What changed

    `apps/swissarmyhammer-cli/src/mcp_integration.rs`
    - `create_tool_registry` adds the four missing registrations: `register_ralph_tools`, `register_diagnostics_tools`, `register_agent_tools`, `register_skill_tools`. The last two need library handles, so the function builds a default `SkillLibrary`, `AgentLibrary` and `TemplateLibrary`, the same way `collect_all_health_checks` does in `health_registry.rs`.
    - New parity test `test_cli_tool_registry_matches_server_registry`. It compares the CLI registry against a real `McpServer` registry and fails on any difference in three directions: missing from the CLI, present only in the CLI, and registered but with no `sah tool` command. `CLI_REGISTRY_EXCLUSIONS` is an explicit list of `(tool, reason)` pairs; it is empty on purpose.

    `crates/swissarmyhammer-tools/src/mcp/tools/agent/mod.rs` and `.../skill/mod.rs`
    - Added `cli_category()`. This was necessary and the card did not know it. The default `cli_category()` derives the category from the tool-name prefix and its match arm list has no `agent` or `skill`, so both tools stayed invisible on the CLI even after registration. Registration alone is not enough; the parity test now checks CLI visibility too.

    `crates/swissarmyhammer-tools/src/lib.rs` and `.../mcp/mod.rs`
    - Re-export `register_agent_tools` and `register_skill_tools` at the crate root, beside the other ten `register_*_tools`.

    `apps/swissarmyhammer-cli/src/main.rs`
    - New `relax_required_tool_args`. This is the third blocker, also unknown to the card. Even after registration, `echo '{"session_id":"probe"}' | sah tool ralph ralph check --` failed with `error: the following required arguments were not provided: --session_id`. The schema marks `session_id` required, clap enforces it, and clap cannot see stdin — so `merge_stdin_arguments` never ran. The fix clears `required` recursively, but only inside the `tool` subcommand tree, and only when stdin is piped. Static commands keep their clap checks (`sah model use` with piped stdin still reports the missing `<NAME>`). A field absent from both the flags and stdin is still rejected by the tool's own schema validation.

    ## RED proof

    Round 1, before the registrations:
    ```
    Registered by the server, missing from the CLI: ["agent", "diagnostics", "ralph", "skill"]
    ```
    Round 2, after the registrations, before `cli_category`:
    ```
    Registered by the CLI but with no `sah tool` command (needs `cli_category`): ["agent", "skill"]
    ```
    Both green after the fix. The clap-required blocker was proved RED at the CLI, then by unit test `required_tool_arg_is_enforced_without_relaxing`.

    ## End-to-end proof, local build only

    ```
    $ ./target/debug/sah tool --help
      agent, code_context, diagnostics, files, git, kanban, question, ralph, review, shell, skill, web

    $ echo '{"session_id":"probe"}' | ./target/debug/sah tool ralph ralph check --
    decision: allow

    $ echo '{"session_id":"probe","instruction":"keep going"}' | ./target/debug/sah tool ralph ralph set --
    $ echo '{"session_id":"probe"}' | ./target/debug/sah tool ralph ralph check --
    decision: block
    iteration: 1
    reason: keep going. Iteration 1 of 50.
    ```

    ## Side effects of the four new tool families

    - Startup cost: `create_tool_registry` now loads the builtin skills and agents to build the two libraries. `sah tool --help` still answers in under 0.2 s. The MCP server already did this work, so the CLI now duplicates it once per process.
    - No new required configuration.
    - No tool needs a live server. All four run in process through the same `execute_tool` path as the other eight.
    - The CLI's skill and agent libraries are separate instances from the server's. Both load only the builtins, so they hold the same content. Nothing writes to them at run time.
    - Help text for the two new commands reads `<agents_instructions>` and `<skills_instructions>`, because that is the first line of their `description.md`. Cosmetic only.

    ## Did not work / found on the way

    - The ralph decision prints as YAML, not JSON. A Claude Code Stop hook reads JSON from stdout, so the hook still cannot read the decision. The global `--format json` flag does not reach the tool output path. New card ^634hqth.
    - The registration list now exists in three places: `register_all_tools`, `create_tool_registry` and `collect_all_health_checks`. The parity test guards the second. New card ^pwaxzy2 proposes one shared entry point.

    ## Test status

    - `cargo nextest run -p swissarmyhammer-cli`: 848 passed, 0 failed.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`: 4995 passed, 4 failed. None of the four belongs to this change:
      - `review_working_emits_progress_notifications_per_pair_when_token_supplied` — the known flake (^pxhzk1s, ^t681xdv, ^yh4m6ed).
      - `skill_e2e::test_task_skill_renders_short_id_guidance` — asserts on builtin content. The marker `last 7 characters of the ULID` now lives only in `builtin/skills/kanban/SKILL.md`, and `builtin/_partials/short-ids.md` is deleted in the working tree.
      - `kanban-app::workspace_init opening_a_board_deploys_the_kanban_tool_skills` and `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder` — both need `task` in the kanban profile. `builtin/skills/task/SKILL.md` carries no `profiles:` key, at the working tree and at HEAD.
    - `cargo fmt`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.

    Task stays in `doing`. Nothing committed.
  timestamp: 2026-07-31T11:43:07.246257+00:00
- actor: claude-code
  id: 01kyw0v03t59r8mfsnkx53wca4
  text: |-
    Adversarial review round. Verdict was REVISE with six findings. Four are fixed, two are deferred with reasons.

    ## Fixed

    **Missing `tools.yaml` pass (HIGH).** `register_all_tools` ends with `load_merged_tool_config()` + `apply_tool_config()`; `create_tool_registry` stopped at the registrations. Two consequences: the CLI offered a `sah tool <name>` command for a tool the server then refuses to execute, and the new parity test compared the server's config-filtered `list_tool_names()` against the CLI's unfiltered set — so any user with one tool disabled in `tools.yaml` would see a false drift failure. `create_tool_registry` now runs the same pass. The test only stayed green because `IsolatedTestEnvironment` hides the real config; the shipped binary had no such cover.

    **Raw XML tag as the help summary (HIGH).** `sah tool --help` showed `agent  <agents_instructions>` and `skill  <skills_instructions>`, because `McpTool::cli_about` takes the first line that is neither empty nor a markdown header, and those two descriptions open with a wrapper tag. `cli_about` now also skips a lone tag line, so both commands read as prose. New `is_wrapper_tag` helper in `tool_registry.rs` with test `test_cli_about_skips_wrapper_tag_lines`, proved RED first (`left: Some("<skills_instructions>")`).

    **`STDIN_ARGS_SUBCOMMAND` untied from the real tree (MEDIUM).** All three relax tests built a synthetic clap tree, so renaming the dynamic `tool` command would make the relaxation a silent no-op with every test still green — the exact failure mode this card exists to kill. New `stdin_args_subcommand_names_a_real_command` builds the real CLI through `CliBuilder`, asserts the constant names one of its subcommands, then runs the Stop hook's argv against it: `MissingRequiredArgument` before relaxing, success after. Real-path RED and GREEN in one test.

    **Stale comments (LOW).** `sah tool <category> <name>` was wrong — the category is only the iteration bucket, the path is `sah tool <tool_name> <noun> <verb>`. Also dropped the claim that the CLI has "no server-owned libraries", and recorded that `load_defaults()` resolves from the process working directory, not from any `working_dir` argument.

    Also softened the `relax_required_tool_args` doc. It claimed schema validation still rejects a missing field; there is no shared validator on the `execute_tool` path. The doc now says the caller loses clap's named-argument diagnostic for the `tool` tree whenever stdin is not a terminal (including `< /dev/null` and CI), and that the field surfaces as the tool's own `execute` error instead. The reviewer sampled `ralph`, `kanban` and `code_context`: all three reject.

    ## Deferred, with reasons

    **Use the server's registry instead of a second one (MEDIUM).** The reviewer is right that `McpServer::get_tool_registry()` would make drift structurally impossible and delete both `create_tool_registry` and the parity test. That is a different design from the one this card specifies, and `McpServerHandle::server()` returns `Option` — one construction path sets it to `None` — so it needs its own verification. Recorded on ^pwaxzy2, which also still owns the third copy in `health_registry.rs`.

    **`sah tool skill --help` prints the whole skill catalog (LOW).** `long_about` is the full description, which carries the dynamic `<available_skills>` block — 142 lines for `skill`, similar for `agent`. Accepted: at `--help` the catalog is the tool's real documentation and is useful. The one-line summary, which is what `sah tool --help` shows, now reads as prose.

    ## Re-verification

    - `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`: 4998 passed, 4 failed — the same four as before, none in this diff (the known review-progress flake, plus three tests that assert on builtin skill content the user is editing in parallel).
    - `cargo nextest run -p swissarmyhammer-cli`: all green.
    - `cargo fmt` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - `sah tool --help` now reads `agent  You have access to subagents...` and `skill  You have access to skills...`.
    - `echo '{"session_id":"probe"}' | ./target/debug/sah tool ralph ralph check --` still returns allow, and returns block after a `set`.
  timestamp: 2026-07-31T12:02:00.442615+00:00
- actor: claude-code
  id: 01kyw24fc687gjsrhks24x2wcj
  text: |-
    ## Independent test verification

    Commands and results:

    - `cargo nextest run -p swissarmyhammer-cli` — 849 passed, 0 failed, 0 skipped.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` — 5003 run, 4999 passed, 4 failed, 2 skipped. No failure comes from this card.
    - `cargo fmt --all -- --check` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, no warnings.

    ### Parity test proved to fail on drift

    - Removed `register_ralph_tools` from `create_tool_registry`. Test failed: `missing from the CLI: ["ralph"]`. Restored.
    - Removed `SkillTool::cli_category()`. Test failed: `Registered by the CLI but with no sah tool command (needs cli_category): ["skill"]`. Restored.

    ### `relax_required_tool_args` bounds proved

    - Piped stdin + `sah model use` — still errors on the missing `<NAME>` positional.
    - Terminal stdin (pty) + `sah tool ralph ralph check` — still errors on the missing `--session_id`.
    - Piped stdin + `sah tool ralph ralph check --` — returns `decision: allow`.

    ### Two real defects found and fixed

    1. `is_wrapper_tag` was inserted between the `McpTool` doc comment and the trait. The whole public trait documentation attached to the private helper, and `pub trait McpTool` had no docs. Moved the helper above the doc block.

    2. The added `tools.yaml` disable pass did not do what its comment claims. `get_cli_categories` and `filter_cli_tools` tested only `hidden_from_cli()`, never `disabled_tools`. A disabled tool stayed in `sah tool --help` and failed at execution with `Tool 'ralph' not found ... Available tools: [... ralph ...]`. Added `ToolRegistry::is_cli_visible`, which both accessors now use. Verified RED then GREEN with `test_disabled_tool_excluded_from_cli_command_tree`, and end to end: a disabled tool is now absent from `sah tool --help`.

    ### Failures not caused by this card

    - `workspace_init::opening_a_board_deploys_the_kanban_tool_skills` and `state::tests::test_open_board_deploys_kanban_tool_skills_at_board_folder` — both need the `task` skill in the `kanban` profile. `builtin/skills/task/SKILL.md` has no `profiles:` key, in the working tree and at HEAD.
    - `skill_e2e::test_task_skill_renders_short_id_guidance` — the marker text is only in `builtin/skills/kanban/SKILL.md`. `builtin/_partials/short-ids.md` is deleted in the working tree.
    - `review_working_emits_progress_notifications_per_pair_when_token_supplied` — the known flake (^pxhzk1s, ^t681xdv, ^yh4m6ed). It passed in an earlier full run and passes alone.

    The 2 skipped tests are `#[ignore]`d heavy real-model proofs in `llama-agent` (`kv_prefix_reuse_recurrent`). They predate this card.
  timestamp: 2026-07-31T12:24:39.558110+00:00
position_column: doing
position_ordinal: '8280'
title: CLI tool registry drifted from register_all_tools (sah tool ralph is missing)
---
The CLI builds its own tool registry and it no longer matches the MCP server registry. Four tool families are absent from the CLI, so their `sah tool <name>` commands do not exist.

## Symptom

```
$ echo '{"session_id":"x"}' | sah tool ralph ralph check --
error: unrecognized subcommand 'ralph'
```

`sah tool --help` lists 8 tools: `code_context` `files` `git` `kanban` `question` `review` `shell` `web`.

This breaks the ralph Stop hook, whose command is `sah tool ralph ralph check --`.

## Cause

`apps/swissarmyhammer-cli/src/mcp_integration.rs:143-152` — `create_tool_registry` makes 8 registrations. Its own doc comment says it "should mirror the registration in `swissarmyhammer_tools::mcp::server::register_all_tools`". It drifted.

`crates/swissarmyhammer-tools/src/mcp/server.rs:968-979` — `register_all_tools` makes 12. The CLI omits:

- `register_ralph_tools`
- `register_agent_tools`
- `register_diagnostics_tools`
- `register_skill_tools`

The `ralph` tool itself is correct. `cli_category()` returns `Some("ralph")` and `cli_name()` returns `"ralph"`, so it becomes a CLI subcommand as soon as it is registered.

## Required change

1. Add the 4 missing registrations to `create_tool_registry`. Match the argument shapes that `register_all_tools` uses — `register_agent_tools` and `register_skill_tools` take library handles.
2. Add a parity test that compares the tool-name set from `create_tool_registry` against the set from `register_all_tools` and fails on any difference. A comment that says "should mirror" did not hold. A test will.

If a tool must stay out of the CLI on purpose, the parity test must name it in an explicit exclusion list with a reason. Silence is what caused this.

## Acceptance

- The parity test fails before the change and passes after it.
- `echo '{"session_id":"probe"}' | sah tool ralph ralph check --` returns the block/allow JSON, not a clap error.
- `sah tool --help` lists `ralph`, `agent`, `diagnostics`, and `skill`.

Note: the hook is also stripped at skill-install time, in a separate card. Both cards are needed before the Stop hook works. #bug #cli #ralph