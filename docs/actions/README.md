# Actions

Every Nimble command triggers exactly one **action** — the thing that happens when you select a result. There are six action types, each configured in the `action:` block of a command YAML file.

---

## Action types

| Action | What it does |
|--------|-------------|
| [Open URL](open-url.md) | Opens a URL in your default browser — supports `{param}` substitution |
| [Paste Text](paste-text.md) | Pastes a predefined block of text into your previously focused app |
| [Copy Text](copy-text.md) | Copies a predefined block of text to the clipboard |
| [Static List](static-list.md) | Displays a named TSV list of items inline when the phrase matches exactly |
| [Dynamic List](dynamic-list.md) | Runs an external script and shows its output as an inline list |
| [Script Action](script-action.md) | Runs a script on Enter and applies the result via open_url, paste_text, or copy_text |

For the full command YAML schema see [Configuring Commands](../guides/configuring-commands.md).

---

## Executing a command

| Method | Description |
|--------|-------------|
| `Enter` | Executes the currently highlighted result |
| `↑` / `↓` | Move the highlight; `Enter` to confirm |
| **Click** | Click any result row to execute it immediately |
