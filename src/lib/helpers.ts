import type { Action, Command, ListItem } from "./types";

// ── Action badge ────────────────────────────────────────────────────────
/** Human-readable badge label for each action type. */
export function actionBadge(cmd: { action: Action }): string {
  const type = cmd.action.type;
  if (type === "builtin" && cmd.action.config.action === "docs_open") return "Docs";
  if (type === "builtin" && cmd.action.config.action === "deploy_skill") return "Deploy";
  switch (type) {
    case "open_url":       return "URL";
    case "paste_text":     return "Paste";
    case "copy_text":      return "Copy";
    case "static_list":    return "List";
    case "dynamic_list":   return "List";
    case "script_action":  return "Script";
    default:               return "";
  }
}

// ── Highlight helper ────────────────────────────────────────────────────
/** Split a phrase around the first occurrence of a query for highlight rendering. */
export function highlight(phrase: string, query: string) {
  const q = query.trim().toLowerCase();
  const idx = phrase.toLowerCase().indexOf(q);
  if (idx === -1 || q === "") return { before: phrase, match: "", after: "" };
  return {
    before: phrase.slice(0, idx),
    match:  phrase.slice(idx, idx + q.length),
    after:  phrase.slice(idx + q.length),
  };
}

// ── Shortcut builder ────────────────────────────────────────────────────
/** Build a Tauri-compatible accelerator string from a KeyboardEvent. */
export function eventToShortcut(e: Pick<KeyboardEvent, "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "key">): string | null {
  const mods: string[] = [];
  if (e.metaKey)  mods.push("Super");
  if (e.ctrlKey)  mods.push("Control");
  if (e.altKey)   mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (mods.length === 0) return null;
  const ignored = new Set(["Meta", "Control", "Alt", "Shift"]);
  if (ignored.has(e.key)) return null;
  const keyMap: Record<string, string> = {
    " ": "Space", "\u00a0": "Space", "ArrowUp": "Up", "ArrowDown": "Down",
    "ArrowLeft": "Left", "ArrowRight": "Right",
  };
  const key = keyMap[e.key] ?? (e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return [...mods, key].join("+");
}

// ── Path shortener ──────────────────────────────────────────────────────
/** Replace the user's home directory prefix with `~` for display. */
export function shortenPath(p: string, defaultCommandsDir: string): string {
  if (defaultCommandsDir) {
    const parts = defaultCommandsDir.split("/");
    const nimbleIdx = parts.lastIndexOf("nimble");
    if (nimbleIdx >= 3) {
      const home = parts.slice(0, nimbleIdx - 2).join("/");
      if (home && p.startsWith(home + "/")) {
        return "~" + p.slice(home.length);
      }
    }
  }
  const linuxMatch = p.match(/^(\/home\/[^/]+)\//);
  if (linuxMatch) {
    return "~" + p.slice(linuxMatch[1].length);
  }
  return p;
}

// ── Command filtering ───────────────────────────────────────────────────
/** Filter and sort commands by a typed query string. */
export function filterCommands(commands: Command[], typed: string): Command[] {
  if (typed === "") return [];
  const lower = typed.toLowerCase();
  const matches = commands.filter(cmd => {
    const phrase = cmd.phrase.toLowerCase();
    return phrase.includes(lower) || lower.startsWith(phrase + " ");
  });
  return matches.slice().sort((a, b) => {
    const ap = a.phrase.toLowerCase();
    const bp = b.phrase.toLowerCase();
    const aParam = lower.startsWith(ap + " ");
    const bParam = lower.startsWith(bp + " ");
    if (aParam && !bParam) return -1;
    if (!aParam && bParam) return 1;
    if (aParam && bParam) return bp.length - ap.length;
    return 0;
  });
}

// ── Param mode detection ────────────────────────────────────────────────
/** Returns true if the raw input is in "param mode" (full phrase + extra text). */
export function isParamMode(raw: string, commands: Command[]): boolean {
  const lower = raw.trim().toLowerCase();
  if (lower === "" || lower.startsWith("/")) return false;
  return commands.some(cmd => lower.startsWith(cmd.phrase.toLowerCase() + " "));
}

// ── Effective input ─────────────────────────────────────────────────────
/** Compute the effective input, appending context when appropriate. */
export function computeEffectiveInput(input: string, activeContext: string, commands: Command[]): string {
  const trimmed = input.trim();
  if (activeContext && trimmed !== "" && !trimmed.startsWith("/") && !isParamMode(trimmed, commands)) {
    return trimmed + " " + activeContext;
  }
  return trimmed;
}

// ── Fuzzy matching ──────────────────────────────────────────────────────

/** Word-boundary characters for fuzzy scoring. */
const BOUNDARY = new Set([" ", "-", "_", ".", "/", "\\"]);

/**
 * fzf-style fuzzy score: all characters in `pattern` must appear in `text` in
 * order (case-insensitive). Returns a numeric score (higher = better match) or
 * `null` when the pattern does not match.
 *
 * Scoring:
 * - +1 per matched character
 * - +2 bonus when the matched character is consecutive with the previous match
 * - +3 bonus when the match is at the start of a word boundary
 * - +2 bonus when the first pattern character matches the first text character
 */
export function fuzzyScore(pattern: string, text: string): number | null {
  if (pattern === "") return 0;
  const pLower = pattern.toLowerCase();
  const tLower = text.toLowerCase();
  let score = 0;
  let pi = 0;
  let lastMatchIdx = -2; // tracks the index of the previous match for consecutive bonus
  for (let ti = 0; ti < tLower.length && pi < pLower.length; ti++) {
    if (tLower[ti] === pLower[pi]) {
      score += 1; // base point
      if (ti === lastMatchIdx + 1) score += 2; // consecutive bonus
      if (ti === 0 || BOUNDARY.has(tLower[ti - 1])) score += 3; // word-boundary bonus
      if (pi === 0 && ti === 0) score += 2; // start-of-string bonus
      lastMatchIdx = ti;
      pi++;
    }
  }
  return pi === pLower.length ? score : null;
}

/**
 * Filter list items using fuzzy matching against both `title` and `subtext`.
 * Returns matching items sorted by descending score.
 * An empty filter returns all items unchanged.
 */
export function fuzzyFilterListItems(items: ListItem[], filter: string): ListItem[] {
  if (filter === "") return items;
  const scored: { item: ListItem; score: number }[] = [];
  for (const item of items) {
    const titleScore = fuzzyScore(filter, item.title);
    const subtextScore = item.subtext ? fuzzyScore(filter, item.subtext) : null;
    const best = Math.max(titleScore ?? -1, subtextScore ?? -1);
    if (best >= 0) {
      scored.push({ item, score: best });
    }
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.map(s => s.item);
}
