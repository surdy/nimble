# Development Environment Setup

This guide walks you through setting up your machine to build and run Nimble locally.

---

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| [Node.js](https://nodejs.org/) | v18+ | Frontend tooling & package manager |
| [Rust](https://rustup.rs/) (via rustup) | stable | Tauri native backend |
| Xcode Command Line Tools | latest | macOS native build tools (C compiler, linker) |
### Linux additional system packages

On Linux you also need the Tauri WebKit dependencies and `libxdo-dev` (required at compile time for the `xdo` crate used by the paste-text focus-restoration feature):

```bash
# Ubuntu / Debian
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev

# Fedora
sudo dnf install webkit2gtk4.1-devel gtk3-devel libayatana-appindicator-gtk3-devel librsvg2-devel libxdo-devel

# Arch
sudo pacman -S webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg xdotool libxdo
```

> **Wayland note:** `libxdo` requires X11. Under a pure Wayland session (no XWayland), the paste-text action still copies text to the clipboard but focus is not automatically restored to the previous application — you will need to click the target window before pressing Ctrl+V.
---

## Step 1 — Xcode Command Line Tools (macOS)

Check if already installed:

```bash
xcode-select -p
```

If missing, install them:

```bash
xcode-select --install
```

---

## Step 2 — Node.js

Check if already installed:

```bash
node --version
npm --version
```

If missing, download and install from [nodejs.org](https://nodejs.org/) or use a version manager like [nvm](https://github.com/nvm-sh/nvm):

```bash
# Using nvm
nvm install --lts
nvm use --lts
```

---

## Step 3 — Rust (via rustup)

Check if already installed:

```bash
rustc --version
cargo --version
```

If missing, install via rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Accept the default installation (option 1). Once complete, source the environment so Rust is available in the current shell session:

```bash
. "$HOME/.cargo/env"
```

To make this permanent, add the above line to your shell profile (`~/.zshrc`, `~/.bashrc`, etc.), or restart your terminal — the rustup installer does this automatically.

Verify:

```bash
rustc --version   # e.g. rustc 1.94.0
cargo --version   # e.g. cargo 1.94.0
```

---

## Step 4 — Clone the repository

```bash
git clone <repository-url>
cd nimble
```

---

## Step 5 — Install frontend dependencies

```bash
npm install
```

---

## Step 6 — Run in development mode

```bash
npm run tauri dev
```

This will:
1. Start the Vite dev server for the SvelteKit frontend
2. Compile the Rust/Tauri backend
3. Open the launcher window with hot-reloading enabled

> **Note:** The first run will take a few minutes as Cargo compiles all Rust dependencies. Subsequent runs are much faster.

---

## Step 7 — Build for production

```bash
npm run tauri build
```

The compiled application bundle will be output to `src-tauri/target/release/bundle/`.

---

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) with the following extensions:

- [Svelte for VS Code](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) — Svelte language support
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) — Tauri commands and snippets
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) — Rust language server

These are also listed in `.vscode/extensions.json` so VS Code will prompt you to install them automatically when you open the project.

---

## Platform build targets

Use `npm run tauri build` to produce a release bundle for the current platform. For a specific artefact type pass `--bundles <target>`:

| Platform | Command | Output artefact | Extra pre-requisites |
|----------|---------|-----------------|----------------------|
| macOS | `npm run tauri build -- --bundles dmg` | `src-tauri/target/release/bundle/dmg/*.dmg` | Xcode CLT |
| Linux | `npm run tauri build -- --bundles flatpak` | `src-tauri/target/release/bundle/flatpak/*.flatpak` | `flatpak`, `flatpak-builder`, GNOME SDK 45 (see below) |
| Windows | `npm run tauri build -- --bundles msi` | `src-tauri/target/release/bundle/msi/*.msi` | WiX Toolset (installed automatically by Tauri CLI) |

### Linux: Flatpak pre-requisites

```bash
sudo apt-get install flatpak flatpak-builder
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive flathub org.gnome.Sdk//45 org.gnome.Platform//45
```

---

## Troubleshooting

**`cargo` or `rustc` not found after install**
Run `. "$HOME/.cargo/env"` or restart your terminal to reload `PATH`.

**`npm run tauri dev` fails on first run**
Ensure Xcode Command Line Tools are installed (`xcode-select -p`). A missing C linker is the most common cause on macOS.

**`permission denied` when running the installer script**
Do not run the rustup installer with `sudo`. It installs to your home directory and does not require elevated permissions.
