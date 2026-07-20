# Example Config Directory

This directory mirrors the layout of the Nimble config directory on disk. Copy its contents into your own config directory to get started with a full set of working examples.

## Quick install

**macOS:**
```bash
cp -r example-config/* ~/Library/Application\ Support/nimble/
```

**Linux:**
```bash
cp -r example-config/* ~/.config/nimble/
```

**Windows (PowerShell):**
```powershell
Copy-Item -Recurse example-config\* "$env:APPDATA\nimble\"
```

Nimble hot-reloads commands, so the examples are available immediately.

## Structure

```
example-config/
├── settings.yaml           # application settings (hotkey, chip, dedup, external paths)
└── commands/
    ├── env.yaml            # global user-defined environment variables for scripts
    └── examples/
        ├── open-google.yaml
        ├── open-github.yaml
        ├── open-reddit.yaml
        ├── open-slack.yaml
        ├── open-notes.yaml
        ├── search-google.yaml
        ├── paste-email.yaml
        ├── paste-greeting.yaml
        ├── copy-email.yaml
        ├── show-team-emails/         # static_list — command + list co-located
        │   ├── show-team-emails.yaml
        │   └── team-emails.tsv
        ├── say-hello/                # dynamic_list — command + script co-located
        │   ├── say-hello.yaml
        │   └── hello.sh
        ├── paste-timestamp/          # script_action — command + script co-located
        │   ├── paste-timestamp.yaml
        │   └── timestamp.sh
        ├── copy-uuid/
        │   ├── copy-uuid.yaml
        │   └── uuid.sh
        ├── open-morning-sites/
        │   ├── open-morning-sites.yaml
        │   └── morning-sites.sh
        ├── paste-team-emails/
        │   ├── paste-team-emails.yaml
        │   └── team-emails.sh
        ├── paste-team-emails-as-task/
        │   ├── paste-team-emails-as-task.yaml
        │   └── team-emails.sh
        ├── show-user-env/            # user-defined env demo (global + sidecar + inline)
        │   ├── show-user-env.yaml
        │   ├── env.yaml              # sidecar env vars for this command
        │   └── user-env.sh
        ├── list-envs/                # dynamic_list, arg: context — runs on a typed
        │   ├── list-envs.yaml        # suffix (override) or a bare phrase + active
        │   └── envs.sh               # context (reads NIMBLE_CONTEXT); neither → no run
        ├── pick-customer/            # static_list, item_action: ctx_set — context picker
        │   ├── pick-customer.yaml
        │   └── customers.tsv
        └── open-dashboard.yaml       # open_url with the {context} URL token
```

## Examples covered

| Phrase | Action type | What it does |
|---|---|---|
| `open google` | [`open_url`](docs/actions/open-url.md) | Opens Google in the browser |
| `open github` | [`open_url`](docs/actions/open-url.md) | Opens GitHub in the browser |
| `open reddit` | [`open_url`](docs/actions/open-url.md) | Opens Reddit in the browser |
| `open slack` | [`open_url`](docs/actions/open-url.md) | Opens Slack via deep link |
| `open notes` | [`open_url`](docs/actions/open-url.md) | Opens an Obsidian vault via deep link |
| `open morning sites` | [`script_action`](docs/actions/script-action.md) | Opens GitHub, HN, and Reddit simultaneously |
| `search google <query>` | [`open_url`](docs/actions/open-url.md) | Searches Google with a typed query |
| `paste email` | [`paste_text`](docs/actions/paste-text.md) | Pastes a static email address |
| `paste greeting` | [`paste_text`](docs/actions/paste-text.md) | Pastes a multi-line greeting template |
| `paste team emails` | [`script_action`](docs/actions/script-action.md) | Pastes all team emails, one per line |
| `paste team emails tasks` | [`script_action`](docs/actions/script-action.md) | Pastes emails as Markdown task list items |
| `copy email` | [`copy_text`](docs/actions/copy-text.md) | Copies a static email address to clipboard |
| `copy uuid` | [`script_action`](docs/actions/script-action.md) | Copies a fresh UUID to clipboard |
| `team emails` | [`static_list`](docs/actions/static-list.md) | Shows pickable list of team email addresses |
| `say hello` | [`dynamic_list`](docs/actions/dynamic-list.md) | Shows a filterable list of greetings |
| `paste timestamp` | [`script_action`](docs/actions/script-action.md) | Pastes the current date/time |
| `show user env` | [`dynamic_list`](docs/actions/dynamic-list.md) | Shows user-defined env vars (global + sidecar + inline demo) |
| `list envs` | [`dynamic_list`](docs/actions/dynamic-list.md) | `arg: context` example — typed suffix overrides, bare phrase falls back to the active context |
| `pick customer` | [`static_list`](docs/actions/static-list.md) | `item_action: ctx_set` example — selecting a customer sets it as the active context |
| `open dashboard` | [`open_url`](docs/actions/open-url.md) | Uses the `{context}` URL token to open a context-scoped dashboard |

### Trying the context examples

`list envs`, `pick customer`, and `open dashboard` demonstrate the three ways commands consume the [active context](docs/guides/contexts.md): `arg: context`, `item_action: ctx_set`, and the `{context}` URL token. Type `pick customer` and select one to set the context (e.g. `acme`), then type `list envs` or `open dashboard` — both act on `acme` with nothing else typed. Typing `list envs globex` overrides the context for that one call.
