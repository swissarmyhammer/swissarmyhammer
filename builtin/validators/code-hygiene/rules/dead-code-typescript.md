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
    cat > "$work/prune.js" <<'PRUNE_JOBS'
    // The two jobs this rule needs from node. Both read ONE operation — how
    // ts-prune's presenter spells the path of a file — so this program carries
    // one copy of it and neither job models the other.
    //
    // `entries` writes the ts-prune --ignore pattern naming the entry modules
    // of one TypeScript project. An entry module is a source file that a
    // package manifest of this workspace publishes, or that a tsconfig `paths`
    // mapping puts under a workspace package's own name. Both are a package
    // stating its own surface.
    //
    // `place` reads the findings of one ts-prune run on stdin and writes each
    // one at the file of the program that finding is about.
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

    // The marker `builtin/validators/README.md` states a declined item by.
    const DIAGNOSTIC_MARKER = "sah-diagnostic:";

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
     * a spelling ts-prune never writes: the `--ignore` pattern then misses the
     * line, and the placement then meets no file of the program.
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
        } catch (failure) {
          decline(
            `the manifest ${reportedPath(file, workspaceRoot)} does not parse, so the entry modules it publishes stay under the gate: ${failure}`,
          );
        }
      }
      return found;
    }

    /** States one thing this run could not read or place, on the report's channel. */
    function decline(message) {
      process.stderr.write(`${DIAGNOSTIC_MARKER} ${message}\n`);
    }

    /** Writes the ts-prune `--ignore` pattern of the project this run stands in. */
    function writeEntryPattern(workspaceRootArgument, configPath) {
      const projectDir = process.cwd();
      const workspaceRoot = path.resolve(workspaceRootArgument);
      const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
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

    /** Every file `tsc` listed for the program, absolute. */
    function programFiles(listPath) {
      let listed;
      try {
        listed = fs.readFileSync(listPath, "utf8");
      } catch {
        return [];
      }
      return listed
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line !== "");
    }

    /** The real path of `file`, or `file` itself when the run cannot read it. */
    function realPath(file) {
      try {
        return fs.realpathSync(file);
      } catch {
        return file;
      }
    }

    /**
     * Each file of the program by its real path, with every spelling ts-prune
     * can write for it.
     *
     * The two readings of the program spell a file differently: `tsc
     * --listFilesOnly` prints the path it globbed, and ts-prune reports
     * `fs.realpathSync(result.file)` — `ts-prune/lib/analyzer.js`. A file the
     * list reached through a symbolic link therefore stands under two
     * spellings, and holding both is what puts the file ts-prune reported into
     * the list its spelling is matched against. Two entries of the list that
     * resolve to ONE real file are one file here, so a link beside its own
     * target is not a collision.
     */
    function realFiles(files, projectDir) {
      const byReal = new Map();
      for (const file of files) {
        const real = realPath(file);
        const spellings = byReal.get(real) || new Set();
        spellings.add(reportedAs(file, projectDir));
        spellings.add(reportedAs(real, projectDir));
        byReal.set(real, spellings);
      }
      return byReal;
    }

    /** The files of the program that carry each spelling ts-prune can write. */
    function filesBySpelling(files, projectDir) {
      const carried = new Map();
      for (const [real, spellings] of realFiles(files, projectDir)) {
        for (const spelling of spellings) {
          const standing = carried.get(spelling);
          if (standing) standing.push(real);
          else carried.set(spelling, [real]);
        }
      }
      return carried;
    }

    /** `workspaceRoot` with a separator after it, so a test names a whole directory. */
    function workspacePrefix(workspaceRoot) {
      return workspaceRoot.endsWith(path.sep)
        ? workspaceRoot
        : workspaceRoot + path.sep;
    }

    /**
     * Whether `file` stands inside the workspace at `workspaceRoot`.
     *
     * This test is anchored and it demands a separator, because the run WRITES
     * this path rather than reading one: nothing here has to guess where a cut
     * fell.
     */
    function insideWorkspace(file, workspaceRoot) {
      return file.startsWith(workspacePrefix(workspaceRoot));
    }

    /**
     * The path the report names `file` at.
     *
     * A file of the workspace is named the way the work-list holds it, and a
     * file outside the workspace keeps its whole path. The report carries the
     * whole path in the message of a declined item alone, never as the path of
     * a finding.
     */
    function reportedPath(file, workspaceRoot) {
      return insideWorkspace(file, workspaceRoot)
        ? file.slice(workspacePrefix(workspaceRoot).length)
        : file;
    }

    /**
     * Writes each finding of stdin at the file of the program it is about.
     *
     * Each line the pipe hands over is `<spelling>:<row>: <message>`, and the
     * spelling is what the presenter wrote. That cut throws text away, so no
     * arithmetic on the line reads the path back. The run spells each file of
     * the PROGRAM the same way instead, and answers with the file that carries
     * the spelling ts-prune wrote.
     *
     * `standing` counts the real files of the program that carry the spelling,
     * each of them spelled both the way `tsc` listed it and the way its real
     * path spells it. So the file ts-prune reported is itself one of the
     * candidates, and one candidate means that candidate is it. No candidate,
     * or two, is an item this run cannot place, so it declines the item and
     * says which one.
     *
     * The row carries the REAL path, which is where the export text stands. A
     * real path outside the workspace is one the report has no row for: the
     * engine keeps a workspace-scope finding only when its path meets a file of
     * the run, and every file of a run stands under the workspace root. So such
     * an item is declined as well, rather than written at a path that leaves
     * the report without a word.
     */
    function placeFindings(config, workspaceRootArgument, listPath) {
      const projectDir = process.cwd();
      const workspaceRoot = path.resolve(workspaceRootArgument);
      const findings = fs
        .readFileSync(0, "utf8")
        .split("\n")
        .filter((line) => line !== "");
      const files = programFiles(listPath);
      if (files.length === 0) {
        if (findings.length > 0) {
          decline(
            `the program of ${config} lists no file, so the run placed none of its ${findings.length} findings`,
          );
        }
        return;
      }

      const carried = filesBySpelling(files, projectDir);
      for (const finding of findings) {
        const at = finding.indexOf(":");
        const spelling = finding.slice(0, at);
        const standing = carried.get(spelling) || [];
        if (standing.length !== 1) {
          decline(
            `${standing.length} files of the program of ${config} carry the spelling \`${spelling}\`, so the run cannot tell which file this finding is about: \`${finding}\``,
          );
          continue;
        }
        if (!insideWorkspace(standing[0], workspaceRoot)) {
          decline(
            `the file this finding is about stands outside the workspace, at \`${standing[0]}\`, and the report carries a file of the workspace alone: \`${finding}\``,
          );
          continue;
        }
        process.stdout.write(
          `${reportedPath(standing[0], workspaceRoot)}:${finding.slice(at + 1)}\n`,
        );
      }
    }

    /** The job the command line names, with the arguments that job takes. */
    function main() {
      if (process.argv[2] === "entries") {
        writeEntryPattern(process.argv[3], process.argv[4]);
        return;
      }
      placeFindings(process.argv[3], process.argv[4], process.argv[5]);
    }

    try {
      main();
    } catch (failure) {
      decline(
        `the ${process.argv[2]} job broke in ${process.cwd()}: ${failure}`,
      );
    }
    PRUNE_JOBS
    root="$(pwd -P)"
    find . -name node_modules -prune -o -name .git -prune -o -name 'tsconfig.json' -print \
      > "$work/projects.txt"
    : > "$work/rows.txt"
    : > "$work/unread.txt"
    while IFS= read -r config; do
      dir="${config%/*}"
      (cd "$dir" && tsc -p tsconfig.json --showConfig) > "$work/config.json" 2>/dev/null
      (cd "$dir" && tsc -p tsconfig.json --listFilesOnly) > "$work/files.txt" 2>/dev/null
      entries="$(cd "$dir" && node "$work/prune.js" entries "$root" "$work/config.json")"
      [ -n "$entries" ] || entries='$^'
      (cd "$dir" && ts-prune -p tsconfig.json --ignore "$entries" --skip '$^') \
        > "$work/prune.out" 2> "$work/prune.err"
      status=$?
      cat "$work/prune.err" >&2
      if [ "$status" -ne 0 ]; then
        printf 'dead-code-typescript: ts-prune exited %s for %s and judged no export of it\n' \
          "$status" "${config#./}" >&2
        printf '%s\n' "${config#./}" >> "$work/unread.txt"
        continue
      fi
      grep -v ' (used in module)$' "$work/prune.out" |
        grep -v '\.d\.ts:' |
        sed -n "s#^\([^:]*\):\([0-9]*\) - \(.*\)\$#\1:\2: unused export '\3'; nothing in the project imports it#p" |
        (cd "$dir" && node "$work/prune.js" place "${config#./}" "$root" "$work/files.txt") \
        >> "$work/rows.txt"
    done < "$work/projects.txt"
    if [ -s "$work/unread.txt" ]; then
      exit 1
    fi
    sort -u "$work/rows.txt"
  doctor:
    check_command: "which ts-prune tsc node find grep sed sort cat mktemp"
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
the workspace root and writes each path relative to it, so the per-project path
question this rule answers does not arise there at all. This rule answers that
one by reading the file list of each program and matching the spelling the
presenter wrote, and "How the run is shaped" states that reading and the items
it declines.

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
tool in that project's own directory, and reports each finding at the path the
file stands at. That is how a monorepo whose root carries no `tsconfig.json`
still gets checked. The engine keeps only the findings in the changed files.

### The path each finding is reported at

ts-prune's presenter writes
`result.file.replace(process.cwd(), "").replace(/^\//, "")`. That first
`replace` is given a STRING, so it cuts the FIRST occurrence of the working
directory wherever that text stands and needs no separator after the match.
The cut throws text away, and nothing left on the line brings it back. A
sibling directory `packages/consumersrc` beside the project
`packages/consumer` states that plainly: `packages/consumersrc/lib.ts` comes
out as `src/lib.ts`, which is also what the project's own
`packages/consumer/src/lib.ts` comes out as.

So the run never reads the cut back. It reads the FILE LIST of the program
instead — `tsc -p tsconfig.json --listFilesOnly` prints the absolute path of
every file the project's own program holds, and stops before the type check —
and it spells each of those files the way the presenter spells it. The file
whose spelling meets the reported one is the file of the finding.

The two readings of the program spell one file two ways, and the run holds
both. `tsc` prints the path it globbed or was given, and ts-prune reports
`fs.realpathSync(result.file)` — `ts-prune/lib/analyzer.js`. A `files` entry
that is a symbolic link is one shape where the two differ: `tsc` lists
`src/link.ts`, and ts-prune reports the export at the path behind the link. So
the run spells each listed file BOTH ways — as `tsc` wrote it, and as its real
path spells it — and counts the REAL files of the list that carry the reported
spelling. Two listed entries that resolve to one real file are one file here,
so a link listed beside its own target is not a collision.

That count is a count of the PROGRAM and not of the filesystem: a file that
merely stands on disk carries no spelling here. One file carrying the spelling
is the file of the finding when `tsc` listed that file, and `tsc` and ts-prune
build their program from the same `tsconfig.json`, so the list holds what
ts-prune read. A file ts-prune reported that `tsc` listed under NEITHER
spelling stands outside what this count can see: the run then reads no
candidate and declines, unless some other file of the list wears the same cut
spelling. That is the whole of what the count rests on, stated rather than
assumed.

The row the run writes carries that REAL path, because that is where the export
text stands. For the `files` entry that is a symbolic link above, the report
reads the file behind the link and never `src/link.ts`, where that file stands
inside the workspace.

A real path OUTSIDE the workspace has no row at all. The engine keeps a
workspace-scope finding only when its path meets a file of the run, and every
file of a run stands under the workspace root, so a row written at such a path
would leave the report without a word. The run declines that item instead.

`reportedAs` in the node script is the presenter's own operation, copied rather
than modelled, and BOTH jobs of that script read it: the `--ignore` pattern has
to name the spelling ts-prune writes, and the placement has to read the
spelling ts-prune wrote. The pipe and the pattern never meet: `runner.js`
applies `--ignore` to the presented line INSIDE ts-prune —
`presented.filter(function (file) { return !file.match(config.ignore); })` —
before a byte reaches stdout, so no rewriting the pipe does can reach that
decision. One operation, one copy, two readers.

The two shapes this replaces each answered the ambiguity with a guess. Putting
the project's path in front of EVERY spelling named a file that stands nowhere,
and the engine keeps a workspace-scope finding only when its path meets a file
of the run, so such a finding was dropped without a word — a silent miss.
Completing one path and testing it against the filesystem instead named a REAL
file the finding was not about wherever the cut fell inside a sibling's name —
a wrong finding. The file list answers both, up to the residue the count states
above. Every path the run writes is a path `tsc` listed for the program, or the
real path of such a path: one is what `tsc` printed, the other is what the
filesystem answers for it, and neither is a path the run made up.

**An item the run cannot report at a file of the workspace is DECLINED.**

The run reports no finding for that item, and states it on stderr, on a line
the engine reads:

```
sah-diagnostic: 2 files of the program of packages/consumer/tsconfig.json carry the spelling `src/lib.ts`, so the run cannot tell which file this finding is about: `src/lib.ts:2: unused export 'trulyDead'; nothing in the project imports it`
```

`builtin/validators/README.md` states that channel: a script that judged the
code and could not judge ONE item says so on a line opening `sah-diagnostic:`
and still exits 0, and the report states each marked line. No file filter can
drop it, because a diagnostic is about the RUN rather than about a reviewed
file and has no path to be kept by. A declined item is therefore a lost finding
and never a silent one. The earlier shape of this rule could not say that: it
wrote its drops at the project's own `tsconfig.json`, and the rule's own
`match.files` globs — `**/*.ts`, `**/*.tsx`, `**/*.js`, `**/*.jsx`, `**/*.mjs`
and `**/*.cjs` — select no `.json` path, so the workspace retain discarded
every one of them on every run.

The placement writes such a line where the node script calls `decline`, and
each reason is a fact rather than a judgment:

- Two files of the program carry the spelling. The `consumersrc` shape above is
  one.
- No file of the program carries it. Nothing `tsc` listed spells, as listed or
  by its real path, the way ts-prune spelled the file it read.
- The program lists no file at all. That is one line for the whole project
  rather than one for each of its findings.
- The one file that carries it stands outside the workspace. The row would
  carry that whole path, and the engine keeps no finding at such a path, so the
  row would leave the report without a word. A `files` entry that is a symbolic
  link onto a file standing outside the workspace is one shape that reaches
  this.
- The job broke. The `try` around the `main()` call catches what it threw and
  names the job, the directory and the failure, so a `place` that could not run
  says so rather than writing nothing. `main` itself holds no `try`.

The entry job writes on the same channel, and "Entry resolution fails OPEN"
below states what its lines carry.

Measured over this workspace, three shipped scripts run one after another and
that cycle repeated three times, warm, on a machine running other work beside
them:

| placement | the three readings | lowest | spread |
|---|---|---|---|
| the shell placement this replaced | 6.31 s, 6.72 s, 6.76 s | 6.31 s | 0.45 s |
| the file-list placement | 6.86 s, 6.96 s, 8.18 s | 6.86 s | 1.32 s |
| the same, reading the real path of each listed file as well | 7.17 s, 7.37 s, 7.85 s | 7.17 s | 0.68 s |

All three answered 58 findings, the same bytes on stdout, 0 items declined and 0
bytes on stderr.

The 0.55 s between the first two lowest readings is what the WHOLE placement
costs over the shell loop it replaced — a `tsc -p tsconfig.json --listFilesOnly`
and a `node "$work/prune.js" place` for each of this workspace's two projects,
against the per-finding shell loop that went. This measurement does not divide
that 0.55 s between them, and it does not say which of them carries it. Reading
the real path of each listed file beside them adds a further 0.31 s at the
lowest reading, under each of the three spreads above, so this measurement does
not tell that cost from noise either.

A reading moves between sessions. The table under "The exported public API,
which the manifests answer" reads 6.2 s for this workspace, taken in an earlier
one. The three readings above are therefore compared with each other, never with
a reading of another session.

**The three library rows below were measured under the SHELL placement**, the
first row of the timing table above, and those checkouts no longer stand on the
machine that measured them, so this change re-measured this workspace alone. The
`declined` column is what the shipped run answers, and it is stated for the row
that was re-measured:

| workspace | findings | naming a file that is nowhere | declined |
|---|---|---|---|
| zod | 76 | 0 | — |
| zustand | 1 | 0 | — |
| redux | 6 | 0 | — |
| this workspace | 58 | 0 | 0 |

The placement reads the same rows ts-prune wrote and writes one row for each row
it places, so it adds no row. A row can leave the list without the report
stating it. The run states each item it declines. `sort -u` collapses two rows
that come to spell one file alike and says nothing — which the
real-path spelling can do where one project reaches a file through a link and
another reaches it directly. So a re-measurement reads a declined item off the
report and a collapse off the duplicate rows, rather than off a diff of two
lists.

The one finding of zod's 76 that this section is about is
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

Five acceptance tests hold this section, and each names the placement it reads
against rather than an earlier state of the file.
`..._names_a_module_outside_the_project_directory` holds a module the program
reaches from outside the project, and drives it through the engine as well: the
placement that put the project's path in front of EVERY spelling answered no
finding there.
`..._names_no_file_that_is_not_the_file_of_the_finding` holds two sibling
packages whose names begin with the project's own.
`..._says_the_finding_it_declines_out_loud` holds the declined item beside the
sentence it states. `..._places_a_file_the_two_readings_spell_differently`
holds a `files` entry that is a symbolic link, which is the shape the two
spellings are read for: the file-list placement that read no real path declined
that item and reported nothing.
`..._says_the_file_outside_the_workspace_out_loud` holds the same shape with the
link's target standing beside the repository rather than inside it, and reads
the diagnostic: the file-list placement that read the real path and wrote it
whatever it was put that row on stdout at its whole absolute path, which is no
file of the run, so the engine dropped it without a word.

`tsc --showConfig` is what reads the tsconfig, because `paths` usually stands in
a file the project EXTENDS: `redux` writes its table in `tsconfig.base.json`, and
`zod` writes `.configs/tsconfig.base.json`. `--showConfig` resolves the
`extends` chain and prints one JSON document, and it also reads the comments and
the trailing commas a `tsconfig.json` may hold and a `JSON.parse` may not —
`redux/tsconfig.json` opens with a comment and `zod/packages/zod/tsconfig.json`
ends with a trailing comma.

Entry resolution fails OPEN, and each failure is stated with the
`sah-diagnostic:` marker so the report carries it; an unmarked line on stderr
reaches a `tracing` record and no reader of the review.

A `tsc` run that writes no configuration a `JSON.parse` can read takes the
whole job with it: the parse throws, the `try` around the `main()` call catches
it and states `the entries job broke in <directory>: <failure>`, the pattern is
empty, the run states
`--ignore '$^'`, and every export of that project stays under the gate. A
manifest that does not parse is NARROWER. The
other manifests still build the pattern, and only the entry modules of that one
package fall out of it, so every export of them reports as dead. That reads on
the report exactly like a module nothing imports, which is why the run names
the manifest it could not read: nothing else on any channel tells the author
why a published entry module is on the list.

Either failure adds findings and never takes one away, so neither can answer
clean for a broken read. The acceptance test
`..._says_the_manifest_it_could_not_read_out_loud` stages one manifest that
parses beside one that does not, and holds both halves: the entry of the whole
package spared, the entry of the broken package reported, and the manifest
named.

`sort -u` collapses the duplicate a nested `tsconfig.json` produces when two
projects hold the same file. The entry pattern is computed for each project, so
the duplicate rows agree. It reads the row file the loop wrote rather than a
pipe, so it stands last with no command above it and the script takes its own
status.

### A run cannot answer zero for a project ts-prune never read

`ts-prune` runs one time for each project of the workspace, and it keeps one
status for a run it made and another for a run it could not make. Measured with
ts-prune 0.10.3, tsc 5.9.3 and node v25.2.1, each run as
`ts-prune -p tsconfig.json --ignore '$^' --skip '$^'`, which is the command line
this script writes for a project that names no entry module, with the project's
own directory the working directory. The table is the record of the ten runs
that were made, and not a roster of every status ts-prune can answer with:

| the run | status | stdout | stderr |
|---|---|---|---|
| a project whose every export another module imports | 0 | 0 bytes | 0 bytes |
| a project holding two exports nothing imports | 0 | 2 rows | 0 bytes |
| a `tsconfig.json` whose `include` reaches no file | 0 | 0 bytes | 0 bytes |
| a `tsconfig.json` naming a `compilerOptions.target` that is not a target | 0 | 2 rows | 0 bytes |
| a `tsconfig.json` whose `extends` names a file that is not there | 0 | 2 rows | 0 bytes |
| a `tsconfig.json` of bytes that are not JSON | 1 | 0 bytes | a `@ts-morph/common` stack |
| a `tsconfig.json` whose root value is `[]` | 1 | 0 bytes | a `@ts-morph/common` stack |
| no file at the path `-p` names | 1 | 0 bytes | a `@ts-morph/common` stack |
| a `tsconfig.json` no read permission admits | 1 | 0 bytes | a stack naming `EACCES` |
| a `package.json` cosmiconfig reads on the way up, of bytes that are not JSON | 1 | 0 bytes | a cosmiconfig stack |

No byte count stands in the table for a row that answered a stack, because a
node stack carries the absolute path of the checkout and a run from another
directory writes another number for the same shape. `0 bytes` and a count of
rows do not move.

The first five rows are runs ts-prune made, and status 0 covers a clean project
and a project with findings alike. The last five are runs it made none of: each
one throws out of `@ts-morph/common` or out of cosmiconfig before a module graph
exists, so 0 bytes on stdout there is the silence of a project ts-prune never
read. The status separates the two for every shape measured here, so this script
tests the status and needs no test on the report and none on stderr. That is
where it differs from the three shipped swiftlint rules, each of which has one
status carrying both answers and must read the report beside it.

The earlier shape of this script threw that status away. Each project's pipe
ended in the node placement and the loop ended in `sort -u`, and a shell
pipeline takes the status of its LAST command. Measured over three trees, the
earlier shape beside the shipped one, each project holding one export nothing
imports:

| the tree | the earlier pipe | the shipped script |
|---|---|---|
| one project ts-prune read | 1 finding, exit 0 | 1 finding, exit 0 |
| one project whose `tsconfig.json` is not JSON | 0 findings, exit 0 | 0 findings, exit 1, the project named |
| two projects, one ts-prune read and one it did not | 1 finding, exit 0 | 0 findings, exit 1, the broken project named |

Row 2 is the whole defect. The engine reads exit 0 as "the tool judged the
code", so a project ts-prune never opened read as a clean tree.

Row 3 is the shape a monorepo makes reach it, and it is why the run BREAKS
rather than declining the project on the `sah-diagnostic:` channel.
`builtin/validators/README.md` gives that channel to "a script that judged the
code and could not judge ONE item", and a project is not one item: ts-prune
judged no export of it at all, so every file under it would read as clean while
the rows of the project beside it made the run look measured. A repository
carrying one `tsconfig.json` is the ordinary case, and there the broken project
and the whole tree are the same thing — "a tool that refused to start reports as
a clean file", which is the trap the README names word for word. So the run
answers both the same way, and the number of projects a repository happens to
hold moves nothing.

The run reads every project before it stops, so the stderr names each project
ts-prune could not read rather than the first one alone. Each such project takes
one line of the run's own, under ts-prune's own stderr:

```
dead-code-typescript: ts-prune exited 1 for packages/other/tsconfig.json and judged no export of it
```

The script forwards ts-prune's own stderr for every project it reads, whole or
broken. On a run that completes, that text carries no `sah-diagnostic:` marker,
so a `tracing` record takes it and no reader of the review does.

A nonzero exit hands the engine the whole of stderr and no finding, so on a run
that breaks ts-prune's own words reach the diagnosing agent beside the line
above. Nothing the run did place reaches stdout for such a run either, because
the row file is read after the loop and only when no project stands in the
unread list. The acceptance test
`..._breaks_on_a_project_ts_prune_cannot_read` holds rows 2 and 3 of the table
above, all three facts of each. It reads the nonzero exit and the named project
off the error the engine answers for the run, and the 0 findings off the stdout
of that same run. The engine answers a nonzero exit before it reads stdout at
all, so the helper the test calls drives the run itself and reads both halves
off one output. Row 3 is where the second half carries the weight: a run that
broke and still wrote the readable project's row would pass the exit half and
state a measured tree.

Measured over this workspace, the shipped script, three runs one after another:
58 findings, exit 0, 0 items declined and 0 bytes on stderr each time, in
10.21 s, 7.54 s and 7.47 s. 58 is the count the table under "How the run is
shaped" states, so the status gate takes no finding away. The three placements
timed in that section were each read before this change, so those readings are
compared with each other and never with the three above.

`mktemp -d` makes the directory the node script is written into, and
`trap 'rm -rf "$work"' EXIT` removes it. The same directory holds everything the
loop writes: the project list the `find` wrote, the resolved configuration and
the file list of the project standing, the stdout and the stderr of that
project's ts-prune run, the rows the placement wrote, and the list of the
projects ts-prune could not read. The trap covers a clean run, a run with
findings and a broken run alike, and it leaves the exit status of the script
alone. The scope is `workspace`, so this script takes no file argument and
writes no zero-argument guard, which is what that scope asks of it.

## What the fixture pair holds, and what it cannot

The fixture pair holds the staging contract: the fail fixture carries one
unannotated unused export of five kinds, and the pass fixture carries the same
five behind the marker.

It cannot hold either carve-out above. Doctor counts only the findings a run
reports ABOUT the fixture under test, and both carve-outs take findings off a
DIFFERENT file — the package's entry module — so a manifest in `fixtures/` would
move neither count. The thirteen acceptance tests in
`tests/shipped/dead_code_typescript.rs` drive the shipped script over probe
repositories instead, and each one names the fact it holds.

It cannot hold the module-level claim either. A fixture directory is flat and
every fixture stands in the same program, so no fixture can be a module NOTHING
imports while another is the entry that spares it. The acceptance test
`the_shipped_typescript_dead_code_tool_rule_names_every_export_of_a_module_nothing_imports`
stages that shape as a probe repository, and it is the test that pins the
property the knip decision turned on.

Nor can it hold the path question. A fixture stands loose in one directory, so
no fixture is a module one project reaches from outside itself, no fixture
stands in a package whose name begins with another package's, and no fixture is
a symbolic link. The five acceptance tests named in "How the run is shaped"
stage those trees instead.
