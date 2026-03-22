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

Add `{param}` anywhere in the URL to capture extra text the user types after the command phrase.

```yaml
phrase: search google
title: Search Google
action:
  type: open_url
  config:
    url: https://www.google.com/search?q={param}
```

Typing `search google rust programming` opens `https://www.google.com/search?q=rust+programming`. The text after the matched phrase is URL-encoded automatically.

---

## Supported URL schemes

Nimble accepts any valid URL scheme — not just `http` and `https`. This means deep links for desktop apps (e.g. `slack://`, `obsidian://`) and other protocols (e.g. `mailto:`, `tel:`) all work out of the box. See [Tips & Tricks](../reference/tips-and-tricks.md) for examples.
