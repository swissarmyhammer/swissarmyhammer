---
title: Add Promise draining and module loader to swissarmyhammer-js
position:
  column: done
  ordinal: c9
---
Update the QuickJS worker loop in `swissarmyhammer-js/src/lib.rs` to support async validation and ES module imports. Two changes are needed:

**1. Promise draining — `rt.execute_pending_job()`**

After each `ctx.eval()` call in the worker loop, drain the pending job queue so Promises resolve:

```rust
// After eval, drain pending jobs (Promise resolution)
loop {
    match rt.execute_pending_job() {
        Ok(false) => break,   // no more pending jobs
        Ok(true) => continue, // executed one, check for more
        Err(e) => { /* log warning, break */ }
    }
}
```

This must happen in both the `Set` and `Get` arms of the request handler. Without this, any JS code that uses `await` or Promises will silently hang.

**2. Module loader for `fields/lib/` imports**

Configure a module loader so `import { foo } from "helpers/text.js"` resolves relative to a configurable base path:

- Add a new `JsRequest` variant: `SetModuleBasePath { path: PathBuf, reply: oneshot::Sender<Result<(), String>> }`
- Or: configure the module loader at worker startup via an init message
- The module loader reads `.js` files from the configured base path
- Use rquickjs's `ModuleLoader` trait implementation

**Public API addition:**

```rust
impl JsState {
    pub async fn set_module_base(&self, path: impl Into<PathBuf>) -> Result<(), String>;
}
```

**Files:** `swissarmyhammer-js/src/lib.rs`

**Subtasks:**
- [ ] Add `rt.execute_pending_job()` drain loop after eval in Set handler
- [ ] Add `rt.execute_pending_job()` drain loop after eval in Get handler
- [ ] Implement module loader resolving from a configurable base path
- [ ] Add `set_module_base()` API
- [ ] Write test: async function with await resolves correctly
- [ ] Write test: Promise.resolve chain works
- [ ] Write test: module import from configured base path works
- [ ] Write test: import from outside base path is rejected