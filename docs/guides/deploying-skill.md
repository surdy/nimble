# Deploying the Nimble Authoring Skill

Nimble ships a [Copilot skill](copilot-skill.md) — **nimble-authoring** — that helps you create commands and write scripts from natural language. The skill files are **bundled with the app** and automatically installed to your config directory on every launch.

---

## One-command deploy

Open Nimble and type:

```
/deploy copilot skill
```

This creates a symlink (macOS/Linux) or junction (Windows) from your config directory's skill files to `~/.copilot/skills/nimble-authoring/`. The skill is immediately available in VS Code Copilot Chat across all workspaces.

Because the files are symlinked, the skill **automatically updates** whenever you update Nimble — no manual re-downloading needed.

---

## Manual setup (alternative)

If you prefer not to use the built-in deploy command, you can create the symlink yourself:

**macOS / Linux:**
```bash
ln -s ~/Library/Application\ Support/nimble/skills/nimble-authoring ~/.copilot/skills/nimble-authoring
```

**Windows (PowerShell — run as admin or with Developer Mode enabled):**
```powershell
cmd /C mklink /J "$env:USERPROFILE\.copilot\skills\nimble-authoring" "$env:APPDATA\nimble\skills\nimble-authoring"
```

---

## Per-project setup

To make the skill available in a specific project (and on GitHub.com Copilot Chat), copy or symlink the files into the project's `.github/skills/` directory:

```bash
mkdir -p .github/skills/nimble-authoring
cp ~/Library/Application\ Support/nimble/skills/nimble-authoring/* .github/skills/nimble-authoring/
```

Or download from the [Nimble repository](https://github.com/surdy/nimble/tree/main/.github/skills/nimble-authoring):

```bash
mkdir -p .github/skills/nimble-authoring
curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/skills/nimble-authoring/nimble-spec.yaml  -o .github/skills/nimble-authoring/nimble-spec.yaml
curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/skills/nimble-authoring/SKILL.md          -o .github/skills/nimble-authoring/SKILL.md
```

---

## What you get

The skill handles both command YAML authoring and script writing in a single workflow. It reads `nimble-spec.yaml` as its source of truth — it never relies on memorised schema. When the spec evolves, the bundled copy updates automatically with the app.

## Keeping the skill up to date

- **Symlink/junction users:** Update Nimble — the skill updates automatically.
- **Copy users:** Compare `spec_version` in your local `nimble-spec.yaml` with the latest in the Nimble repository. When a new spec version is released, re-download both files.

## Where the skill works

- **VS Code** — Copilot Chat panel (skill auto-activates)
- **GitHub.com** — Copilot Chat on any repository containing the skill files
- **Copilot CLI** — available when working in a directory with `.github/skills/nimble-authoring/`

## See also

- [Copilot Skill overview](copilot-skill.md) — what the skill can do and example prompts
- [Configuring Commands](configuring-commands.md) — the YAML schema reference
- [Writing Scripts](writing-scripts.md) — script output formats, env vars, and debugging
