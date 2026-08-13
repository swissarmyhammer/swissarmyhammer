---
assignees:
- claude-code
position_column: todo
position_ordinal: ffba80
title: dead-code-typescript reports library entry points and framework-registered exports as dead
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` runs `ts-prune` per `tsconfig.json` and declares `supersedes: [dead-code]`.

Two carve-outs of `dead-code.md` are dropped.

- "**Exported public API**: ... Its callers live outside this repo, so an empty inbound callgraph is expected, not dead." The rule states its position outright: "TypeScript has no `pub` and no `__all__` — every `export` is the module's surface." So for a published library package, every entry of `src/index.ts` that no in-repo module imports is reported as dead. ts-prune has no concept of a package entry point.
- "**Entry points**: ... framework-invoked handlers, CLI command callbacks, registered hooks/callbacks". The rule names the two shapes and turns the exemption into a mandatory marker: "a module a bundler aliases by path, and a function a framework registers by name." Its own hand-check found 3 of 10 sampled findings were exactly this — `resolve.alias` targets in `vite.config.ts`, and vitest browser commands.

The test carve-out depends on a file the rule reads but does not control: if a project's `tsconfig.json` excludes `*.test.ts`, every helper exported only for tests looks unimported and reports.

`// ts-prune-ignore-next` works, so an annotation contract is available. `package.json` `main`/`exports` names the entry points and could be read. Decide.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity