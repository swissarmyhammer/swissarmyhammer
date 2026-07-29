---
name: make-readme
description: Replace `README.md` completely with a minimal, high-quality README, modeled on the best open source packages. It writes a whole new file; it never makes a small edit to the old one. There are two modes — `library` (no logo; leads with a runnable inline usage example) and `application` (logo, installation, getting started). Use this skill when the user says "make a readme", "write the readme", "readme", "improve the README", or when a new project needs one. Pass `library` or `application` to force the mode; otherwise, the skill detects the mode from the project manifest.
license: MIT OR Apache-2.0
compatibility: This skill works in any project with a manifest, for example Cargo.toml, package.json, pyproject.toml, or go.mod. It needs no MCP tools. It uses the file system and git remotes to gather facts.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Make README

Replace `README.md` at the repo root **completely**: write a complete new file from the facts you gather. Do not patch, append to, or make a small edit to the existing file. A README is a **landing page, not a manual**. It must answer exactly three questions: **what the project is, why it matters, and how to start** — an inline example for a library, or a getting-started section for an application. Everything else is a link out to another page. The tightest READMEs of the most popular packages, such as requests, serde, zod, ripgrep, and httpie, win by doing only this.

## Modes

- **library** — other projects use this project as a dependency. It has no logo. The visual proof is a short **runnable code example** near the top. It has one recommended install command. The target length is 100 lines or fewer.
- **application** — end users install and run this project. It has a logo and a screenshot or GIF, install instructions for each package manager, and a short getting-started section. The target length is 150 lines or fewer.

## Requested mode

$ARGUMENTS

If the argument above names a mode, `library` or `application`, use it. It overrides detection. Anything else is extra guidance, for example a subdirectory in a monorepo, or notes on tone; follow it along with the detected mode. If the argument is empty, detect the mode.

### Mode detection (when not forced)

Read the manifest and entry points:

| Signal | Mode |
|--------|------|
| `Cargo.toml` with `[[bin]]` / `src/main.rs`, `package.json` with `bin`, pyproject `[project.scripts]`, `go.mod` + `main.go` | application |
| `Cargo.toml` lib-only, `package.json` with `main`/`exports` and no `bin`, pyproject without scripts | library |
| Both a library and a thin CLI wrapper | Choose by who reads the README. If the package is published to a registry for `import` or `use`, choose library. If people install it through brew or a releases page to run it, choose application. |

State which mode you chose, and why, before you write the file.

## Process

### 1. Gather facts — never fabricate

Every line in the README must be backed by something you read:

- **Name and tagline**: use the manifest `description` field. Sharpen it into one sentence that names the main capability, for example "Fast, unopinionated, minimalist web framework for Node.js". Do not invent claims.
- **Repository slug**: run `git remote get-url origin`. Badges and links must use the real owner and repository name.
- **License**: `LICENSE*` files and the manifest `license` field. Rust dual `MIT OR Apache-2.0` gets the standard two-line note.
- **CI**: add a badge only if `.github/workflows/*.yml`, or the equivalent, actually exists. Point the badge at the real workflow file.
- **Registry version badge** (crates.io, npm, or PyPI): add one only if the package is actually published. Check the manifest name against the registry, and check for `publish = false`. An unpublished package gets no version badge.
- **Existing README**: read it first. Keep anything load-bearing, such as hard-won caveats or the support policy. Move deep but valuable content to `docs/`, or to `CONTRIBUTING.md` or `docs/FAQ.md`, and link to it. Delete content that only restates the source browser or the manifest, such as directory trees, requirements, or dependency lists. Do not relocate this content; delete it.

### 2. Get the one example (library) or the visuals (application)

- **library**: take a real, runnable example, 8 to 25 lines long, from `examples/`, the doc tests, or the integration tests. Choose the single most representative use. Verify that it compiles and runs before you put it in the README, for example with `cargo test --doc`, `node example.js`, or a real REPL session. Do not write an example from imagination.
- **application**: find an existing logo or screenshot asset, for example in `assets/`, `doc/`, `.github/`, or `media/`. If none exists, use a plain-text H1, and add one clearly marked placeholder comment, for example `<!-- TODO: screenshot of ... -->`. Do not invent or generate branding. Do not link to an image that does not exist.

### 3. Write `README.md`

**library skeleton** (the order matters: put the example before install, or right after it, within the first screen):

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

- The line count is within budget: 100 lines or fewer for a library, 150 lines or fewer for an application. If it is over budget, cut content or link it out. Do not compress it into denser prose.
- Every relative link resolves to a file that exists. Every badge URL uses the real repository slug and a workflow or package that exists.
- You actually ran or compiled the code example, in this session.
- Check the render: no broken code fences, and no raw HTML that GitHub strips out.

### 5. Summary

Report: the mode you chose, and the detection signal; the line count; where the example came from; which badges you included or left out, and why; and what existing content you moved, and to where.

## Rules

- **Minimal is the feature.** When in doubt, cut content and link it out. Do not include a table of contents, an inline changelog, a roadmap, an FAQ, a contributor or sponsor wall, or an API reference tour. The manual lives in the docs. The README sells the first five minutes.
- **Do not restate what the source browser or manifest already shows.** Do not include a "Package layout" or "Project structure" directory tree, a dependency list, or a "Requirements" or "Prerequisites" section. A library's requirements are already stated in its manifest. An application's real prerequisites, such as a runtime or an API key, belong as one line inside Getting started, not in their own section.
- **Use 5 badges or fewer**, each backed by a real service. A cluttered badge row, 7 or more, reads as noise.
- **Use one visual proof**: code for a library, or a screenshot or GIF for an application. Do not use both, and do not use several.
- **Give one recommended install command for a library.** A matrix of many package managers is an application concern, and even there, keep it small, or collapse it.
- **Never invent facts**: no fake benchmarks, no badges for an unpublished registry, no invented quotes or user counts, and no generated logos.
- **Do not destroy content, but do not hoard it either.** Move existing content with real value, such as hard-won caveats or config guidance, to `docs/`, and link to it. Delete content that only restates the source or manifest, such as layout trees, requirements, or dependency lists. State what you moved, and where, and what you cut.
- For a monorepo, the root README covers the whole workspace. A named subdirectory argument scopes the work to the README of that package.
