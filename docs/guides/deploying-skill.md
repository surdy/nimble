# Deploying the Nimble Authoring Skill

Nimble ships a [Copilot skill](copilot-skill.md) — **nimble-authoring** — that helps you create commands and write scripts from natural language. The skill definition lives in the Nimble repository under `.github/skills/nimble-authoring/`.

To use it in **your own project**, copy the skill files into your workspace.

---

## Quick setup

1. **Clone or download** the two files from the [Nimble repository](https://github.com/surdy/nimble/tree/main/.github/):

   ```
   .github/
   └── skills/
       └── nimble-authoring/
           ├── nimble-spec.yaml              ← canonical schema (source of truth)
           └── SKILL.md                      ← skill definition
   ```

2. **Place them** in your project:

   ```bash
   mkdir -p .github/skills/nimble-authoring
   curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/skills/nimble-authoring/nimble-spec.yaml  -o .github/skills/nimble-authoring/nimble-spec.yaml
   curl -sL https://raw.githubusercontent.com/surdy/nimble/main/.github/skills/nimble-authoring/SKILL.md          -o .github/skills/nimble-authoring/SKILL.md
   ```

3. **Open the project** in VS Code (or any editor with Copilot skill support).

4. **Use the skill** — in Copilot Chat, describe what you want to build. The skill activates automatically when your request matches Nimble command or script authoring.

---

## What you get

The skill handles both command YAML authoring and script writing in a single workflow. It reads `nimble-spec.yaml` as its source of truth — it never relies on memorised schema. When the spec evolves, pull the latest files to stay in sync.

## Keeping the skill up to date

Compare `spec_version` in your local `nimble-spec.yaml` with the latest in the Nimble repository. When a new spec version is released, re-download both files.

## Where the skill works

- **VS Code** — Copilot Chat panel (skill auto-activates)
- **GitHub.com** — Copilot Chat on any repository containing the skill files
- **Copilot CLI** — available when working in a directory with `.github/skills/nimble-authoring/`

## See also

- [Copilot Skill overview](copilot-skill.md) — what the skill can do and example prompts
- [Configuring Commands](configuring-commands.md) — the YAML schema reference
- [Writing Scripts](writing-scripts.md) — script output formats, env vars, and debugging
