<p align="center">
  <img src=".github/banner.svg" alt="Nimble — A fast, command-driven desktop launcher" width="100%" />
</p>

<p align="center">
  <em>A fast, cross-platform desktop launcher driven entirely by commands — think Alfred or Raycast, built around multi-word phrase commands with real-time partial matching and extensibility through sandboxed scripts.</em>
</p>

---

## How it works

You define commands by dropping YAML files into a config directory on your machine. Each file maps a **phrase** (the words you type) to an **action** (open a URL, paste text, run a script, etc.). Nimble watches that directory and hot-reloads commands the moment you save a file — no restart needed.

When you invoke the launcher and start typing, Nimble shows partial matches in real time. As you type more of the phrase the results narrow down; when you hit the full phrase of a list command the list expands automatically. Press `Enter` or click to execute.

---

## Tech Stack

- **[Tauri 2](https://tauri.app/)** — Rust-powered native shell (tiny binary, fast startup, strong sandboxing)
- **[SvelteKit](https://kit.svelte.dev/) + TypeScript** — lightweight, reactive frontend UI
- **[Vite](https://vitejs.dev/)** — frontend build tooling

---

## Features

### Built-in actions
- **Open URL** — navigate to any URL in your default browser; supports an optional `{param}` placeholder for user-supplied input
- **Paste Text** — paste pre-defined text snippets into the previously focused application via `⌘V` simulation
- **Copy Text** — copy pre-defined text to the clipboard without simulating a keystroke
- **Static List** — display a named YAML list of items inline as soon as the phrase is typed; selecting an item can paste, copy, or open a URL
- **Dynamic List** — run an external script and show its output as an inline list; supports `none`, `optional`, and `required` argument modes for real-time filtering

---

## Installation

### macOS

1. Download the latest `.dmg` from the [Releases page](https://github.com/surdy/nimble/releases)
2. Open the `.dmg`, drag **Nimble.app** into **Applications**, then eject the disk image

**Gatekeeper warning** — because Nimble is not yet notarized by Apple, macOS will block the first launch. To allow it:

1. Open **System Settings → Privacy & Security**
2. Scroll to the **Security** section and click **Open Anyway**
3. Confirm by clicking **Open** in the dialog

> **Alternative:** Right-click **Nimble.app** in Finder and choose **Open** — the dialog will include an **Open** button even for unsigned apps.

**Accessibility permission** — required for the Paste Text action to simulate `⌘V`:

1. Open **System Settings → Privacy & Security → Accessibility**
2. Click **+** and add **Nimble.app**, then enable its toggle

Without this, paste commands write to the clipboard but skip the keystroke simulation.

### Linux

1. Download the latest `.flatpak` from the [Releases page](https://github.com/surdy/nimble/releases)
2. Install it:
   ```bash
   flatpak install --user nimble.flatpak
   ```
3. Run it:
   ```bash
   flatpak run Nimble
   ```

> **Runtime dependency:** `xdotool` must be available inside the Flatpak sandbox for the Paste Text action to restore focus on X11. The distributed Flatpak includes this.

> **Wayland note:** Focus restoration is not available under a pure Wayland session (no XWayland). The paste action still writes to the clipboard, but you must click the target window before pressing Ctrl+V.

### Windows

1. Download the latest `.msi` from the [Releases page](https://github.com/surdy/nimble/releases)
2. Run the installer
3. Launch **Nimble** from the Start Menu or system tray

**SmartScreen warning** — because the installer is not yet code-signed, Windows SmartScreen may show a warning on first run. Click **More info → Run anyway** to proceed.

---

## Using Nimble

| Guide | Description |
|-------|-------------|
| [Getting started](docs/getting-started.md) | Onboarding walkthrough, choosing your shortcut, creating your first command |
| [Actions](docs/actions/README.md) | All six action types: Open URL, Paste Text, Copy Text, Static List, Dynamic List, Script Action |
| [Guides](docs/guides/README.md) | Configuring commands, writing scripts, contexts, Copilot agents |
| [Reference](docs/reference/README.md) | Config directory structure, duplicate commands, tips & tricks |

---

## Your first command

Once Nimble is running and your shortcut is set, open the commands directory and create a file:

```
~/Library/Application Support/Nimble/commands/open-github.yaml
```

```yaml
phrase: open github
title: Open GitHub
action:
  type: open_url
  config:
    url: https://github.com
```

Invoke the launcher with your shortcut, start typing `open github`, and the result appears instantly. Press `Enter` to open the URL. That's it — every other action type follows the same pattern. See [Actions](docs/actions/README.md) for next steps.

---

## Building from source

For a full environment setup guide see [docs/development-setup.md](docs/development-setup.md).

For the implementation roadmap see [docs/roadmap.md](docs/roadmap.md).

### Quick start

```bash
npm install
npm run tauri dev
```

### Platform build targets

| Platform | Prerequisites | Artefact | Command |
|----------|--------------|----------|---------|
| macOS | Xcode CLT, Rust, Node 18+ | `.dmg` | `npm run tauri build -- --bundles dmg` |
| Linux | Tauri WebKit deps, `xdotool`, `flatpak-builder`, GNOME SDK 45 | `.flatpak` | `npm run tauri build -- --bundles flatpak` |
| Windows | Rust, Node 18+, WiX Toolset | `.msi` | `npm run tauri build -- --bundles msi` |

See [docs/development-setup.md](docs/development-setup.md) for the full per-platform dependency list.

### Build (all targets for current platform)

```bash
npm run tauri build
```

---

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) with:
- [Svelte for VS Code](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode)
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

---

## Project Structure

```
bootstrap/     — Initial planning notes and project brief
docs/          — User and developer documentation
src/           — SvelteKit frontend source
src-tauri/     — Rust/Tauri backend source
static/        — Static frontend assets
```

---

## License

MIT — see [LICENSE](LICENSE).
