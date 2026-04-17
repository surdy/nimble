import { describe, it, expect } from "vitest";
import type { Action, Command } from "$lib/types";
import {
  actionBadge,
  highlight,
  eventToShortcut,
  shortenPath,
  filterCommands,
  isParamMode,
  computeEffectiveInput,
} from "$lib/helpers";

// ── Helper to build a minimal Command ──────────────────────────────────
function cmd(phrase: string, action: Action): Command {
  return { phrase, title: phrase, env: {}, action, source_dir: "", source_file: "" };
}
const openUrl = (url: string): Action => ({ type: "open_url", config: { url } });
const pasteText = (text: string): Action => ({ type: "paste_text", config: { text } });
const copyText = (text: string): Action => ({ type: "copy_text", config: { text } });
const staticList = (list: string): Action => ({ type: "static_list", config: { list } });
const dynamicList = (script: string): Action => ({ type: "dynamic_list", config: { script } });
const scriptAction = (script: string): Action => ({ type: "script_action", config: { script, result_action: "paste_text" } });
const builtin = (action: string, url?: string): Action => ({ type: "builtin", config: { action, ...(url ? { url } : {}) } } as Action);

// ═══════════════════════════════════════════════════════════════════════
// actionBadge
// ═══════════════════════════════════════════════════════════════════════
describe("actionBadge", () => {
  it("returns 'URL' for open_url", () => {
    expect(actionBadge({ action: openUrl("https://example.com") })).toBe("URL");
  });

  it("returns 'Paste' for paste_text", () => {
    expect(actionBadge({ action: pasteText("hello") })).toBe("Paste");
  });

  it("returns 'Copy' for copy_text", () => {
    expect(actionBadge({ action: copyText("hello") })).toBe("Copy");
  });

  it("returns 'List' for static_list", () => {
    expect(actionBadge({ action: staticList("items.tsv") })).toBe("List");
  });

  it("returns 'List' for dynamic_list", () => {
    expect(actionBadge({ action: dynamicList("run.sh") })).toBe("List");
  });

  it("returns 'Script' for script_action", () => {
    expect(actionBadge({ action: scriptAction("run.sh") })).toBe("Script");
  });

  it("returns 'Docs' for builtin docs_open", () => {
    expect(actionBadge({ action: builtin("docs_open", "https://example.com") })).toBe("Docs");
  });

  it("returns 'Deploy' for builtin deploy_skill", () => {
    expect(actionBadge({ action: builtin("deploy_skill") })).toBe("Deploy");
  });

  it("returns empty string for other builtins", () => {
    expect(actionBadge({ action: builtin("ctx_set") })).toBe("");
  });
});

// ═══════════════════════════════════════════════════════════════════════
// highlight
// ═══════════════════════════════════════════════════════════════════════
describe("highlight", () => {
  it("returns full phrase with no match for empty query", () => {
    expect(highlight("open google", "")).toEqual({
      before: "open google",
      match: "",
      after: "",
    });
  });

  it("matches at the beginning", () => {
    expect(highlight("open google", "open")).toEqual({
      before: "",
      match: "open",
      after: " google",
    });
  });

  it("matches in the middle", () => {
    expect(highlight("open google", "goo")).toEqual({
      before: "open ",
      match: "goo",
      after: "gle",
    });
  });

  it("is case-insensitive", () => {
    expect(highlight("Open Google", "OPEN")).toEqual({
      before: "",
      match: "Open",
      after: " Google",
    });
  });

  it("returns no match when query not found", () => {
    expect(highlight("open google", "zzz")).toEqual({
      before: "open google",
      match: "",
      after: "",
    });
  });

  it("trims query whitespace", () => {
    expect(highlight("open google", "  open  ")).toEqual({
      before: "",
      match: "open",
      after: " google",
    });
  });

  it("matches the full phrase", () => {
    expect(highlight("abc", "abc")).toEqual({
      before: "",
      match: "abc",
      after: "",
    });
  });
});

// ═══════════════════════════════════════════════════════════════════════
// eventToShortcut
// ═══════════════════════════════════════════════════════════════════════
describe("eventToShortcut", () => {
  it("returns null when no modifier is pressed", () => {
    expect(eventToShortcut({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, key: "a" })).toBeNull();
  });

  it("returns null when only a modifier key is pressed", () => {
    expect(eventToShortcut({ metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, key: "Meta" })).toBeNull();
    expect(eventToShortcut({ metaKey: false, ctrlKey: true, altKey: false, shiftKey: false, key: "Control" })).toBeNull();
  });

  it("builds Super+key for Cmd+letter", () => {
    expect(eventToShortcut({ metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, key: "k" })).toBe("Super+K");
  });

  it("combines multiple modifiers", () => {
    expect(eventToShortcut({ metaKey: true, ctrlKey: false, altKey: true, shiftKey: false, key: "p" })).toBe("Super+Alt+P");
  });

  it("maps space key", () => {
    expect(eventToShortcut({ metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, key: " " })).toBe("Super+Space");
  });

  it("maps arrow keys", () => {
    expect(eventToShortcut({ metaKey: false, ctrlKey: true, altKey: false, shiftKey: false, key: "ArrowUp" })).toBe("Control+Up");
    expect(eventToShortcut({ metaKey: false, ctrlKey: true, altKey: false, shiftKey: false, key: "ArrowDown" })).toBe("Control+Down");
  });

  it("preserves multi-character key names", () => {
    expect(eventToShortcut({ metaKey: false, ctrlKey: true, altKey: false, shiftKey: false, key: "Enter" })).toBe("Control+Enter");
  });

  it("handles Shift+letter", () => {
    expect(eventToShortcut({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: true, key: "A" })).toBe("Shift+A");
  });
});

// ═══════════════════════════════════════════════════════════════════════
// shortenPath
// ═══════════════════════════════════════════════════════════════════════
describe("shortenPath", () => {
  it("shortens macOS path using defaultCommandsDir", () => {
    const defaultDir = "/Users/foo/Library/Application Support/nimble/commands";
    expect(shortenPath("/Users/foo/Library/Application Support/nimble/commands", defaultDir))
      .toBe("~/Library/Application Support/nimble/commands");
  });

  it("returns path unchanged when no defaultCommandsDir", () => {
    expect(shortenPath("/Users/foo/some/path", "")).toBe("/Users/foo/some/path");
  });

  it("shortens Linux /home path without defaultCommandsDir", () => {
    expect(shortenPath("/home/foo/.config/nimble/commands", "")).toBe("~/.config/nimble/commands");
  });

  it("returns path unchanged when it does not match home prefix", () => {
    const defaultDir = "/Users/foo/Library/Application Support/nimble/commands";
    expect(shortenPath("/opt/nimble/commands", defaultDir)).toBe("/opt/nimble/commands");
  });

  it("handles Windows-like paths gracefully (returns unchanged)", () => {
    expect(shortenPath("C:\\Users\\foo\\nimble", "")).toBe("C:\\Users\\foo\\nimble");
  });
});

// ═══════════════════════════════════════════════════════════════════════
// filterCommands
// ═══════════════════════════════════════════════════════════════════════
describe("filterCommands", () => {
  const cmds = [
    cmd("open google", openUrl("https://google.com")),
    cmd("open github", openUrl("https://github.com")),
    cmd("paste greeting", pasteText("hello")),
    cmd("search google", openUrl("https://google.com/search?q={param}")),
  ];

  it("returns empty array for empty query", () => {
    expect(filterCommands(cmds, "")).toEqual([]);
  });

  it("finds substring matches", () => {
    const result = filterCommands(cmds, "goo");
    const phrases = result.map(c => c.phrase);
    expect(phrases).toContain("open google");
    expect(phrases).toContain("search google");
    expect(phrases).not.toContain("paste greeting");
  });

  it("matches full phrase (exact)", () => {
    const result = filterCommands(cmds, "open google");
    expect(result.map(c => c.phrase)).toContain("open google");
  });

  it("enters param mode when typed extends a phrase", () => {
    const result = filterCommands(cmds, "search google react hooks");
    expect(result.length).toBe(1);
    expect(result[0].phrase).toBe("search google");
  });

  it("sorts param-mode matches before partial matches", () => {
    const result = filterCommands(cmds, "open google maps");
    // "open google" is in param mode, so it should come first
    expect(result[0].phrase).toBe("open google");
  });

  it("is case-insensitive", () => {
    const result = filterCommands(cmds, "OPEN");
    expect(result.length).toBe(2);
  });

  it("prefers longer phrase in param mode", () => {
    const commands = [
      cmd("search", openUrl("https://search.com?q={param}")),
      cmd("search google", openUrl("https://google.com/search?q={param}")),
    ];
    const result = filterCommands(commands, "search google hello");
    // Both are in param mode; "search google" is longer so it should be first
    expect(result[0].phrase).toBe("search google");
  });
});

// ═══════════════════════════════════════════════════════════════════════
// isParamMode
// ═══════════════════════════════════════════════════════════════════════
describe("isParamMode", () => {
  const cmds = [
    cmd("open google", openUrl("https://google.com")),
    cmd("search google", openUrl("https://google.com/search?q={param}")),
  ];

  it("returns false for empty input", () => {
    expect(isParamMode("", cmds)).toBe(false);
  });

  it("returns false for slash commands", () => {
    expect(isParamMode("/ctx set work", cmds)).toBe(false);
  });

  it("returns false for partial match", () => {
    expect(isParamMode("open goo", cmds)).toBe(false);
  });

  it("returns true when full phrase + extra text", () => {
    expect(isParamMode("search google react hooks", cmds)).toBe(true);
  });

  it("returns false for exact phrase match (no trailing text)", () => {
    expect(isParamMode("open google", cmds)).toBe(false);
  });

  it("is case-insensitive", () => {
    expect(isParamMode("OPEN GOOGLE something", cmds)).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════════════
// computeEffectiveInput
// ═══════════════════════════════════════════════════════════════════════
describe("computeEffectiveInput", () => {
  const cmds = [
    cmd("open google", openUrl("https://google.com")),
  ];

  it("returns trimmed input when no context is active", () => {
    expect(computeEffectiveInput("  hello  ", "", cmds)).toBe("hello");
  });

  it("appends context when active and not in param mode", () => {
    expect(computeEffectiveInput("open", "work", cmds)).toBe("open work");
  });

  it("does not append context when input starts with /", () => {
    expect(computeEffectiveInput("/ctx set", "work", cmds)).toBe("/ctx set");
  });

  it("does not append context in param mode", () => {
    expect(computeEffectiveInput("open google maps", "work", cmds)).toBe("open google maps");
  });

  it("does not append context when input is empty", () => {
    expect(computeEffectiveInput("", "work", cmds)).toBe("");
  });

  it("does not append context when input is only whitespace", () => {
    expect(computeEffectiveInput("   ", "work", cmds)).toBe("");
  });
});
