---
partial: true
---

{% if available_skills.size > 0 %}
## Skills

You have access to skills via the `skill` tool. When a user's request matches a skill,
use the skill tool to load the full instructions, then follow them.

### Available Skills

{% for skill in available_skills %}
- **{{ skill.name }}**: {{ skill.description }} ({{ skill.source }})
{% endfor %}

Use `{"op": "use skill", "name": "<name>"}` to activate a skill.
Use `{"op": "search skill", "query": "<query>"}` to find skills by keyword.
{% endif %}
