# Script Action

A `script_action` command runs an external script when you press **Enter** and immediately applies the result through one of the launcher's built-in actions — it opens URLs, pastes text, or copies to the clipboard without showing any intermediate list.

This is the right choice when a script produces a **value to act on** rather than a list of items to browse. To display an interactive list instead, use [Dynamic List](dynamic-list.md).

All scripts automatically receive [built-in `NIMBLE_*` environment variables](../guides/writing-scripts.md#built-in-environment-variables) with context, paths, and platform metadata. You can also define your own variables via [user-defined environment variables](../guides/writing-scripts.md#user-defined-environment-variables).

---

## YAML schema

```yaml
phrase: <string>          # phrase users type to trigger this command
title: <string>           # human-readable label shown in results

action:
  type: script_action
  config:
    script: <string>      # script filename co-located with this YAML (no path separators),
                          # or a ${VAR}-substituted path to an external script
    arg: none | optional | required   # default: none
    result_action: open_url | paste_text | copy_text
    prefix: <string>      # optional — prepended to each value (paste_text / copy_text only)
    suffix: <string>      # optional — appended to each value (paste_text / copy_text only)
```

---

## Minimal example — paste current timestamp

Create a subdirectory in `commands/` with the command YAML and script together:

**`commands/paste-timestamp/paste-timestamp.yaml`:**

```yaml
phrase: paste timestamp
title: Paste current date/time
action:
  type: script_action
  config:
    script: timestamp.sh
    arg: none
    result_action: paste_text
```

**`commands/paste-timestamp/timestamp.sh`:**

```sh
#!/bin/sh
date
```

```sh
chmod +x ~/Library/Application\ Support/Nimble/commands/paste-timestamp/timestamp.sh
```

Type `paste timestamp` and press **Enter** — the current date/time is pasted at the cursor.

---

## Accepting an argument

Set `arg` to let the user supply input after the phrase:

| Value | Behaviour |
|-------|-----------|
| `none` (default) | Script always runs with no argument; any text after the phrase is ignored. |
| `optional` | If the user types text after the phrase, it is passed to the script as `$1`; otherwise the script runs without an argument. |
| `required` | The command only executes when the user has typed text after the phrase. |

```yaml
phrase: open repo
title: Open GitHub repo for…
action:
  type: script_action
  config:
    script: get-repo-url.sh
    arg: required
    result_action: open_url
```

Type `open repo myorg/myrepo` → `get-repo-url.sh myorg/myrepo` → opens the returned URL.

---

## Script output format

The script writes to **stdout**. Nimble accepts two formats:

| Format | Example | Interpreted as |
|--------|---------|----------------|
| Plain text | `hello world` | A single value: `["hello world"]` |
| JSON array of strings | `["https://a.com","https://b.com"]` | Multiple values |

Any output on **stderr** is written to the launcher's console log but does not affect execution.

A **5-second timeout** is enforced. If the script exceeds this limit the action is cancelled and an error is logged.

---

## Applying the result

### `result_action: open_url`

Each returned value is opened as a URL (in the default browser or the matching app). When the script returns multiple URLs they are all opened, one after the other.

```yaml
phrase: open docs urls
title: Open all docs pages
action:
  type: script_action
  config:
    script: get-doc-urls.sh
    result_action: open_url
```

### `result_action: paste_text`

All returned values are concatenated — each value wrapped with the optional `prefix` and `suffix` — into a single string, which is then pasted at the cursor.

```yaml
phrase: paste team emails
title: Paste team email addresses (To: header)
action:
  type: script_action
  config:
    script: team-emails.sh
    result_action: paste_text
    prefix: ""
    suffix: "\n"
```

If `team-emails.sh` outputs `["alice@example.com","bob@example.com"]`, the pasted text is:

```
alice@example.com
bob@example.com
```

### `result_action: copy_text`

Identical to `paste_text` but copies the combined text to the clipboard instead of simulating a keystroke. The user pastes manually.

---

## Script requirements

- Scripts must be executable (`chmod +x`).
- Scripts are co-located with their command YAML file — filename only, no subdirectory paths or `..` components allowed.
- Scripts run in a sandboxed subprocess: they can read files and make network calls but **cannot** directly trigger launcher actions. All actions go through the launcher's built-in layer.

---

## Platform-specific notes

**macOS / Linux** — use a shebang line:

```sh
#!/usr/bin/env python3
import json, sys
print(json.dumps(["value1", "value2"]))
```

**Windows** — use `.bat` or `.ps1` files; PowerShell scripts must have execution policy set appropriately.
