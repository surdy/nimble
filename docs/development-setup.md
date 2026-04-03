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

---

## macOS Code Signing & Notarization

Unsigned macOS builds trigger Gatekeeper warnings ("Apple cannot check it for malicious software"). To distribute Nimble without this, you need to **code-sign** and **notarize** the DMG with an Apple Developer certificate.

Tauri 2 handles the entire process automatically when these environment variables are set. No `tauri.conf.json` changes are needed.

### Prerequisites

- An [Apple Developer Program](https://developer.apple.com/programs/) membership ($99/year)
- macOS with Keychain Access (for certificate export)

### Step 1 — Create a Developer ID Application certificate

1. Sign in to [developer.apple.com/account](https://developer.apple.com/account).
2. Go to **Certificates, Identifiers & Profiles** → **Certificates**.
3. Click **+** to create a new certificate.
4. Select **Developer ID Application** and click Continue.
5. You need a Certificate Signing Request (CSR):
   - Open **Keychain Access** on your Mac.
   - Menu bar → **Keychain Access** → **Certificate Assistant** → **Request a Certificate From a Certificate Authority**.
   - Enter your email, leave CA Email blank, select **Saved to disk**, click Continue.
   - Save the `.certSigningRequest` file.
6. Upload the CSR file on the Apple Developer portal and click Continue.
7. Download the generated `.cer` file and double-click it to install into Keychain Access.

### Step 2 — Export the certificate as `.p12`

1. Open **Keychain Access**.
2. In the **login** keychain, find your certificate under **My Certificates** (look for "Developer ID Application: Your Name (TEAMID)").
3. Right-click the certificate → **Export** → choose **.p12** format.
4. Set a strong password when prompted — you will need this for `APPLE_CERTIFICATE_PASSWORD`.
5. Save the file (e.g. `DeveloperID.p12`).

### Step 3 — Base64-encode the `.p12` file

GitHub Secrets cannot store binary files, so encode it as base64:

```bash
base64 -i DeveloperID.p12 | pbcopy
```

This copies the encoded string to your clipboard. You will paste this as the `APPLE_CERTIFICATE` secret.

### Step 4 — Find your signing identity

The signing identity is the full common name of your certificate. To find it:

```bash
security find-identity -v -p codesigning
```

Look for the line containing "Developer ID Application". Copy the full name, e.g.:

```
Developer ID Application: Harpreet Singh Gulati (ABC123XYZ0)
```

This is your `APPLE_SIGNING_IDENTITY` value.

### Step 5 — Find your Team ID

1. Go to [developer.apple.com/account](https://developer.apple.com/account) → **Membership details**.
2. Your **Team ID** is the 10-character alphanumeric string shown there (e.g. `ABC123XYZ0`).

This is your `APPLE_TEAM_ID` value.

### Step 6 — Generate an app-specific password

Apple's notarization service requires an app-specific password (not your account password):

1. Go to [account.apple.com](https://account.apple.com).
2. Navigate to **Sign-In and Security** → **App-Specific Passwords**.
3. Click **Generate an app-specific password**.
4. Give it a label (e.g. "Nimble CI Notarization").
5. Copy the generated password.

This is your `APPLE_PASSWORD` value. Your Apple ID email is `APPLE_ID`.

### Step 7 — Add GitHub Secrets

Go to **your repo** → **Settings** → **Secrets and variables** → **Actions** → **New repository secret** and add:

| Secret name | Value | Where it comes from |
|-------------|-------|---------------------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` content | Step 3 |
| `APPLE_CERTIFICATE_PASSWORD` | Password you set during `.p12` export | Step 2 |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Name (TEAMID)` | Step 4 |
| `APPLE_ID` | Your Apple ID email address | Your Apple account |
| `APPLE_PASSWORD` | App-specific password | Step 6 |
| `APPLE_TEAM_ID` | 10-character team ID | Step 5 |

### How Tauri uses these

When `APPLE_CERTIFICATE` is set, Tauri's build process automatically:

1. Creates a temporary keychain on the CI runner.
2. Imports the `.p12` certificate into it.
3. Signs the app binary with `codesign` using `APPLE_SIGNING_IDENTITY`.
4. Submits the DMG to Apple's notarization service via `notarytool` using `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`.
5. Waits for notarization to complete, then staples the ticket to the DMG.

No manual `codesign` or `notarytool` commands are needed — Tauri handles everything.

### Verifying a signed build

After building, verify the signature locally:

```bash
# Check code signature
codesign --verify --verbose=4 src-tauri/target/release/bundle/macos/Nimble.app

# Check notarization
spctl --assess --type exec --verbose=4 src-tauri/target/release/bundle/macos/Nimble.app

# Check the DMG
spctl --assess --type open --context context:primary-signature --verbose=4 path/to/Nimble.dmg
```

### Troubleshooting

**"The identity cannot be found"**
The `APPLE_SIGNING_IDENTITY` value does not match any certificate in the keychain. Run `security find-identity -v -p codesigning` to get the exact string.

**Notarization fails with "hardened runtime" error**
The app may need explicit entitlements. Create `src-tauri/Entitlements.plist` and reference it in `tauri.conf.json` under `bundle.macOS.entitlements`. See the entitlements section below.

**Notarization times out**
Apple's service can take 5–15 minutes. If CI times out, increase the job timeout. Tauri's `notarytool submit --wait` polls until complete.

**"App is damaged and can't be opened"**
This usually means the app was modified after signing (e.g. by a zip/unzip round-trip that strips extended attributes). Use DMG distribution to avoid this.

### Entitlements

Nimble uses `macOSPrivateApi: true` for CGEvent keystroke simulation (paste_text). If notarization rejects the build, create an entitlements file:

**`src-tauri/Entitlements.plist`:**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <false/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.automation.apple-events</key>
    <true/>
</dict>
</plist>
```

Reference it in `tauri.conf.json`:

```json
"bundle": {
  "macOS": {
    "entitlements": "Entitlements.plist"
  }
}
```
