Autonomous issue resolution that works through your entire backlog hands-free.

Let AI systematically implement all pending issues while you focus on design,
architecture, and planning. The implement command is your automated development
team that never stops until every issue is resolved.

AUTONOMOUS OPERATION

The implement command runs completely hands-free:
• Discovers all pending issues in ./issues directory
• Works through each issue systematically from oldest to newest
• Implements solutions using AI-powered coding
• Tests and validates each implementation
• Continues until all issues are resolved
• Provides progress updates throughout execution

WORKFLOW INTELLIGENCE

Smart Issue Processing:
• Understands issue context and requirements
• Reviews existing code and patterns
• Makes informed implementation decisions
• Follows project coding standards and conventions
• Creates tests and validates functionality

Continuous Execution:
• Processes issues one at a time with focus
• Maintains context across issue transitions
• Recovers from errors and continues
• Updates issue status and metadata
• Reports completion and results

WHY USE IMPLEMENT

• Hands-Free Automation - Let AI handle the coding while you plan
• Consistent Quality - Every issue follows the same rigorous process
• Time Savings - Parallel work on planning while implementation runs
• Complete Coverage - Never miss an issue or partial implementation
• Systematic Approach - Methodical progress from first to last issue

USAGE

Run autonomous implementation:
  swissarmyhammer implement

Monitor progress with verbose output:
  swissarmyhammer --verbose implement

Quiet mode for background execution:
  swissarmyhammer --quiet implement

WORKFLOW INTEGRATION

This command is a convenience shortcut for:
  swissarmyhammer flow run implement

For more control over execution, use the full flow command:

Interactive mode (approve each step):
  swissarmyhammer flow run implement --interactive

Preview without execution:
  swissarmyhammer flow run implement --dry-run

Resume interrupted implementation:
  swissarmyhammer flow resume <run_id>

HOW IT WORKS

The implement workflow executes these steps:
1. Check for pending issues in ./issues directory
2. If issues exist, run 'do_issue' workflow on next issue
3. Complete implementation including tests and validation
4. Mark issue complete and move to next
5. Repeat until all issues are resolved
6. Provide completion summary

TYPICAL WORKFLOWS

Morning routine - start implementation:
  swissarmyhammer implement &

After planning session:
  swissarmyhammer plan feature.md
  swissarmyhammer implement

Background processing:
  nohup swissarmyhammer implement > implement.log 2>&1 &

TROUBLESHOOTING

If implementation stops or fails:
• Check ./issues directory exists and contains valid issues
• Verify file permissions allow reading and writing issues
• Review workflow logs with: swissarmyhammer flow logs <run_id>
• Use --verbose flag for detailed execution information
• Ensure git repository is in clean state

Prerequisites:
• Valid issues in ./issues directory
• Proper file system permissions
• Git repository (for issue branch management)
• Active agent configuration (Claude Code or alternative)

The implement command transforms your issue backlog into working code
automatically, letting you focus on what matters most: solving problems
and building great software.