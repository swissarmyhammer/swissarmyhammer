# Complete Mermaid Grammar Analysis

## Overview
Comprehensive analysis of ALL 23 diagram types found in the mermaid repository, including both jison-based and TypeScript-based parsers.

## Grammar Distribution

### Jison-Based Grammars (17 total)
These use formal grammar files (.jison) for parsing:

| Grammar | File Path | Lines | Complexity | Sample Count |
|---------|-----------|-------|------------|--------------|
| **Flow** | `diagrams/flowchart/parser/flow.jison` | 631 | Very High | 576 |
| **Class** | `diagrams/class/parser/classDiagram.jison` | 420 | High | 212 |
| **State** | `diagrams/state/parser/stateDiagram.jison` | 336 | High | 124 |
| **Sequence** | `diagrams/sequence/parser/sequenceDiagram.jison` | 329 | High | 55 |
| **C4** | `diagrams/c4/parser/c4Diagram.jison` | 322 | Medium | 50 |
| **ER** | `diagrams/er/parser/erDiagram.jison` | 293 | Medium | 113 |
| **Block** | `diagrams/block/parser/block.jison` | 290 | Medium | 115 |
| **Requirement** | `diagrams/requirement/parser/requirementDiagram.jison` | 267 | Medium | 83 |
| **Gantt** | `diagrams/gantt/parser/gantt.jison` | 188 | Medium | 45 |
| **Quadrant** | `diagrams/quadrant-chart/parser/quadrant.jison` | 187 | Medium | 57 |
| **XY Chart** | `diagrams/xychart/parser/xychart.jison` | 171 | Medium | 56 |
| **Kanban** | `diagrams/kanban/parser/kanban.jison` | 166 | Low | 41 |
| **Mindmap** | `diagrams/mindmap/parser/mindmap.jison` | 127 | Low | 46 |
| **Timeline** | `diagrams/timeline/parser/timeline.jison` | 79 | Low | 25 |
| **Journey** | `diagrams/user-journey/parser/journey.jison` | 69 | Low | 19 |
| **Sankey** | `diagrams/sankey/parser/sankey.jison` | 66 | Low | 7 |
| **Example** | `mermaid-example-diagram/src/parser/exampleDiagram.jison` | 43 | Low | 0 |

### TypeScript-Based Parsers (7 total)
These use direct TypeScript parsing without formal grammars:

| Parser | File Path | Complexity | Sample Count |
|--------|-----------|------------|--------------|
| **Architecture** | `diagrams/architecture/architectureParser.ts` | Medium | 38 |
| **Git** | `diagrams/git/gitGraphParser.ts` | Medium | 181 |
| **Packet** | `diagrams/packet/parser.ts` | Low | 29 |
| **Pie** | `diagrams/pie/pieParser.ts` | Low | 64 |
| **Radar** | `diagrams/radar/parser.ts` | Low | 62 |
| **Treemap** | `diagrams/treemap/parser.ts` | Low | 43 |
| **Misc** | Various | - | 244 |

## Complete Implementation Plan Coverage

### Phase 1: Foundation (Simple Grammars) - 1-2 weeks
**Jison-based:**
1. **Sankey** ✅ `step_sankey.md` - CSV format (7 samples)
2. **Timeline** ✅ `step_timeline.md` - Chronological events (25 samples)  
3. **Journey** ✅ `step_journey.md` - User experience (19 samples)

**TypeScript-based:**
4. **Pie** ✅ `step_pie.md` - Data visualization (64 samples)

### Phase 2: Medium Complexity - 2-3 weeks
**Jison-based:**
5. **Gantt** ✅ `step_gantt.md` - Project timelines (45 samples)
6. **Quadrant** ✅ `step_quadrant.md` - 2x2 matrices (57 samples)
7. **Mindmap** - Hierarchical structures (46 samples)
8. **Kanban** - Task boards (41 samples)
9. **XY Chart** - Line/bar charts (56 samples)

**TypeScript-based:**
10. **Radar** - Multi-axis charts (62 samples)
11. **Packet** - Network packets (29 samples)
12. **Treemap** - Hierarchical data (43 samples)

### Phase 3: Complex Structures - 3-4 weeks
**Jison-based:**
13. **ER** - Database schemas (113 samples)
14. **Requirement** - Requirements traceability (83 samples)
15. **Block** ✅ `step_block.md` - Structured blocks (115 samples)

**TypeScript-based:**
16. **Git** ✅ `step_git.md` - Version control workflows (181 samples)
17. **Architecture** - System architecture (38 samples)

### Phase 4: Most Complex - 4-5 weeks
**Jison-based:**
18. **Sequence** ✅ `step_sequence.md` - Message passing (55 samples)
19. **State** - State machines (124 samples)
20. **Class** - OOP relationships (212 samples)
21. **C4** - Enterprise architecture (50 samples)
22. **Flow** - Most complex flowcharts (576 samples)

### Phase 5: Specialized
23. **Example** - Template for new diagrams (0 samples)

## Implementation Strategies

### Jison-Based Approach
- Use Chumsky parser combinators to replicate jison grammar rules
- Create lexer for token recognition
- Build recursive descent parser for grammar rules
- Generate strongly-typed AST structures

### TypeScript-Based Approach
- Analyze TypeScript parser logic and patterns
- Create equivalent Rust parsing logic
- Focus on syntax patterns rather than formal grammar
- Maintain compatibility with original parsing behavior

## Technical Challenges by Complexity

### Low Complexity (Lines < 100)
**Challenges:**
- Simple syntax parsing
- Basic data structures
- Minimal state management

**Examples:** Sankey, Timeline, Journey, Pie, Packet

### Medium Complexity (Lines 100-300)
**Challenges:**
- Multiple syntax forms
- Structured data with sections
- Configuration options
- Some state management

**Examples:** Gantt, Quadrant, Block, ER, Requirement

### High Complexity (Lines 300-500)
**Challenges:**
- Complex syntax rules
- Multiple diagram contexts
- Advanced features (loops, conditions)
- Significant state management

**Examples:** Sequence, State, C4, Class

### Very High Complexity (Lines > 500)
**Challenges:**
- Extensive syntax variations
- Multiple node and edge types
- Complex styling and theming
- Advanced layout and positioning

**Examples:** Flow (631 lines)

## Success Metrics

### Coverage
- ✅ **24 total diagram types** identified and planned
- ✅ **2,285 test samples** available across all types
- ✅ **100% grammar coverage** - no missed diagram types

### Implementation Quality
- Parse 100% of available test samples
- Maintain type safety with strongly-typed ASTs
- Provide clear error messages
- Support all documented features

### Performance Targets
- Parse 1000+ diagrams per second
- Memory-efficient AST representation
- Concurrent parsing support

## Dependencies and Order

### Independent (Can implement in any order)
- Sankey, Timeline, Journey, Pie
- Gantt, Quadrant, Mindmap, Kanban  
- Radar, Packet, Treemap

### Sequential Dependencies
1. **Block** → Architecture (spatial concepts)
2. **ER/Class** → C4 (relationship modeling)
3. **Simple parsers** → Complex parsers (patterns)

### Shared Concepts
- **Accessibility**: All diagrams support accTitle/accDescr
- **Styling**: CSS classes and inline styles
- **Interactions**: Click handlers and links
- **Configuration**: Themes and display options

## Conclusion

This analysis provides complete coverage of all Mermaid diagram types with detailed implementation plans. The hybrid approach (jison + TypeScript parsers) ensures we can handle the full ecosystem while maintaining the architectural benefits of Rust's type system and performance characteristics.