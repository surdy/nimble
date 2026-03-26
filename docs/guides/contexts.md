# Contexts

A **context** is a word or phrase that is silently appended to every command you type, narrowing all matching to a particular topic without you having to keep retyping it.

---

## How it works

When a context is active, Nimble substitutes your raw input with:

```
effective input = raw input + " " + context
```

**Exception — parameter mode:** if the raw input already matches a known command phrase plus trailing text (a parameter), context is **not** appended. This keeps user-supplied parameters clean; scripts and URLs receive only what the user explicitly typed. Scripts can still read the active context via the `NIMBLE_CONTEXT` environment variable.

This means:

- Type `open` with context `reddit` → matches any command whose phrase contains `"open reddit"`.
- Type `search google` with context `rust programming` → phrase `search google` matches exactly and the context `"rust programming"` fills the `{param}` value.
- Type `search google cats` with context `rust programming` → the user supplied a parameter `"cats"`, so context is **not** appended; `{param}` is `"cats"`, not `"cats rust programming"`. The script/URL still has access to `NIMBLE_CONTEXT=rust programming`.
- The context is invisible in the input bar — you just see what you type, but the matching uses the full phrase.

When no context is set the launcher behaves exactly as usual.

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
/ctx set reddit
```

The input is cleared and the launcher stays open. You can now type any partial phrase and matching will happen as if you had added `" reddit"` to everything you type.

### Previewing the value before confirming

While typing `/ctx set <value>`, the `/ctx set` result row shows a subtext preview:

```
→ set context to "reddit"
```

This confirms what will be stored before you press Enter.

### Clearing the context

```
/ctx reset
```

Press Enter to clear the context. All matching returns to normal.

---

## Manual testing walkthrough

The examples below use commands that ship in the `example-config/` directory. Copy that directory into your live config to follow along:

```bash
cp -r example-config/* ~/Library/Application\ Support/Nimble/
```

### Test 1 — Context as phrase completion (`open_url`)

**Goal:** type `open` and have it match `open reddit` automatically.

1. Open the launcher and run `/ctx set reddit` → press **Enter**.  
   The input clears; the launcher stays open.
2. Type `open`.  
   You should see `open-reddit.yaml`'s entry (`Open Reddit`) in the results, because the effective input is `"open reddit"`.
3. Press **Enter** to open `https://www.reddit.com`.

**What happened:** `effectiveInput = "open" + " " + "reddit" = "open reddit"`, which matches the phrase `open reddit`.

### Test 2 — Context as a search parameter (`open_url` with `{param}`)

**Goal:** type `search google` and have the context value pass as the query.

1. Run `/ctx set rust programming` → **Enter**.
2. Type `search google` → press **Enter**.  
   Your browser opens `https://www.google.com/search?q=rust+programming`.

**What happened:** `effectiveInput = "search google" + " " + "rust programming"`. The phrase `search google` matches exactly, and the suffix `"rust programming"` is URL-encoded as the `{param}` value.

### Test 2b — User-supplied parameter overrides context

**Goal:** type `search google cats` and confirm the context is *not* appended to the parameter.

1. Context is still `rust programming` from the previous test.
2. Type `search google cats` → press **Enter**.  
   Your browser opens `https://www.google.com/search?q=cats`.

**What happened:** `"search google cats"` already places the raw input in param mode for the `search google` command, so context is skipped. The param is `"cats"`, not `"cats rust programming"`. The script could still read `NIMBLE_CONTEXT=rust programming` if it needed the context.

### Test 3 — Context as a static list trigger

The context supplies the *end* of a phrase, so you type the beginning and the context completes it. For the `team emails` static list (phrase: `team emails`):

1. Run `/ctx set emails` → **Enter**.
2. Type `team`.
   `effectiveInput = "team" + " " + "emails" = "team emails"` — an exact match with the phrase.
   The `team-emails` static list expands immediately, without pressing Enter.
3. Select an item to paste its value.

Regardless of what context is active, typing `/` always works normally. Effective input is not applied when the raw input starts with `/`.

### Test 4 — Clearing the context

1. Type `/ctx reset` → **Enter**.  
   The context is cleared.
2. Type `open` — now only commands whose phrase literally contains `"open"` are shown.

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

### Scoped site browsing

Set context to a site keyword, then use short phrases to navigate:

```
/ctx set github
```

Then type:
- `open` → matches `open github` → opens GitHub
- `search` → if you have a `search github` command with `{param}`, the context fills the query

### Topic-locked searches

Set context to a search topic once at the start of a research session:

```
/ctx set typescript generics
```

Then type `search google` → opens `google.com/search?q=typescript+generics` without retyping the topic each time.

If you then type `search google advanced types`, the parameter `"advanced types"` overrides the context — the context is *not* appended. The search term is just `"advanced types"`.

### Clearing when done

```
/ctx reset
```

All commands go back to matching against raw input only.
