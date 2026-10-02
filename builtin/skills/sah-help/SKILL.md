---
name: sah-help
description: Learn what SwissArmyHammer (sah) makes available and how to use it. Use when the user says "sah help", "/sah-help", "what can sah do", "what skills are there", "what tools do I have", "how do I use swissarmyhammer", "which skill do I use for X", or is new to sah. Gives a short tour of the skills, tools, subagents, validators, and CLI commands. Given a topic, explains that one topic instead.
license: MIT OR Apache-2.0
compatibility: Read-only. Requires the `skill`, `agent`, and `review` MCP tools from the sah server for live discovery (`list skill`, `list agent`, `list validators`). Reads the tool list the harness gives the agent. Changes nothing.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# sah-help

Teach the person what SwissArmyHammer (sah) makes available. Find the inventory at run time. Do not give a list from memory.

$ARGUMENTS

## Input

The argument is an optional topic.

- **No topic**: give the tour. See "The tour".
- **A topic**: explain that one topic. See "One topic". A topic is a skill name, a tool name, a task ("commit my work"), or a question ("how do I plan").

## Collect the inventory

Run these three ops. Run them at the same time.

```json
{"op": "list skill"}
{"op": "list agent"}
{"op": "list validators"}
```

- `list skill` returns every skill with its description and its source (builtin, user, or project).
- `list agent` returns every subagent the main agent can delegate to.
- `list validators` returns every review rule set and the files it applies to.

The MCP tools are in the tool list your harness gives you. Each sah tool has an `op` field. The tool description names the ops. Do not call a tool to list the tools.

From a terminal, the same inventory comes from `sah tool list` and `sah tool <name> --help`. Tell the person these commands exist.

## The tour

Keep the tour to one screen. Give details only when the person asks.

Show these sections in this order.

### 1. What sah is

Two sentences. sah gives the agent a kanban board and a set of sharp tools. The person picks the tool for the moment, in any order.

### 2. The skills

Start with the lifecycle skills. They are the front door.

| Skill | What it does |
|-------|--------------|
| `/plan` | Turns a spec or a conversation into kanban tasks |
| `/implement` | Picks up one task and drives it to green |
| `/test` | Runs the test suite and reports failures as tasks |
| `/review` | Reviews the changes and records findings on the task |
| `/commit` | Writes a clean conventional commit |
| `/finish` | Loops implement, test, review, and commit until each task is done |

Then group the other skills from `list skill` by purpose. Use the description each skill returns. Suggested groups:

- **Understand code**: `/explore`, `/code-context`, `/lsp`, `/map`, `/detected-projects`
- **Quality**: `/tdd`, `/coverage`, `/deduplicate`, `/double-check`, `/ci`
- **Track work**: `/task`, `/kanban`
- **Utilities**: `/shell`, `/make-readme`, `/check-sah`

A skill in the list that is not named above goes in the group that fits. Mark a skill from the user or project store as **custom**. Do not omit a skill that the list returns.

### 3. The tools

One line per MCP tool. Name the tool and its purpose. Name two or three of its ops as examples. Take the purpose from the tool description.

### 4. The subagents

One line per agent from `list agent`. Say what task the main agent delegates to it.

### 5. The validators

Group the rules from `list validators` by rule set. One line per set. Say which files the set applies to. Tell the person `/review` runs these rules.

### 6. The CLI

| Command | What it does |
|---------|--------------|
| `sah init` | Sets up sah for every detected coding agent |
| `sah doctor` | Diagnoses configuration and setup problems |
| `sah validate` | Checks skills and workflows for errors |
| `sah tools` | Enables or disables MCP tools |
| `sah tool list` | Lists every MCP tool with its description |
| `sah --help` | Shows the full command set |

### 7. Where things live and how to add your own

Skills, agents, and validators are markdown files.

| Kind | Project store | One item is |
|------|---------------|-------------|
| Skill | `.skills/<name>/SKILL.md` | A folder with a `SKILL.md` |
| Agent | `.agents/<name>/AGENT.md` | A folder with an `AGENT.md` |
| Validator set | `.validators/<name>/VALIDATOR.md` | A manifest plus a `rules/` folder |

Precedence, later wins: builtin, then user (home directory), then project. A project item with the same name as a builtin replaces the builtin. `sah init` re-deploys the builtins on each run and never touches items the person added.

### 8. Start here

Close with three suggestions:

- New work: `/plan <spec-file>` or `/plan` and a conversation.
- Unfamiliar code: `/explore <question>`.
- Changes ready to ship: `/review`, then `/commit`.

## One topic

1. Run `search skill` with the topic. Run `search agent` with the topic. Match the topic against the tool names too. Run the searches at the same time.

```json
{"op": "search skill", "query": "<topic>"}
{"op": "search agent", "query": "<topic>"}
```

2. Pick the best match. Then answer with:
   - **What it is**: the name and the description.
   - **When to use it**: the trigger phrases from the description.
   - **How to start it**: `/<name> <argument>` for a skill; one example call for a tool op.
   - **What it needs**: the `compatibility` line of a skill, or the required tool.
   - **What comes next**: the skill or tool a person usually uses after it.

3. To explain a skill in depth, load it with `use skill` and read its body. Explain the steps. Do not run the steps.

4. No match: say so. Show the three nearest items from the tour. Tell the person to run `/sah-help` with no topic for the full tour.

## Rules

- **Read-only.** Do not change files, the board, or configuration.
- **Live inventory only.** Never list skills, agents, or validators from memory. The lists include the person's custom items. A list from memory does not.
- **Explain, do not do.** `/sah-help commit` explains `/commit`. It does not commit.
- **Short first.** One screen for the tour. Details when asked.
