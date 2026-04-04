# Changelog

All notable changes to Nimble are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.8.0] — 2026-04-04

### Added
- **Custom commands directory** — new optional `commands_dir` setting in `settings.yaml` lets you load commands from any absolute path instead of the default `commands/` subdirectory; useful for dotfiles sync and team-shared command folders

## [0.7.0] — 2026-04-03

### Changed
- **Simplified config directory path** — changed Tauri identifier from `io.switchpanel.nimble` to `nimble`; config directory is now `~/Library/Application Support/nimble/` (macOS), `~/.config/nimble/` (Linux), `%APPDATA%\nimble\` (Windows)

### Added
- **Environment variables reference** — new `docs/reference/environment-variables.md` page documenting all six built-in `NIMBLE_*` variables, `${VAR}` substitution, and user-defined env precedence

## [0.6.0] — 2026-04-02

### Added
- **External context API** — active context is now exposed via `state.json` (read) and `nimble://` deep links (write); other apps can query or set the context without interacting with the launcher window
- **Context persistence** — active context survives app restarts; backed by `state.json` in the config directory (replaces localStorage)
- **Deep link support** — registered `nimble://` URL scheme via `tauri-plugin-deep-link`; supports `nimble://ctx/set/<value>` and `nimble://ctx/reset`
- **macOS code signing & notarization** — CI now signs and notarizes the DMG using Apple Developer certificates; entitlements plist added for hardened runtime

### Changed
- **App icon refresh** — adopted Feathered Prompt as the canonical app icon; regenerated all platform icon assets

## [0.5.1] — 2026-03-26

### Fixed
- **Accessibility permission prompt on macOS** — the app now calls `AXIsProcessTrustedWithOptions` on startup to trigger the system Accessibility permission dialog if not already granted; previously CGEvent paste simulation silently failed after brew upgrades changed the code signature
- **Release tag push** — updated release prompt to push tags explicitly instead of relying on `--follow-tags` which silently skips lightweight tags

## [0.5.0] — 2026-03-26

### Fixed
- **Paste action now targets correct window on macOS** — replaced enigo with `core-graphics` CGEvent API for Cmd+V simulation; events post at HID level which reliably reaches the previously focused app. Timing adjusted to 100ms focus restore + 30ms clipboard settle.
- **Overlay scrollbar now visible in production builds** — added `color-scheme: dark` to `<html>` so macOS WKWebView overlay scrollbars render a light thumb against the dark launcher background; previously only worked in dev mode.

## [0.4.0] — 2026-03-25

### Changed
- **Context no longer appended to parameters** — when the user types a command phrase plus trailing text (a parameter), the active context is no longer appended to the effective input; parameters now contain only what the user explicitly typed. Scripts can still read the context via `NIMBLE_CONTEXT` env var.

## [0.3.0] — 2026-03-25

### Added
- **Empty dynamic list feedback** — when a `dynamic_list` script returns zero items, the launcher now displays a "No results" row instead of silently collapsing; a loading guard prevents the message from flashing while the script is still running

### Changed
- **Launcher border visibility** — switched to a 1.5px solid white border (45% opacity) and increased background opacity to 0.92 for better contrast on dark desktops

## [0.2.1] — 2026-03-25

### Changed
- **Longest phrase wins → sort, not filter** — overlapping commands now both appear in results, with the longer phrase sorted first as the default Enter target; shorter-phrase commands remain accessible via arrow keys

### Fixed
- **Scrollbar visibility** — increased scrollbar thumb opacity from 20% to 45% so it is visible against the dark background

## [0.2.0] — 2026-03-25

### Added
- **Longest phrase wins** — when two commands overlap in param mode and one phrase is a prefix of the other, the longer phrase takes priority and the shorter one is hidden
- **Scrollable results list** — when matching commands or list items exceed the visible area (8 rows), a thin scrollbar appears and the window no longer clips results
- **Keyboard scroll-into-view** — arrow-key navigation auto-scrolls the selected row into the visible area
- **`/docs` built-in command** — five doc topics (`skill`, `commands`, `scripts`, `actions`, `contexts`) open their GitHub documentation page in the default browser
- **Spec versioning** — `nimble-spec.yaml` now carries an independent integer `spec_version` bumped on every schema or API change
- **Copilot authoring skill** — unified `nimble-authoring` skill replaces the previous two-agent setup for command YAML and script writing

### Changed
- **Release notes from CHANGELOG** — GitHub Releases now extract notes from CHANGELOG.md instead of auto-generating commit diffs
- **Copilot agents → skill** — replaced `@nimble-command` and `@nimble-script` agents plus `nimble-conventions.md` with a single `nimble-authoring` SKILL.md; spec co-located in `.github/skills/nimble-authoring/`
- **Docs renamed** — `copilot-agents.md` → `copilot-skill.md`, `deploying-agents.md` → `deploying-skill.md`; all internal cross-references updated
- **Sync workflow updated** — `.github/agents/` exclusion removed from `sync-public.yml` (directory no longer exists); spec and skill now sync to the public repo

### Fixed
- **Homebrew install instructions** — updated tap step to use Cask instead of Formula
- **macOS Gatekeeper workaround** — added `xattr -cr` instructions to getting-started docs

## [0.1.0] — 2026-03-22

Initial public release.
