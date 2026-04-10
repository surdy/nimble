# Environment Variables

Nimble injects environment variables into every script it runs (`dynamic_list` and `script_action`). There are two categories: **built-in** variables that Nimble sets automatically, and **user-defined** variables you configure yourself.

---

## Built-in `NIMBLE_*` variables

These are injected into every script subprocess. They always take precedence over user-defined variables — you cannot override them.

| Variable | Description | Example |
|----------|-------------|---------|
| `NIMBLE_CONTEXT` | Active context string (empty if none set) | `reddit` |
| `NIMBLE_PHRASE` | Command phrase that triggered this script | `search contacts` |
| `NIMBLE_CONFIG_DIR` | Absolute path to the Nimble config root | `/Users/you/Library/Application Support/nimble` |
| `NIMBLE_COMMANDS_ROOT` | Absolute path to the commands root directory | `/Users/you/Library/Application Support/nimble/commands` |
| `NIMBLE_COMMAND_DIR` | Absolute path to the directory containing the command YAML | `/Users/you/Library/Application Support/nimble/commands/search-contacts` |
| `NIMBLE_OS` | Operating system: `macos`, `linux`, or `windows` | `macos` |
| `NIMBLE_VERSION` | Nimble app version string | `0.6.0` |
| `NIMBLE_DEBUG` | Set to `1` when debug mode is active (via `/debug`); absent otherwise | `1` |

### Notes

- `NIMBLE_CONTEXT` is an empty string (not unset) when no context is active.
- `NIMBLE_PHRASE` contains the full phrase, not the user's partial input.
- `NIMBLE_CONFIG_DIR`, `NIMBLE_COMMANDS_ROOT`, and `NIMBLE_COMMAND_DIR` are absolute paths and always set, even if invoked from a global `env.yaml`.
- `NIMBLE_OS` is one of exactly three values: `macos`, `linux`, `windows`.
- `NIMBLE_DEBUG` is only set when debug mode is on (toggled via `/debug`). Scripts can check for it to emit extra diagnostic output.

### `${VAR}` substitution in YAML fields

The `script:` and `list:` fields in command YAML support `${VAR}` tokens for path resolution. All built-in variables (plus user-defined ones) are available:

```yaml
action:
  type: dynamic_list
  config:
    script: ${SHARED_SCRIPTS}/shared-lookup.sh
```

A plain filename (no `${…}`) always resolves relative to the command's own directory.

---

## User-defined variables

You can define your own variables via `env.yaml` files or inline `env:` blocks. These are passed as standard environment variables to scripts.

### Precedence layers

Variables are merged in order — later layers override earlier ones:

| Layer | Location | Scope |
|-------|----------|-------|
| 1. Global | `env.yaml` at the commands root | All commands |
| 2. Sidecar | `env.yaml` in the command's directory | Commands in that directory |
| 3. Inline | `env:` block in the command YAML | That command only |

Built-in `NIMBLE_*` variables always win, regardless of layer.

### Key rules

- Must match `[A-Za-z_][A-Za-z0-9_]*`
- Must **not** start with `NIMBLE_` (reserved namespace; rejected at load time)
- Values are always strings — YAML auto-conversions (booleans, numbers) are coerced to strings

### Examples

**Global** (`~/Library/Application Support/nimble/commands/env.yaml`):

```yaml
WORK_EMAIL: alice@example.com
JIRA_BASE_URL: https://mycompany.atlassian.net
```

**Sidecar** (`commands/jira/env.yaml`):

```yaml
JIRA_PROJECT: MYPROJ
JIRA_BOARD_ID: "42"
```

**Inline** (in a command YAML):

```yaml
phrase: create bug
title: Create a bug report
env:
  TICKET_TYPE: bug
action:
  type: script_action
  config:
    script: create-ticket.sh
    arg: required
    result_action: open_url
```

> Sidecar `env.yaml` applies only to commands in the exact same directory. There is no directory walking — a parent's `env.yaml` does not apply to subdirectories.

---

## See also

- [Writing Scripts](../guides/writing-scripts.md) — full scripting guide with usage examples
- [Script Action](../actions/script-action.md) — `script_action` reference
- [Dynamic List](../actions/dynamic-list.md) — `dynamic_list` reference
- [Config Directory](config-directory.md) — directory layout and `env.yaml` placement
