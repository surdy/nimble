# External Context API

Nimble exposes the active **context** to other applications through two mechanisms:

1. **State file** (read) — a JSON file in the config directory that always reflects the current context.
2. **Deep links** (write) — `nimble://` URLs that set or reset the context from any app.

---

## Reading the context — state file

Whenever the context changes (via `/ctx set`, `/ctx reset`, the context chip, or a deep link), Nimble writes the current value to:

| Platform | Path |
|----------|------|
| macOS    | `~/Library/Application Support/Nimble/state.json` |
| Linux    | `~/.config/Nimble/state.json` |
| Windows  | `%APPDATA%\Nimble\state.json` |

The file contains a single JSON object:

```json
{
  "context": "reddit"
}
```

When no context is active the value is an empty string:

```json
{
  "context": ""
}
```

### Shell example

```bash
# Read the current context (macOS)
cat ~/Library/Application\ Support/Nimble/state.json | python3 -c "import sys,json; print(json.load(sys.stdin)['context'])"

# Or with jq
jq -r .context ~/Library/Application\ Support/Nimble/state.json
```

### Python example

```python
import json, pathlib

state = pathlib.Path.home() / "Library/Application Support/Nimble/state.json"
ctx = json.loads(state.read_text()).get("context", "")
print(f"Active context: {ctx!r}")
```

---

## Writing the context — deep links

Open a `nimble://` URL to set or reset the context. The app does not need to be in the foreground.

| Action | URL |
|--------|-----|
| Set context to `reddit` | `nimble://ctx/set/reddit` |
| Set context with spaces | `nimble://ctx/set/my%20project` |
| Reset (clear) context   | `nimble://ctx/reset` |

### Shell examples

```bash
# macOS
open "nimble://ctx/set/reddit"
open "nimble://ctx/reset"

# Linux (xdg)
xdg-open "nimble://ctx/set/reddit"

# Windows
start nimble://ctx/set/reddit
```

### Keyboard Maestro / Shortcuts.app

Create a "Run Shell Script" or "Open URL" action with `nimble://ctx/set/your-value`.

### Hammerspoon (macOS)

```lua
hs.urlevent.openURL("nimble://ctx/set/work")
```

---

## URL encoding

Values in the URL path are percent-decoded. Use `%20` or `+` for spaces. Standard percent-encoding rules apply for any special characters.

| Character | Encoding |
|-----------|----------|
| space     | `%20` or `+` |
| `/`       | `%2F` |
| `&`       | `%26` |

---

## Context persistence

The context now **survives app restarts**. On launch, Nimble reads `state.json` and restores the previous context automatically.

---

## Combining both mechanisms

The state file and deep links work together:

1. An external script calls `open nimble://ctx/set/project-x`.
2. Nimble receives the deep link, updates `state.json`, and reflects the new context in the UI.
3. Another tool reads `state.json` to check the current context.
4. Later, the user types `/ctx reset` in the launcher — `state.json` is updated to `""`.

This makes the context a lightweight, cross-app coordination signal.
