# Why Nimble?

---

## The problem

Knowledge work involves a lot of repetitive, low-value actions that interrupt the actual work: opening the same URLs, digging up the same snippets of text, switching between apps to copy a value and paste it somewhere else. Each of these micro-tasks is individually cheap but collectively they add up to a significant source of friction.

More importantly, the *path* to completing them — click the dock, find the window, navigate to the right place — pulls your attention away from what you were doing. By the time you're back, you've lost a bit of the mental state you had before.

Nimble is built around a single principle: **the fastest way to do something is to type what you want and press Enter**.

---

## Goals

### Reduce friction in repetitive tasks
Common workflows — opening a frequently used URL, pasting a template, copying an account number — should take one or two keystrokes from anywhere on the system, not a sequence of window switches and clicks.

### Reduce context-switching overhead
Nimble lets you define commands that encapsulate multi-step actions. Instead of clicking through several apps to accomplish something, you type a phrase and it's done. The [Contexts](guides/contexts.md) feature extends this further: you can set an active context (e.g. a project name, a customer, a ticket) so that all subsequent commands are automatically scoped to it, without you having to retype that context for every action.

### Reduce distraction
Every time you navigate to a browser tab, open Slack, or hunt through Finder, you're exposed to notifications, unread counts, and other triggers that can pull you off-task. Nimble keeps you in the keyboard and out of the GUI.

---

## Why not Alfred or Raycast?

Alfred and Raycast are excellent tools. Nimble isn't trying to replace them — but there are a few reasons it exists:

- **Platform availability.** Alfred is macOS-only. Raycast is macOS-only. Nimble is built with Tauri and targets macOS, Linux, and Windows. For anyone who works across platforms, having the same muscle memory everywhere matters.

- **Privacy.** Some workflows involve sensitive data — customer names, account numbers, internal URLs. Running that through a third-party tool with cloud sync or telemetry is a hard no for many teams. Nimble is local-only by design; your commands and data never leave your machine.

- **The Contexts feature.** The ability to set an active context and have it silently appended to every command phrase is something I really wanted in my own workflow, and it's not a natural fit for how existing tools are structured. Building it from scratch made more sense than trying to bolt it onto someone else's plugin system.

- **A genuine learning exercise.** Nimble was built using GitHub Copilot as the primary coding assistant. That was an explicit goal — to explore what AI-assisted development looks like end-to-end on a real project. You can't learn that by extending an existing app.

---

## Non-goals

These are things Nimble is deliberately *not* trying to be, at least for now:

- **Not an application launcher.** Nimble does not index or launch desktop applications. Its command model is explicit: you define what phrases do. Spotlight and your OS's built-in launcher already handle app launching well.

- **Not a full Alfred / Raycast replacement.** Nimble covers a focused, opinionated slice of what those tools do. It is not trying to match their breadth of built-in integrations, file search, calculator, clipboard history, and so on.

- **No plugin / workflow system.** There are no plans to build a public plugin API or marketplace. Users can extend behaviour through scripts (shell, Python, Node, etc.) called by the `dynamic_list` and `script_action` types, but the extension mechanism is intentionally simple and file-based — not a platform.
