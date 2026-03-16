---
position_column: done
position_ordinal: u5
title: 'CODE-CONTEXT-SKILL: Create builtin/skills/code-context/SKILL.md'
---
Create the code-context skill to teach the agent when and how to use the tool per spec lines 474-507.

**Requirements:**
- Document when to trigger skill (code exploration, bug investigation, refactoring)
- Create use-case table: scenario → instead of X → use code_context operation
- Key scenarios (from spec):
  - Callgraph instead of grepping function names
  - Blast radius instead of guessing from imports
  - List symbol instead of reading whole file
  - Get symbol instead of reading files/guessing
  - Find symbol instead of Glob
  - Search code for semantic queries
  - Grep code for exact keywords
  - Get status to check index health
- Workflow guidance: check get status first, prefer structured queries, combine operations
- Teach agent to read next-step hints from tool responses

**Quality Test Criteria:**
1. SKILL.md exists and is valid markdown
2. All major operations documented with when to use
3. All scenarios from spec table covered
4. Examples show proper operation chains
5. Skill loads without errors
6. Agent uses code_context operations instead of grep when appropriate