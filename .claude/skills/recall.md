# Recall

Search project memory for prior work, lessons, and decisions on a topic.

## Usage
```
/recall authentication
/recall MCP configuration
/recall error patterns
```

## Instructions

When the user runs `/recall`:
1. Search memory for the topic: `memory_search { query: "<topic>" }`
2. Check for prior conversations: `memory_recall { topic: "<topic>" }`
3. Check for any corrections on the topic: `memory_search { query: "<topic> correction" }`
4. Present findings organized by type:
   - **Lessons**: Things learned the hard way
   - **Decisions**: Design choices and their rationale
   - **Corrections**: Facts that were updated or superseded
   - **Prior work**: What was done before on this topic
5. Note trust levels — user-verified memories are more reliable than agent-generated ones.
6. If contradictions exist, flag them.
