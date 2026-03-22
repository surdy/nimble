# Tips & Tricks

Nimble's built-in actions are deceptively powerful. Here are practical patterns that let you do a lot more than might be obvious at first glance.

---

## App deep links

Many macOS applications register their own URL schemes. You can use `open_url` commands to jump straight into specific parts of those apps without touching the mouse.

### Slack — jump to a channel or DM

```yaml
phrase: slack general
title: Open #general in Slack
action:
  type: open_url
  config:
    url: slack://channel?team=T0000000&id=C0000000
```

> Find your team ID and channel ID in Slack's web app URL while you have that channel open:  
> `https://app.slack.com/client/T0000000/C0000000`

### Obsidian — open a vault or note

```yaml
phrase: open daily note
title: Open today's daily note
action:
  type: open_url
  config:
    url: obsidian://open?vault=MyVault&file=Daily%20Notes%2F2026-03-09
```

### Notion — open a specific page

```yaml
phrase: open roadmap
title: Open Product Roadmap in Notion
action:
  type: open_url
  config:
    url: notion://www.notion.so/your-workspace/page-id
```

### Linear — jump to your assigned issues

```yaml
phrase: my issues
title: My open Linear issues
action:
  type: open_url
  config:
    url: linear://linear.app/team/my-issues
```

### Zoom — start or join a meeting

```yaml
phrase: join standup
title: Join daily standup
action:
  type: open_url
  config:
    url: zoommtg://zoom.us/join?confno=123456789&pwd=secret
```

The pattern is the same for any app that supports URL schemes: look up the app's URL scheme in its documentation or support pages, then create a command pointing at it.

---

## Web search via Open URL + parameters

Use `{param}` in the URL to turn any command into a search launcher. Whatever you type _after_ the command phrase becomes the search query, URL-encoded automatically.

### General search engines

```yaml
phrase: search google
title: Search Google
action:
  type: open_url
  config:
    url: https://www.google.com/search?q={param}
```

```yaml
phrase: search duckduckgo
title: Search DuckDuckGo
action:
  type: open_url
  config:
    url: https://duckduckgo.com/?q={param}
```

### Developer resources

```yaml
phrase: search npm
title: Search npm packages
action:
  type: open_url
  config:
    url: https://www.npmjs.com/search?q={param}
```

```yaml
phrase: search mdn
title: Search MDN Web Docs
action:
  type: open_url
  config:
    url: https://developer.mozilla.org/en-US/search?q={param}
```

```yaml
phrase: search crates
title: Search crates.io
action:
  type: open_url
  config:
    url: https://crates.io/search?q={param}
```

```yaml
phrase: search github
title: Search GitHub
action:
  type: open_url
  config:
    url: https://github.com/search?q={param}&type=repositories
```

### Translation and reference

```yaml
phrase: translate
title: Translate with DeepL
action:
  type: open_url
  config:
    url: https://www.deepl.com/translator#auto/en/{param}
```

```yaml
phrase: define
title: Define a word
action:
  type: open_url
  config:
    url: https://www.merriam-webster.com/dictionary/{param}
```

**Usage example:** type `search npm svelte` in the launcher → opens `https://www.npmjs.com/search?q=svelte`.

---

## Frequently pasted text snippets

`paste_text` shines for anything you type repeatedly. Because you define the text once in a YAML file, you keep it perfectly consistent every time — no typos, no reformatting.

### Email signature

```yaml
phrase: paste signature
title: Paste email signature
action:
  type: paste_text
  config:
    text: |
      Best regards,
      Jane Smith
      Product Designer · Acme Inc.
      jane@acme.com | +1 555 0100
      https://acme.com
```

### Canned email reply

```yaml
phrase: paste ooo reply
title: Paste out-of-office reply
action:
  type: paste_text
  config:
    text: "Thanks for your message. I'm currently out of the office and will respond when I return on Monday 16 March. For urgent matters please contact support@acme.com."
```

### Code snippets

```yaml
phrase: paste eslint disable
title: Paste eslint-disable comment
action:
  type: paste_text
  config:
    text: "// eslint-disable-next-line"
```

```yaml
phrase: paste todo
title: Paste TODO comment
action:
  type: paste_text
  config:
    text: "// TODO(yourname): "
```

### Meeting details

```yaml
phrase: paste zoom link
title: Paste Zoom meeting link
action:
  type: paste_text
  config:
    text: "Join the meeting: https://zoom.us/j/123456789 | Passcode: abc123"
```

### Legal / compliance boilerplate

```yaml
phrase: paste disclaimer
title: Paste legal disclaimer
action:
  type: paste_text
  config:
    text: |
      This message and any attachments are confidential and intended solely
      for the addressee. If you have received this message in error, please
      notify the sender immediately and delete it from your system.
```

> **Tip:** organise related snippet files into subdirectories inside your config directory (e.g. `snippets/`, `emails/`, `code/`). Nimble discovers YAML files recursively, so any directory layout works.

---

## Using contexts for scoped matching

A **context** is a word or phrase that is silently appended to every command you type, so you can focus on a topic without retyping it. Type `ctx` in the launcher to manage it.

### Topic-locked web search

```
/ctx set rust programming   → Enter
```

Now type `search google` and press Enter → opens `google.com/search?q=rust+programming`. The context fills the `{param}` slot automatically.

Clear when done:

```
ctx reset   → Enter
```

### Single-word site navigation

```
/ctx set reddit   → Enter
```

Type `open` → matches the `open reddit` command immediately. No need to type the full phrase every time.

### Scoped to a sub-phrase of a long command

The context supplies the *end* of a phrase; you type the beginning. For example, with context `github` and a `search github` command:

- Type `search` — effective input is `"search github"` — the command matches.
- Press Enter with a `{param}` URL → the context word becomes the query.

For a static list with phrase `team emails`, set context to `emails` and type `team` — effective input is `"team emails"` — the list auto-expands.

For the full reference including the built-in `/ctx set` and `/ctx reset` commands see [Contexts](../guides/contexts.md).
