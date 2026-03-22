# Getting Started

When you launch Nimble for the first time you will see the onboarding screen. This only happens once — every subsequent launch goes straight to the background.

---

## Choosing your global shortcut

The onboarding screen asks you to press the key combination you want to use to open the launcher from anywhere on your Mac.

1. Click anywhere on the onboarding window to make sure it has focus
2. Press your desired key combination — for example `⌘Space`, `⌃⌥Space`, or `⌘⇧/`
3. The combination you pressed appears in the preview area
4. If you want a different shortcut, press another combination — it replaces the previous one
5. Click **Confirm shortcut** to save it

**Requirements for a valid shortcut:**
- Must include at least one modifier: `⌘` (Cmd), `⌃` (Ctrl), `⌥` (Option), or `⇧` (Shift)
- Plus at least one letter, number, or special key (`Space`, `Return`, arrow keys, etc.)
- Bare modifier keys on their own are not valid

If you choose a combination already claimed by another application, an error is shown — try a different one.

---

## After onboarding

Once you confirm your shortcut:

- The launcher hides itself and continues running silently in the background
- No Dock icon is shown
- Your shortcut is saved to local storage and automatically re-registered every time Nimble starts

Press your shortcut at any time to summon the command bar. Press it again — or press `Escape`, or click anywhere outside the window — to dismiss it.

---

## Changing your shortcut later

Your shortcut is stored in `settings.yaml` inside the [config directory](reference/config-directory.md). To change it, either:

1. **Edit the file directly** — open `settings.yaml`, change the `hotkey:` value, then restart Nimble.
2. **Re-trigger onboarding** — delete the `hotkey:` line from `settings.yaml` (or the entire file), then restart Nimble. The onboarding screen will appear again so you can choose a new combination.

> A proper "change shortcut" UI will be added in a future release.

---

## Supported platforms

- **macOS** — primary target, fully supported
- **Linux** — supported (Flatpak distribution; some Wayland limitations)
- **Windows** — supported (MSI installer)

---

## Your first command

After the shortcut is set the launcher is ready to use. Out of the box, Nimble ships with a few example commands in your `commands/examples/` folder. Try invoking the launcher and typing `open` — you should see them appear.

To create your own command, open your `commands/` directory:

```
~/Library/Application Support/Nimble/commands/
```

Create a file — for example `open-github.yaml` — with this content:

```yaml
phrase: open github
title: Open GitHub
action:
  type: open_url
  config:
    url: https://github.com
```

Save the file. Within 300 ms Nimble reloads automatically. Invoke the launcher and start typing `open github` — the result appears as you type. Press `Enter` or click it to open the URL in your browser.

That one file is all it takes. Every other action type (`paste_text`, `copy_text`, `static_list`, `dynamic_list`, `script_action`) follows the same pattern — just a different `action` block. See [Actions](actions/README.md) to continue.
