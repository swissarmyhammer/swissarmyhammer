---
name: debug-logs
title: Analyze Log Files
description: Analyze log files to identify issues and patterns
parameters:
  - name: log_content
    description: The log content to analyze
    required: true
  - name: issue_description
    description: Description of the issue you're investigating
    required: false
    default: "general analysis"
  - name: time_range
    description: Specific time range to focus on
    required: false
    default: "all"
  - name: log_format
    description: Log format (json, plaintext, syslog, etc.)
    required: false
    default: "auto-detect"
---

# Log Analysis: {{issue_description}}

## Log Content
```
{{ log_content }}
```

## Analysis Parameters
- **Issue**: {{issue_description}}
- **Time Range**: {{time_range}}
- **Format**: {{log_format}}

## Log Analysis Strategy

### 1. Pattern Recognition
Identify recurring patterns:
- Error patterns and frequency
- Warning indicators
- Performance anomalies
- Unusual sequences

### 2. Timeline Analysis
{% if time_range %}
Focusing on: {{time_range}}
{% endif %}
- Event sequence
- Time gaps or clusters
- Correlation with issues
- Peak activity periods

### 3. Error Analysis
- Error types and severity
- Stack traces
- Root cause indicators
- Error progression

### 4. Performance Insights
- Response time patterns
- Resource usage spikes
- Bottleneck indicators
- Capacity issues

### 5. Anomaly Detection
- Unusual patterns
- Missing expected events
- Configuration issues
- Security concerns

### 6. Recommendations
Based on the analysis:
- Immediate actions
- Monitoring improvements
- Prevention strategies
- Further investigation needed