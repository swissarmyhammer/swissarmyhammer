Transform high-level specifications into actionable implementation steps with AI-powered planning.

The plan command analyzes your requirements and generates a comprehensive,
numbered sequence of focused issues ready for implementation. It understands
your codebase, follows your patterns, and creates plans that actually work.

AI-POWERED PLANNING

Intelligent Analysis:
• Reads and understands your specification markdown
• Analyzes existing codebase structure and patterns
• Reviews existing issues to avoid conflicts and duplication
• Considers project context from memos and documentation
• Breaks complex features into logical, implementable steps

Smart Issue Generation:
• Creates numbered, sequential issue files in ./issues directory
• Each issue is focused, testable, and independently implementable
• Issues build incrementally toward the complete feature
• Includes context and requirements for AI implementation
• Names issues clearly with descriptive identifiers

Context-Aware:
• Leverages existing memos for project knowledge
• Understands current codebase architecture
• Follows established patterns and conventions
• Avoids duplicating existing functionality
• Maintains consistency with project standards

USAGE

Generate implementation plan from specification:
  swissarmyhammer plan <PLAN_FILENAME>

The plan file path can be absolute or relative:
  swissarmyhammer plan ./specification/user-authentication.md
  swissarmyhammer plan /home/user/projects/plans/api-redesign.md

Verbose mode for detailed planning insights:
  swissarmyhammer --verbose plan ./specification/feature.md

PLAN FILE REQUIREMENTS

Create effective plan files using markdown:
• Clear feature description and goals
• Specific requirements and acceptance criteria
• Context about why this feature is needed
• Any technical constraints or considerations
• Examples of desired behavior or output

The AI uses this information to generate intelligent, context-aware
implementation steps that align with your goals.

OUTPUT FORMAT

Creates numbered issue files in ./issues/ directory:
```
PLANNAME_000001_step-description.md
PLANNAME_000002_step-description.md
PLANNAME_000003_step-description.md
...
```

Each issue contains:
• Clear description of what to implement
• Context from the original specification
• Dependencies on previous steps
• Testing requirements
• Acceptance criteria

WORKFLOW INTEGRATION

Planning integrates seamlessly with implementation:

1. Create a specification:
   echo "# User Authentication\nImplement OAuth2..." > spec.md

2. Generate implementation plan:
   swissarmyhammer plan spec.md

3. Review generated issues:
   ls ./issues

4. Run autonomous implementation:
   swissarmyhammer implement

COMMON WORKFLOWS

Feature planning:
  swissarmyhammer plan ./specification/user-authentication.md

Bug fix planning:
  swissarmyhammer plan ./docs/bug-fixes.md

Refactoring planning:
  swissarmyhammer plan ./notes/refactor-database.md

Architecture change planning:
  swissarmyhammer plan ./architecture/microservices-migration.md

BEST PRACTICES

Keep Plans Focused:
• One plan per major feature or component
• Break large features into multiple focused plans
• Use clear, descriptive filenames
• Include specific goals and acceptance criteria

Review Before Implementation:
• Check generated issues for completeness
• Verify issue sequence makes sense
• Adjust or add issues if needed
• Ensure each issue is independently testable

Plan File Organization:
• Store plans in dedicated directory (./specification/, ./plans/)
• Use consistent naming conventions
• Version control your plan files
• Keep plans updated as requirements evolve

EXAMPLES

Plan a new feature:
  swissarmyhammer plan ./specification/user-profile.md

Plan with detailed output:
  swissarmyhammer --verbose plan ./plans/api-v2.md

Plan and immediately implement:
  swissarmyhammer plan ./spec/auth.md && swissarmyhammer implement

Review planned issues:
  swissarmyhammer plan ./spec/feature.md
  ls -la ./issues/FEATURE_*

TROUBLESHOOTING

If planning fails:
• Verify plan file exists and is readable
• Check ./issues directory exists and is writable
• Ensure adequate disk space for issue files
• Use --debug flag for detailed execution information
• Verify plan file contains valid markdown

Common issues:
• Plan file not found - check path and filename
• Permission denied - verify directory permissions
• No issues generated - ensure plan file has content
• Numbering conflicts - review existing issues

Prerequisites:
• Valid markdown specification file
• ./issues directory exists (created automatically)
• File system write permissions
• Active agent configuration

The plan command bridges the gap between ideas and implementation,
transforming specifications into structured, actionable development tasks
that AI can execute autonomously.