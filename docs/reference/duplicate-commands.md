# Duplicate Commands

Nimble detects when two or more command files define the same `phrase` and handles the conflict automatically rather than silently picking one.

---

## What counts as a duplicate

Two commands are duplicates if their `phrase` fields are identical after case-normalisation. The `title`, `action`, or any other field does not matter — only the phrase is compared.

```yaml
# file-a.yaml
phrase: open github
title: Open GitHub (personal)
...

# file-b.yaml
phrase: open github       # same phrase → duplicate
title: Open GitHub (work)
...
```

---

## How duplicates are resolved

When duplicates are found, Nimble keeps the command from the **oldest file** (by last-modified time) and ignores all later definitions of the same phrase. If two files share the same modification time, the file with the lexicographically earlier path is kept.

This policy ensures that the outcome is deterministic and stable: whichever version of the phrase you created first wins, and adding a new file never silently overwrites an existing command.

---

## Duplicate warning in the UI

When at least one duplicate is found, the launcher displays an amber warning bar at the top of the results:

```
⚠  2 duplicate command(s) ignored  ×
```

Clicking `×` dismisses the bar for the current session. The bar reappears the next time the launcher is opened if the duplicate files still exist.

The warning is purely informational — all non-duplicate commands continue to work normally.

---

## Fixing duplicates

To resolve a duplicate warning, open the two conflicting files and either:

1. **Change one phrase** — give the second command a different `phrase` so both are usable
2. **Delete one file** — if you only need one version, remove the redundant file
3. **Disable one file** — set `enabled: false` in the file you want dormant for now

After saving or deleting, the live-reload watcher picks up the change within ~300 ms and the warning disappears automatically if no duplicates remain.

---

## Identifying which files conflict

The duplicate warning in the UI currently shows the count. To identify the specific files, check the Nimble log output (visible in the terminal if you launched Nimble from a shell, or in the Tauri dev console). Each ignored duplicate logs a message like:

```
[nimble] duplicate phrase \"open github\": keeping snippets/open-github.yaml, ignoring work/open-github.yaml
```
