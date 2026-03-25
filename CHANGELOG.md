# Changelog

All notable changes to Nimble are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.2.0] — 2026-03-25

### Added
- **Scrollable results list** — when matching commands or list items exceed the visible area (8 rows), a thin scrollbar appears and the window no longer clips results
- **Keyboard scroll-into-view** — arrow-key navigation auto-scrolls the selected row into the visible area
- **`/docs` built-in command** — five doc topics (`skill`, `commands`, `scripts`, `actions`, `contexts`) open their GitHub documentation page in the default browser
- **Spec versioning** — `nimble-spec.yaml` now carries an independent integer `spec_version` bumped on every schema or API change
- **Copilot authoring skill** — unified `nimble-authoring` skill replaces the previous two-agent setup for command YAML and script writing

### Changed
- **Copilot agents → skill** — replaced `@nimble-command` and `@nimble-script` agents plus `nimble-conventions.md` with a single `nimble-authoring` SKILL.md; spec co-located in `.github/skills/nimble-authoring/`
- **Docs renamed** — `copilot-agents.md` → `copilot-skill.md`, `deploying-agents.md` → `deploying-skill.md`; all internal cross-references updated
- **Sync workflow updated** — `.github/agents/` exclusion removed from `sync-public.yml` (directory no longer exists); spec and skill now sync to the public repo

### Fixed
- **Homebrew install instructions** — updated tap step to use Cask instead of Formula
- **macOS Gatekeeper workaround** — added `xattr -cr` instructions to getting-started docs

## [0.1.0] — 2026-03-22

Initial public release.
