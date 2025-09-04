# Memoranda System

The memoranda system in SwissArmyHammer provides a powerful note-taking and knowledge management solution designed for developers and teams. It stores notes as structured documents with seamless integration with your development workflow.

## Overview

The memoranda system enables you to:
- Create and organize notes with unique identifiers
- Export and import memo collections
- Integration with issues and workflows
- Version-controlled knowledge base
- Collaborative note sharing

## Core Concepts

### Memo Structure

Memoranda are stored with the following structure:
- **Title**: Human-readable memo title
- **Content**: Markdown-formatted memo body
- **ID**: Unique ULID identifier (e.g., `01ARZ3NDEKTSV4RRFFQ69G5FAV`)
- **Timestamp**: Creation and modification times
- **Metadata**: Additional structured data

### Storage Format

Memos are stored in a structured format that supports:
- Efficient querying and indexing
- Metadata extraction and filtering
- Import/export operations
- Version tracking

## Basic Operations

### Creating Memos

Create a new memo:
```bash
sah memo create --title "Project Architecture Notes" --content "
# System Architecture

## Overview
The system follows a modular architecture with clear separation of concerns.

## Components
- API Gateway: Handles external requests
- Service Layer: Business logic processing
- Data Layer: Persistence and caching

## Design Decisions
- Microservices for scalability
- Event-driven communication
- CQRS pattern for complex queries
"
```

Create from file:
```bash
sah memo create --title "Meeting Notes" --file meeting_2024_01_15.md
```

Interactive creation:
```bash
echo "Quick note about bug in login validation" | sah memo create --title "Login Bug"
```

### Listing Memos

List all memos with previews:
```bash
sah memo list
```

Example output:
```
ID: 01ARZ3NDEKTSV4RRFFQ69G5FAV
Title: Project Architecture Notes
Created: 2024-01-15T10:30:00Z
Preview: # System Architecture\n\n## Overview\nThe system follows...

ID: 01BSZ4OFDLTSV5SSGGQ70H6GBW
Title: API Design Guidelines
Created: 2024-01-14T15:45:00Z
Preview: # API Standards\n\n## REST Conventions\nAll endpoints should...
```

### Viewing Memos

Get a specific memo by ID:
```bash
sah memo get 01ARZ3NDEKTSV4RRFFQ69G5FAV
```

Get all memo content for AI context:
```bash
sah memo get-all-context
```

This returns all memos sorted by most recent first, formatted for AI consumption.

## Advanced Features

### Updating Memos

Update memo content:
```bash
sah memo update 01ARZ3NDEKTSV4RRFFQ69G5FAV --content "
# Updated System Architecture

## New Requirements
Added real-time messaging capabilities.

## Implementation Notes
- WebSocket connections for live updates
- Message queuing for reliability
- Load balancing for scalability
"
```

The title remains unchanged when updating content.

### Deleting Memos

Remove a memo permanently:
```bash
sah memo delete 01ARZ3NDEKTSV4RRFFQ69G5FAV
```

**Warning**: This action cannot be undone.

## Organization Strategies

### Categorization by Title

Use consistent title patterns:
```bash
# Project documentation
sah memo create --title "[PROJECT] Architecture Overview"
sah memo create --title "[PROJECT] API Documentation"

# Meeting notes
sah memo create --title "[MEETING] Team Standup 2024-01-15"
sah memo create --title "[MEETING] Architecture Review"

# Learning notes
sah memo create --title "[LEARN] Rust Async Programming"
sah memo create --title "[LEARN] Database Optimization"
```

### Content Structure

Organize memo content with consistent structure:

```markdown
# Topic Title

## Summary
Brief overview of the topic.

## Key Points
- Main concept 1
- Main concept 2
- Main concept 3

## Details
Comprehensive information, code examples, and explanations.

## References
- [Link 1](https://example.com)
- Related memos: 01ARZ3NDEKTSV4RRFFQ69G5FAV

## Action Items
- [ ] Task 1
- [ ] Task 2

## Tags
#architecture #microservices #design-patterns
```

### Linking Related Content

Reference other memos by ID:
```markdown
See also:
- Architecture overview: 01ARZ3NDEKTSV4RRFFQ69G5FAV
- API guidelines: 01BSZ4OFDLTSV5SSGGQ70H6GBW
```

Cross-reference with issues:
```markdown
Related to issue: FEATURE_001_user-authentication
```

## Integration with Development Workflow

### With Issues

Link memos to issues for comprehensive documentation:

```markdown
# Issue Research: FEATURE_001_user-auth

## Background Research
Created memo: 01ARZ3NDEKTSV4RRFFQ69G5FAV - "OAuth Implementation Patterns"

## Design Decisions
Documented in memo: 01BSZ4OFDLTSV5SSGGQ70H6GBW - "Authentication Architecture"

## Implementation Notes
See memo: 01CSZ5PGEMT7V6TTHHQ81I7HCX - "User Session Management"
```

### With Workflows

Incorporate memo creation into workflows:

```markdown
# Development Workflow

1. Research phase:
   - `sah memo create --title "[RESEARCH] {topic}"`
   - Document findings and decisions

2. Design phase:
   - `sah memo create --title "[DESIGN] {component}"`
   - Architecture and interface documentation

3. Implementation phase:
   - `sah memo create --title "[IMPL] {feature}"`
   - Implementation notes and gotchas

4. Review phase:
   - `sah memo list`
   - Review and consolidate learnings
```

### Knowledge Sharing

Use memos for team knowledge sharing:

```markdown
# Team Knowledge Base

## Onboarding
- System Overview: 01ARZ3NDEKTSV4RRFFQ69G5FAV
- Development Setup: 01BSZ4OFDLTSV5SSGGQ70H6GBW
- Code Standards: 01CSZ5PGEMT7V6TTHHQ81I7HCX

## Architecture
- Service Architecture: 01DSZ6QHFNU8W7UUIIR92J8IDY
- Database Schema: 01ESZ7RIGOV9X8VVJJS03K9JEZ
- API Design: 01FSZ8SJHPWAZ9WWKKTP4LAKFA
```

## Export and Import

### Exporting Memos

Export all memos for backup or sharing:
```bash
# Export to JSON
sah memo list --format json > memos_backup.json

# Export individual memo
sah memo get 01ARZ3NDEKTSV4RRFFQ69G5FAV --format markdown > memo_export.md
```

### Importing Memos

Import from external systems:
```bash
# Convert from other formats
cat external_notes.md | sah memo create --title "Imported Notes"

# Bulk import from directory
for file in notes/*.md; do
  sah memo create --title "$(basename "$file" .md)" --file "$file"
done
```

## Best Practices

### Content Creation

**Write clear, searchable content**:
- Use descriptive titles with keywords
- Include technical terms and concepts
- Add context and background information
- Structure content with headers and lists

**Make content discoverable**:
- Add relevant tags and keywords
- Include synonyms for technical terms
- Reference related memos and issues
- Use consistent naming conventions

### Organization

**Develop a taxonomy**:
```
[CATEGORY] Specific Topic
[PROJECT-NAME] Component/Feature
[MEETING] Date and Participants  
[RESEARCH] Technology/Approach
[DECISION] What was decided
[HOW-TO] Step-by-step guides
```

**Maintain memo hygiene**:
- Regularly review and update content
- Remove outdated or duplicate information
- Consolidate related memos when appropriate
- Archive historical memos that are no longer relevant

### Collaborative Use

**Team conventions**:
- Agree on title formatting standards
- Define categories and tags
- Establish update and ownership policies
- Create shared memo indexes for important topics

**Knowledge management**:
- Regular knowledge sharing sessions
- Memo review and consolidation processes
- Cross-team memo sharing mechanisms
- Documentation of team decisions and rationales

## Troubleshooting

### Common Issues

**Memo not found**:
- Verify the ULID is correct
- Check if memo was deleted
- Use `sah memo list` to browse available memos

**Search returns no results**:
- Check spelling and terminology
- Try alternative keywords or terms
- Check memo titles and content patterns
- Verify memos exist with expected content

**Performance issues**:
- Large memo collections may have slower operations
- Consider archiving old memos
- Use efficient filtering approaches

### Error Messages

**"Invalid ULID"**: Check that the memo ID is a valid ULID format
**"Memo not found"**: The specified memo ID doesn't exist
**"Content too large"**: Memo content exceeds size limits

## API Integration

For programmatic access to the memoranda system, see the [Rust API documentation](rust-api.md#memoranda).

Key operations:
- `memo_create()` - Create new memos
- `memo_get()` - Retrieve specific memos
- `memo_list()` - List all memos with metadata
- `memo_update()` - Modify existing memo content
- `memo_delete()` - Remove memos permanently

The memoranda system provides a foundation for building institutional knowledge and supporting effective development workflows through organized documentation.