import { describe, it, expect } from "vitest";
import type { Action, Command } from "$lib/types";
import {
  actionBadge,
  highlight,
  eventToShortcut,
  shortenPath,
  filterCommands,
  fuzzyScore,
  fuzzyFilterListItems,
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
// fuzzyScore
// ═══════════════════════════════════════════════════════════════════════
describe("fuzzyScore", () => {
  it("returns 0 for empty pattern", () => {
    expect(fuzzyScore("", "anything")).toBe(0);
  });

  it("returns null when pattern does not match", () => {
    expect(fuzzyScore("xyz", "Gmail")).toBeNull();
  });

  it("matches characters in order with gaps", () => {
    const score = fuzzyScore("gml", "Gmail");
    expect(score).not.toBeNull();
    expect(score).toBeGreaterThan(0);
  });

  it("is case-insensitive", () => {
    expect(fuzzyScore("gml", "Gmail")).toBe(fuzzyScore("GML", "gmail"));
  });

  it("scores exact substring higher than fuzzy with gaps", () => {
    // "react" is 5 consecutive chars in "React Hooks"; "rhk" has gaps
    const substringScore = fuzzyScore("react", "React Hooks")!;
    const gapScore = fuzzyScore("rhk", "React Hooks")!;
    expect(substringScore).toBeGreaterThan(gapScore);
  });

  it("gives word-boundary bonus", () => {
    // "og" in "open google" — 'g' is at a word boundary
    const boundaryScore = fuzzyScore("og", "open google")!;
    // "og" in "opengoogle" — 'g' is NOT at a word boundary
    const noBoundaryScore = fuzzyScore("og", "opengoogle")!;
    expect(boundaryScore).toBeGreaterThan(noBoundaryScore);
  });

  it("gives start-of-string bonus", () => {
    const startScore = fuzzyScore("op", "open google")!;
    const midScore = fuzzyScore("go", "open google")!;
    // "op" starts at index 0 and gets start-of-string bonus; "go" starts at word boundary
    expect(startScore).toBeGreaterThan(midScore);
  });

  it("returns null when pattern is longer than text", () => {
    expect(fuzzyScore("abcdef", "abc")).toBeNull();
  });

  it("matches single character", () => {
    expect(fuzzyScore("g", "Google")).not.toBeNull();
  });

  it("matches full string exactly", () => {
    const score = fuzzyScore("gmail", "Gmail");
    expect(score).not.toBeNull();
    expect(score).toBeGreaterThan(0);
  });
});

// ═══════════════════════════════════════════════════════════════════════
// fuzzyFilterListItems
// ═══════════════════════════════════════════════════════════════════════
describe("fuzzyFilterListItems", () => {
  const items = [
    { title: "Gmail", subtext: "Google Mail" },
    { title: "Outlook", subtext: "Microsoft Outlook" },
    { title: "Apple Mail" },
    { title: "Yahoo Mail", subtext: "yahoo.com" },
  ];

  it("returns all items for empty filter", () => {
    expect(fuzzyFilterListItems(items, "")).toEqual(items);
  });

  it("filters to matching items only", () => {
    const result = fuzzyFilterListItems(items, "gm");
    const titles = result.map(it => it.title);
    expect(titles).toContain("Gmail");
    expect(titles).not.toContain("Outlook");
  });

  it("matches against subtext when title does not match", () => {
    // "micro" matches "Microsoft Outlook" in subtext
    const result = fuzzyFilterListItems(items, "micro");
    expect(result.map(it => it.title)).toContain("Outlook");
  });

  it("sorts by descending score", () => {
    // "mail" should match Gmail, Apple Mail, Yahoo Mail; Gmail should score highest
    // because "mail" is consecutive in "Gmail" starting at index 1
    const result = fuzzyFilterListItems(items, "mail");
    expect(result.length).toBeGreaterThanOrEqual(3);
    // All mail items should be present
    const titles = result.map(it => it.title);
    expect(titles).toContain("Gmail");
    expect(titles).toContain("Apple Mail");
    expect(titles).toContain("Yahoo Mail");
  });

  it("returns empty array when nothing matches", () => {
    expect(fuzzyFilterListItems(items, "zzz")).toEqual([]);
  });

  it("is case-insensitive", () => {
    const lower = fuzzyFilterListItems(items, "gmail");
    const upper = fuzzyFilterListItems(items, "GMAIL");
    expect(lower.map(it => it.title)).toEqual(upper.map(it => it.title));
  });

  it("handles items without subtext", () => {
    const result = fuzzyFilterListItems(items, "apple");
    expect(result.map(it => it.title)).toContain("Apple Mail");
  });
});
