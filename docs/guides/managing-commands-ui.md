# Managing Commands in the UI

Nimble includes a built-in command editor in the Preferences window. You can create, edit, organise, and delete commands without touching YAML files directly.

> **Tip:** You can always edit YAML files by hand instead — see [Configuring Commands](configuring-commands.md). The UI and file-based workflows are fully interchangeable; changes made in either are picked up instantly.

---

## Opening the command editor

- Type `/commands` in the launcher and press Enter, or
- Open Preferences from the system tray and select the **Commands** tab

---

## Browsing commands

The Commands tab displays all commands in a **table view** with sortable columns:

| Column | Description |
|--------|-------------|
| ☑ | Bulk-select checkbox (visible on hover or when checked) |
| **Phrase** | The trigger phrase in monospace |
| **Title** | Human-readable description |
| **Action** | Colour-coded badge showing the action type |
| **Modified** | Relative timestamp (e.g. "2d ago") |
| **On** | Toggle switch to enable/disable |

### Sorting

Click any column header (Phrase, Title, Action, Modified) to sort. Click again to reverse the direction. The active sort column shows ▲ or ▼.

### Grouping

Use the **Group** segmented control in the toolbar to group commands by:

- **None** — flat list sorted by the active column
- **Folder** — grouped by directory structure, with collapsible folder headers
- **Type** — grouped by action type

### Filtering

Use the **Filter** field in the toolbar to search by phrase or title. While filtering, all folder groups auto-expand to show matches.

---

## Bulk actions

Select multiple commands using the checkboxes in the first column:

- The **header checkbox** selects/deselects all visible commands.
- Row checkboxes appear on hover or when any row is already checked.
- Press **Escape** to clear the selection.

When one or more commands are selected, a **bulk-action bar** appears above the table with:

- **Enable all** — enables every selected command
- **Disable all** — disables every selected command
- **Delete all** — deletes every selected command (with confirmation)
- **Clear selection** — deselects all

---

## Creating a command

1. Click the **+ New** button in the toolbar (or press ⌘N).
2. Fill in the required fields:
   - **Phrase** — The phrase users type to trigger this command.
   - **Folder** — Choose an existing folder from the dropdown, click **New…** to create one, or leave it as **Root** to save at the top level of `commands/`.
   - **Title** — A human-readable description shown in search results.
   - **Action** — Select the action type and fill in its configuration.
3. Click **Save**.

The command is saved as a YAML file in the chosen folder and is immediately available in the launcher.

---

## Editing a command

Double-click a row, or select it and click **Edit** (or press Enter). The detail editor opens showing all fields. Make your changes and click **Save**, then click **← Back to list** to return to the table.

---

## Deleting a command

Select the command and click **Delete** (or press ⌫). A confirmation prompt appears — click **Delete** again to confirm. The YAML file is removed from disk.

You can also right-click a row to access **Delete Command** from the context menu.

---

## Context menu

Right-click any row to access quick actions:

- **Edit Command** — open the detail editor
- **Enable / Disable** — toggle the command without opening the editor
- **Delete Command** — delete with confirmation

---

## Action types

The action dropdown offers all six types:

| Action | What it does |
|--------|-------------|
| **Open URL** | Opens a URL in the default browser. Use `{param}` in the URL to accept user input. |
| **Paste Text** | Pastes text into the app that had focus before the launcher. |
| **Copy Text** | Copies text to the clipboard. |
| **Static List** | Displays a named `.tsv` list file. Each item can trigger paste, copy, or open. |
| **Dynamic List** | Runs a script and displays its output as a list. |
| **Script Action** | Runs a script and applies a built-in action to each returned value. |

For full details on each action type, see [Actions](../actions/README.md).

---

## Inline script editor

When the action type is **Dynamic List** or **Script Action**, the detail panel shows an inline script editor below the script name field.

- If the script file exists, its content is loaded into a monospace text area. Edit and click **Save script** to write changes to disk.
- If the script file doesn't exist, click **Create from template** to generate a starter script with a shebang and sample output.
- An **unsaved** badge appears when you have pending script changes.
- Script files are saved with executable permissions on macOS and Linux.

> **Note:** The inline editor works for **co-located scripts** (plain filenames like `hello.sh`) and **shared scripts** (using the `shared:` prefix, e.g. `shared:contacts.sh`). If the script field uses legacy `${VAR}` substitution, the editor shows a migration hint.

---

## Organising commands in folders

Folders in the table reflect the actual directory structure inside `commands/`. To organise commands:

- **When creating a new command**, select a folder from the dropdown or click **New…** to create a new subfolder.
- **Existing commands** show their location but can't be moved between folders from the UI. To move a command, use Finder or your file manager (or the **Reveal in file manager** button).

### Recommended folder patterns

```
commands/
  open-github.yaml              ← simple commands at root
  search-google.yaml
  work/
    open-jira.yaml              ← grouped by topic
    open-confluence.yaml
  snippets/
    paste-greeting.yaml
    paste-signature.yaml
  search-contacts/
    search-contacts.yaml        ← command + script co-located
    search.sh
```

---

## File management buttons

When viewing a command in the detail editor, the action bar includes two icon buttons:

- **Reveal in file manager** — Opens Finder (macOS), Files (Linux), or Explorer (Windows) with the YAML file selected.
- **Open in default editor** — Opens the YAML file in your system's default text editor.

These are useful for making advanced edits (e.g. adding `env:` blocks, editing `.tsv` list files, or working with complex scripts).

---

## Tips

- **Live reload**: Changes saved from the UI or from an external editor are picked up instantly — no restart needed.
- **Enabled toggle**: Disable a command without deleting it by toggling the **On** switch in the table row. The YAML file stays on disk with `enabled: false`.
- **Phrase conflicts**: The editor warns if another command already uses the same phrase.
- **Keyboard shortcuts**: ⌘N to create, Enter to edit, ⌫ to delete, Escape to close or clear selection.
- **Copilot skill**: For AI-assisted command creation, deploy the Nimble skill with `/deploy copilot skill` — see [Copilot Skill](copilot-skill.md).
