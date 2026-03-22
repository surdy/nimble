# Config Directory Structure

Nimble stores all per-user data in a single platform-specific config directory. The directory and its subdirectories are created automatically on first launch.

---

## Root directory location

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/Nimble/` |
| Linux | `$XDG_CONFIG_HOME/Nimble/` (falls back to `~/.config/Nimble/`) |
| Windows | `%APPDATA%\Nimble\` |

---

## Subdirectory layout

```
Nimble/
  settings.yaml     ← application settings (hotkey, show_context_chip, allow_duplicates)
  env.yaml          ← global user-defined environment variables for scripts (optional)
  commands/         ← YAML command files (watched and hot-reloaded by Nimble)
    examples/       ← seeded on first launch if commands/ is empty
    …               ← your own files and subdirectories
```

All command-related files — YAML configs, list files, and scripts — live within the `commands/` tree. New subdirectories will be introduced in future releases as new features are added; each will be documented in this file.

---

## `settings.yaml`

The `settings.yaml` file at the root of the config directory controls application-level behaviour. It is created automatically the first time you run Nimble.

```yaml
# hotkey: Super+Space   # uncomment and set your preferred global shortcut

# Show the active-context chip inside the launcher bar (default: true)
show_context_chip: true

# When false, the first file that defines a phrase wins and duplicate
# phrases in other files generate warnings. Default is true (all loaded).
allow_duplicates: true

# When false, ${VAR}-substituted script/list paths must resolve inside
# the command directory. Default is true (external paths allowed).
allow_external_paths: true
```

**`hotkey`** — The global shortcut that summons the launcher from anywhere. You normally set this via the onboarding screen; editing it here manually is possible but requires a restart. Deleting this line triggers the onboarding screen on the next launch.

**`show_context_chip`** — When `true` (default), a pill badge showing the active context is displayed inside the launcher bar alongside a clear button. Set to `false` to hide it. Takes effect on next relaunch.

**`allow_duplicates`** — When `true` (default), all command files are loaded regardless of phrase conflicts. Set to `false` to enable first-file-wins deduplication and surface warnings for any conflicting phrases. Takes effect on next relaunch.

**`allow_external_paths`** — When `true` (default), `script:` and `list:` fields that use `${VAR}` substitution may resolve to paths outside the command directory. Set to `false` to restrict all resolved paths to the command directory. See [Writing Scripts — External scripts and lists](../guides/writing-scripts.md#external-scripts-and-lists).

---

## `env.yaml`

An optional file at the config root that defines **global user-defined environment variables** injected into every script run by `dynamic_list` or `script_action` commands.

```yaml
# ~/Library/Application Support/Nimble/env.yaml
WORK_EMAIL: alice@example.com
JIRA_BASE_URL: https://mycompany.atlassian.net
```

Keys must match `[A-Za-z_][A-Za-z0-9_]*`. Keys starting with `NIMBLE_` are reserved and rejected. Values are strings (numeric and boolean YAML values are coerced automatically).

You can also define **command-scoped** variables by placing an `env.yaml` in the same directory as a command YAML, or by adding an inline `env:` block in the command YAML itself. See [Writing Scripts — User-defined environment variables](../guides/writing-scripts.md#user-defined-environment-variables) for precedence rules and examples.

---

## `commands/`

Contains all YAML command files. Nimble watches this subdirectory recursively and reloads commands within ~300 ms whenever a file is added, changed, or removed — no restart required.

You can organise your command files into any subdir structure you like:

```
commands/
  open-github.yaml
  search-google.yaml
  snippets/
    email-signature.yaml
    legal-disclaimer.yaml
  show-team-emails/
    show-team-emails.yaml      ← static_list command
    team-emails.tsv            ← list file (TSV), co-located with its command
  say-hello/
    say-hello.yaml             ← dynamic_list command
    hello.sh                   ← script, co-located with its command
  work/
    open-jira.yaml
    paste-standup-template.yaml
```

Commands that use a `static_list` action keep their list file in the same directory as the command YAML. Commands that use `dynamic_list` or `script_action` keep their script in the same directory. See [Static List](../actions/static-list.md), [Dynamic List](../actions/dynamic-list.md), and [Script Action](../actions/script-action.md) for details.

For the full command YAML schema, action types, and live-reload details see [Configuring Commands](../guides/configuring-commands.md).

---

- [Configuring Commands](../guides/configuring-commands.md) — YAML schema, enable/disable, live reload
- [Actions](../actions/README.md) — Open URL, Paste Text, Copy Text, Static List, Dynamic List, Script Action
