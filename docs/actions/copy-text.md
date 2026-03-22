# Copy Text

Copies a predefined block of text to the clipboard. The launcher dismisses immediately — no paste occurs. You paste the text yourself wherever you need it.

---

## How it differs from Paste Text

| | `paste_text` | `copy_text` |
|---|---|---|
| Writes to clipboard | ✅ | ✅ |
| Simulates ⌘V / Ctrl+V | ✅ | ❌ |
| Requires Accessibility permission | ✅ | ❌ |
| You paste manually | ❌ | ✅ |

Use `copy_text` when you want the text on the clipboard but prefer to control where it goes, or when the target app blocks simulated keystrokes.

---

## Example

```yaml
phrase: copy email
title: Copy email address
action:
  type: copy_text
  config:
    text: hello@example.com
```
