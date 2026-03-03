---
title: Scaffold frontend with Vite, React, TypeScript, Tailwind, shadcn
position:
  column: done
  ordinal: a4
---
# Header

[] do a thing

Create the ui/ subdirectory with a complete frontend scaffold.

Steps:
1. npm init in ui/, create package.json
2. Install deps: react, react-dom, @tauri-apps/api v2, @tauri-apps/plugin-dialog (for folder picker)
3. Install dev deps: typescript, @types/react, @types/react-dom, vite, @vitejs/plugin-react, tailwindcss, postcss, autoprefixer
4. Create vite.config.ts — react plugin, server port 5173, clearScreen false
5. Create tsconfig.json and tsconfig.node.json
6. Create index.html with root div
7. Create tailwind.config.ts with shadcn-compatible settings
8. Create postcss.config.mjs
9. npx shadcn@latest init — sets up components.json, src/lib/utils.ts, CSS variables
10. npx shadcn@latest add badge button dropdown-menu separator
11. Create src/main.tsx entry point
12. Create src/index.css with @tailwind directives + shadcn CSS variables

Depends on: crate scaffold (tauri.conf.json must exist for vite to work with Tauri).
Verify: npm run build produces dist/ with index.html and bundled JS.