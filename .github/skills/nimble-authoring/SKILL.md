---
description: >
  Create, edit, debug, or improve Nimble launcher commands and scripts.
  Use when: writing YAML command files, configuring open_url paste_text
  copy_text static_list dynamic_list script_action commands, setting up
  env.yaml variables, creating list files, configuring contexts,
  troubleshooting command matching, writing shell/Python/Node.js/PowerShell
  scripts for dynamic_list or script_action, debugging script output format,
  fixing JSON output, handling NIMBLE_* environment variables, script
  timeout issues, or argument handling.
---

# Nimble Authoring

You help users create, edit, debug, and improve **YAML command files** and
**scripts** for the Nimble desktop launcher.

---

## Bootstrap — Read Before Answering

1. Read `.github/skills/nimble-authoring/nimble-spec.yaml` — the canonical schema for commands,
   action types, env vars, contexts, settings, and co-location patterns.
2. Check the `changelog` section at the bottom of the spec for recent changes.

**Do not rely on memorised schema. The spec file is the single source of truth.**

---

## Scope Decision

Determine what the user needs and work in the appropriate mode:

| User needs | Mode |
|-----------|------|
| A YAML command file (any action type) | Command authoring |
| A list file (TSV) | Command authoring |
| env.yaml changes | Command authoring |
| A script for dynamic_list or script_action | Script writing |
| Debug script output / format | Script writing |
| End-to-end feature (command + script) | Both — write command YAML first, then script |

---

## Command Authoring

### Constraints

- DO NOT modify Rust source code or the SvelteKit frontend.
- DO NOT modify `settings.yaml` without explicit user confirmation.
- ALWAYS place command files inside the `commands/` subdirectory.
- ALWAYS use the correct co-location pattern (command YAML + resource files
  in the same directory).

### Workflow

1. **Read the spec** — load `nimble-spec.yaml`.
2. **Understand the use case** — ask clarifying questions if the user's intent
   is ambiguous (what should trigger, what should happen).
3. **Choose the action type** — use the `action_selection_guide` from the spec.
4. **Generate the YAML** — write the command file(s) to the appropriate config
   directory location.
5. **If a list file is needed** — write it directly (list files are TSV —
   tab-separated title and optional subtext, one item per line).
6. **Verify** — confirm the files are in the right location and the YAML is valid.

---

## Script Writing

### Constraints

- DO NOT run scripts with elevated privileges (sudo) or modify system state.
- DO NOT create scripts that make destructive system changes — scripts produce
  output only.
- ALWAYS include a shebang line on macOS/Linux scripts.
- ALWAYS output valid JSON when returning structured data.
- ALWAYS handle the case where `$1` may be empty when `arg: optional`.

### Workflow

1. **Read the spec** — load `nimble-spec.yaml`, specifically `script_interface`
   and `environment_variables`.
2. **Understand what output is needed** — what items should appear (dynamic_list)
   or what value should be produced (script_action)?
3. **Choose the language** — match the user's preference or platform (sh for
   macOS/Linux, PowerShell for Windows).
4. **Write the script** — produce correct stdout output in the expected format.
5. **Make it executable** — `chmod +x` on macOS/Linux.
6. **Test it** — run the script in the terminal to verify output format and
   correctness.
7. **Handle edge cases** — empty arguments, no results, network failures,
   timeout risk.

---

## Conventions

### File Placement

- Command YAML files go inside `commands/` under the config directory.
- One command per `.yaml` file.
- Commands that reference a script or list file get their own subdirectory
  (co-located pattern — see `co_location` in the spec).
- Simple commands (open_url, paste_text, copy_text) can live as standalone
  `.yaml` files at any nesting depth under `commands/`.

### Reserved Phrases

- Phrases starting with `/` are reserved for built-in commands.
- Never create command files with a `/`-prefixed phrase.

### Testing

- After creating or editing a command, verify the YAML is valid.
- After writing a script, run it in the terminal to confirm output format.
- Validate JSON output with `python3 -m json.tool` or equivalent.

### Updating the Spec

When the Nimble command schema, settings schema, script interface, or
environment variable API changes:

1. Update `nimble-spec.yaml` to reflect the change.
2. Add a changelog entry at the bottom of the spec with the date and
   a one-line description of what changed.
3. Bump `spec_version` for any of these changes:
   - A field is removed or renamed
   - A field's type or semantics change
   - A new action type is added
   - A new environment variable is added
   - A new config field is added to any action or settings
   - The script interface changes (output format, timeout, arg modes)
