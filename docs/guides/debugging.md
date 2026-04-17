# Debugging

Nimble surfaces errors directly in the launcher so you can see what went wrong without leaving the keyboard. For deeper diagnostics, debug mode adds a detailed session log.

---

## Error reporting

When a command fails — a missing list file, a script that exits non-zero, a permission error — Nimble shows the error inline as a ⚠️ item in the results list. The error message is the actual message from the backend, not a generic placeholder.

| Situation | What you see |
|-----------|-------------|
| No command matches what you typed | **No matching commands** |
| A list/script command ran successfully but returned zero items | **No results** |
| A `static_list` file could not be loaded | ⚠️ **Error loading list** — _error details_ |
| A `dynamic_list` script failed | ⚠️ **Script error** — _error details_ |
| A `script_action` script failed | ⚠️ **Action error** — _error details_ |

Error items are interactive — press **Enter** or click to copy the error message to your clipboard.

Errors are always visible. You do not need debug mode to see them.

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

1. **Something broke?** Read the ⚠️ error message in the launcher — it tells you what went wrong.
2. **Need more detail?** Type `/debug` to turn on logging, reproduce the issue, then `/debug log` to inspect.
3. **Sharing a bug report?** Use `/debug log open` to open the log file, then attach it.
