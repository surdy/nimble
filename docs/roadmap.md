# Nimble — Roadmap

---

## Implemented ✅

- [x] Frameless launcher window — always-on-top, dismisses on Escape / focus loss
- [x] Global hotkey — user-chosen shortcut, registered at startup, persisted to `settings.yaml`
- [x] Onboarding screen — first-run shortcut capture
- [x] Command loading from YAML files in a platform config directory
- [x] Recursive command discovery — organise files into any subdir structure
- [x] Live config reload — edits to command files take effect within ~300 ms, no restart
- [x] Partial / substring matching with real-time filtering (up to 8 results)
- [x] Result highlighting — matching portion of phrase shown bold/blue
- [x] Keyboard navigation — Up/Down to move, Enter to execute, Escape to dismiss
- [x] Dynamic window resize — window grows/shrinks to fit the current result count
- [x] Action: `open_url` — open any URL; optional `{param}` substitution from typed suffix
- [x] Action: `paste_text` — paste a predefined string into the previously focused app
- [x] Action: `copy_text` — copy a predefined string to clipboard without pasting
- [x] Action: `static_list` — keyword-triggered inline list from a `lists/` YAML file; items can paste, copy, or open a URL
- [x] Action: `dynamic_list` — script-backed list with `none` / `optional` / `required` argument modes
- [x] Action: `script_action` — run a script and pipe its output into `open_url`, `paste_text`, or `copy_text`; supports prefix/suffix wrapping
- [x] Contexts — `/ctx set` and `/ctx reset` built-in commands; context is ambient-only, delivered to scripts via `NIMBLE_CONTEXT` and never appended to typed input or matching
- [x] Context chip — pill badge in the launcher bar showing the active context with a one-click clear button
- [x] `arg: context` mode — `script_action` and `dynamic_list` argument mode that is "required, but an active context satisfies it"; a typed suffix always overrides and is passed as `$1`, the context is never passed positionally
- [x] `item_action: ctx_set` — selecting a `static_list`/`dynamic_list` item sets the active context and clears the input without dismissing the launcher (same as `/ctx set`)
- [x] `{context}` URL token — `open_url` templates can substitute the active context value, independent of `{param}`
- [x] Duplicate-command warnings — banner shown when two files define the same phrase (`allow_duplicates: false`)
- [x] System tray icon — persistent tray presence with show/hide and quit options
- [x] `settings.yaml` — human-editable file for hotkey, context chip visibility, and dedup behaviour
- [x] Example config — `example-config/` directory in the repo covering every action type
- [x] Rust test suite — 56 unit tests covering YAML parsing, dedup, URL validation, script sandboxing
- [x] External context API — `state.json` for reading the active context + `nimble://` deep links for setting/resetting context from other apps
- [x] Context persistence — active context survives app restarts via `state.json`

---

## Planned

### Distribution & Updates
- [x] Add license — MIT license added
- [x] GitHub Actions CI — automated builds for macOS, Windows, and Linux on every push
- [ ] Homebrew tap — `brew install nimble` via a dedicated tap repository
- [ ] Update notifications — show an indicator when a newer version is available
- [ ] Beta channel — opt-in channel for pre-release builds
- [ ] `ctx update` command — trigger an in-app update from the launcher itself
- [ ] Release notes viewer — `ctx release notes` shows all release notes between your current version and the latest, scrollable inline

### Theming
- [ ] System theme following — automatic light/dark mode that matches the OS appearance
- [ ] Advanced custom theming — user-editable theme file (colours, fonts, border radius, etc.)

### Configuration & Usability
- [x] Configuration UI — Preferences window with Commands and Settings tabs, table view with sortable columns, bulk-select, group-by, inline script editor, and settings sidebar navigation
- [ ] Bug / issue reporter — `ctx report issue` opens a pre-filled GitHub issue in the browser with version and platform info attached
- [ ] Global variables — built-in variables (e.g. `{{date}}`, `{{clipboard}}`) and user-defined variables reusable across any command's URL, text, or script arguments
- [x] Built-in script environment variables — inject `NIMBLE_*` variables (`NIMBLE_CONTEXT`, `NIMBLE_PHRASE`, `NIMBLE_CONFIG_DIR`, `NIMBLE_COMMANDS_ROOT`, `NIMBLE_COMMAND_DIR`, `NIMBLE_OS`, `NIMBLE_VERSION`) into every script execution
- [x] User-defined script variables — global `env.yaml`, command-dir sidecar `env.yaml`, and inline `env:` with deterministic precedence; reserved `NIMBLE_*` keys protected
- [x] Spec versioning — independent integer version for `nimble-spec.yaml`, bumped on every schema/API change
- [x] Shared scripts/lists — `shared:` prefix for referencing scripts and lists in a central shared directory; configurable `shared_dir` setting (replaces `${VAR}` substitution and `allow_external_paths`)
- [x] Custom commands directory — optional `commands_dir` setting in `settings.yaml` to load commands from an absolute path instead of the default `commands/` subdirectory
- [x] Client-side fuzzy filtering — static and dynamic list results are filtered in real time as the user types, without re-invoking scripts
- [x] Bundled Copilot skill — nimble-authoring skill files embedded in the binary, auto-installed to config dir, deployable via `/deploy copilot skill` command
- [x] Script debugging & verbose logs — `/debug` toggle enables session debug mode: logs all actions, script invocations, and errors to `debug.log`; injects `NIMBLE_DEBUG=1` into scripts; surfaces errors inline; view log via `/debug log`
- [ ] Profiles — named configuration profiles (e.g. `work`, `home`) each with their own command set, scripts, and settings; switch profiles from the launcher or on a schedule

### Community
- [ ] Contributing guide — CONTRIBUTING.md covering how to set up a dev environment, the branching model, and how to submit a pull request
