# GitHub Copilot Agents

Nimble ships two specialised [GitHub Copilot agents](https://docs.github.com/en/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot) that can help you create commands and write scripts without leaving your editor.

## Available agents

### `@nimble-command`

Helps you **create, edit, and debug YAML command files**. It knows the full command schema, all six action types, environment variable layering, co-location rules, and context matching.

**Use when you want to:**

- Write a new command YAML from a plain-English description
- Choose the right action type for a use case
- Set up `env.yaml` variables or `${VAR}` substitution
- Create static list files
- Configure contexts for scoped matching
- Troubleshoot why a command isn't matching

**Example prompts:**

```
@nimble-command Create a command that searches DuckDuckGo with whatever I type
@nimble-command I want a static list of my team's Slack handles that copies on select
@nimble-command Why isn't my open_url command matching when I type "github"?
```

When a script is needed, `@nimble-command` delegates to `@nimble-script` automatically.

### `@nimble-script`

Helps you **write, debug, and improve scripts** for `dynamic_list` and `script_action` commands. It knows the required output formats, argument passing conventions, built-in `NIMBLE_*` environment variables, timeout constraints, and platform differences.

**Use when you want to:**

- Write a shell, Python, Node.js, or PowerShell script for a Nimble command
- Debug script output format issues (empty results, JSON parse errors)
- Handle `NIMBLE_*` environment variables in a script
- Fix permission or timeout problems
- Write cross-platform scripts (macOS/Linux/Windows)

**Example prompts:**

```
@nimble-script Write a bash script that lists my recent git branches as a dynamic list
@nimble-script My script runs but shows no results — help me debug
@nimble-script Convert this shell script to PowerShell for Windows
```

## How to use

These agents work in any editor or environment that supports GitHub Copilot agent mode:

1. **VS Code** — open Copilot Chat, type `@nimble-command` or `@nimble-script` followed by your request
2. **GitHub.com** — use the agents in Copilot Chat on the repository page
3. **CLI** — if using GitHub Copilot in the terminal, invoke with the agent prefix

The agent definitions live in `.github/agents/` in the Nimble repository.

## Agent boundaries

Each agent has a focused scope:

| Agent | Creates/edits | Does NOT touch |
|-------|---------------|----------------|
| `@nimble-command` | YAML command files, list files, env.yaml | Scripts, Rust source, frontend code |
| `@nimble-script` | Script files (.sh, .py, .js, .ps1) | YAML commands, env.yaml, settings.yaml |

This separation keeps each agent's output reliable — `@nimble-command` produces valid YAML, `@nimble-script` produces working scripts, and neither steps outside its domain.

## Tips

- **Start with `@nimble-command`** — it will delegate to `@nimble-script` when a script is needed
- **Be specific about what should happen** — "paste", "copy", or "open" helps the agent choose the right action type
- **Mention the platform** if you need Windows PowerShell scripts instead of shell scripts
- **Include context** — if your command should only work with a specific context active, mention it up front
