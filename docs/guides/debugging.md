# Debugging

Nimble surfaces errors directly in the launcher so you can see what went wrong without leaving the keyboard. For deeper diagnostics, debug mode adds a detailed session log.

---

## Error reporting

When a command fails — a missing list file, a script that exits non-zero, a permission error — Nimble shows the error inline as a ⚠️ item in the results list. The error message is the actual message from the backend, not a generic placeholder.

| Situation | What you see |
|-----------|-------------|
| No command matches what you typed | **No matching commands** |
| A list/script command ran successfully but returned zero items | **No results returned** — _script name_ |
| Script is still executing | **Running…** → **Taking longer than expected…** → **Script may be close to the 5s timeout…** |
| A `static_list` file could not be loaded | ⚠️ **Error loading list** — _error details_ |
| A `dynamic_list` script failed | ⚠️ **Script error** — _first line of stderr or exit code_ |
| A `script_action` script failed | ⚠️ **Action error** — _first line of stderr or exit code_ |

Error items are interactive — press **Enter** or click to copy the error message to your clipboard.

Errors are always visible. You do not need debug mode to see them.

### Script loading state

When a `dynamic_list` or `script_action` command is running, the results area shows a live status:

- **Running…** — shown immediately when the script starts
- **Taking longer than expected…** — shown after 2 seconds
- **Script may be close to the 5s timeout…** — shown after 4 seconds

This lets you distinguish a slow script from a broken command or a phrase that simply didn't match.

### Script error messages

When a script exits with a non-zero exit code, Nimble shows the **first non-empty line of stderr** as the error subtext. For example:

```
⚠️ Script error — exit 1: jq: command not found
```

You do not need debug mode on to see this.

---

## Load-time warnings in Preferences

When Nimble loads your command files, it validates them and surfaces any problems in **Preferences → Commands**. Warnings appear in a collapsible section at the top of the command list.

### Skipped files

If a YAML file cannot be parsed, it is skipped and listed under **Skipped Files** with the exact serde error message (including line and column numbers). Common causes:

- Invalid YAML syntax
- Unknown field names — e.g. `phrases:` instead of `phrase:`, `urls:` instead of `url:` inside the config block. Nimble uses strict schema validation (`deny_unknown_fields`), so any unrecognised key becomes a visible error rather than a silent no-op.
- Missing required fields — e.g. `action:` block present but `config:` omitted

Example skipped-file entry:

```
commands/my-command.yaml
  unknown field `phrases`, expected one of `phrase`, `title`, `enabled`, `env`, `action` at line 1 column 8
```

### Command warnings

Successfully-parsed commands that have potential problems appear under **Command Warnings**:

| Warning | Meaning |
|---------|---------|
| Script not found | The `script:` file does not exist — check the name and path |
| Script not executable | The script file exists but lacks the execute bit (`chmod +x`) |
| Env key `NIMBLE_*` is reserved | An inline `env:` key starts with the reserved prefix |
| Invalid env key | An inline `env:` key contains illegal characters |

Warnings do not prevent the command from loading — they are advisory notices so you can fix problems before they cause a confusing runtime failure.

---

## Test Run button

The inline script editor in **Preferences → Commands** has a **Test** button that runs the script in-place and shows the raw output — without going through the launcher flow.

To use it:
1. Select a `dynamic_list` or `script_action` command in the sidebar
2. The script editor appears below the command fields
3. Click **Test** (only active when the script file exists on disk)
4. Results appear inline:
   - A green **exit 0** badge (or red **exit N**) and wall-clock duration
   - **stdout** — exactly what the script printed
   - **stderr** — shown in red if non-empty
   - **(no output)** — if the script produced nothing

The test run passes no argument and uses an empty context and phrase. It is useful for a quick sanity-check that the script exists, is executable, and produces valid output.

---

## Debug mode

Debug mode adds a session-scoped log that records every action, script invocation, and error with full detail (paths, arguments, environment, exit codes, stdout/stderr, timing).

### Toggling debug mode

Type `/debug` in the launcher and press Enter. A brief confirmation appears:

- **"Debug mode ON"** — logging is active for this session
- **"Debug mode OFF"** — logging stopped

Debug mode resets when Nimble restarts. It is not persisted across launches.

### What gets logged

When debug mode is on, Nimble records:

- **Actions** — every `open_url`, `paste_text`, and `copy_text` invocation with the resolved value
- **Script runs** — script path, arguments, environment variables, exit code, stdout, stderr, and wall-clock duration
- **Errors** — the full error chain for any failure
- **JSON fallback** — a `WARN` entry when a script's stdout is not valid JSON and Nimble falls back to treating the entire output as a single plain-text item

### Viewing the log

| Command | What it does |
|---------|-------------|
| `/debug log` | Shows the log inline as a scrollable list (newest first). The first item opens the log file in your editor. |
| `/debug log open` | Opens the `debug.log` file directly in your default text editor. |

The log file is written to `debug.log` inside your [config directory](../reference/config-directory.md):

```
~/Library/Application Support/nimble/debug.log      # macOS
~/.config/nimble/debug.log                           # Linux
%APPDATA%\nimble\debug.log                           # Windows
```

### Sample log output

```
[2026-04-16 09:12:01] ACTION open_url → https://github.com
[2026-04-16 09:12:15] SCRIPT commands/say-hello/hello.sh (arg: none)
  env: NIMBLE_CONTEXT= NIMBLE_CONFIG_DIR=/Users/you/Library/Application Support/nimble
  exit: 0  (42 ms)
  stdout: [{"title":"Hello, world!","subtext":"A friendly greeting"}]
  stderr: (empty)
[2026-04-16 09:12:22] ACTION paste_text → "Best regards,\nJane Smith"
[2026-04-16 09:12:30] ERROR script_action commands/broken/fail.sh
  exit: 1  (108 ms)
  stderr: /bin/sh: line 3: jq: command not found
[2026-04-16 09:12:45] WARN run_script: script output was not valid JSON, treating as plain text (single item)
  output contains 5 lines but was returned as a single item — did you mean to output JSON?
```

### `NIMBLE_DEBUG` environment variable

When debug mode is active, Nimble sets `NIMBLE_DEBUG=1` in the environment of every script it runs. Scripts can check for this to emit extra diagnostic output:

```sh
#!/bin/sh
if [ "$NIMBLE_DEBUG" = "1" ]; then
  echo "debug: running with arg=$1" >&2
fi
```

When debug mode is off, `NIMBLE_DEBUG` is not set at all (not set to `0` — it is absent).

---

## Quick reference

1. **Something broke?** Read the ⚠️ error message in the launcher — it tells you what went wrong, including the first line of stderr.
2. **Command not loading?** Open Preferences → Commands and check the Skipped Files and Command Warnings sections.
3. **Script producing unexpected output?** Use the **Test** button in the preferences script editor to see raw stdout/stderr/exit code.
4. **Need more detail?** Type `/debug` to turn on logging, reproduce the issue, then `/debug log` to inspect.
5. **Sharing a bug report?** Use `/debug log open` to open the log file, then attach it.
