# Validators

Validators are the guardrails of SwissArmyHammer. They run as Claude Code hooks — firing on every tool call — to enforce code quality, security, and test integrity automatically. Instead of catching problems in review, validators prevent them from being introduced in the first place.

## What a Validator Is

A validator is an AVP (Agent Validator Protocol) rule set: a collection of rules organized under a `VALIDATOR.md` file. Each rule is a markdown document that describes what to check and how to report violations. AVP processes these rules against the agent's tool calls and file changes in real time.

## Built-in Validators

SwissArmyHammer ships with four validator sets:

### Code Quality

Enforces structural code quality rules:

- **Cognitive complexity** — flags overly complex functions
- **Function length** — catches functions that are too long
- **Naming consistency** — enforces naming conventions
- **No commented-out code** — prevents dead code from accumulating
- **No hard-coded values** — catches embedded credentials and config
- **No magic numbers** — requires named constants
- **No string equality** — flags fragile string comparisons
- **No log truncation** — ensures complete error logging
- **Missing docs** — flags undocumented public APIs

### Security Rules

Catches security vulnerabilities:

- **Input validation** — ensures user input is validated at boundaries
- **No secrets** — prevents credentials, tokens, and keys from being committed

### Command Safety

Guards against destructive shell commands:

- **Safe commands** — blocks `rm -rf /`, `DROP TABLE`, force pushes, and other dangerous operations

### Test Integrity

Prevents test cheating:

- **No test cheating** — catches mocking of the thing under test, assertion-free tests, and other patterns that make tests pass without testing anything

## How Validators Work

Validators run as hooks in Claude Code's hook system. When the agent makes a tool call (writing a file, running a command), AVP intercepts it and runs the relevant validators:

```
Agent calls tool (e.g., write file)
    │
    ├─ AVP hook fires
    │    ├─ Code quality validator checks the change
    │    ├─ Security validator checks for secrets
    │    └─ Results fed back to agent
    │
    └─ Agent sees feedback, can self-correct
```

This happens transparently. The agent doesn't need to explicitly invoke validators — they run automatically on every relevant action.

## Setting Up Validators

Install AVP hooks into your project:

```bash
avp init
```

This registers AVP as a Claude Code hook and creates the `.avp/` directory. Built-in validators are always available; project-specific validators go in `.avp/validators/`.

## Creating Custom Validators

Create a new validator rule set:

```bash
avp new my-rules
```

This scaffolds a validator with a `VALIDATOR.md` and `rules/` directory. Each rule is a markdown file describing what to check.

## Sharing Validators

Validators can be published and installed via Mirdan:

```bash
# Create and publish
mirdan new validator my-team-rules
mirdan publish

# Install on another project
mirdan install my-team-rules
```

This lets teams codify their standards as installable packages — new projects get the team's quality rules with a single command.

## Validator Locations

| Location | Scope |
|----------|-------|
| Built-in (embedded in binary) | Always available |
| Project `.avp/validators/` | Project-specific rules |
| Global `~/.local/share/avp/validators/` | User-wide rules |
| Installed via Mirdan | Project or global |
