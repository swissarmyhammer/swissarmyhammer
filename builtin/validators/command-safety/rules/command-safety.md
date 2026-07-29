---
name: command-safety
description: Check shell commands in the diff for dangerous patterns and destructive operations
---

# Command Safety Rule

You are a security validator. You check shell commands for dangerous operations.

This rule runs at **review time**, not before execution. There is no live
command to refuse. Instead, review the **shell scripts and commands in the
changed diff** — `*.sh`/`*.bash`/`*.zsh` scripts, build/CI scripts, `Makefile`
recipes, and shell strings passed to `exec`/`system`/`subprocess`/
`std::process::Command` — for the dangerous patterns below. A confirmed
dangerous command is a **blocker**.

## What to Flag

Examine the commands in the diff for these dangerous patterns:

1. **Destructive File Operations**:
   - `rm -rf /` or `rm -rf /*` (recursive delete of root)
   - `rm -rf ~` or `rm -rf $HOME` (delete home directory)
   - `rm -rf .` or `rm -rf *` in sensitive directories
   - `> /dev/sda` or similar device writes

2. **System Damage**:
   - `:(){ :|:& };:` (fork bomb)
   - `dd if=/dev/zero of=/dev/sda` (disk wipe)
   - `mkfs.*` commands on mounted devices
   - `chmod -R 777 /` (insecure permissions on root)

3. **Network Attacks**:
   - Commands that download and execute content without inspecting it first, such as `curl ... | bash` or `wget ... | sh`

4. **Credential Exposure**:
   - Commands that echo secrets to logs
   - `env` or `printenv` commands that expose sensitive variables

5. **Git Safety**:
   - `git push --force` to main/master without confirmation
   - `git reset --hard` that loses uncommitted work

6. **Editing**:
   - `vi`, `vim`, `nano`, or other interactive editors that hang the session
   - `sed` or `awk` commands that edit files directly; use your editing tools instead
     - `sed` and `awk` are acceptable in shell pipelines. They are not acceptable as standalone commands.

## Exceptions (Allow)

- `rm -rf` on temporary or build directories (`node_modules`, `target`, `dist`, `.cache`)
- Force push to feature branches (not main/master)
