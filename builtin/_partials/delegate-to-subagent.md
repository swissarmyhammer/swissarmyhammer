---
title: Delegate to subagent
description: Run the skill's work in its configured agent via the Task tool, keeping verbose output out of the main thread and allowing alternate runners (e.g. the llama-agent tool).
partial: true
---
{% if agent %}
**Run this in a subagent.** Delegate the work below to the `{{ agent }}` agent by launching it with the Task tool (`subagent_type: {{ agent }}`) — or your configured agent runner, such as the `llama-agent` tool. Pass the instructions below as the subagent's prompt and relay only its final result.

Running it in a subagent keeps the verbose output out of this conversation, and a Task-launched subagent inherits the MCP tools (`kanban`, `code_context`, …) — unlike a `context: fork` skill, which does not.
{% endif %}
