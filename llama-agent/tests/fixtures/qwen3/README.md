# Qwen3 chat-template golden fixtures

This directory holds the golden reference output for the canonical Qwen3
chat template's `# Tools` block. The Rust `Qwen3` strategy in
`llama-agent/src/chat_template.rs` is required to match `tool_block.txt`
byte-for-byte when given the same `(tools)` input as `render_tool_block.py`.

## Why a golden file?

The whole point of the `Qwen3` strategy is that the model was trained
against the literal output of the chat template baked into
`Qwen/Qwen3-8B`'s `tokenizer_config.json`. If our system-prompt rendering
drifts even by a space or a comma, the model sees out-of-distribution
tokens and silently degrades to "0 tool calls extracted". A "contains"
assertion can't catch that — only a byte-for-byte comparison can.

## Regenerating the golden

`tool_block.txt` was produced by piping the canonical chat template
through Jinja2 against a fixed `(tools)` list. To regenerate after a
template upgrade:

```bash
python3 -m venv /tmp/qwen3-venv
/tmp/qwen3-venv/bin/pip install jinja2
/tmp/qwen3-venv/bin/python3 render_tool_block.py > tool_block.txt
```

The script's tool list is intentionally fixed and small (two tools:
`read_file` and `write_file`) so the golden is short, hand-readable, and
covers both the basic and the multi-tool branches of the for-loop in the
chat template.

## Why no trailing newline?

The chat template's literal places `</tool_call>` directly adjacent to
`<|im_end|>` — i.e. no `\n` between them. So the *content* of the system
message ends with `</tool_call>` without a trailing newline. Test files
that paste this content into an `<|im_start|>system\n…<|im_end|>\n`
wrapper produce the byte-for-byte canonical output.
