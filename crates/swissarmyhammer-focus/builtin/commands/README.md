# (retired) focus builtin command YAML

This directory once held `nav.yaml` — the nine universal `nav.*` spatial-
navigation command stubs (id / name / keys / menu placement only), composed
into the app's `CommandsRegistry` via `compose_registry!` and executed by React
closures in `app-shell.tsx`. That YAML-merge / overlay approach is retired.

The `nav.*` commands now live in the `nav-commands` builtin **plugin**
(`builtin/plugins/nav-commands/index.ts`): it registers all nine ids on the
`CommandService` with their `keys` + `menu` placement, and routes execution
through the `focus` kernel (directional / drill, host-driven) or the webview
command bus (`nav.jump`). The OS menu is built FROM that service catalogue, not
from a YAML merge into the registry snapshot.

`builtin_yaml_sources()` therefore returns an empty list for this crate. This
file is a non-`.yaml` placeholder so the `include_dir!` path resolves and the
directory survives in version control; the loader filters it out by extension.
