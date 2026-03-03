---
title: Integrate FieldsContext into KanbanContext
position:
  column: done
  ordinal: d1
---
Compose `FieldsContext` into `KanbanContext`, provide `kanban_defaults()`.

**kanban_defaults() — ALL kanban entities:**
- **Task fields:** title, status, priority, tags (computed), assignees (reference, multiple), due, depends_on (reference, multiple), body
- **Tag fields:** tag_name (with validation), color, description, usage (computed), last_used (computed)
- **Shared fields:** name (text), order (number), actor_type (select: human/agent)
- **Entity templates:** task (body_field: body), tag (no body_field), actor, column, swimlane

**KanbanContext changes:**
- Add `fields: FieldsContext` field
- Update open() to create FieldsContext at `root.join("fields")` with `kanban_defaults()`
- Expose `pub fn fields(&self) -> &FieldsContext`
- Implement `EntityLookup` for kanban stores (dispatches on entity_type to check tasks/, tags/, actors/, etc.)

**Subtasks:**
- [ ] Add swissarmyhammer-fields dependency to swissarmyhammer-kanban
- [ ] Implement kanban_defaults() with all built-in fields
- [ ] Implement kanban_defaults() with all 5 entity templates (task, tag, actor, column, swimlane)
- [ ] Implement EntityLookup for kanban stores
- [ ] Add FieldsContext field to KanbanContext
- [ ] Update KanbanContext::open() to compose FieldsContext
- [ ] Expose fields() accessor
- [ ] Write test: init creates fields/ directory with defaults
- [ ] Write test: fields are accessible via context.fields()
- [ ] Write test: re-open preserves user customizations
- [ ] Verify all existing kanban tests still pass