# Memory Health

Check the health and status of project memory.

## Usage
```
/memory-health
```

## Instructions

When the user runs `/memory-health`:
1. Get memory stats: `memory_stats {}`
2. View working memory: `memory_working { action: "view" }`
3. Check for contradictions: `memory_contradictions { action: "list" }`
4. Run health check: `memory_maintain { tasks: ["health"] }`
5. Present a summary:
   - Total chunks, entities, categories
   - Working memory size and freshness
   - Any pending contradictions
   - Health report findings
   - Recommendations (e.g., "working memory is stale, run regenerate")
