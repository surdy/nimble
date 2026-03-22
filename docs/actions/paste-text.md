# Paste Text

Pastes a predefined block of text into whichever application had focus before you opened the launcher.

---

## How it works

1. You invoke the launcher and select a `paste_text` command
2. Nimble hides the launcher window and restores focus to your previous app
3. After a brief delay (80 ms), Nimble writes your configured text to the clipboard and simulates `⌘V`
4. The text appears in your previously focused app as if you had pasted it yourself

---

## Example

```yaml
phrase: paste email signature
title: Paste email signature
action:
  type: paste_text
  config:
    text: |
      Best regards,
      Jane Smith
      jane@example.com | +1 555 0100
```

---

## macOS Accessibility permission

Nimble uses macOS Accessibility APIs to simulate the paste keystroke. The first time you run a `paste_text` command, macOS will prompt you to grant Accessibility access in **System Settings → Privacy & Security → Accessibility**. Without this permission the keystroke simulation is blocked and the text will not be pasted.
