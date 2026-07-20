# Open URL

Opens a URL in your default browser when the command is executed.

---

## Minimal example

```yaml
phrase: open github
title: Open GitHub
action:
  type: open_url
  config:
    url: https://github.com
```

When you type `open github` (or any substring of it) in the launcher and press `Enter` or click the result, your default browser opens `https://github.com`.

---

## With parameter substitution

Add `{param}` anywhere in the URL to capture extra text the user types after the phrase.

```yaml
phrase: search google
title: Search Google
action:
  type: open_url
  config:
    url: https://www.google.com/search?q={param}
```

Typing `search google rust programming` opens `https://www.google.com/search?q=rust+programming`. The text after the matched phrase is URL-encoded automatically — spaces become `+`, special characters become percent-encoded, so you never need to encode the value yourself.

---

## With the active context

Add `{context}` anywhere in the URL to insert the current [active context](../guides/contexts.md) value, independent of any `{param}` matching.

```yaml
phrase: open dashboard
title: Open Dashboard
action:
  type: open_url
  config:
    url: https://{context}.example.com/dashboard
```

With context set to `acme` (`/ctx set acme`), typing `open dashboard` opens `https://acme.example.com/dashboard`. Like `{param}`, the context value is URL-encoded automatically.

**When no context is set**, `{context}` is substituted with an empty string rather than left as a literal placeholder — so `https://{context}.example.com/dashboard` would resolve to `https://.example.com/dashboard`. Design URLs that use `{context}` with this in mind.

`{param}` and `{context}` can both appear in the same URL:

```yaml
phrase: search
title: Search within project
action:
  type: open_url
  config:
    url: https://example.com/{context}/search?q={param}
```

---

## Supported URL schemes

Nimble accepts any valid URL scheme — not just `http` and `https`. This means deep links for desktop apps (e.g. `slack://`, `obsidian://`) and other protocols (e.g. `mailto:`, `tel:`) all work out of the box. See [Tips & Tricks](../reference/tips-and-tricks.md) for examples.
