---
name: dead-code-typescript
description: TypeScript and JavaScript exports nothing imports — checked by ts-prune, not by prompt.
match:
  files:
    - "**/*.ts"
    - "**/*.tsx"
    - "**/*.js"
    - "**/*.jsx"
    - "**/*.mjs"
    - "**/*.cjs"
  project_types:
    - nodejs
supersedes: dead-code
tool:
  scope: workspace
  run: |
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    cat > "$work/entries.js" <<'ENTRY_MODULES'
    // Writes the ts-prune --ignore pattern naming the entry modules of one
    // TypeScript project. An entry module is a source file that a package
    // manifest of this workspace publishes, or that a tsconfig `paths` mapping
    // puts under a workspace package's own name. Both are a package stating
    // its own surface.
    const fs = require("fs");
    const path = require("path");

    // The suffixes TypeScript resolves a module specifier through.
    const SOURCE_SUFFIXES = [
      ".ts",
      ".tsx",
      ".mts",
      ".cts",
      ".js",
      ".jsx",
      ".mjs",
      ".cjs",
    ];

    // The package.json fields that name a published module beside `exports`.
    const PUBLISHED_FIELDS = ["main", "module", "browser", "types", "typings"];

    // The directories a workspace walk never enters.
    const UNWALKED = new Set(["node_modules", ".git"]);

    /** Collects every string leaf of a package.json field. */
    function leaves(value, out) {
      if (typeof value === "string") out.push(value);
      else if (value && typeof value === "object") {
        for (const inner of Object.values(value)) leaves(inner, out);
      }
      return out;
    }

    /** Escapes a literal for use inside a regular expression. */
    function quote(text) {
      return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    }

    /** Every file under `dir`, absolute, skipping the unwalked directories. */
    function walk(dir, out) {
      let entries;
      try {
        entries = fs.readdirSync(dir, { withFileTypes: true });
      } catch {
        return out;
      }
      for (const entry of entries) {
        if (UNWALKED.has(entry.name)) continue;
        const at = path.join(dir, entry.name);
        if (entry.isDirectory()) walk(at, out);
        else out.push(at);
      }
      return out;
    }

    /** The paths a manifest publishes, relative to the manifest's directory. */
    function publishedPaths(manifest) {
      const out = [];
      for (const field of PUBLISHED_FIELDS) {
        if (typeof manifest[field] === "string") out.push(manifest[field]);
      }
      leaves(manifest.exports, out);
      leaves(manifest.bin, out);
      return out.filter(
        (p) => (p.startsWith("./") || p.startsWith("../")) && !p.includes("*"),
      );
    }

    /** The tsconfig `paths` targets keyed on a workspace package's own name. */
    function selfPaths(table, names) {
      if (!table) return [];
      const out = [];
      for (const [key, targets] of Object.entries(table)) {
        const bare = key.endsWith("/*") ? key.slice(0, -2) : key;
        if (names.has(bare)) out.push(...targets);
      }
      return out;
    }

    /** The spellings a declared path resolves through, extensions included. */
    function spellings(declared) {
      const out = [declared];
      const dot = declared.lastIndexOf(".");
      const slash = declared.lastIndexOf("/");
      const stem = dot > slash ? declared.slice(0, dot) : declared;
      for (const suffix of SOURCE_SUFFIXES) out.push(stem + suffix);
      return out;
    }

    /** The existing files one absolute spelling names, `*` expanded. */
    function resolve(spelling) {
      if (!spelling.includes("*")) {
        try {
          return fs.statSync(spelling).isFile() ? [spelling] : [];
        } catch {
          return [];
        }
      }
      const head = spelling.slice(0, spelling.indexOf("*"));
      const dir = head.endsWith(path.sep) ? head : path.dirname(head);
      const parts = spelling.split("*").map(quote);
      const pattern = new RegExp(`^${parts.join("(.*)")}$`);
      return walk(dir, []).filter((file) => pattern.test(file));
    }

    /**
     * How ts-prune spells the path of a file in its report.
     *
     * This is the presenter's own operation, copied rather than modelled:
     * `result.file.replace(process.cwd(), "").replace(/^\//, "")`. The first
     * replace takes a STRING, so it cuts the FIRST occurrence of the working
     * directory wherever that text stands and needs no separator after the
     * match. A reading that anchors the cut, or that demands a separator, names
     * a spelling ts-prune never writes and the pattern then misses the line.
     */
    function reportedAs(absolute, projectDir) {
      const cut = absolute.replace(projectDir, "");
      return cut.replace(new RegExp(`^${quote(path.sep)}`), "");
    }

    /** Every package manifest of the workspace, with the directory of each. */
    function manifests(workspaceRoot) {
      const found = [];
      for (const file of walk(workspaceRoot, [])) {
        if (path.basename(file) !== "package.json") continue;
        try {
          found.push({
            dir: path.dirname(file),
            manifest: JSON.parse(fs.readFileSync(file, "utf8")),
          });
        } catch {
          continue;
        }
      }
      return found;
    }

    function main() {
      const projectDir = process.cwd();
      const workspaceRoot = path.resolve(process.argv[2]);
      const config = JSON.parse(fs.readFileSync(process.argv[3], "utf8"));
      const found = manifests(workspaceRoot);
      const names = new Set(
        found
          .map(({ manifest }) => manifest.name)
          .filter((name) => typeof name === "string"),
      );

      const declared = [];
      for (const { dir, manifest } of found) {
        for (const p of publishedPaths(manifest)) {
          declared.push(path.resolve(dir, p));
        }
      }
      const table = (config.compilerOptions || {}).paths;
      for (const p of selfPaths(table, names)) {
        declared.push(path.resolve(projectDir, p));
      }

      const entries = new Set();
      for (const one of declared) {
        for (const spelling of spellings(one)) {
          for (const file of resolve(spelling)) {
            entries.add(reportedAs(file, projectDir));
          }
        }
      }

      if (entries.size === 0) return;
      const alternation = [...entries].sort().map(quote).join("|");
      process.stdout.write(`^(?:${alternation}):`);
    }

    try {
      main();
    } catch (failure) {
      process.stderr.write(
        `dead-code-typescript: no entry module read in ${process.cwd()}: ${failure}\n`,
      );
    }
    ENTRY_MODULES
    root="$(pwd -P)"
    find . -name node_modules -prune -o -name .git -prune -o -name 'tsconfig.json' -print |
      while IFS= read -r config; do
        dir="${config%/*}"
        cwd="$(cd "$dir" && pwd -P)"
        (cd "$dir" && tsc -p tsconfig.json --showConfig) > "$work/config.json" 2>/dev/null
        entries="$(cd "$dir" && node "$work/entries.js" "$root" "$work/config.json")"
        [ -n "$entries" ] || entries='$^'
        (cd "$dir" && ts-prune -p tsconfig.json --ignore "$entries" --skip '$^') |
          grep -v ' (used in module)$' |
          grep -v '\.d\.ts:' |
          sed -n "s#^\([^:]*\):\([0-9]*\) - \(.*\)\$#\1:\2: unused export '\3'; nothing in the project imports it#p" |
          while IFS= read -r finding; do
            # The presenter cuts the FIRST occurrence of its own working
            # directory out of the absolute path — a `String.replace` given a
            # string, so the cut is not anchored and needs no separator after
            # the match — and then cuts one leading separator. The working
            # directory ts-prune ran in rebuilds three of the spellings that
            # cut makes: the path under that directory, the path with that
            # directory written straight back on for a sibling whose name
            # merely BEGINS with it, and the absolute path the cut never
            # touched. A path that holds the working directory at a LATER
            # position — a nested copy of the tree, such as a backup mount —
            # is cut in the middle, and no rebuild reaches it. That spelling
            # stands at no candidate below, so the run drops it.
            cut="${finding%%:*}"
            stands=""
            found=0
            for candidate in "$cwd/$cut" "$cwd$cut" "/$cut"; do
              [ -f "$candidate" ] || continue
              stands="$candidate"
              found=$((found + 1))
            done
            # No spelling that stands, or two that both stand, is a path the
            # run cannot confirm: the cut threw away what told those files
            # apart. Naming the wrong file is worse than naming none, so the
            # run drops the finding and says the drop out loud on STDOUT, at
            # the project's own tsconfig. That file made the program, and it
            # always stands. stderr reaches nobody here: the runner pipes it
            # and reads it only for a run that exits nonzero, and this script
            # exits 0.
            if [ "$found" -ne 1 ]; then
              printf '%s:1: dead-code-typescript dropped one finding. %s files of this program stand at the spelling `%s`. The run cannot tell which file the finding is about. The dropped line is `%s`\n' \
                "${config#./}" "$found" "$cut" "$finding"
              continue
            fi
            case "$stands" in
              "$root"/*) printf '%s:%s\n' "${stands#"$root"/}" "${finding#*:}" ;;
              *) printf '%s:%s\n' "$stands" "${finding#*:}" ;;
            esac
          done
      done | sort -u
  doctor:
    check_command: "which ts-prune tsc node find grep sed sort mktemp"
    check_version_command: "npm list -g --depth 0 ts-prune | grep -o 'ts-prune@.*'"
  install:
    commands:
      - "npm install -g ts-prune@0.10.3 typescript@5.9.3"
---

# Dead Code — TypeScript

`ts-prune` reports every exported symbol no other module in the project
imports. That is a narrow, objective claim about the module graph, and it is the
whole of what this rule owns.

An entry module is the one shape that claim gets wrong. Its callers stand
outside the repository, so no module imports it and every one of its exports
reports. The run therefore reads the modules a package PUBLISHES out of the
project's own manifests, and hands them to ts-prune's own `--ignore`.

## The corpus every number below was measured over

Three published TypeScript libraries, cloned at HEAD on 2026-08-14, beside this
workspace:

| repository | commit | `.ts` and `.tsx` files | `tsconfig.json` projects |
|---|---|---|---|
| colinhacks/zod | `4e1720c` | 424 | 9 |
| pmndrs/zustand | `2115efb` | 34 | 2 |
| reduxjs/redux | `3084fc3` | 53 | 3 |
| this workspace | HEAD | — | 2 |

Each of the three publishes a real `src/index.ts`, which is the shape the
exported-public-API carve-out is about. This workspace is the control: both of
its TypeScript projects are private applications that publish nothing.

**Every count in this file states whether the repository's dependencies were
installed**, because both tools move with that state. Measured over the three
libraries, each cloned twice — once bare and once after the package manager the
repository names had run:

| workspace | this rule, bare | this rule, installed | time bare | time installed |
|---|---|---|---|---|
| zod | 78 | 76 | 7.8 s | 11.5 s |
| zustand | 1 | 1 | 0.7 s | 0.9 s |
| redux | 6 | 6 | 1.0 s | 1.7 s |

The two zod findings that go are `packages/docs/source.config.ts` `docs` and
`blogPosts`: that package's `postinstall` runs `fumadocs-mdx`, which writes a
`.source` module importing both. So an install can only ADD callers, and this
rule reads more findings without one rather than fewer. The tool it was compared
against behaves the other way, and the survey below states that.

## The exported public API, which the manifests answer

`dead-code`, the prompt rule this one supersedes, exempts "a `pub`/exported item
that is the crate's/library's surface for *external* callers", and it names how
to read that surface: "Where a language names its surface in one place — Python's
`__all__`, a module's export list — that list is the answer."

TypeScript has no `pub` and no `__all__`, so every `export` is a module's
surface and only the module graph says whether the surface is reached. The
PACKAGE, not the module, is where the published surface stands, and a package
states it in two places. Both are build configuration a project already keeps,
never lint configuration:

- **`package.json`** — `main`, `module`, `browser`, `types`, `typings`, every
  string leaf of `exports`, and every value of `bin`. The run reads the whole
  `exports` map rather than one condition, because a library that publishes its
  source states it in a condition of its own: `zod` writes
  `"@zod/source": "./src/index.ts"` beside `"types": "./index.d.cts"`, and its
  own `tsconfig.json` names `customConditions: ["@zod/source"]`.
- **`tsconfig.json` `compilerOptions.paths`** — every target of a mapping whose
  key is a workspace package's own NAME, or that name with `/*`. That is the
  self-reference a repository writes so its own tests can import the package the
  way an outside caller does, and its target is the source entry. `zustand`
  writes `"zustand": ["./src/index.ts"]` and `"zustand/*": ["./src/*.ts"]`;
  `redux` writes `"redux": ["./src/index.ts"]`.

A key that names no package of the workspace states nothing. `redux` writes
`"@internal/*": ["./src/*"]` in the same table, which maps every source file, and
the run leaves every one of them under the gate.

This is `--retain-public` for TypeScript. `dead-code-swift` passes that flag and
Alamofire drops from 493 findings to 103; the numbers here are the same shape.

Measured over the corpus, each repository bare:

| workspace | findings, no entry carve-out | findings, the shipped run | time |
|---|---|---|---|
| zod | 1946 | 78 | 7.6 s |
| zustand | 9 | 1 | 0.7 s |
| redux | 14 | 6 | 1.0 s |
| this workspace | 58 | 58 | 6.2 s |

The 1868 findings the carve-out takes off `zod` stand in seven modules the
package's own `exports` map names: `src/index.ts`, `src/v4/index.ts`,
`src/mini/index.ts`, `src/v4-mini/index.ts`, `src/v4/mini/index.ts`,
`src/v3/index.ts` and `src/locales/index.ts`, holding 284, 285, 250, 250, 216,
247 and 52 findings. `src/index.ts` is counted twice, because `packages/bench`
is a second project whose program reaches the same file.

The 8 the carve-out takes off `redux` are the whole of `src/index.ts`. The 8 it
takes off `zustand` are `combine`, `redux`, `ssrSafe`, `subscribeWithSelector`,
`useShallow`, `shallow`, `create` and `ExtractState`, each a name the package
publishes under a subpath.

This workspace does not move, and that is the answer a private application
should get. `apps/kanban-app/ui/package.json` and `apps/mirdan-app/ui/package.json`
each state `"private": true` and name no `main`, no `exports` and no self
`paths` mapping, so neither declares a surface and nothing is exempt.

### What the manifest alone cannot answer

`package.json` names the paths a package PUBLISHES, and a library that builds
publishes build output. Measured over the corpus, counting each entry path of
each named package and asking whether it names a source file of the tree:

| package | entry paths | resolve to a source file |
|---|---|---|
| zod | 37 | 9 |
| redux | 4 | 0 |
| zustand | 5 | 0 |

`redux` names `dist/cjs/redux.cjs`, `dist/redux.mjs` and `dist/redux.d.mts`;
`zustand` names `./index.js` and `./*.js`. The source of each is `src/index.ts`,
and only the build configuration — `tsup.config.ts` for `redux` — states that
mapping. No arithmetic on the published path finds it: the bundle carries the
package's name and the source carries `index`.

So the manifest answers `zod` on its own and answers neither of the other two,
and the `paths` table answers those two. Both facts are read, because one of
them is not enough.

A `*` inside an `exports` path is dropped, and a `*` inside a `paths` target is
expanded. An `exports` subpath pattern maps the published tree, so `"./*"` over
build output at the package root would match every file of the repository, tests
included. A `paths` target names SOURCE, which is what TypeScript resolves it
against, so `./src/*.ts` expands to the source modules and to nothing else.

### The entry the run does not find

A package whose manifest names build output alone, and whose tsconfig writes no
self-reference, keeps its entry module under the gate. The author answers that
one with the staging marker below, or by adding the self-reference the
repository's own tests already want.

## The entry points a framework registers, which the marker answers

`dead-code` also exempts "framework-invoked handlers, CLI command callbacks,
registered hooks/callbacks — anything the runtime or a framework calls by
convention rather than by an in-repo call site". A module the module graph never
reaches, because a configuration file names it by path or a framework loads it by
file name, is that shape.

ts-prune has no plugin, no configuration reader, and no framework roster, so the
run cannot answer this one. The author writes `// ts-prune-ignore-next`.

The measurement states the cost. Of the 143 findings the shipped run leaves over
the four workspaces, 69 are this shape:

| shape | findings | where |
|---|---|---|
| a Next.js app-router `page`, `layout`, `route` or `not-found` module | 24 | `zod` `packages/docs/app/**` |
| a page-directory module of Next.js or of docusaurus | 4 | `zod` `packages/docs/pages/api/`, `redux` `website/src/pages/` |
| a build or test configuration module's `default` export | 10 | `vite.config.ts`, `vitest.config.ts`, `vitest.config.mts`, `tsup.config.ts`, `docusaurus.config.ts`, `source.config.ts` |
| a component only an `.mdx` page names | 16 | `zod` `packages/docs/components/**` |
| a module a bundler aliases by path, and a vitest browser command | 15 | this workspace, `src/test/stubs/` and `src/test/integration-commands.ts` |

The rest are real. This workspace's own 58 were hand-checked ten at a time
before this change and seven of the ten were dead: `BoardProgress`, an exported
React component nothing renders; `info`, `debug` and `trace`, three names of a
five-name re-export facade in `src/lib/log.ts` that no caller ever asked for;
and `clickInAct`, `getStrList`, `RecentBoard` and `minimalTheme`, each defined
once and referenced nowhere. The whole
`src/components/fields/displays/index.ts` barrel is in the same list: it is a
NESTED barrel that every consumer bypasses by importing the concrete module, no
manifest publishes it, and it stays reported.

`knip` answers this shape with a plugin for each framework, and the survey below
records that verdict.

## The tool survey, and what ts-prune cannot do

`ts-prune` 0.10.3 was published on 2021-12-12 and its repository is ARCHIVED.
Its README opens with a maintenance notice added by its own author on
2025-09-19: "**ts-prune is now in maintenance mode** - For new projects, we
recommend [knip](https://github.com/webpro/knip) which carries forward the same
mission with more features." The whole space was read again before this rule was
changed:

| tool | latest | published | state |
|---|---|---|---|
| `ts-prune` | 0.10.3 | 2021-12-12 | archived, maintenance mode, names knip |
| `knip` | 6.32.2 | 2026-08-11 | active |
| `ts-unused-exports` | 11.0.1 | 2024-11-25 | quiet, volunteer-maintained |
| `unimported` | 1.31.1 | 2023-11-18 | archived, deprecated on npm, names knip |
| `depcheck` | 1.4.7 | 2023-10-17 | archived, names knip; dependencies only |
| `eslint-plugin-import` `no-unused-modules` | 2.32.0 | 2025-06-20 | active |
| `oxlint` | 1.78.0 | 2026-08-10 | no unused-export rule |
| `@biomejs/biome` | 2.5.8 | 2026-08-11 | no unused-export rule |

`oxlint` and Biome cannot answer the question at all today. `oxlint` leaves
`import/no-unused-modules` unchecked in its tracking issue, and Biome's
`noUnusedExports` is an open discussion.

### The knip decision, and what decided it

`ts-prune` IS KEPT. The whole question was re-opened against `knip 6.32.2` and
settled on measurement, and this section is that record so the next reader
re-opens it with numbers rather than a survey.

**The table this decision replaces was measured in a broken state.** An earlier
comparison read `zod` 78 against 13, `zustand` 1 against 0 and `redux` 6 against
2, and every one of those knip numbers came from a repository with NO
`node_modules`. knip is DEGRADED there and says so on stderr —
`ERROR: Error loading vitest.config.ts (Cannot find module 'vitest/config')` —
while still exiting 1. `zustand`'s 0 was a failed configuration load, not a clean
tree. Two spellings in that command were dead as well: `nsExports` and `nsTypes`
select nothing in 6.32.2, and the live key is `namespaceMembers`.

Re-measured with dependencies installed, and with knip TUNED — handed the entry
list this rule's own node script already computes, an `ignore` built from each
tsconfig's `exclude`, and `ignoreExportsUsedInFile: true`, which is the lever
that matches this rule's own `(used in module)` drop:

| workspace | this rule | knip tuned | this rule, time | knip, time |
|---|---|---|---|---|
| zod | 76 | 7 | 10.8 s | 0.4 s |
| zustand | 1 | 0 | 0.9 s | 0.4 s |
| redux | 6 | 1 | 1.7 s | 0.5 s |

Both losses an untuned knip showed are configurable away, so knip was NOT
rejected on a configuration nobody tried — the earlier rejection in
`VALIDATOR.md` made that error once. Naming zustand's `paths` entries takes it
from 8 findings to 0; an `ignore` naming `examples/**` takes redux from 4 to 1.

Every finding of both tools was then read by hand:

| workspace | tool | findings | genuinely dead | framework entry | used in own file | wrong |
|---|---|---|---|---|---|---|
| zod | this rule | 76 | 28 | 47 | 0 | 1 |
| zod | knip tuned | 7 | 4 | 0 | 3 | 0 |
| zustand | this rule | 1 | 0 | 1 | 0 | 0 |
| zustand | knip tuned | 0 | 0 | 0 | 0 | 0 |
| redux | this rule | 6 | 1 | 5 | 0 | 0 |
| redux | knip tuned | 1 | 1 | 0 | 0 | 0 |

knip wins PRECISION: 5 of its 8 findings are genuinely dead, 63 %, against 29 of
this rule's 83, 35 %, and no knip finding is wrong against one of this rule's.
It is faster on every workspace, and 0.4 s against 10.8 s on zod. It loses
RECALL, and recall is what decided this.

The two finding lists were then compared symbol by symbol, which is what the
counts above cannot show on their own:

| workspace | genuinely dead, this rule | genuinely dead, knip | both name | this rule alone | knip alone | union |
|---|---|---|---|---|---|---|
| zod | 28 | 4 | 0 | 28 | 4 | 32 |
| zustand | 0 | 0 | 0 | 0 | 0 | 0 |
| redux | 1 | 1 | 1 | 0 | 0 | 1 |
| all three | 29 | 5 | 1 | 28 | 4 | 33 |

**Of the 33 genuinely dead symbols the two tools jointly name, this rule names
29 and tuned knip names 5.** The two agree on ONE symbol, `redux`
`scripts/mangleErrors.mts:187` `default`. Every other symbol belongs to one list
alone, so the swap gives up 28 and gains 4.

The 4 knip names alone all stand in `zod`. One is
`packages/docs/components/tabs.tsx:61` `Tab`, which the local module re-exports
and every `.mdx` page takes from `fumadocs-ui` instead. Three are members of the
`StandardSchemaV1` namespace in `packages/zod/src/v4/core/standard-schema.ts` —
`Types` at 86, `InferInput` at 89 and `InferOutput` at 92. This rule reports none
of the 4. The one export it does report in that second file,
`StandardSchemaWithJSON` at 157, was the one finding with the corrupted path,
and the hand-check counted it wrong for that path alone. "How the run is shaped"
below states the change that gave it the path it stands at.

knip names 6 members of that ONE namespace, and the tables above split them: 3
are counted genuinely dead and 3 are counted false positives. **The criterion is
whether any declaration names the member at all, its own file included.** A
member some other declaration in the same file names is live, and knip reporting
it is the false positive; a member nothing names anywhere is dead. Each of the 6
was read at the source and searched for over the whole checkout:

| the member knip names | named at | counted |
|---|---|---|
| `Result` at 50 | 46, in the same namespace | false positive |
| `SuccessResult` at 53 | 50, in the same namespace | false positive |
| `Issue` at 72 | 68, in the same namespace | false positive |
| `Types` at 86 | nowhere | genuinely dead |
| `InferInput` at 89 | nowhere | genuinely dead |
| `InferOutput` at 92 | nowhere | genuinely dead |

The two groups hold no symbol in common, so the counts stand. Three near-misses
were each read rather than counted: `StandardSchemaV1.Result` at
`packages/zod/src/v3/types.ts:247`, and `StandardSchemaV1.InferInput` and
`.InferOutput` at `packages/zod/src/v3/tests/standard-schema.test.ts:19` and
`:21`, each import a specifier that resolves to
`packages/zod/src/v3/standard-schema.ts` — `types.ts` writes
`./standard-schema.js` and the test file writes `../standard-schema.js` — which
is the SEPARATE `v3` declaration of the same name. The `Types`, `InferInput` and `InferOutput` at
141, 144 and 147 belong to a third namespace of the same file,
`StandardJSONSchemaV1`. Nothing outside
`packages/zod/src/v4/core/standard-schema.ts` names any of the 6.

The 3 false positives have ONE cause, and it is a PRECISION defect rather than a
recall one: **`ignoreExportsUsedInFile` does not reach `namespaceMembers`**, so
the lever that drops every other used-in-own-file export leaves these standing.
It holds back no genuinely dead symbol, so it is not one of the causes below.

Two causes hold the other 28 back, and no option reaches either:

- **knip will not enumerate the exports of a module no entry reaches.** It
  writes one `files` entry carrying a name and NO ROW, and stops. Measured on a
  probe publishing `src/index.ts` beside an unreferenced `src/orphan.ts` holding
  three dead exports: `--include exports,types,namespaceMembers,enumMembers`
  answers `{"issues":[]}` at exit 0, adding `files` answers one entry carrying
  `"exports":[]`, and this rule names all three at rows 2, 5 and 10. Over `zod`
  that shape swallows 27 of the 28 into six module lines with no names:

  | the module `zod` never reaches | dead symbols this rule names |
  |---|---|
  | `packages/zod/src/v4/core/zsf.ts` | 15 |
  | `packages/zod/src/v3/tests/language-server.source.ts` | 7 |
  | `packages/bench/memory/retainers.ts` | 2 |
  | `packages/treeshake/zod-full.ts` | 1 |
  | `packages/treeshake/zod-mini-full.ts` | 1 |
  | `packages/treeshake/zod3-full.ts` | 1 |
  | all six | 27 |

- **A dead export inside a module that IS an entry is invisible.** That is the
  28th symbol: `zod` `packages/tsc/generate.ts` `VALIBOT` is dead and knip
  answers `No exports found`, because a `package.json` script names the module.
  The one lever, `includeEntryExports`, is all or nothing: it takes zod to 362
  and zustand to 13.

27 + 1 exhausts the 28.

The first cause is fatal for THIS rule. Its claim is "every exported symbol no
other module in the project imports", reported as `path:line`, and the everyday
shape of that claim — an author adds a module nothing imports yet — is the one
shape knip cannot report by symbol. knip makes a different claim: no entry
reaches this file, and then, inside the files an entry does reach, nothing
imports this export. A better gate and a worse inventory. The acceptance test
`the_shipped_typescript_dead_code_tool_rule_names_every_export_of_a_module_nothing_imports`
holds the probe above, so the property that decided this cannot be lost quietly.

The other measurements the swap needed, recorded so no one repeats them:

| the question | the answer |
|---|---|
| the machine-readable shape | `{"issues":[{"file":..., "<type>":[{"name","line","col","pos"}]}]}`, one object for each FILE, `issues` the only top-level key |
| does it need an installed `node_modules` | YES. `redux` reports 2 findings without one and 7 with one, both at exit 1, and the only sign is stderr |
| `-c <file>` | takes a path outside the project and REPLACES `knip.json`, `.knip.json`, `knip.jsonc` and `knip.config.ts`, but NOT `package.json#knip`, so every option has to be stated |
| exit 0 | a clean run, `{"issues":[]}` |
| exit 1 | a run that found issues, and also a run degraded by a missing `node_modules` |
| exit 2 | a run knip refused to make; stdout is prose rather than JSON, and `--no-exit-code` cannot mask it |
| a `tsconfig.json` that is not JSON | NO signal at all: exit 1, the count unmoved, 174 bytes on stderr the only difference |

The last row is why the swap would NOT have answered the open card about this
rule answering zero for a tool that broke. Both tools are silent for that shape,
and a fail-closed test has to read stderr either way.

The swap would have taken the path defect away structurally: knip runs once at
the workspace root and writes each path relative to it, so the per-project
arithmetic this rule performs has no counterpart at all. This rule answers that
one by rebuilding every spelling the presenter's cut can have made, and "How the
run is shaped" states the measurement and the shapes that rebuild leaves.

`ts-prune` being archived is a real risk and it is not answered by keeping it.
The successor is named, the corpus is kept, and the tables above are directly
comparable, so this decision can be re-taken the moment knip enumerates the
exports of an unreachable module.

## The staging contract

Write `// ts-prune-ignore-next` on the line above an export a later change will
import. Nothing else counts. A staged export with no marker is dead.

Measured on a probe project: an unannotated `export const` reports, and the same
export behind `// ts-prune-ignore-next` does not. Put the reason on the same line
after the marker, so the next reader can tell staged work from a leftover.

**This marker does not expire, and that is a defect of the shipped tool rather
than of the contract.** `builtin/validators/README.md` states "Prefer a marker
that expires", because Rust's `#[expect(dead_code)]` raises
`unfulfilled_lint_expectations` the moment the consumer lands and cleans itself
up. `ts-prune` has no such report, so a `// ts-prune-ignore-next` outlives the
change that justified it and no run ever asks for it back.

`knip` DOES have one, and it is the one property where the successor is plainly
stronger. Measured with knip 6.32.2: a custom JSDoc tag —
`/** @staged the importer lands in the next change */` — filtered with
`--tags=-staged` silences an export of every kind, and the moment a real importer
lands knip writes `Unused tag in src/lib.ts: dStaged → @staged`, which
`--treat-tag-hints-as-errors` turns into a nonzero status. Three constraints
came with it: the tag must stand in a BLOCK comment, because `// @staged` is read
as nothing; the name must be one alpha word, because `dist/util/tag.js` matches
`/[a-zA-Z]+/` and `--tags=-sah` would therefore silence `@sah-staged` too; and
`@public`, `@beta` and `@alias` never expire, because `isAlwaysIgnored` returns
before the hint runs — beside carrying a TSDoc release-stage meaning that states
the opposite of "a consumer lands next".

The expiry did not carry the decision, because the section above shows the swap
giving up 28 of the 29 genuinely dead symbols this rule names, against 4 it
would gain. It is recorded here because it is the strongest argument for
re-opening the question.

## The rule owns its own gate

`ts-prune` reads a configuration of its own through cosmiconfig, and
`package.json#ts-prune` is one of the places it searches. It merges what it finds
UNDER the command line, so an option the run does not state is the project's to
set.

Measured with ts-prune 0.10.3 over a probe holding one dead export, beside a
`package.json` stating `"ts-prune": { "ignore": "src", "skip": "src" }`:

| the run | findings |
|---|---|
| `ts-prune -p tsconfig.json` | 0 |
| `ts-prune -p tsconfig.json --ignore '$^' --skip '$^'` | 1 |

Row 1 is a project turning the whole gate off without saying so. The run
therefore states both options on every call. `$^` is a regular expression that
matches no line, so a project with no entry module keeps the whole gate. The
acceptance test
`the_shipped_typescript_dead_code_tool_rule_keeps_its_own_gate_beside_a_project_config`
holds row 2.

`--ignore` is matched against the whole REPORT LINE rather than against the
path, so the pattern the run builds is anchored — `^(?:<path>|<path>):` — and
each path in it is escaped. Without the anchor a path would also silence an
export whose NAME held the same text.

## The program is the project's own

`ts-prune` reads a `tsconfig.json` to build the module graph, and that file is
the project's list of its own files. `dead-code` exempts "test functions and
test-only helpers", and a test inside the program needs no exemption: it is a
caller, so the export it imports is not dead.

A project that excludes its tests from its own `tsconfig.json` takes those
callers out of the graph. Measured on a probe holding one export a test imports
and one export nothing imports:

| the project's `tsconfig.json` | findings |
|---|---|
| `include: ["src"]` | the export nothing imports |
| the same, plus `exclude: ["src/**/*.test.ts"]` | both exports |

Which files a program holds is the project's decision, and
`builtin/validators/README.md` states that a script may read the project's own
configuration for the FILE LIST. So the run reads the list and never rewrites
it, and an author whose project excludes its tests answers with the marker.

Measured over the 16 `tsconfig.json` projects the corpus table counts — 9, 2, 3
and 2 — each one holds every test file that stands beside the sources it names.
The acceptance test
`the_shipped_typescript_dead_code_tool_rule_reads_the_program_the_project_states`
holds both rows of the table.

## What the pipe drops, and why

**`(used in module)`.** `ts-prune` marks an export that its own file uses. That
symbol is alive; only the `export` keyword is surplus. This rule reports dead
code, not surplus keywords, so those lines are dropped. Measured over
`apps/kanban-app/ui`: 178 raw lines, 103 of them carrying the marker.

**`.d.ts` files.** A declaration file is generated or ambient, and its
declarations exist to be read by the compiler rather than imported. Measured on
the same tree: 18 of the remaining 75 lines came from one generated file,
`src/lang-filter/parser.terms.d.ts`, which a lezer grammar build writes.

Both are attribution, not exemption. To exempt one export, write the marker in
the code.

## How the run is shaped

The scope is `workspace` because "nothing imports it" is a whole-project
question. The script finds every `tsconfig.json` outside `node_modules`, runs the
tool in that project's own directory, and rebuilds each finding at the path the
file stands at. That is how a monorepo whose root carries no `tsconfig.json`
still gets checked. The engine keeps only the findings in the changed files.

### The path each finding is rebuilt at

ts-prune's presenter writes
`result.file.replace(process.cwd(), "").replace(/^\//, "")`. That first
`replace` is given a STRING, so it cuts the FIRST occurrence of the working
directory wherever that text stands and needs no separator after the match.
Four spellings come out of it. The working directory ts-prune ran in tells the
first three apart, and it rebuilds none of the fourth:

| the file the program holds | what the presenter writes |
|---|---|
| inside the project directory | the path under that directory |
| in a sibling directory whose name BEGINS with the project's | the rest of that name and then the path: `packages/zod-bench/src/x.ts` under the project `packages/zod` comes out as `-bench/src/x.ts` |
| outside the project, and its path holds the project path nowhere | the whole absolute path less its leading separator |
| outside the project, and its path holds the project path at a LATER position | the path cut at that position: `/mnt/backup/w/packages/a/src/x.ts` under the project `/w/packages/a` comes out as `mnt/backup/src/x.ts`. A nested copy of an absolute tree — a backup mount, a bind mount, a staged copy — makes that shape |

The run rebuilds the first three from that working directory and reports the
finding at the one that stands as a file. The fourth stands at none of them, so
the run drops it.

`reportedAs` in the node script is the same presenter operation, copied rather
than modelled, because the `--ignore` pattern has to name the spelling ts-prune
writes. The pipe and the pattern never meet: `runner.js` applies `--ignore` to
the presented line INSIDE ts-prune —
`presented.filter(function (file) { return !file.match(config.ignore); })` —
before a byte reaches stdout, so no rewriting the pipe does can reach that
decision. Both sides copy the presenter, and neither reads the other.

Putting the project's path in front of EVERY spelling names a file that stands
nowhere. The engine keeps a workspace-scope finding only when its path meets a
file of the run, so such a finding is dropped without a word — a silent miss.
Testing one completed path against the filesystem instead is worse: where the
cut fell inside a sibling's name it lands on a REAL file the finding is not
about, which is a wrong finding. Measured over the corpus with the dependencies
installed:

| workspace | findings | naming a file that is nowhere | dropped |
|---|---|---|---|
| zod | 76 | 0 | 0 |
| zustand | 1 | 0 | 0 |
| redux | 6 | 0 | 0 |
| this workspace | 58 | 0 | 0 |

The count does not move, and one finding of zod's 76 is respelled. It is
`packages/zod/src/v4/core/standard-schema.ts:157` `StandardSchemaWithJSON`,
which `packages/bench` reaches: that project's program holds
`packages/zod/src`. That finding has been spelled three ways.
`packages/bench/<the absolute path of the checkout>/packages/zod/src/…` named
no file at all. The absolute path of the checkout named the right file on the
machine that ran it and no file on any other. The path above names it
everywhere.

One leak is what the entry carve-out leaves. With the carve-out off, the same
project reaches `packages/zod/src/index.ts` as well, and its 284 findings carry
the same cut spelling — 285 of that run's 1944 rows. Every one of the 284
stands in a module the package publishes, so the carve-out already silences
them, and the shipped run leaks the one finding that stands outside a published
module.

**Two shapes the rebuild cannot close, and the run drops rather than guesses.**

Two files of one program can wear the SAME spelling. A sibling directory
`packages/consumersrc` beside the project `packages/consumer` makes
`packages/consumersrc/lib.ts` come out as `src/lib.ts`, which is also what the
project's own `packages/consumer/src/lib.ts` comes out as. The cut threw the
text that told them apart away, and nothing left on the line brings it back.

One file can also wear a spelling NO file answers. That is the fourth row of
the table above: `/mnt/backup/w/packages/a/src/x.ts` under the project
`/w/packages/a` comes out as `mnt/backup/src/x.ts`, and no rebuild reaches it.

The run reports no finding at either shape. It says the drop out loud instead,
on STDOUT, at row 1 of the project's own `tsconfig.json`:

```
packages/consumer/tsconfig.json:1: dead-code-typescript dropped one finding. 2 files of this program stand at the spelling `src/lib.ts`. The run cannot tell which file the finding is about. The dropped line is `src/lib.ts:2: unused export 'trulyDead'; nothing in the project imports it`
```

stdout is the channel a finding travels, so the drop travels it. stderr reaches
nobody: the runner pipes that stream and reads it only for a run that exits
NONZERO, and this script exits 0. The tsconfig is the file whose program made
the spelling, and it always stands as a file, so the line names a path the run
can confirm. A drop is therefore observably different from a clean run, which a
line on stderr was not. The engine still keeps a workspace-scope finding only
when its path meets a file of the run, so the drop reaches the report when the
project's own `tsconfig.json` is one of the changed files.

The drop is a lost finding rather than a wrong one. Measured over the four
workspaces of the table above: 0 dropped of their 141 findings.

The acceptance tests
`the_shipped_typescript_dead_code_tool_rule_names_a_module_outside_the_project_directory`,
`the_shipped_typescript_dead_code_tool_rule_names_no_file_that_is_not_the_file_of_the_finding`
and
`the_shipped_typescript_dead_code_tool_rule_says_the_finding_it_drops_out_loud`
hold the first three spellings over probe repositories, hold the drop and the
sentence it writes, and hold the outside module through the engine as well,
where the run answered NO finding before this change.

`tsc --showConfig` is what reads the tsconfig, because `paths` usually stands in
a file the project EXTENDS: `redux` writes its table in `tsconfig.base.json`, and
`zod` writes `.configs/tsconfig.base.json`. `--showConfig` resolves the
`extends` chain and prints one JSON document, and it also reads the comments and
the trailing commas a `tsconfig.json` may hold and a `JSON.parse` may not —
`redux/tsconfig.json` opens with a comment and `zod/packages/zod/tsconfig.json`
ends with a trailing comma.

Entry resolution fails OPEN. A `tsc` run that writes no configuration, and a
manifest that does not parse, leave the pattern empty, the run then states
`--ignore '$^'`, and every export of that project stays under the gate. A failure
adds findings and never takes one away, so it cannot answer clean for a broken
read. The node script names the project on stderr when that happens.

`sort -u` collapses the duplicate a nested `tsconfig.json` produces when two
projects hold the same file. The entry pattern is computed for each project, so
the duplicate rows agree.

Ending the pipe in `sed`, and the loop in `sort`, normalizes the exit status.

`mktemp -d` makes the directory the node script is written into, and
`trap 'rm -rf "$work"' EXIT` removes it. The scope is `workspace`, so this
script takes no file argument.

## What the fixture pair holds, and what it cannot

The fixture pair holds the staging contract: the fail fixture carries one
unannotated unused export of five kinds, and the pass fixture carries the same
five behind the marker.

It cannot hold either carve-out above. Doctor counts only the findings a run
reports ABOUT the fixture under test, and both carve-outs take findings off a
DIFFERENT file — the package's entry module — so a manifest in `fixtures/` would
move neither count. The nine acceptance tests in
`tests/shipped/dead_code_typescript.rs` drive the shipped script over probe
repositories instead, and each one names the fact it holds.

It cannot hold the module-level claim either. A fixture directory is flat and
every fixture stands in the same program, so no fixture can be a module NOTHING
imports while another is the entry that spares it. The acceptance test
`the_shipped_typescript_dead_code_tool_rule_names_every_export_of_a_module_nothing_imports`
stages that shape as a probe repository, and it is the test that pins the
property the knip decision turned on.

Nor can it hold the path arithmetic. A fixture stands loose in one directory, so
no fixture is a module one project reaches from outside itself, and no fixture
stands in a package whose name begins with another package's. The three
acceptance tests named in "How the run is shaped" stage sibling packages for
that.
