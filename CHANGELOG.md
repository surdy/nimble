# Changelog

All notable changes to Nimble are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

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
