# Glossary

Quick definitions for terms used throughout the Nimble documentation.

---

| Term | Definition |
|------|-----------|
| **Action** | The operation a command performs when executed. One of: `open_url`, `paste_text`, `copy_text`, `static_list`, `dynamic_list`, or `script_action`. |
| **Phrase** | The multi-word string you type to trigger a command (e.g. `open github`, `search google`). Matching is case-insensitive and works on partial input. |
| **Subtext** | The secondary line shown below a result's title. In lists, subtext also serves as the value used for the item action (paste, copy, or open). |
| **Param / `{param}`** | The text the user types after a matched phrase. Substituted into `open_url` URLs or passed as an argument to scripts. Nimble URL-encodes the value automatically. |
| **Context** | A persistent word or phrase silently appended to every query, narrowing matches to a topic. Set with `/ctx set <value>`, cleared with `/ctx reset`. |
| **Config directory** | The platform-specific folder where Nimble reads commands, lists, scripts, and settings (`~/Library/Application Support/nimble/` on macOS). |
| **Commands directory** | The `commands/` subfolder inside the config directory. Each `.yaml` file here defines one command. |
| **Co-located file** | A script or list file that lives in the same directory as its command YAML (e.g. `commands/say-hello/hello.sh` alongside `say-hello.yaml`). |
| **Shared file** | A script or list stored in the top-level `scripts/` or `lists/` directory and referenced with the `shared:` prefix in YAML. |
| **Sidecar `env.yaml`** | An `env.yaml` file placed next to a command YAML, providing environment variables scoped to that command's scripts. |
| **Item action** | The built-in operation performed when a user selects an item from a static or dynamic list (`paste_text`, `copy_text`, or `open_url`). |
| **Partial match** | When the user's typed input is a substring of a command's phrase, causing that command to appear in the results list. |
| **Live reload** | Nimble watches the commands directory and reloads commands automatically whenever a file is added, changed, or removed — no restart needed. |
| **Debug mode** | A session-scoped logging mode toggled with `/debug`. Records actions, script runs, and errors to `debug.log`. Resets on restart. |
| **Built-in command** | A command provided by Nimble itself (prefixed with `/`), such as `/debug`, `/ctx set`, and `/ctx reset`. User commands cannot start with `/`. |
