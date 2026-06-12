---
name: esm-first
description: ESM modules, exports field, explicit .js extensions, no require/module.exports
severity: error
---

# JavaScript/TypeScript ESM-First

- `"type": "module"` in `package.json`. No exceptions.
- `"exports"` field, not `"main"`.
- All imports use full relative paths with explicit `.js` extensions: `import x from './utils.js'` — not `'./utils'` or `'.'`.
- No `require()`, no `module.exports`. Hard disqualifiers.
- No `'use strict'` — implicit in ESM.
- Built-in Node.js modules use the `node:` protocol prefix: `import fs from 'node:fs'` — not `'fs'`.
- Target Node.js 18+ in `"engines"`.
