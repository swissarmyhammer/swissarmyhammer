---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffb980
position_swimlane: null
title: 'FILTER-3: Lezer grammar + CM6 language extension'
---
## What

Create a Lezer grammar for the filter DSL and wrap it as a CM6 `LanguageSupport` extension. This provides syntax highlighting, bracket matching, and error recovery in the filter editor.

### Grammar (from design notes)

```
@precedence { not, and @left, or @left }

expr {
  Tag | Mention | Ref | Not | And | Or | Group
}

Not { (!not \"!\" | !not kw<\"not\", \"NOT\">) expr }
And {
  expr !and \"&&\" expr |
  expr !and kw<\"and\", \"AND\"> expr |
  expr !and expr
}
Or {
  expr !or \"||\" expr |
  expr !or kw<\"or\", \"OR\"> expr
}
Group { \"(\" expr \")\" }

kw<term, upper> { @specialize[@name={term}]<Keyword, term> | @specialize[@name={term}]<Keyword, upper> }

@tokens {
  Tag     { \"#\" ![ \\t\\n\\r#]+ }
  Mention { \"@\" ![ \\t\\n\\r@]+ }
  Ref     { \"^\" ![ \\t\\n\\r^]+ }
  Keyword { $[a-zA-Z]+ }
  @precedence { Tag, Mention, Ref, Keyword }
}

@skip { space }
@tokens { space { $[ \\t\\n\\r]+ } }
```

### Files to create
- `kanban-app/ui/src/lang-filter/filter.grammar` — the Lezer grammar file
- `kanban-app/ui/src/lang-filter/filter.grammar.d.ts` — TS type declarations for generated parser
- `kanban-app/ui/src/lang-filter/index.ts` — exports `filterLanguage()` as `LanguageSupport`
- `kanban-app/ui/src/lang-filter/highlight.ts` — `styleTags` mapping node types to highlight classes (Tag → tag, Mention → variableName, Ref → link, operators → keyword, etc.)

### Build pipeline
- Add `@lezer/generator` as a devDependency
- Add a build script or vite plugin to compile `.grammar` → `.js` parser
- Consider `lezer-generator` CLI in a `prebuild` npm script

### Highlighting classes
- `Tag` (→ `tags` highlight style (colored like tags)
- `Mention` (@will) → `variableName` or custom `mention` style
- `Ref` (^card-123) → `link` style
- `not`/`and`/`or`/`!`/`&&`/`||` → `keyword` / `operator`
- `(` `)` → `paren`
- Errors → `invalid` (red underline via Lezer error recovery)

## Acceptance Criteria
- [ ] Grammar compiles without errors via `lezer-generator`
- [ ] Parser correctly tokenizes `#bug && @will || !#done`
- [ ] Syntax highlighting renders tags, mentions, refs, operators in distinct colors
- [ ] Bracket matching works for `(` `)`
- [ ] Error recovery: `#bug &&` (incomplete) still highlights `#bug` correctly
- [ ] No shift/reduce conflicts in generated parser

## Tests
- [ ] `kanban-app/ui/src/lang-filter/__tests__/parser.test.ts` — parse tree assertions for representative expressions
- [ ] `kanban-app/ui/src/lang-filter/__tests__/highlight.test.ts` — verify highlight classes are applied to correct node types
- [ ] `npm test` in kanban-app passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.