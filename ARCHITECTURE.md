# Architecture

## Context Objects (Blackboard Pattern)

Every subsystem exposes a **Context** struct that bundles its I/O primitives, configuration, and indexes into a single value. Prefer passing a context object over long argument lists.

### Why contexts

- **One argument instead of many.** A function that needs storage, field definitions, and validation takes one `&KanbanContext` instead of three separate parameters. When requirements grow the signature stays stable.
- **Blackboard pattern.** Higher-level contexts compose lower-level ones as fields. A consumer receives the top-level context and reaches through it to whatever layer it needs. No need to thread individual pieces through the call stack.
- **Clear ownership.** Each context owns its resources (paths, indexes, caches, locks). Callers don't manage lifetimes of the internals.

### Context hierarchy

```
CliContext                     # CLI flags, output format, prompt library
  └─ TemplateContext           # config key/value pairs

KanbanContext                  # .kanban/ root, file locking
  ├─ FieldsContext (Arc)       # field definitions, entity templates, name/ID indexes
  ├─ EntityContext (Arc)       # entity I/O, changelogs, undo stack, validation, compute
  │    └─ FieldsContext (Arc)  # shared — same instance as KanbanContext.fields
  └─ ViewsContext (RwLock)     # view definitions, CRUD, disk persistence

CommandContext                 # scope chain, target, args, UI state
  └─ extensions: HashMap<TypeId, Arc<dyn Any>>
       └─ KanbanContext (Arc)  # injected as a typed extension
```

Contexts at the bottom of the hierarchy (FieldsContext, ViewsContext) are self-contained. Contexts higher up compose them via `Arc` so the same instance is shared without copying.

### Conventions

**Create with `open()` or a builder, not bare constructors.** Most contexts have an async `open()` that loads definitions from disk and builds indexes. Use `new()` only for lightweight/partial initialization (e.g., tests).

```rust
// Full initialization — loads YAML, builds indexes
let ctx = KanbanContext::open(&root).await?;

// Builder pattern when there are many optional parts
let fields = FieldsContext::open(&fields_root).build().await?;
```

**Compose via `Arc` fields.** When a higher-level context needs a lower-level one, store it as `Arc<T>` so it can be shared across contexts without lifetime gymnastics.

```rust
pub struct KanbanContext {
    fields: Option<Arc<FieldsContext>>,
    entities: OnceCell<Arc<EntityContext>>,
    views: Option<RwLock<ViewsContext>>,
}

pub struct EntityContext {
    fields: Arc<FieldsContext>,   // same Arc as KanbanContext.fields
    validation: Option<Arc<ValidationEngine>>,
    compute: Option<Arc<ComputeEngine>>,
}
```

**Use `with_*` builder methods for optional capabilities.** Attach engines, registries, or configuration after construction rather than requiring everything upfront.

```rust
let entity_ctx = EntityContext::new(&root, fields.clone())
    .with_validation(validation_engine)
    .with_compute(compute_engine);
```

**Extensions for cross-cutting services.** `CommandContext` uses a `TypeId`-keyed extension map so domain contexts (KanbanContext) can be injected without the command framework knowing about them.

```rust
// Registration
cmd_ctx.set_extension(kanban_ctx);

// Retrieval
let kanban = cmd_ctx.extension::<KanbanContext>()?;
```

### Anti-patterns to avoid

- **Long argument lists.** If a function takes more than 2-3 related parameters, bundle them into a context or introduce a new one.
- **Passing internals instead of the context.** Don't destructure a context to pass its fields individually — pass the context and let the callee reach in.
- **Cloning instead of sharing.** Use `Arc` to share contexts. Cloning a FieldsContext with its indexes is wasteful.
- **Logic in the context.** Contexts provide *access*, not behavior. Business logic belongs in commands and helpers that receive the context.
