



- Rename the built in standards.md to .system.md
- Other prompts that include:
  {% render "principals" %}
  {% render "coding_standards" %}
  {% render "tool_use" %}
  ... remove those includes
- When we call claude code, use `--append-system-prompt` and use rendered text of .system.md as the switch value
