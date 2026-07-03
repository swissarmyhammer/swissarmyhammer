---
name: make-readme
description: Replace README.md wholesale with a minimal, high-quality README modeled on the best open source packages — it writes a complete new file, never an incremental edit of the old one. Two modes — `library` (no logo, leads with an inline runnable usage example) and `application` (logo, installation, getting started). Use when the user says "make a readme", "write the readme", "readme", "improve the README", or a new project needs one. Pass `library` or `application` to force the mode; otherwise it is detected from the project manifest.
license: MIT OR Apache-2.0
compatibility: Works in any project with a manifest (Cargo.toml, package.json, pyproject.toml, go.mod, etc.). No MCP tools required; uses the filesystem and git remotes to gather facts.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Make README

Replace `README.md` at the repo root **wholesale**: write a complete new file from the facts you gather — never patch, append to, or incrementally edit the existing one. A README is a **landing page, not the manual** — it answers exactly three things: **what it is, why it matters, and how to start** (an inline example for a library, getting started for an application). Everything else is a hard link out. The tightest READMEs of the most popular packages (requests, serde, zod, ripgrep, httpie) win by doing only this.

## Modes

- **library** — the project is consumed as a dependency. No logo. The visual proof is a short **runnable code example** near the top. One blessed install command. Target ≤ 100 lines.
- **application** — the project is installed and run by end users. Logo and screenshot/GIF, install instructions per package manager, a short getting-started. Target ≤ 150 lines.

## Requested mode

$ARGUMENTS

If the argument above names a mode (`library` or `application`), use it — it overrides detection. Anything else is extra guidance (e.g. a subdirectory in a monorepo, tone notes); honor it alongside the detected mode. Empty → detect.

### Mode detection (when not forced)

Read the manifest and entry points:

| Signal | Mode |
|--------|------|
| `Cargo.toml` with `[[bin]]` / `src/main.rs`, `package.json` with `bin`, pyproject `[project.scripts]`, `go.mod` + `main.go` | application |
| `Cargo.toml` lib-only, `package.json` with `main`/`exports` and no `bin`, pyproject without scripts | library |
| Both a lib and a thin CLI wrapper | pick by who the README's reader is — published to a registry for `import`/`use` → library; installed via brew/releases to run → application |

Say which mode you chose and why before writing.

## Process

### 1. Gather facts — never fabricate

Every line in the README must be backed by something you read:

- **Name + tagline**: manifest `description` field; sharpen it into one capability-focused sentence ("Fast, unopinionated, minimalist web framework for Node.js"), don't invent claims.
- **Repo slug**: `git remote get-url origin` — badges and links must use the real owner/repo.
- **License**: `LICENSE*` files and the manifest `license` field. Rust dual `MIT OR Apache-2.0` gets the standard two-line note.
- **CI**: a badge only if `.github/workflows/*.yml` (or equivalent) actually exists — point it at the real workflow file.
- **Registry version badge** (crates.io / npm / PyPI): only if the package is actually published — check the manifest name against the registry or for `publish = false`. An unpublished package gets no version badge.
- **Existing README**: read it first. Keep anything load-bearing (hard-won caveats, support policy); deep-but-valuable content moves to `docs/` (or `CONTRIBUTING.md`, `docs/FAQ.md`) and gets a link. Content that merely restates the source browser or manifest — directory trees, requirements, dependency lists — is deleted, not relocated.

### 2. Get the one example (library) or the visuals (application)

- **library**: extract a real, runnable 8–25 line example from `examples/`, doc tests, or integration tests — the single most representative use. Verify it compiles/runs before putting it in the README (`cargo test --doc`, `node example.js`, actual REPL). Never write an example from imagination.
- **application**: find an existing logo/screenshot asset (`assets/`, `doc/`, `.github/`, `media/`). If none exists, use a plain-text H1 and add one clearly marked placeholder comment (`<!-- TODO: screenshot of ... -->`) — never fabricate or generate branding, and never link a nonexistent image.

### 3. Write `README.md`

**library skeleton** (order matters — example before or immediately after install, within the first screen):

````markdown
# name

[CI badge] [version badge] [license badge]        <!-- 3–5 max, all real -->

One-sentence tagline. One short paragraph (2–3 sentences) on what it does
and why it exists — fold would-be feature bullets in here.

```lang
// 8–25 lines, runnable, the single most representative use
```

## Install

one command (`cargo add name` / `npm install name` / `pip install name`)

## Documentation

Full documentation at <docs link>.        <!-- plus 2–4 quick links max -->

## License

One line (or the standard dual-license two-liner for Rust).
````

**application skeleton**:

```markdown
<img src="existing/logo.png" width="..."> or plain # name

[CI badge] [version/packaging badge] [license badge]

One-sentence tagline.

![screenshot or GIF]                       <!-- or the TODO placeholder -->

## Why name?                               <!-- 4–6 bullets or 2–3 example
                                                commands; persuade before install -->
## Installation

The 2–4 managers users actually use (brew, cargo install, releases page,
apt/winget). More than ~5? Collapse the rest in <details> blocks or link
an INSTALL.md — never a 100-line matrix inline.

## Getting started

2–4 commands showing the first-run experience, then link the full
guide/config docs.

## License

One line.
```

### 4. Verify

- Line count within budget (library ≤ 100, application ≤ 150). Over budget → cut or link out, don't compress into denser prose.
- Every relative link resolves to a file that exists; every badge URL uses the real repo slug and a workflow/package that exists.
- The code example was actually run/compiled, this session.
- Render check: no broken fences, no raw HTML that GitHub strips.

### 5. Summary

Report: mode chosen (and the detection signal), line count, where the example came from, which badges were included/omitted and why, and what existing content was relocated where.

## Rules

- **Minimal is the feature.** When in doubt, cut and link out. No table of contents, no inline changelog, no roadmap, no FAQ, no contributor/sponsor walls, no API reference tour — the manual lives in docs, the README sells the first five minutes.
- **Never restate what the source browser or manifest already shows.** No "Package layout" / "Project structure" directory trees, no dependency lists, no "Requirements"/"Prerequisites" section. A library's requirements are already declared in its manifest; an application's genuine prerequisites (a runtime, an API key) belong as a line inside Getting started, not their own section.
- **≤ 5 badges**, each backed by a real service. Badge clutter (7+) reads as noise.
- **One visual proof**: code for a library, screenshot/GIF for an application. Not both, not several.
- **One blessed install command for libraries.** Multi-manager matrices are an application concern, and even there stay small or collapse.
- **Never invent**: no fake benchmarks, no unpublished-registry badges, no imagined quotes or user counts, no generated logos.
- **Don't destroy — but don't hoard**: existing content with real value (hard-won caveats, config guidance) is relocated to `docs/` and linked; content that merely restates the source or manifest (layout trees, requirements, dependency lists) is deleted outright. Say what moved where and what was cut.
- Monorepo: the root README covers the workspace; a named subdirectory argument scopes to that package's README.
