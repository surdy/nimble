# Dynamic List

Runs an external script and displays its output as a list of items the moment the phrase is matched. Like [Static List](static-list.md), no `Enter` is required to expand the list. Scripts can optionally accept a typed argument to filter or parameterise their output in real time.

Scripts are co-located with their command YAML file in the same directory inside `commands/`. They can be any executable — shell scripts, Python programs, compiled binaries, etc. You can also reference external scripts using `${VAR}` substitution in the `script:` field — see [External scripts and lists](../guides/writing-scripts.md#external-scripts-and-lists).

All scripts automatically receive [built-in `NIMBLE_*` environment variables](../guides/writing-scripts.md#built-in-environment-variables) with context, paths, and platform metadata. You can also define your own variables via [user-defined environment variables](../guides/writing-scripts.md#user-defined-environment-variables).

---

## Script output format

Each script writes to stdout and Nimble parses the result:

| Output type | How Nimble interprets it |
|-------------|----------------------|
| **JSON array** | Array of `{ "title": "...", "subtext": "..." }` objects (subtext optional) |
| **Plain text** | Entire stdout (trimmed) becomes the title of a single result item |
| **Empty stdout** | Empty list (no items shown) |

**Example JSON output:**
```json
[
  { "title": "Alice Smith", "subtext": "alice@example.com" },
  { "title": "Bob Jones" }
]
```

---

## Command YAML

```yaml
# Runs once on exact match; extra typing collapses the list
# Script lives alongside this YAML (e.g., commands/team-emails/team-emails.sh)
phrase: team emails
title: Team email addresses
action:
  type: dynamic_list
  config:
    script: team-emails.sh
    arg: none

# List appears on exact match; re-runs with suffix as the user types
phrase: search contacts
title: Search contacts
action:
  type: dynamic_list
  config:
    script: contacts.sh
    arg: optional
    item_action: paste_text

# No list until user types a suffix; suffix is always passed to the script
phrase: find file
title: Find a file
action:
  type: dynamic_list
  config:
    script: find-file.sh
    arg: required
```

---

## Argument modes

| `arg` value | When the list appears | Script receives |
|-------------|----------------------|-----------------|
| `none` *(default)* | Exact phrase match only | No arguments |
| `optional` | Exact match (immediately) **or** phrase + space + suffix | No arg on exact match; suffix when present |
| `required` | Only when a non-empty suffix follows the phrase | The typed suffix as its first argument |

For `optional` and `required` modes the script is re-invoked as the user types, with a 200 ms debounce to prevent excessive calls. For how to use arguments inside a script, see [Writing Scripts](../guides/writing-scripts.md#accepting-an-argument).

---

## `item_action`

When an item is selected, Nimble uses the item's **`subtext`** as the value for the action (falling back to `title` if `subtext` is absent). This means `subtext` serves a dual purpose: it's the secondary line shown in the UI *and* the payload that gets pasted, copied, or opened.

| `item_action` value | What happens on selection |
|---------------------|---------------------------|
| *(absent)* | Launcher dismisses; no further action |
| `paste_text` | Item's `subtext` (or `title`) is pasted into the previously focused app |
| `copy_text` | Item's `subtext` (or `title`) is copied to the clipboard |
| `open_url` | Item's `subtext` (or `title`) is opened as a URL in the default browser |

---

## Behaviour

| Phase | What happens |
|-------|------|
| Partial match | Command appears as a single result row |
| Trigger condition met (see arg modes above) | Script runs; list items replace result row |
| Script returns no output | List stays empty; command row shows in results |
| Backspace past trigger condition | List collapses; standard results return |
| Select an item | Performs `item_action`, or dismisses if absent |
| Script times out (> 5 s) | Empty list shown; no error surfaced to user |

For how to write scripts and advanced examples see [Writing Scripts](../guides/writing-scripts.md).
