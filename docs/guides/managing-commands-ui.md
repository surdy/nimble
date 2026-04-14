# Managing Commands in the UI

Nimble includes a built-in command editor in the Preferences window. You can create, edit, organise, and delete commands without touching YAML files directly.

> **Tip:** You can always edit YAML files by hand instead — see [Configuring Commands](configuring-commands.md). The UI and file-based workflows are fully interchangeable; changes made in either are picked up instantly.

---

## Opening the command editor

- Type `/commands` in the launcher and press Enter, or
- Open Preferences from the system tray and select the **Commands** tab

---

## Browsing commands

The sidebar lists all commands grouped by **folder**. Each folder shows a header with its name, a command count, and a **+** / **−** indicator you can click to collapse or expand the group.

- **Root-level commands** (files directly in `commands/`) appear under the **Commands** heading.
- **Subfolder commands** appear under their folder path (e.g. `work/jira`).
- Use the **Filter** field at the top to search by phrase or title. While filtering, all folders auto-expand to show matches. You can still manually collapse individual folders during a search.

Click any command in the list to view and edit it in the detail panel.

---

## Creating a command

1. Click the **＋** button in the sidebar header.
2. Fill in the required fields:
   - **Phrase** — The multi-word phrase users type to trigger this command.
   - **Folder** — Choose an existing folder from the dropdown, click **New…** to create one, or leave it as **Root** to save at the top level of `commands/`.
   - **Title** — A human-readable description shown in search results.
   - **Action** — Select the action type and fill in its configuration.
3. Click **Create**.

The command is saved as a YAML file in the chosen folder and is immediately available in the launcher.

---

## Editing a command

Select a command from the sidebar. The detail panel shows all its fields. Make your changes and click **Save**.

The **Location** field shows which folder the command lives in (read-only).

---

## Deleting a command

Select the command and click **Delete**. A confirmation prompt appears — click **Delete** again to confirm. The YAML file is removed from disk.

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

Folders in the sidebar reflect the actual directory structure inside `commands/`. To organise commands:

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

When viewing an existing command, the bottom action bar includes two icon buttons:

- **Reveal in file manager** — Opens Finder (macOS), Files (Linux), or Explorer (Windows) with the YAML file selected.
- **Open in default editor** — Opens the YAML file in your system's default text editor.

These are useful for making advanced edits (e.g. adding `env:` blocks, editing `.tsv` list files, or working with complex scripts).

---

## Tips

- **Live reload**: Changes saved from the UI or from an external editor are picked up instantly — no restart needed.
- **Enabled toggle**: Disable a command without deleting it by unchecking **Enabled**. The YAML file stays on disk with `enabled: false`.
- **Phrase conflicts**: The editor warns if another command already uses the same phrase.
- **Copilot skill**: For AI-assisted command creation, deploy the Nimble skill with `/deploy copilot skill` — see [Copilot Skill](copilot-skill.md).
