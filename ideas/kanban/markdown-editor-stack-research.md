# Markdown Editor Stack Research
## For Tauri + React Kanban App

---

## THE RECOMMENDATION (TL;DR)

**Editor: CodeMirror 6** via `@uiw/react-codemirror`  
**Rendering: `react-markdown`** with remark/rehype plugin ecosystem  
**Keybindings: Replit's CM6 extensions** — CUA (default), Vim (`@replit/codemirror-vim`), Emacs (`@replit/codemirror-emacs`)  

This is the same stack that powers **Obsidian**, **Replit**, **Zettlr**, and **ink-mde/Octo**. It's battle-tested at massive scale and is the only architecture that cleanly solves all three of your requirements simultaneously.

---

## 1. MARKDOWN EDITOR COMPONENT

### The Landscape (Ranked)

| Component | Foundation | Bundle | Keybinding Support | Markdown-Native | Verdict |
|-----------|-----------|--------|-------------------|----------------|---------|
| **@uiw/react-codemirror + @codemirror/lang-markdown** | CodeMirror 6 | ~26.9 kB gzip | ✅ CUA + Vim + Emacs | ✅ First-class | **🏆 Winner** |
| ink-mde | CodeMirror 6 | Moderate | ✅ Has `vim: true` option | ✅ Purpose-built | Strong but opinionated |
| @uiw/react-md-editor | textarea | ~4.6 kB gzip | ❌ No modal editing | ✅ Markdown-first | Too simple for power users |
| MDXEditor | Lexical (Meta) | 851 kB gzip | ❌ None | ⚠️ MDX-focused | Way too heavy, wrong focus |
| Milkdown | ProseMirror | ~40 kB gzip | ❌ None | ✅ Markdown | Requires building your own UI |
| Tiptap | ProseMirror | Varies | ❌ None | ⚠️ Rich-text first | Great editor, wrong paradigm |
| Lexical | Meta's framework | Small core | ❌ None | ❌ Must build yourself | Too low-level, not mature enough |

### Why CodeMirror 6 Wins

**It's not even close**, and here's why:

1. **Only editor with production Vim AND Emacs keybinding extensions.** Replit built and open-sourced both `@replit/codemirror-vim` (23k weekly downloads, 6.3.0) and `@replit/codemirror-emacs` (53k weekly downloads, 6.1.0). No other editor framework has anything comparable. ProseMirror-based editors (Tiptap, Milkdown) and Lexical have zero modal editing support.

2. **Markdown is a first-class language mode**, not an afterthought. `@codemirror/lang-markdown` provides syntax highlighting, GFM support, fenced code block language detection (delegates to other language modes for syntax highlighting inside code blocks), and markdown-specific keybindings (auto-continue lists on Enter, smart backspace through markup).

3. **Obsidian proved this exact stack works for markdown editing.** Obsidian migrated to CM6 and added Vim mode via the same Replit extension. Their vimrc-support plugin demonstrates that the CM6 Vim API is extensible enough for power users to define custom Ex commands, mappings, and leader keys.

4. **Replit proved this stack works at scale.** They migrated off Monaco (VSCode's editor) specifically because CM6 is more modular, more performant, and better on mobile. They built and maintain the Vim/Emacs extensions as production dependencies.

5. **Extension architecture is composable.** You swap keymaps by reconfiguring a CM6 Compartment — no teardown, no state loss, just a transaction that replaces the keymap extension. This means users can switch modes mid-session.

6. **Tauri compatibility is proven.** Multiple open-source projects (CodeNest, Huditor, danmugh/rust-tauri-markdown-editor) use Tauri + React + CodeMirror 6 with zero compatibility issues. CM6 is pure DOM — no Electron dependencies, no Node.js requirements.

### The React Integration

`@uiw/react-codemirror` is the canonical React wrapper for CM6:

```tsx
import CodeMirror from '@uiw/react-codemirror';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { languages } from '@codemirror/language-data';

<CodeMirror
  value={content}
  onChange={(val) => setContent(val)}
  extensions={[
    markdown({ 
      base: markdownLanguage, 
      codeLanguages: languages  // syntax highlighting in fenced blocks
    })
  ]}
/>
```

### What About WYSIWYG / Hybrid Mode?

For a kanban app, you probably want **two modes**:

1. **Source editing mode** — raw markdown with syntax highlighting (CodeMirror)
2. **Preview/display mode** — rendered HTML (react-markdown)

If you later want a hybrid "Typora-like" experience where formatting appears inline while you type, the `codemirror-rich-markdoc` plugin demonstrates this for CM6 — it hides markdown syntax except around the cursor. Obsidian does the same thing with their "Live Preview" mode. This is achievable with CM6 decorations and widgets, and it would be built on top of the same CodeMirror foundation.

---

## 2. MARKDOWN DISPLAY / RENDERING ENGINE

### The Landscape (Ranked)

| Library | Weekly Downloads | React-Native | XSS-Safe by Default | Plugin Ecosystem | Verdict |
|---------|-----------------|-------------|---------------------|-----------------|---------|
| **react-markdown** | ~10.6M | ✅ Yes | ✅ Yes | ✅ remark + rehype | **🏆 Winner** |
| marked | ~24.8M | ❌ Returns HTML string | ❌ No (needs DOMPurify) | ⚠️ Limited | Fast but requires `dangerouslySetInnerHTML` |
| markdown-it | ~15.1M | ❌ Returns HTML string | ❌ No | ✅ Good plugin ecosystem | Same `dangerouslySetInnerHTML` problem |
| showdown | ~978K | ❌ Returns HTML string | ❌ No | ⚠️ Few plugins | Legacy, declining |

### Why react-markdown Wins

1. **No `dangerouslySetInnerHTML`.** This is the killer feature. `react-markdown` converts markdown AST nodes directly into React components. Every other library (marked, markdown-it, showdown) returns an HTML string that you have to inject via `dangerouslySetInnerHTML`, which is an XSS vector. In a Tauri app where users might paste markdown from untrusted sources, this matters enormously.

2. **Component overrides.** You can replace any markdown element with your own React component:

```tsx
<ReactMarkdown
  components={{
    h1: ({children}) => <h1 className="task-title">{children}</h1>,
    a: ({href, children}) => <a href={href} onClick={handleLink}>{children}</a>,
    input: ({checked}) => <Checkbox checked={checked} />,  // task list checkboxes!
    code: ({className, children}) => <SyntaxHighlightedCode ... />
  }}
>
  {markdownContent}
</ReactMarkdown>
```

This is critical for a kanban app — you'll want task checkboxes to be interactive, links to open in the system browser (via Tauri), and code blocks to match your theme.

3. **The remark/rehype plugin ecosystem is massive.** Key plugins you'll want:

| Plugin | What It Does |
|--------|-------------|
| `remark-gfm` | GitHub Flavored Markdown (tables, strikethrough, task lists, autolinks) |
| `remark-math` + `rehype-katex` | LaTeX math rendering |
| `rehype-sanitize` | Whitelist-based HTML sanitization |
| `remark-breaks` | Treat single newlines as `<br>` (Slack/Discord style) |
| `rehype-highlight` | Syntax highlighting in code blocks |
| `remark-emoji` | `:emoji_name:` → emoji |

4. **100% CommonMark compliant**, and 100% GFM compliant with the `remark-gfm` plugin.

5. **Performance is fine for task descriptions.** `react-markdown` is slower than `marked` on huge documents because it goes through React's virtual DOM. But task descriptions in a kanban app are short — paragraphs, not pages. Performance is a non-issue at this scale.

### Rendering Architecture

```
User types markdown → CodeMirror 6 (editing)
                           ↓
                    Raw markdown string
                           ↓
              react-markdown (display/preview)
                           ↓
                    React components
```

Both the editor and the renderer consume the same plain markdown string. No intermediate format, no lock-in.

---

## 3. KEYBINDING MODES: CUA, EMACS, VIM

### What "CUA" Means

Quick clarification since this trips people up: "CUA" (Common User Access) technically refers to IBM's original keybindings (Shift-Del for cut, Ctrl-Ins for copy, Shift-Ins for paste). But in modern usage, "CUA-style" means the standard Windows/Mac keybindings everyone expects: Ctrl/Cmd+C, Ctrl/Cmd+V, Ctrl/Cmd+Z, etc. This is what CodeMirror 6's `defaultKeymap` provides. It's the default — no extension needed.

### The Three Modes

| Mode | Package | How to Enable | Weekly Downloads |
|------|---------|--------------|-----------------|
| **CUA (Standard)** | `@codemirror/commands` → `defaultKeymap` | Built-in, always active | (Part of CM6 core) |
| **Vim** | `@replit/codemirror-vim` | `vim()` extension, must load before other keymaps | ~23.5K |
| **Emacs** | `@replit/codemirror-emacs` | `emacs()` extension, must load before other keymaps | ~53.6K |

### Implementation: Hot-Swappable Keymaps via Compartments

CM6's Compartment system lets you swap keybindings at runtime without destroying the editor state. This is the clean way to do a settings toggle:

```tsx
import { Compartment } from '@codemirror/state';
import { keymap } from '@codemirror/view';
import { defaultKeymap } from '@codemirror/commands';
import { vim } from '@replit/codemirror-vim';
import { emacs } from '@replit/codemirror-emacs';

// Create a compartment for the keymap
const keymapCompartment = new Compartment();

// Initial setup — CUA by default
const extensions = [
  keymapCompartment.of(keymap.of(defaultKeymap)),
  markdown({ base: markdownLanguage, codeLanguages: languages }),
  // ... other extensions
];

// Switch keymaps at runtime
function setKeymapMode(view: EditorView, mode: 'cua' | 'vim' | 'emacs') {
  let newKeymap;
  switch (mode) {
    case 'vim':
      newKeymap = vim();
      break;
    case 'emacs':
      newKeymap = emacs();
      break;
    case 'cua':
    default:
      newKeymap = keymap.of(defaultKeymap);
  }
  
  view.dispatch({
    effects: keymapCompartment.reconfigure(newKeymap)
  });
}
```

### Important Implementation Notes

1. **Vim and Emacs extensions MUST be loaded before other keymaps** in the extension array. They need highest precedence to intercept keys before the default bindings handle them.

2. **Vim mode needs `drawSelection`** — the built-in DOM selection can't render Vim's block cursor or visual mode selection correctly. The `basicSetup` bundle includes this, but if you're building a custom setup, don't forget it.

3. **Vim's Ex commands are extensible.** You can define custom commands like `:w` to save:
```ts
import { Vim, getCM } from '@replit/codemirror-vim';
Vim.defineEx('write', 'w', () => { saveTask(); });
Vim.defineEx('quit', 'q', () => { closeEditor(); });
Vim.defineEx('wq', 'wq', () => { saveTask(); closeEditor(); });
```

4. **Emacs kill ring works.** `@replit/codemirror-emacs` implements C-k (kill-line), C-y (yank), M-y (yank-pop), C-w (kill-region), M-w (copy-region), and the standard movement commands (C-a, C-e, C-f, C-b, C-n, C-p, etc.).

5. **CUA is the safe default.** New users expect Ctrl+C/V/Z. Only show the Vim/Emacs toggle in an "Advanced" or "Editor Preferences" section. Power users will find it.

6. **Tauri keybinding conflicts.** Be aware that Tauri may capture certain system-level shortcuts (like Cmd+Q on macOS) before they reach the webview. You may need to configure Tauri's global shortcuts to avoid conflicts with Emacs bindings (which use Ctrl heavily on all platforms).

---

## 4. THE COMPLETE PACKAGE LIST

```json
{
  "dependencies": {
    // Editor core
    "@uiw/react-codemirror": "^4.x",
    "codemirror": "^6.x",
    "@codemirror/lang-markdown": "^6.x",
    "@codemirror/language-data": "^6.x",
    "@codemirror/commands": "^6.x",
    "@codemirror/state": "^6.x",
    
    // Keybinding modes
    "@replit/codemirror-vim": "^6.x",
    "@replit/codemirror-emacs": "^6.x",
    
    // Markdown rendering (display/preview)
    "react-markdown": "^9.x",
    "remark-gfm": "^4.x",
    "rehype-sanitize": "^6.x",
    
    // Optional but recommended
    "remark-breaks": "^4.x",
    "rehype-highlight": "^7.x"
  }
}
```

Total additional bundle: roughly 30-40 kB gzipped for the editor + keybindings, plus ~12 kB for react-markdown and plugins. Very lean for what you get.

---

## 5. PRIOR ART — WHO ELSE DOES THIS

| App | Stack | Keybinding Support | Notes |
|-----|-------|-------------------|-------|
| **Obsidian** | CodeMirror 6 + custom renderer | Vim (via CM6 plugin) | Dominant markdown editor. Proved CM6+Vim works for millions of users. |
| **Replit** | CodeMirror 6 | Vim + Emacs (their own extensions) | Built and maintain both keymap extensions. Production at massive scale. |
| **Zettlr** | CodeMirror 6 | Vim + Emacs (via Replit extensions) | Academic markdown editor. Migrated from CM5 → CM6. |
| **ink-mde / Octo** | CodeMirror 6 | Vim (built-in `vim: true` option) | Beautiful markdown editor. Has a `vim: false` toggle in options. |
| **Joplin** | CodeMirror 6 | Vim mode | Note-taking app, migrated to CM6. |

Every serious markdown editor that supports alternative keybindings has converged on the same answer: CodeMirror 6 + Replit's extensions.

---

## 6. RISKS AND WATCH-OUTS

**Replit extension maintenance.** Both `@replit/codemirror-vim` (last published ~7 months ago) and `@replit/codemirror-emacs` (also ~7 months ago) are Replit production dependencies, so they're unlikely to be abandoned. But they're not on a rapid release cadence. The Vim extension is more actively maintained and has more community involvement than the Emacs one.

**CM6 Vim mode is not full Vim.** It covers the vast majority of what users expect (normal/insert/visual modes, motions, text objects, registers, macros, basic Ex commands, mappings) but edge cases exist. Joplin users have reported minor issues like `o` not working in some contexts. For a kanban task editor (not a full IDE), this coverage is more than sufficient.

**Emacs is simpler than Vim** in this context. The Emacs extension provides the standard movement and editing commands but doesn't attempt to emulate Emacs Lisp or the full minibuffer. It's "Emacs keybindings" not "Emacs the editor." This is what users expect.

**`react-markdown` re-renders on every keystroke** if you're showing a live preview. For short task descriptions this is fine. If you ever need to render huge documents, memoize or debounce the preview update.

---

## 7. RECOMMENDED ARCHITECTURE FOR THE KANBAN APP

```
┌─────────────────────────────────────────────┐
│  Task Card (Kanban Board View)              │
│  ┌─────────────────────────────────────┐    │
│  │  react-markdown (read-only display) │    │
│  │  Renders task description as HTML   │    │
│  │  Interactive checkboxes for [x]     │    │
│  └─────────────────────────────────────┘    │
│  Click to edit →                            │
│  ┌─────────────────────────────────────┐    │
│  │  CodeMirror 6 (editing mode)        │    │
│  │  @codemirror/lang-markdown          │    │
│  │  Keymap: CUA | Vim | Emacs          │    │
│  │  (user preference, hot-swappable)   │    │
│  └─────────────────────────────────────┘    │
└─────────────────────────────────────────────┘

Settings → Editor Preferences:
  Keymap Mode: [CUA ▾] / Vim / Emacs
  → Dispatches compartment reconfigure to all active editors
```

The raw markdown string is the single source of truth. The editor writes it, the renderer displays it, and it's what gets saved to disk (Tauri filesystem API) or synced.
