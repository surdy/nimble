# Configuring Commands

Commands are stored as individual YAML files inside the `commands/` subdirectory of the platform-specific config directory. Nimble watches this subdirectory recursively and reloads commands automatically whenever a file is added, changed, or removed — no restart required.

For the overall config directory layout see [Config Directory Structure](../reference/config-directory.md).

---

## Commands directory location

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/Nimble/commands/` |
| Linux | `$XDG_CONFIG_HOME/Nimble/commands/` (falls back to `~/.config/Nimble/commands/`) |
| Windows | `%APPDATA%\Nimble\commands\` |

> **Tip:** open a new command file in your editor directly from shell:  
> `code ~/Library/Application\ Support/Nimble/commands/my-command.yaml`

---

## One command per file

Each `.yaml` (or `.yml`) file contains exactly one command. Files can be nested in any subdirectory structure you like — Nimble discovers them all recursively.

```
commands/
  open-github.yaml
  search-google.yaml
  snippets/
    email-signature.yaml
    legal-disclaimer.yaml
  work/
    open-jira.yaml
    paste-standup-template.yaml
```

---

## Full command schema

```yaml
# Required fields
phrase: <string>         # multi-word phrase users type to trigger this command
title: <string>          # human-readable label shown as the result title

# Optional fields
enabled: true            # set to false to disable without deleting the file (default: true)
env:                     # inline user-defined environment variables for scripts (optional)
  KEY: value

# Required: one action block
action:
  type: open_url | paste_text | copy_text | static_list | dynamic_list | script_action
  config:
    # --- open_url ---
    url: <string>        # any valid URL; use {param} for user-supplied input

    # --- paste_text ---
    text: <string>       # the text to paste (multi-line supported with YAML | block scalar)

    # --- copy_text ---
    text: <string>       # the text to copy to the clipboard

    # --- static_list ---
    list: <string>       # name of the list file (without .tsv) co-located with the command,
                         # or a ${VAR}-substituted path to an external list file
    item_action: paste_text | copy_text | open_url   # optional

    # --- dynamic_list ---
    script: <string>     # name of the script file co-located with this command YAML,
                         # or a ${VAR}-substituted path to an external script
    arg: none | optional | required   # default: none
    item_action: paste_text | copy_text | open_url   # optional

    # --- script_action ---
    script: <string>     # name of the script file co-located with this command YAML,
                         # or a ${VAR}-substituted path to an external script
    arg: none | optional | required   # default: none
    result_action: open_url | paste_text | copy_text   # required
    prefix: <string>     # optional — prepended to each value (paste_text / copy_text only)
    suffix: <string>     # optional — appended to each value (paste_text / copy_text only)
```

### Open URL example

```yaml
phrase: open github
title: Open GitHub
action:
  type: open_url
  config:
    url: https://github.com
```

### Open URL with parameter

```yaml
phrase: search npm
title: Search npm packages
action:
  type: open_url
  config:
    url: https://www.npmjs.com/search?q={param}
```

Type `search npm svelte` → opens `https://www.npmjs.com/search?q=svelte`.

### Paste Text example

```yaml
phrase: paste signature
title: Paste email signature
action:
  type: paste_text
  config:
    text: |
      Best regards,
      Jane Smith
      jane@example.com
```

---

## Enabling and disabling commands

Set `enabled: false` to temporarily hide a command from the launcher without deleting the file.

```yaml
phrase: open staging site
title: Open staging environment
enabled: false          # command is hidden until you change this back to true
action:
  type: open_url
  config:
    url: https://staging.example.com
```

- Omitting the `enabled` field is equivalent to `enabled: true`
- Changing the value in an open file and saving it takes effect within ~300 ms (live reload)
- Disabled commands are filtered out entirely on load — they never reach the frontend

---

## Live reload

Nimble watches your config directory with a file-system watcher. After you save any `.yaml` file the launcher updates itself within 300 ms. You do **not** need to restart the app.

Events that trigger a reload:
- Creating a new `.yaml`/`.yml` file anywhere in the config tree
- Editing and saving an existing file
- Deleting or moving a file

---

## Supported URL schemes

`open_url` accepts any URL that follows RFC 3986 — including `http://`, `https://`, `slack://`, `obsidian://`, `mailto:`, `tel:`, and other app deep-link schemes. See [Tips & Tricks](../reference/tips-and-tricks.md#app-deep-links) for examples.

---

## Reserved phrase prefix

Phrases that start with `/` are reserved for built-in app commands (e.g. `/ctx set`, `/ctx reset`). Any command file whose `phrase` starts with `/` will be rejected at load time and a warning will appear in the launcher. Rename the phrase to avoid the conflict.

---

## Longest phrase wins

When two commands have phrases where one is a prefix of the other (e.g. `open` and `open google`), a conflict can occur once the user has typed enough to match both in "param mode" — meaning the full phrase plus additional text. In this case, the **longest matching phrase is sorted first** so it becomes the default action when you press Enter. Both commands remain visible — you can still arrow-down to the shorter-phrase command if that's what you intended.

**Example:** Given two commands:

```yaml
# Command A
phrase: open
# Command B
phrase: open google
```

| User types | Result order | Reason |
|---|---|---|
| `ope` | A, B (discovery order) | Both match via partial/discovery — no reordering |
| `open goo` | A first (param mode), B (discovery) | Only A is in param mode |
| `open google maps` | **B first** (`open google`, arg = `maps`), then A (`open`, arg = `google maps`) | Both in param mode — longer phrase sorted first |

This ensures the most specific command is the default Enter target when the user has typed past the longer phrase.
