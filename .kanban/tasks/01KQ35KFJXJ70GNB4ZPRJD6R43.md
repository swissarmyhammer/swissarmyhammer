---
assignees:
- wballard
depends_on:
- 01KQ4WEHKG6E3X6ZPPBGJNRA5T
position_column: done
position_ordinal: fffffffffffffffffffffff880
title: Qwen3 (non-Coder) tool-call strategy in llama-agent — match the tokenizer chat template
---
**Belongs entirely in llama-agent.** Per the agent-client-protocol contract, `avp-common` only hands the agent an MCP tool list and expects the right thing to come back through `prompt()`. How the agent renders that tool list into the system message and how it parses tool calls out of model output is a llama-agent concern, model-keyed by `ToolParsingStrategy`.

## Source of truth: the tokenizer chat template

The authoritative spec for what Qwen3 emits is the Jinja `chat_template` in `Qwen/Qwen3-8B`'s `tokenizer_config.json`. Earlier framing of this task used "Hermes" as a label — that was sloppy, taken from a vLLM-flavored search blurb without verification. The right approach is: **read the tokenizer config first, mirror what it actually does.** Whatever wrapper tags and JSON shape it produces is what we render and what we parse — no derivative label.

The actions below describe the *shape* of the work; the *details* (tag names, escape conventions, system-message wording) come from the tokenizer config the implementer fetches before writing code.

## The gap

Today the strategy enum in `llama-agent/src/chat_template.rs` is `Default | Qwen3Coder | OpenAI | Claude`, and `detect_from_model_name` only fires `Qwen3Coder` when the model name contains both `qwen3` AND `coder`. Plain `qwen3` / `qwen3.6` falls through to `Default` — a one-size-fits-all strategy that:

- On the **input side** (`format_tools_for_template`, around line 568), renders tools in a generic format that the Qwen3-Instruct chat template wasn't trained on.
- On the **output side**, runs `JsonToolCallParser`, `XmlToolCallParser`, `FunctionCallParser` in sequence — none of which targets the wrapper Qwen3 actually emits.

Result: even when avp wires tools correctly via MCP, qwen3.6 either improvises a JSON shape (because its system message didn't show it canonical schemas in the format it was trained on) or emits canonical wrappers that the parser ignores. Both halves need work and they pair up.

## Observed (2026-04-25 qwen test run)

```
<think>
The user wants me to validate ... I need to read the file ...
</think>

```json
{"call_tool":{"arguments":"{\"file_path\": \"...sample_avp_test.rs\"}","name":"read_file"}}
```
```

The `Default` strategy missed this; the agent reported "0 tool calls executed."

## What to change

### 1. Read the tokenizer config first

Fetch `Qwen/Qwen3-8B`'s `tokenizer_config.json` (HuggingFace) and read the Jinja `chat_template`. Render a known `(messages, tools)` pair through it — either by hand or by running the tokenizer's `apply_chat_template` once. Capture the rendered string. That string is the canonical reference for what we need to emit. Note in the task what the actual wrapper tags are, what the JSON shape is, where reasoning tags fit (if any) — don't guess.

### 2. New strategy variant

Add a `Qwen3` variant to `ToolParsingStrategy`. Distinct from `Qwen3Coder` (XML), `OpenAI`, `Claude`. Detection update in `detect_from_model_name`:
- `qwen3` AND `coder` → `Qwen3Coder` (existing — more specific match wins).
- `qwen3` (anywhere — including `qwen3.6`, `qwen3-instruct`, `qwen3-30b`) without `coder` → `Qwen3`.
- All others unchanged.

### 3. Input rendering (system-message side)

When the strategy is `Qwen3`, `format_tools_for_template` produces output matching the tokenizer chat template's tool rendering byte-for-byte (modulo any agreed-on minor variations, documented in code). The implementer asserts this in tests via reference comparison — see Testing section.

### 4. Output parsing (model-output side)

When the strategy is `Qwen3`, the parser pipeline targets the wrapper Qwen3 actually emits and is robust to common model-output noise:

- **Strip reasoning blocks first.** Remove any `<think>...</think>` regions (or whatever reasoning-block delimiter Qwen3 uses, per the tokenizer config) before further parsing.
- **Find the canonical wrapper regions** (whatever the tokenizer config says — likely `<tool_call>...</tool_call>` but **verify**). Inside, parse the JSON object — accept whichever `arguments` encoding the chat template renders (object or stringified-JSON string).
- **Markdown-fence tolerance as a safety net.** Strip ```json / ``` / bare-language fences before the JSON parse attempt. Don't rely on this — once registration is correct (avp side), the canonical wrapper should be the dominant shape.
- **Fallback wrapper shapes (cheap insurance for misbehaving / fine-tuned variants).** After fence-stripping, if the JSON object looks like a tool call but uses a different wrapper, recognize it: `{"function_call": {...}}` (OpenAI legacy), `{"tool_calls": [...]}` (OpenAI array), `{"type":"tool_use","name":"...","input":{...}}` (Anthropic), `{"call_tool": {"name":"...","arguments":"<stringified>"}}` (the improvised qwen-without-schemas shape we saw in our log). `arguments` accepts either an object or a stringified-JSON string.
- **Negative cases must still return "no tool calls."** Plain narrative with no JSON, or non-tool-call JSON (e.g. `{"status":"passed"}`), MUST NOT match.

### 5. Don't break Qwen3-Coder

The existing `Qwen3Coder` strategy stays as-is. Detection precedence is `Qwen3Coder` before `Qwen3` (more specific wins).

## Testing

**Depends on:** the test-infrastructure task that fixes llama-agent's shorted-out tool-call test path (every test session today is constructed with `available_tools: Vec::new()` — no real-model run ever exercises tool rendering or parsing end-to-end). That task adds a real-model integration test using `unsloth/Qwen3-0.6B-GGUF` (the existing `TEST_MODEL_REPO`) with non-empty `available_tools`. Once that exists, the present task piggybacks on it.

**Tests for this task specifically:**

1. **Detection unit tests** in `chat_template.rs`:
   - `detect_from_model_name("Qwen3.6-27B")` → `Qwen3`.
   - `detect_from_model_name("Qwen/Qwen3-8B-Instruct")` → `Qwen3`.
   - `detect_from_model_name("unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF")` → `Qwen3Coder` (regression — more specific wins).

2. **Reference-compare input rendering.** Capture the canonical rendered string by running `Qwen/Qwen3-8B`'s actual chat template (via a Jinja engine like `minijinja`, or by running the tokenizer once and saving the output) on a fixed `(messages, tools)` pair. Check the captured string in as a golden file. Assert that our `Qwen3`-strategy `format_tools_for_template` output matches the golden byte-for-byte (modulo any documented allowed differences). This is the *only* test that proves we're emitting in-distribution prompts; "contains" assertions don't.

3. **Output parsing unit tests** for each shape we want to support:
   - Canonical (whatever the tokenizer says): `<tool_call>{"name":"read_file","arguments":{"file_path":"a.rs"}}</tool_call>` → 1 ToolCall (assuming that's the form; verify against the config).
   - With reasoning prefix: `<think>...</think>\n<wrapper>...</wrapper>` → 1 ToolCall (reasoning stripped).
   - Markdown-fenced fallback: ` ```json\n{"name":"read_file","arguments":{"file_path":"a.rs"}}\n``` ` → 1 ToolCall.
   - Improvised wrapper from our log: ` ```json\n{"call_tool":{"arguments":"{\"file_path\": \"a.rs\"}","name":"read_file"}}\n``` ` → 1 ToolCall.
   - Multiple tool calls in one response → multiple ToolCalls.
   - Negatives: `{"status":"passed","message":"ok"}` → 0; plain narrative → 0.

4. **End-to-end integration test against Qwen3-0.6B** (relies on the test-infra task landing first): a session with one real `ToolDefinition` (`read_file(path: string)`), a prompt asking the model to read a specific file, assert the model's response parses to a non-empty `Vec<ToolCall>` with the expected `name` and an `arguments` object containing the right `file_path`. This is the real correctness signal — exercises detection → input rendering → model generation → output parsing in one shot.

## Acceptance

- The test-infra prerequisite has landed (real-model tool-call test exists with non-empty tools).
- All four test categories above pass on CI.
- Existing strategies (`Default`, `Qwen3Coder`, `OpenAI`, `Claude`) regress in zero tests.
- `cargo test -p llama-agent` and `cargo clippy -p llama-agent --all-targets -- -D warnings` are clean.

## Pairs with (avp side)

`01KQ35MHFJQPMEKQ08PZKBKFY0` (validator tools wiring + in-process MCP fallback). avp's only job is putting tools in front of the agent; this task makes plain Qwen3 actually use them.

## Sources

- `Qwen/Qwen3-8B`'s `tokenizer_config.json` on HuggingFace — primary source of truth for tool rendering and parsing format.
- [Qwen Function Calling docs](https://qwen.readthedocs.io/en/latest/framework/function_call.html) — secondary, less authoritative than the tokenizer config.
- [vLLM Tool Calling](https://docs.vllm.ai/en/latest/features/tool_calling/) — for reference on what serving-layer parsers exist; we're implementing the equivalent behavior in llama-agent's parser, not depending on vLLM names. #llama-agent