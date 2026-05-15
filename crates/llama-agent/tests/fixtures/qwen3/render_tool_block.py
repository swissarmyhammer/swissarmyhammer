#!/usr/bin/env python3
"""Render *only* the canonical Qwen3 `# Tools` block content.

This is what `format_tools_for_qwen3` should produce in Rust. We render
through Jinja2's `tojson` filter to match exactly what the canonical
Qwen3 chat template emits — including its alphabetical key ordering
(Jinja2's `tojson` calls `json.dumps(sort_keys=True)`).

Output (without trailing newline) becomes the Rust golden file.
"""

import json
from jinja2 import Environment, BaseLoader


# A tiny one-shot template that emits exactly the same bytes the Qwen3
# chat template's tool-block branch emits — minus the surrounding
# `<|im_start|>system\n` and `<|im_end|>\n` markers, which are added by
# the chat template wrapper (here: by `apply_chat_template` in production,
# by manual concatenation in tests).
TOOL_BLOCK_TEMPLATE = (
    '{{- "# Tools\\n\\nYou may call one or more functions to assist with the user query.\\n\\nYou are provided with function signatures within <tools></tools> XML tags:\\n<tools>" }}'
    '{%- for tool in tools %}'
    '{{- "\\n" }}'
    '{{- tool | tojson }}'
    '{%- endfor %}'
    '{{- "\\n</tools>\\n\\nFor each function call, return a json object with function name and arguments within <tool_call></tool_call> XML tags:\\n<tool_call>\\n{\\"name\\": <function-name>, \\"arguments\\": <args-json-object>}\\n</tool_call>" }}'
)


def main():
    env = Environment(loader=BaseLoader())
    template = env.from_string(TOOL_BLOCK_TEMPLATE)
    tools = [
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file from the filesystem",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute path to the file to read",
                        }
                    },
                    "required": ["path"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write text to a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"},
                    },
                    "required": ["path", "content"],
                },
            },
        },
    ]
    rendered = template.render(tools=tools)
    print(rendered, end="")


if __name__ == "__main__":
    main()
