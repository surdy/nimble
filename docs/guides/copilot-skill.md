# Copilot Skill — Nimble Authoring

Nimble ships a [GitHub Copilot skill](https://docs.github.com/en/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot) that can help you create commands and write scripts without leaving your editor. The skill handles both **command YAML authoring** and **script writing** in a single workflow.

## What it does

The **nimble-authoring** skill helps you:

- Write a new command YAML from a plain-English description
- Choose the right action type for a use case
- Set up `env.yaml` variables or `${VAR}` substitution
- Create static list files (TSV)
- Configure contexts for scoped matching
- Troubleshoot why a command isn't matching
- Write shell, Python, Node.js, or PowerShell scripts for `dynamic_list` and `script_action`
- Debug script output format issues (empty results, JSON parse errors)
- Handle `NIMBLE_*` environment variables in scripts
- Fix permission or timeout problems
- Build end-to-end features (command YAML + script) in one pass

## Example prompts

```
Create a Nimble command that searches DuckDuckGo with whatever I type
I want a static list of my team's Slack handles that copies on select
Why isn't my open_url command matching when I type "github"?
Write a bash script that lists my recent git branches as a dynamic list
My script runs but shows no results — help me debug
Convert this shell script to PowerShell for Windows
```

Copilot automatically activates the skill when your request matches command or script authoring for Nimble.

## How to use

The skill works in any editor or environment that supports GitHub Copilot skills:

1. **VS Code** — open Copilot Chat and describe what you want; the skill activates automatically
2. **GitHub.com** — use Copilot Chat on any repository containing the skill files
3. **CLI** — the skill is available when working in a directory that contains `.github/skills/nimble-authoring/`

The skill definition lives in `.github/skills/nimble-authoring/SKILL.md` and reads `nimble-spec.yaml` as its schema source of truth.

## Tips

- **Be specific about what should happen** — "paste", "copy", or "open" helps the skill choose the right action type
- **Mention the platform** if you need Windows PowerShell scripts instead of shell scripts
- **Include context** — if your command should only work with a specific context active, mention it up front
- **Ask for end-to-end** — the skill can write both the command YAML and the script in one pass
