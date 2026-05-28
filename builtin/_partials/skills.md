---
partial: true
---

{% if available_skills.size > 0 %}
## Skills

Skills extend your capabilities. When a request matches one, load it with the `skill` tool and follow its instructions.

### Available Skills

{% for skill in available_skills %}
- **{{ skill.name }}**: {{ skill.description }} ({{ skill.source }})
{% endfor %}

- Activate: `{"op": "use skill", "name": "<name>"}`
- Search: `{"op": "search skill", "query": "<query>"}`
{% endif %}
