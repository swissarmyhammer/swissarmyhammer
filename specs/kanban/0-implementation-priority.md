# Kanban Implementation Priorities

## Overview

This directory contains specs for 8 missing features in the kanban tool. This document outlines the recommended implementation order.

## Priority Order

### 1. Activity Logging (CRITICAL)
**File:** `1-activity-logging.md`

**Why first:**
- Core audit trail missing
- Blocks timestamp derivation
- Required for compliance/debugging
- Affects all operations
- Spec explicitly describes it as core feature

**Scope:** Update all 40 operations to log

**Estimated operations added:** 0 (infrastructure)

---

### 2. Enhanced List Tasks Filtering (HIGH)
**File:** `6-list-tasks-filtering.md`

**Why second:**
- Makes existing features more useful
- Critical for multi-agent scenarios
- Agents need to find "my work"
- Small change, high value

**Scope:** Add filters: assignee, tag, swimlane, exclude_done

**Estimated operations added:** 0 (enhancement to existing op)

---

### 3. Unassign Task (HIGH)
**File:** `4-unassign-operation.md`

**Why third:**
- Complements assign task (added today)
- Simple, self-contained
- Agents need this for workflow
- Quick win

**Scope:** Single new operation

**Estimated operations added:** 1

---

### 4. Subtask Operations (MEDIUM)
**File:** `2-subtask-operations.md`

**Why fourth:**
- Common kanban feature
- Data model exists
- Useful for task breakdowns
- 4 related operations

**Scope:** Add subtask CRUD operations

**Estimated operations added:** 4

---

### 5. Board Overview with Counts (MEDIUM)
**File:** `8-board-overview-counts.md`

**Why fifth:**
- Improves get board usefulness
- Helps with project visibility
- Enhancement to existing operation
- Quick overview for agents

**Scope:** Enhance get board response

**Estimated operations added:** 0 (enhancement to existing op)

---

### 6. Derived MCP Schema (MEDIUM)
**File:** `9-derived-mcp-schema.md`

**Why sixth:**
- Significantly improves discoverability
- Auto-generates from operation metadata
- Never gets out of sync
- Important for LLM tool use

**Scope:** Generate schema from KANBAN_OPERATIONS static

**Estimated operations added:** 0 (infrastructure)

---

### 7. Complete Tool Description (LOW)
**File:** `5-tool-description-complete.md`

**Why seventh:**
- Documentation only
- Doesn't block functionality
- Can be done anytime
- Less critical if schema is comprehensive

**Scope:** Expand description.md from ~25 to ~200 lines

**Estimated operations added:** 0 (documentation)

---

### 8. Attachment Operations (LOW)
**File:** `3-attachment-operations.md`

**Why eighth:**
- Less commonly used than subtasks
- More complex (file handling)
- 5 operations
- Can wait

**Scope:** Add attachment CRUD operations

**Estimated operations added:** 5

---

### 9. Board Actor Storage Cleanup (LOWEST)
**File:** `7-board-actor-storage.md`

**Why last:**
- Cleanup/consistency only
- System works as-is
- Potentially breaking change
- Needs migration strategy

**Scope:** Remove actors field from Board struct

**Estimated operations added:** 0 (cleanup)

---

## Operation Count Growth

| Priority | Feature | Ops Added | Running Total |
|----------|---------|-----------|---------------|
| Current  | - | - | 40 |
| 1 | Activity Logging | 0 | 40 |
| 2 | List Tasks Filtering | 0 | 40 |
| 3 | Unassign Task | 1 | 41 |
| 4 | Subtask Operations | 4 | 45 |
| 5 | Board Overview | 0 | 45 |
| 6 | Derived MCP Schema | 0 | 45 |
| 7 | Tool Description | 0 | 45 |
| 8 | Attachment Operations | 5 | 50 |
| 9 | Board Storage Cleanup | 0 | 50 |

**Final: 50 total operations**

## Implementation Phases

### Phase 1: Core Infrastructure (1-2)
Get logging working and improve task filtering. These make the existing system more robust and useful.

### Phase 2: Task Management (3-4)
Add unassign and subtasks. These complete the core task management workflow.

### Phase 3: Polish (5-7)
Board overview, schema generation, and documentation. Make the tool more discoverable and user-friendly.

### Phase 4: Advanced Features (8-9)
Attachments and storage cleanup. Nice-to-have features that can wait.

## Notes

- All features are independent except:
  - Activity logging should come first (affects all other operations)
  - Board storage cleanup might conflict with logging if done in parallel

- Each spec file contains:
  - Status
  - Problem statement
  - Current state
  - Required changes
  - Testing requirements
  - File changes needed
  - Implementation notes
