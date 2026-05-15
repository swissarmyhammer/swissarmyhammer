---
position_column: done
position_ordinal: ffffb080
title: Use TypeId instead of type_name for CommandContext extensions
---
**File:** `swissarmyhammer-commands/src/context.rs:57-68`\n\n**What:** `set_extension`/`extension` use `std::any::type_name::<T>()` as HashMap key. `type_name` is not guaranteed stable across compiler versions or compilation units. The extension is set in `swissarmyhammer-kanban-app` and retrieved in `swissarmyhammer-kanban` — different crates.\n\n**Why:** Latent risk if crates are ever compiled separately or with LTO boundaries. `TypeId` is guaranteed stable within a process.\n\n**Fix:** Replace `HashMap<String, Arc<dyn Any + Send + Sync>>` with `HashMap<TypeId, Arc<dyn Any + Send + Sync>>`, add `'static` bound (already implicit via `Any`).\n\n- [ ] Change extensions map key from String to TypeId\n- [ ] Update set_extension and extension methods\n- [ ] Verify tests pass"