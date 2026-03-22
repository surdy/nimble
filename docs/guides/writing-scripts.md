# Writing Scripts

The `dynamic_list` and `script_action` actions can run any executable co-located with the command YAML file.

---

## Overview

Scripts live in the same directory as their command YAML file inside `commands/`. Any executable file can be used: shell scripts, Python programs, Node.js scripts, compiled binaries, etc. Nimble spawns the script, captures its stdout, and renders the output as a list of selectable items.

---

## Writing a script

### Output format

Your script must write to **stdout** only. Stderr is captured and logged internally but never shown to the user.

**Option 1 — JSON array (recommended):**

Return a JSON array of objects. Each object must have a `title` field; `subtext` is optional.

```json
[
  { "title": "Alice Smith", "subtext": "alice@example.com" },
  { "title": "Bob Jones", "subtext": "bob@example.com" },
  { "title": "Carol White" }
]
```

**Option 2 — Plain text:**

Return a single line of text. Nimble treats the entire trimmed output as the title of one item.

```
Hello, World!
```

**Empty output:** Return nothing (exit immediately) to show an empty list.

### Accepting an argument

When the command's `arg` mode is `optional` or `required`, Nimble passes the user's typed suffix as the first positional argument (`$1` in shell, `sys.argv[1]` in Python, `process.argv[2]` in Node.js).

```sh
#!/bin/sh
# Filter contacts by the typed query (first argument)
QUERY="$1"
if [ -z "$QUERY" ]; then
  # No argument — return all contacts
  echo '[{"title":"Alice Smith","subtext":"alice@example.com"},{"title":"Bob Jones","subtext":"bob@example.com"}]'
else
  # Filter by query (simple grep example)
  echo '[{"title":"Alice Smith","subtext":"alice@example.com"}]'
fi
```

### Timeout

Nimble enforces a **5-second timeout**. If the script does not exit within 5 seconds, an empty list is shown and the script process is abandoned. Keep scripts fast.

---

## Registering a script as a command

Create a subdirectory in `commands/` containing both the command YAML and the script:

```
commands/search-contacts/
  search-contacts.yaml    ← command YAML
  contacts.sh             ← script, co-located
```

**`commands/search-contacts/search-contacts.yaml`:**

```yaml
phrase: search contacts
title: Search contacts
action:
  type: dynamic_list
  config:
    script: contacts.sh     # resolves to the same directory as this YAML
    arg: optional           # none | optional | required
    item_action: paste_text # optional; same as static_list
```

See [Dynamic List](../actions/dynamic-list.md) for the full YAML schema and argument mode reference.

---

## Argument modes

| `arg` value | When the list appears | Script receives |
|-------------|----------------------|--------------------|
| `none` *(default)* | Exact phrase match only | No arguments |
| `optional` | Exact match (immediately) **and** phrase + suffix | No arg on exact match; suffix otherwise |
| `required` | Only when a non-empty suffix follows the phrase | The typed suffix as `$1` |

**`none`** — good for static or slow-changing data that does not need filtering.  
**`optional`** — good for searchable lists where seeing all items first is useful.  
**`required`** — good for queries that only make sense with user input (e.g. search, lookup).

---

## Security boundaries

- By default, `script` and `list` field values containing `/`, `\`, or `..` are **rejected** at invocation time. Plain filenames must be co-located with the command YAML — no subdirectories or path traversal.
- When a `script` or `list` value uses `${VAR}` substitution, the resolved path may point outside the command directory. This is allowed by default (`allow_external_paths: true` in `settings.yaml`). Set `allow_external_paths: false` to restrict all resolved paths to the command directory.
- Scripts run with the **same user privileges** as the Nimble launcher process. They are never elevated.
- Scripts **cannot** directly trigger launcher actions. They can only produce output. The launcher decides what to do with each item based on `item_action`.
- Script output is parsed and validated. Malformed JSON is treated as plain text; an entirely unparseable response shows an empty list.

---

## External scripts and lists

By default, `script:` and `list:` fields must be plain filenames co-located with the command YAML. To reference files outside the command directory, use `${VAR}` substitution.

### How it works

Any `${VAR}` token in a `script:` or `list:` field is replaced with the variable's value before the path is resolved. Variables are looked up in this order:

1. Built-in `NIMBLE_*` variables (e.g. `NIMBLE_CONFIG_DIR`, `NIMBLE_COMMAND_DIR`)
2. User-defined variables (global `env.yaml` → sidecar `env.yaml` → inline `env:`)

If the resolved path is absolute, it is used directly. If relative, it is resolved against the command directory.

### Example: shared scripts directory

```yaml
# ~/Library/Application Support/Nimble/env.yaml
SHARED_SCRIPTS: /opt/team/scripts
```

```yaml
# commands/team-emails.yaml
phrase: team emails
title: Team email addresses
action:
  type: dynamic_list
  config:
    script: ${SHARED_SCRIPTS}/contacts.sh
    arg: none
```

### Example: using NIMBLE_CONFIG_DIR

```yaml
# References a script in the config root's scripts/ folder
phrase: run utility
title: Run shared utility
action:
  type: script_action
  config:
    script: ${NIMBLE_CONFIG_DIR}/scripts/utility.sh
    result_action: paste_text
```

### Disabling external paths

If you want to restrict scripts and lists to their co-located directories only, set `allow_external_paths: false` in `settings.yaml`:

```yaml
# ~/Library/Application Support/Nimble/settings.yaml
allow_external_paths: false
```

When disabled, any `${VAR}`-substituted path that resolves outside the command directory is rejected with an error. Plain filenames (without `${…}`) are always co-located and unaffected by this setting.

---

## Debugging tips

1. **Run the script directly** from your terminal to see its output and any errors:
   ```sh
   ~/Library/Application\ Support/Nimble/commands/search-contacts/contacts.sh "test arg"
   ```

2. **Check stderr** — Nimble logs scripts' stderr output with `[ctx] script "..." stderr:` prefix. Look in the app's log output (visible when running via `npm run tauri dev`).

3. **Validate JSON** — if items aren't appearing, paste your script's output into a JSON validator. Common issues: trailing commas, unescaped quotes, non-UTF-8 bytes.

4. **Check permissions** — on macOS/Linux the script file must be executable:
   ```sh
   chmod +x ~/Library/Application\ Support/Nimble/commands/search-contacts/contacts.sh
   ```

5. **Live reload** — editing any script file in `commands/` triggers a reload. If a dynamic list is currently displayed it re-runs automatically within the 300 ms debounce window.

---

## Built-in environment variables

Nimble injects a set of `NIMBLE_*` environment variables into every script it runs. These are available to both `dynamic_list` and `script_action` scripts without any configuration.

| Variable | Value | Example |
|----------|-------|--------|
| `NIMBLE_CONTEXT` | Active context string (empty if none) | `reddit` |
| `NIMBLE_PHRASE` | Command phrase that triggered this script | `search contacts` |
| `NIMBLE_CONFIG_DIR` | Absolute path to the Nimble config root | `/Users/you/Library/Application Support/Nimble` |
| `NIMBLE_COMMAND_DIR` | Absolute path to the directory containing the command YAML | `/Users/you/Library/Application Support/Nimble/commands/search-contacts` |
| `NIMBLE_OS` | Operating system: `macos`, `linux`, or `windows` | `macos` |
| `NIMBLE_VERSION` | Nimble app version | `0.1.0` |

### Usage examples

**Shell:**
```sh
#!/bin/sh
if [ "$NIMBLE_OS" = "macos" ]; then
    open "https://example.com"
fi
echo "Running in context: $NIMBLE_CONTEXT"
```

**PowerShell:**
```powershell
if ($env:NIMBLE_OS -eq "windows") {
    Write-Output "Windows detected"
}
```

**Python:**
```python
import os
context = os.environ.get("NIMBLE_CONTEXT", "")
command_dir = os.environ["NIMBLE_COMMAND_DIR"]
```

These variables are always present and read-only. Scripts do not need to declare or request them.

---

## User-defined environment variables

You can inject your own environment variables into scripts using `env.yaml` files and inline `env:` blocks. This is useful for shared values like team IDs, base URLs, or email addresses.

### Precedence

Variables are merged in order (later layers override earlier ones):

| Layer | Location | Scope |
|-------|----------|-------|
| 1. Global | `Nimble/env.yaml` (config root) | All commands |
| 2. Sidecar | `env.yaml` in the command's directory | Commands in that directory |
| 3. Inline | `env:` block in the command YAML | That command only |

Built-in `NIMBLE_*` variables always take precedence and cannot be overridden.

### Global env.yaml

Create `env.yaml` at the Nimble config root (alongside `settings.yaml`):

```yaml
# ~/Library/Application Support/Nimble/env.yaml
WORK_EMAIL: alice@example.com
JIRA_BASE_URL: https://mycompany.atlassian.net
TEAM_SLACK_CHANNEL: C0123456789
```

Every script will receive these as environment variables.

### Sidecar env.yaml

Place an `env.yaml` in the same directory as your command YAML:

```
commands/jira/
  env.yaml              ← shared by all commands in this directory
  create-ticket.yaml
  create-ticket.sh
  close-ticket.yaml
  close-ticket.sh
```

```yaml
# commands/jira/env.yaml
JIRA_PROJECT: MYPROJ
JIRA_BOARD_ID: "42"
```

Only commands in that exact directory see these variables. There is no directory walking — a parent directory's `env.yaml` does not apply to subdirectories.

### Inline env

Add an `env:` block at the top level of a command YAML for single-command overrides:

```yaml
phrase: create bug
title: Create a bug report
env:
  TICKET_TYPE: bug
action:
  type: script_action
  config:
    script: create-ticket.sh
    arg: required
    result_action: open_url
```

### Key naming rules

- Keys must match `[A-Za-z_][A-Za-z0-9_]*` (letters, digits, underscores).
- Keys starting with `NIMBLE_` are reserved and rejected.
- Values are always strings. Numeric and boolean YAML values are coerced to strings automatically in `env.yaml` files. In inline `env:` blocks, quote non-string values (e.g. `PORT: "8080"`).

---

## Windows: PowerShell scripts

On Windows, use `.ps1` files instead of shell scripts. PowerShell is available on all Windows 10/11 systems without installation.

### Minimal example (`hello.ps1`)

```powershell
# Example dynamic list script.
# Output a JSON array or plain text — same format as on macOS/Linux.
Write-Output '[{"title":"Hello from a script","subtext":"Edit scripts/hello.ps1 to customise"},{"title":"Dynamic lists are powerful","subtext":"Return JSON or plain text from any executable"}]'
```

### Accepting an argument

```powershell
param([string]$Query = "")
# Filter a list by the typed query
$items = @("Alice", "Bob", "Carol") | Where-Object { $_ -match $Query }
$json = $items | ForEach-Object { '{"title":"' + $_ + '"}' }
Write-Output ('[' + ($json -join ',') + ']')
```

### Registering a PowerShell script

Point the `script` field at the `.ps1` filename:

```yaml
phrase: search names
title: Search names
action:
  type: dynamic_list
  config:
    script: search-names.ps1
    arg: optional
```

> **Note:** Nimble invokes scripts via `cmd /C <script>` on Windows. PowerShell scripts with a `.ps1` extension are launched via `powershell -ExecutionPolicy Bypass -File <script>`. Ensure your PowerShell execution policy permits running scripts, or use `-ExecutionPolicy Bypass` as shown above.

