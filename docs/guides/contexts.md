# Contexts

A **context** is an ambient word or phrase that Nimble carries alongside the launcher. It never changes how commands are matched and never becomes a parameter — it is simply made available to every script through the `NIMBLE_CONTEXT` environment variable, and to external tools through `state.json`.

---

## How it works

Command matching and parameter extraction always use exactly what you type — nothing more:

- Typed text is the **only** source of command matching. A phrase matches when your input contains it (or extends it with a parameter), regardless of any active context.
- Typed text is the **only** source of parameters. The `{param}` value for `open_url`, and the argument passed to `dynamic_list` / `script_action` scripts, come solely from the text you type after the phrase.
- The active context is delivered to every script it runs as the `NIMBLE_CONTEXT` environment variable (an empty string when no context is set). Scripts decide for themselves whether and how to use it.

- `open_url` commands can read the active context via the `{context}` URL token (see [Open URL](../actions/open-url.md#with-the-active-context)).
- `script_action` and `dynamic_list` commands with [`arg: context`](../actions/script-action.md#arg-context) treat an active context as satisfying an otherwise-required argument: with no typed suffix the command still fires (passing no positional argument, so the script reads `NIMBLE_CONTEXT`), while a typed suffix overrides and is passed as `$1`. The context is never passed as the positional argument. (See [Dynamic List](../actions/dynamic-list.md#arg-context) for the list-specific nuances, e.g. cache invalidation when the context changes.)
- Any `static_list` or `dynamic_list` item with [`item_action: ctx_set`](../actions/static-list.md#item_action) sets the active context to the selected item's value (its `subtext`, or `title` if absent) and keeps the launcher open — a "context picker" that behaves like `/ctx set` without typing.

When no context is set the only difference is that `NIMBLE_CONTEXT` is empty (and `{context}` substitutes to an empty string).

---

## Managing contexts with built-in commands

Two built-in commands, always available, control the active context. Type `/` to see them:

| Command | What it does |
|---------|-------------|
| `/ctx set <value>` | Sets the context to `<value>` |
| `/ctx reset` | Clears the active context |

These commands never dismiss the launcher — the window stays open so you can immediately see the effect and start typing.

### Setting a context

Type `/ctx set` followed by a space and your context value, then press Enter:

```
/ctx set project-x
```

The input is cleared and the launcher stays open. The context chip in the input bar shows the active value, and every script you run from now on receives `NIMBLE_CONTEXT=project-x`.

### Previewing the value before confirming

While typing `/ctx set <value>`, the `/ctx set` result row shows a subtext preview:

```
→ set context to "project-x"
```

This confirms what will be stored before you press Enter.

### Clearing the context

```
/ctx reset
```

Press Enter to clear the context. Scripts then receive `NIMBLE_CONTEXT` as an empty string.

---

## Using the context in scripts

Any `dynamic_list` or `script_action` script can read the context and adapt its behaviour:

```bash
#!/bin/bash
if [ -n "$NIMBLE_CONTEXT" ]; then
  # Scope results to the active context (e.g. a project name)
  search_issues --project "$NIMBLE_CONTEXT" "$1"
else
  search_issues "$1"
fi
```

The script's argument (`$1`) is always just the text the user typed after the command phrase — the context never leaks into it.

### Walkthrough

1. Open the launcher and run `/ctx set project-x` → press **Enter**.
   The input clears; the launcher stays open and the chip shows `project-x`.
2. Type a phrase that triggers a script command (e.g. `search issues login bug`).
   The script receives `login bug` as its argument and `NIMBLE_CONTEXT=project-x` in its environment.
3. Run `/ctx reset` when you are done. The same command now runs with an empty `NIMBLE_CONTEXT`.

Matching behaves identically in all three steps — the context only changes what scripts see in their environment.

---

## Reserved namespace

User-defined YAML commands whose phrase starts with `/` are automatically rejected at load time. If any such file exists, the warnings bar increments its count. This ensures the built-in app commands are never shadowed by user commands.

Accepted examples:
- `"open github/issues"` — `/` is not the first character
- `"search/replace"` — same rule

Rejected examples:
- `"/ctx set my command"` — starts with `/`
- `"/ctx reset"` — starts with `/`

---

## Typical workflows

### Project-scoped scripts

Set the context to a project identifier once, then let your scripts scope themselves:

```
/ctx set acme-webshop
```

A `search tickets` script can read `NIMBLE_CONTEXT` and restrict its query to that project, while the launcher itself keeps matching commands purely on what you type.

### Picking a context from a list

Instead of typing `/ctx set <value>` from memory, a `static_list` or `dynamic_list` command with `item_action: ctx_set` lets you pick the value from a list — e.g. a list of customers or projects, with the slug as each item's `subtext`. Selecting an item sets the context and keeps the launcher open, exactly like `/ctx set`. See the [`pick customer`](../../example-config/commands/examples/pick-customer/pick-customer.yaml) example in `example-config/`.

Commands using [`arg: context`](../actions/script-action.md#arg-context) (or the equivalent `dynamic_list` mode) then act on whatever was picked with nothing further typed — see the [`list envs`](../../example-config/commands/examples/list-envs/list-envs.yaml) example.

### Cross-app coordination

Because the context is persisted to `state.json` and settable via `nimble://` deep links, it doubles as a lightweight signal shared between Nimble scripts and other tools — set it from a Hammerspoon binding, read it from a shell script, clear it with `/ctx reset`.

### Clearing when done

```
/ctx reset
```

`NIMBLE_CONTEXT` becomes an empty string for all subsequent script runs.

---

## External access

Other applications can read and write the active context without interacting with the launcher window. See [External Context API](external-context.md) for details on the `state.json` file and `nimble://` deep links.
