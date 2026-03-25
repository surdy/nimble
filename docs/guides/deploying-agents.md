# Deploying Copilot Agents

Nimble ships two [GitHub Copilot agents](copilot-agents.md) — `@nimble-command` and `@nimble-script` — that help you create commands and write scripts from natural language. The agent definitions live in the Nimble repository under `.github/agents/`.

To use them in **your own project**, copy the agent files into your workspace.

---

## Quick setup

1. **Clone or download** the four agent files from the [Nimble repository](https://github.com/surdy/nimble/tree/main/.github/agents):

   ```
   .github/agents/
   ├── nimble-spec.yaml           ← canonical schema (single source of truth)
   ├── nimble-command.agent.md    ← YAML command author agent
   ├── nimble-script.agent.md     ← script writer agent
   └── nimble-conventions.md      ← shared rules and boundaries
   ```

2. **Place them** in your project's `.github/agents/` directory:

   ```bash
   mkdir -p .github/agents
   curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/agents/nimble-spec.yaml      -o .github/agents/nimble-spec.yaml
   curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/agents/nimble-command.agent.md -o .github/agents/nimble-command.agent.md
   curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/agents/nimble-script.agent.md  -o .github/agents/nimble-script.agent.md
   curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/agents/nimble-conventions.md   -o .github/agents/nimble-conventions.md
   ```

3. **Open the project** in VS Code (or any editor with Copilot agent support).

4. **Use the agents** — in Copilot Chat, type `@nimble-command` or `@nimble-script` followed by your request.

---

## What you get

| Agent | What it does |
|-------|--------------|
| `@nimble-command` | Creates, edits, and debugs YAML command files for all six action types |
| `@nimble-script` | Writes, debugs, and improves shell/Python/Node.js/PowerShell scripts |

Both agents read `nimble-spec.yaml` as their source of truth — they never rely on memorised schema. When the spec evolves, pull the latest files to stay in sync.

## Keeping agents up to date

Each agent file contains a `spec_version` field in its frontmatter. Compare it with `spec_version` in `nimble-spec.yaml` to check for drift. When a new spec version is released, re-download the files to get the latest schema knowledge.

## Where agents work

- **VS Code** — Copilot Chat panel (`@nimble-command ...`)
- **GitHub.com** — Copilot Chat on any repository containing the agent files
- **Copilot CLI** — agents are available as custom agents when working in a directory with `.github/agents/`

## See also

- [Copilot Agents overview](copilot-agents.md) — what the agents can do and example prompts
- [Configuring Commands](configuring-commands.md) — the YAML schema reference
- [Writing Scripts](writing-scripts.md) — script output formats, env vars, and debugging
