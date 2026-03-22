# Static List

Displays a named list of items inline in the launcher the moment the typed phrase matches exactly. No `Enter` is needed to expand the list — it appears as soon as the full phrase is typed. By default, selecting an item simply dismisses the launcher. You can optionally configure an action to be performed on the selected item's value.

---

## Directory layout

A `static_list` command requires two files — the command YAML and the list TSV file — living together in the same subdirectory of `commands/`:

```
commands/
  show-team-emails/
    show-team-emails.yaml      ← command YAML
    team-emails.tsv            ← list file (referenced by name)
```

The command YAML references the list file by name (without the `.tsv` extension), or uses a `${VAR}`-substituted path for external lists (see [External scripts and lists](../guides/writing-scripts.md#external-scripts-and-lists)):

```yaml
action:
  type: static_list
  config:
    list: team-emails          # resolves to team-emails.tsv in the same directory
```

---

## List file format

List files use **TSV (tab-separated values)** format: one item per line, with a tab character separating the title from an optional subtext. Lines starting with `#` are comments. Blank lines are ignored.

**`commands/show-team-emails/team-emails.tsv`:**
```
# Team email addresses
Alice Smith	alice@example.com
Bob Jones	bob@example.com
Carol White	carol@example.com
```

| Column | Required | Notes |
|--------|----------|-------|
| Title (before tab) | ✅ | Displayed as the result title; used as the paste value if no subtext |
| Subtext (after tab) | No | Secondary display line; also the value used when the item is selected |

If a line has no tab character, the entire line is used as the title (subtext is absent).

> **Tip:** You can paste directly from a spreadsheet — Excel and Google Sheets copy cell ranges as TSV by default.

---

## Command YAML

```yaml
# Minimal — selecting an item just dismisses the launcher
phrase: team emails
title: Team email addresses
action:
  type: static_list
  config:
    list: team-emails

# With paste action — selecting an item pastes its subtext
phrase: paste snippet
title: Pick a text snippet
action:
  type: static_list
  config:
    list: snippets
    item_action: paste_text

# With copy action — selecting an item copies its subtext to clipboard
phrase: copy link
title: Pick a link to copy
action:
  type: static_list
  config:
    list: links
    item_action: copy_text

# With open_url — the item's subtext must be a valid URL
phrase: open bookmark
title: Pick a bookmark to open
action:
  type: static_list
  config:
    list: bookmarks
    item_action: open_url
```

| `item_action` value | What happens on selection |
|---------------------|---------------------------|
| *(absent)* | Launcher dismisses; no further action |
| `paste_text` | Item's `subtext` (or `title`) is pasted into the previously focused app |
| `copy_text` | Item's `subtext` (or `title`) is copied to the clipboard |
| `open_url` | Item's `subtext` (or `title`) is opened as a URL in the default browser |

> **Note:** `subtext` serves a dual purpose — it's the secondary hint shown in the UI *and* the value used when the item is acted on. If `subtext` is absent, `title` is used instead.

---

## Behaviour

| Phase | What happens |
|-------|--------------|
| Partial match (e.g. `team`) | Command appears as a single result row, like any other command |
| Exact phrase match (`team emails`) | List items replace the result row immediately — no `Enter` required |
| Backspace past exact match | List collapses; standard partial-match results return |
| Select an item (`Enter` / click) | Performs the configured `item_action`, or dismisses if none is set |
