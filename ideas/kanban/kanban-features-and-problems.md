# "Best Possible" Simple Kanban — Feature List & Common Problems

---

## Part 1: Features Stolen from the Best

### From Things 3 🏆
- **Quick capture from anywhere** — global keyboard shortcut to add a task instantly, no matter what you're doing. Frictionless inbox.
- **Today / Upcoming / Anytime / Someday views** — time-based lenses on top of the board (not just columns). "When should I do this?" is a first-class question.
- **Morning / Evening split** — the Today view partitions tasks into time-of-day slots without requiring exact times on every task.
- **Headings within projects** — lightweight grouping inside a list/column (not just cards, but visual section dividers).
- **Checklists inside tasks** — subtasks as simple checkboxes within a card. No need to promote everything to a full task.
- **Schedule date vs. Due date** — two distinct dates: "when do I *want* to work on this" vs. "when is it actually *due*." This is the killer feature most tools miss.
- **Progress indicators on projects** — little pie/progress rings showing how far along a project is at a glance.
- **Markdown in notes** — rich text in the task body without a heavyweight editor.
- **Type-to-navigate (Type Travel)** — start typing and instantly jump to any project, tag, or task. No clicking through menus.
- **Drag-and-drop between open panes** — multi-window/multi-pane support for reorganizing across projects.

### From GitHub Issues 🏆
- **Labels/tags as first-class citizens** — colored labels for type (bug, feature, task), priority, area. Filterable, combinable.
- **Sub-issues with progress tracking** — parent-child hierarchy where the parent shows a progress bar of completed children.
- **Issue types** — structured categorization (Bug, Task, Feature, Initiative) with shared vocabulary across projects.
- **Milestones** — group tasks toward a goal/release/sprint and track aggregate completion.
- **Markdown everywhere** — descriptions, comments, checklists all in markdown.
- **Cross-references** — mention other tasks inline (`#123`) and see a backlink timeline. Know what's connected.
- **Multiple views of the same data** — board view, table view, list view, roadmap/timeline view. One dataset, many lenses.
- **Custom fields** — add your own metadata (priority, effort, category) without it being baked in.
- **Filter/sort/group by anything** — slice the board by assignee, label, milestone, custom field. Saved views for recurring filters.
- **Automation rules** — "when moved to Done, archive after 7 days" or "when created, set field X." Simple if-this-then-that.
- **WIP limits on columns** — optional cap on how many items can be in a column (core kanban principle).

### From Apple Reminders 🏆
- **Sections (column view / kanban built-in)** — the columns-as-sections model where you can flip between list view and kanban board view of the same data.
- **Smart Lists** — auto-populated views based on filters (tagged X, due this week, flagged, assigned to me). Incredibly powerful.
- **Natural language input** — type "finish report next Tuesday" and it parses the date automatically.
- **Location-based reminders** — trigger a task notification when you arrive at or leave a place.
- **Deep links to source** — a task can link back to the email, document, or URL that spawned it. Context lives with the task.
- **Siri / voice input** — add tasks by voice without opening the app.
- **Shared lists with assignment** — simple collaboration: share a list, assign tasks to people, they get notified.
- **Templates** — save a list as a reusable template for repeated workflows (sprint setup, onboarding checklist, etc.).
- **Flagging** — a quick binary "this is important" marker that aggregates into a Flagged smart list.
- **iCloud sync / instant cross-device** — real-time sync across every device, no accounts to manage.

---

## Part 2: The Synthesized Feature List (Prioritized)

### Must-Have (Core)
1. **Board + List + Table views** of the same data
2. **Drag-and-drop cards** across customizable columns
3. **Quick capture** — global shortcut / natural language / voice
4. **Checklists inside cards** (subtasks)
5. **Labels/tags** — colored, filterable, combinable
6. **Schedule date + Due date** (separate concepts)
7. **Markdown notes** on every card
8. **Today / Upcoming smart views** — time-based lenses
9. **Search + type-to-navigate** — instant find across everything
10. **Cross-device sync** — real-time, seamless

### Should-Have (Power)
11. **Sub-tasks with progress tracking** (parent shows completion %)
12. **Smart lists / saved filters** — auto-populated views by tag, date, flag, etc.
13. **Milestones / goals** — group cards toward a target
14. **Templates** — reusable task lists for repeated workflows
15. **Deep links** — attach source URL/email/doc to a card
16. **Flagging / priority** — quick "important" marker
17. **Sections / headings** within columns for visual grouping
18. **Morning / Evening** time-of-day slots in Today view
19. **WIP limits** on columns (optional)
20. **Simple automation** — move-to-column triggers, auto-archive

### Nice-to-Have (Delight)
21. **Progress rings** on projects/milestones
22. **Cross-references** between tasks (`#123` mentions)
23. **Custom fields** — user-defined metadata
24. **Location-based reminders**
25. **Shared boards with assignment** + notifications
26. **Keyboard-first UX** — every action has a shortcut
27. **Multiple windows / split panes**
28. **Natural language date parsing**

### Core Philosophy

> Things nails the **"when"** (scheduling + today focus), GitHub nails the **"what"** (structure + metadata + views), and Apple Reminders nails the **"where"** (deep OS integration + smart automation). The best simple kanban combines Things' opinionated time management with GitHub's flexible data model, wrapped in Apple Reminders' frictionless capture and system integration.

---

## Part 3: Common Complaints & Problems Across All Tools

What users consistently hate, grouped into themes.

### 🔴 1. THE PRODUCTIVITY TRAP
**"I spend more time managing tasks than doing them"**

The #1 universal complaint. Users describe downloading dozens of apps, spending hours setting up perfect systems with tags, priorities, and projects, feeling productive... then never actually using it. Notion users specifically call out getting lost in "building systems" instead of working. ClickUp users say it's so feature-rich it becomes a procrastination tool itself.

**Our solve:** Opinionated defaults with zero setup. Board starts with 3 columns (Backlog, Doing, Done). You type a task and press enter. That's it. Power features exist but are invisible until you need them. The golden rule: **if setup takes more than 60 seconds, it's too complex.**

---

### 🔴 2. THE KANBAN vs. TODO LIST DIVIDE
**"I need BOTH a daily checklist AND a project board, but tools only do one"**

Trello is great for project overview but terrible as a daily to-do list. Todoist is great for daily tasks but its kanban board is described as "dull and unresponsive." Things 3 nails the daily planning but has no board view at all. Users end up running two apps (e.g., Todoist + Trello) and things get out of sync.

**Our solve:** Same data, multiple views. Every task lives in one place but you can see it as a kanban board, a daily checklist (Today view), or a simple list. Toggle between views with one click. No sync problems because it's one dataset.

---

### 🔴 3. SCHEDULE DATE vs. DUE DATE CONFUSION
**"Todoist has no start dates. Things nails this but nothing else does."**

One of the most repeated complaints about Todoist specifically. Users want to separate "when should I work on this?" from "when is it actually due?" Most tools only offer a due date, so everything becomes overdue-red-text guilt. Things 3's separation of schedule date and deadline is called out as its killer feature by multiple sources.

**Our solve:** Two distinct date fields as a core concept. "Do date" (when you plan to work on it) and "Due date" (hard deadline). Today view shows tasks by do-date, not due-date. Overdue items are gentle, not guilt-tripping red text.

---

### 🔴 4. PLATFORM LOCK-IN
**"Things is Apple-only. No web app. No Android. No Windows."**

The #1 complaint about Things 3 across every review. Users love the design but can't use it at work (Windows laptop) or share with partners (Android). Apple Reminders has the same problem. GitHub Issues requires a GitHub account and is developer-centric.

**Our solve:** Tauri app — runs native on macOS, Windows, Linux. Web version possible later. No platform lock-in.

---

### 🔴 5. NO COLLABORATION (OR TOO MUCH)
**"Things has zero collaboration. Asana/Jira are overkill for sharing a list with my partner."**

Things 3 is purely solo — you can't even share a grocery list. On the flip side, Asana and Jira's collaboration features are enterprise-grade bloat that overwhelms small teams. Users want something in between: share a board, assign tasks, get notified. That's it.

**Our solve:** Simple sharing. Share a board with a link. Assign tasks to people. They get notified. No "workspaces," no "organizations," no seat-based pricing debates. Just sharing.

---

### 🔴 6. TRELLO STAGNATION / BOARDS-ONLY LIMITATION
**"Trello looks the same as it did in 2011. Kanban is now table stakes."**

Multiple sources describe Trello as a "relic" that hasn't meaningfully evolved. Its only differentiator (kanban boards) is now available in every competitor. Meanwhile, good features are locked behind paid tiers. Users also complain that Trello boards get cluttered fast with no good way to archive or filter at scale.

**Our solve:** Start as simple as Trello, but with the table/list/timeline views that Trello charges $12.50/user/month for. Don't stagnate on the board metaphor — make it a data-first tool with board as one view.

---

### 🔴 7. CLUNKY MOBILE / OFFLINE
**"Trello's mobile app wouldn't work offline. Things' iOS menus are frustrating."**

Things 3 users specifically complain about too many taps to set up repeating tasks on mobile. Trello's offline was historically broken. Apple Reminders has sync glitches where completed tasks reappear or dates reset randomly.

**Our solve:** Offline-first architecture. Changes sync when connectivity returns. Mobile UX is gesture-native, not desktop-menus-shrunk-down.

---

### 🔴 8. NO QUICK CAPTURE / TOO MUCH FRICTION TO ADD
**"Too much friction to add items. By the time I open the app and navigate, I forgot what I wanted to add."**

Apple Reminders is missing a quick-entry keyboard shortcut on iPad. Things 3's share sheet integration is inconsistent. Users say Todoist's natural language input is the gold standard but Things 3 doesn't have it.

**Our solve:** Quick capture is the #1 UX priority. Global hotkey, natural language parsing ("finish report tuesday p1"), and an inbox that captures everything first, organizes later.

---

### 🔴 9. GITHUB PROJECTS: TOO DEVELOPER-CENTRIC, MISSING REPORTS
**"No burndown charts, no velocity, no cross-repo tracking, can't see the big picture"**

GitHub Projects is great for devs but has no built-in analytics, reporting, or sprint insights. Cross-repository work requires manual effort. The hierarchy was flat until sub-issues were recently added.

**Our solve:** For personal/small-team use, we don't need enterprise reporting. But simple progress indicators (how much of this project is done?), a completion history ("what did I finish this week?"), and milestone progress are essential. Keep it visual, not spreadsheet-y.

---

### 🔴 10. OVERDUE GUILT / EMOTIONAL UX
**"Red overdue text everywhere makes me feel terrible and want to avoid the app"**

Things 3 is praised for its gentle handling of missed deadlines — just subtle text showing how many days past, no screaming red. Most other tools pile on the guilt with aggressive overdue notifications and styling that makes users feel bad and avoid the app entirely.

**Our solve:** Gentle, non-judgmental UX. Past-due items simply float to the top of Today with a soft indicator. No red, no "OVERDUE!!!", no guilt. The app should make you feel *capable*, not *behind*.

---

### 🔴 11. IMPORT/EXPORT LOCK-IN
**"Can't import my past todos or export them if I don't like Things"**

Things 3 specifically is called out for having no import/export. You're locked in once you start. This is a recurring theme across proprietary tools.

**Our solve:** Data is yours. Export to markdown, JSON, CSV anytime. Import from common formats. No lock-in. Plain-text-friendly data model.

---

## Summary Table

| # | Problem | How We Solve It |
|---|---------|----------------|
| 1 | **Productivity trap** — setup takes forever | Zero-config start, opinionated defaults, 60-second onboarding |
| 2 | **Board vs. checklist** — tools do one, not both | Same data → board, list, and today views |
| 3 | **No schedule vs. due date** — everything is "overdue" | Two date fields: "do date" + "due date" |
| 4 | **Platform lock-in** — Apple/GitHub only | Tauri app: macOS, Windows, Linux native |
| 5 | **Collaboration extremes** — zero or enterprise bloat | Simple sharing: link, assign, notify |
| 6 | **Stagnant tools** — Trello hasn't evolved | Multiple views included free, not paywalled |
| 7 | **Clunky mobile / offline** — broken sync, too many taps | Offline-first architecture, gesture-native mobile |
| 8 | **Capture friction** — too many taps to add a task | Global hotkey, natural language, inbox-first |
| 9 | **No progress visibility** — can't see the big picture | Progress rings on projects, weekly done review |
| 10 | **Overdue guilt** — red text drives users away | Gentle, non-judgmental time handling |
| 11 | **Data lock-in** — can't export your own tasks | Open formats: markdown, JSON, CSV export |
