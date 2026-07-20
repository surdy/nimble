<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { listen, emit } from "@tauri-apps/api/event";
  import type { Action, AppSettings, Command, CommandFileMeta, CommandsPayload, CommandWarning, DuplicateWarning, ListItem, ReservedPhraseWarning, SkippedFileWarning } from "$lib/types";
  import { actionBadge, highlight, eventToShortcut, shortenPath, filterCommands, fuzzyFilterListItems } from "$lib/helpers";

  // ── State ──────────────────────────────────────────────────────────────
  let input = $state("");
  let inputEl: HTMLInputElement | undefined = $state();
  let onboardingEl: HTMLDivElement | undefined = $state();
  const appWindow = getCurrentWindow();

  // Detect whether we are running inside the unified Preferences window
  // (label "preferences") vs the main launcher window (label "main").
  // The active starting tab is read from the URL hash synchronously.
  // This is all synchronous so it affects initial render without any flash.
  const isPreferencesWindow = appWindow.label === "preferences";
  if (isPreferencesWindow && typeof document !== "undefined") {
    document.documentElement.classList.add("preferences-window-mode");
  }

  // Active tab in the preferences window: "commands" or "settings".
  // Initialised to "commands"; updated in onMount via get_preferences_initial_tab.
  let activePreferencesTab = $state("commands");


  // Onboarding: shown on first launch until a shortcut is chosen
  let onboarding = $state(false);
  let capturedShortcut = $state("");
  let shortcutError = $state("");

  // Command store — loaded once on mount
  let commands = $state<Command[]>([]);

  // Duplicate-command warnings from the last load / reload cycle
  let warnings = $state<DuplicateWarning[]>([]);
  let reservedWarnings = $state<ReservedPhraseWarning[]>([]);
  let skippedWarnings = $state<SkippedFileWarning[]>([]);
  let commandWarnings = $state<CommandWarning[]>([]);
  let warningsDismissed = $state(false);
  const totalWarnings = $derived(warnings.length + reservedWarnings.length + skippedWarnings.length + commandWarnings.length);
  const warningVisible = $derived(totalWarnings > 0 && !warningsDismissed);

  // Active context — empty string means no context is set
  let activeContext = $state("");

  // Whether the context chip should be rendered (from settings.yaml)
  let showContextChip = $state(true);

  // Full settings object (loaded on mount, kept in sync when settings are saved)
  let currentSettings = $state<AppSettings>({
    show_context_chip: true,
    allow_duplicates: true,
    shared_dir: "shared",
  });

  // In the preferences window the settings panel is always visible when the
  // settings tab is active; in the launcher it can be toggled (legacy path).
  let showSettings = $state(false);
  let settingsShowContextChip = $state(true);
  let settingsAllowDuplicates = $state(true);
  let settingsSharedDir = $state("shared");
  let settingsCommandsDir = $state("");
  let defaultCommandsDir = $state("");
  let settingsChangingHotkey = $state(false);
  let settingsCapturedShortcut = $state("");
  let settingsHotkeyError = $state("");
  let settingsSavedTimer: ReturnType<typeof setTimeout> | null = null;
  let settingsSaved = $state(false);

  // ── Command editor state ────────────────────────────────────────────
  let cmdList = $state<CommandFileMeta[]>([]);
  let cmdFilter = $state("");
  let cmdSelectedFile = $state<string | null>(null);
  let cmdIsNew = $state(false);
  let cmdPhrase = $state("");
  let cmdTitle = $state("");
  let cmdEnabled = $state(true);
  let cmdActionType = $state<"open_url" | "paste_text" | "copy_text" | "static_list" | "dynamic_list" | "script_action">("open_url");
  let cmdUrl = $state("");
  let cmdText = $state("");
  let cmdListName = $state("");
  let cmdItemAction = $state("");
  let cmdScript = $state("");
  let cmdArgMode = $state("none");
  let cmdResultAction = $state("paste_text");
  let cmdPrefix = $state("");
  let cmdSuffix = $state("");
  let cmdSaving = $state(false);
  let cmdSaveError = $state("");
  let cmdDeleteConfirm = $state(false);
  // ── Folder & script editor state ──────────────────────────────────────
  let cmdFolders = $state<string[]>([]);
  let cmdTargetDir = $state("");  // folder for new commands
  let cmdNewFolderName = $state("");  // when creating a new folder
  let cmdShowNewFolder = $state(false); // toggle inline new-folder input
  let cmdCollapsedFolders = $state<Set<string>>(new Set());
  // Tracks the previous filter text so we can auto-expand when the query changes
  let cmdPrevFilter = $state("");
  // Script editor
  let cmdScriptContent = $state("");
  let cmdScriptExists = $state(false);
  let cmdInlineEnv = $state<Record<string, string>>({});
  let cmdScriptLoading = $state(false);
  let cmdScriptDirty = $state(false);
  let cmdScriptSaving = $state(false);
  let cmdScriptError = $state("");
  // Test run
  let cmdTestResult = $state<import("$lib/types").ScriptTestResult | null>(null);
  let cmdTestRunning = $state(false);
  let cmdTestArg = $state("");
  // When the filter query changes, auto-expand all folders so matches are visible.
  // The user can still manually collapse folders during the same search.
  $effect(() => {
    if (cmdFilter !== cmdPrevFilter) {
      cmdPrevFilter = cmdFilter;
      if (cmdFilter) {
        cmdCollapsedFolders = new Set(); // expand all
      }
    }
  });
  const cmdFilteredList = $derived(
    cmdList.filter(c =>
      cmdFilter === "" ||
      c.phrase.toLowerCase().includes(cmdFilter.toLowerCase()) ||
      c.title.toLowerCase().includes(cmdFilter.toLowerCase())
    )
  );
  // Group filtered commands by the active cmdGroupBy mode
  const cmdGroupedList = $derived(() => {
    const groups: { folder: string; label: string; items: CommandFileMeta[] }[] = [];
    if (cmdGroupBy === "none") {
      // Flat list — single group with all items sorted
      const sorted = [...cmdFilteredList].sort(cmdSortCompare);
      groups.push({ folder: "__all__", label: "All Commands", items: sorted });
      return groups;
    }
    const map = new Map<string, CommandFileMeta[]>();
    for (const c of cmdFilteredList) {
      const key = cmdGroupBy === "type" ? c.action_type : (c.source_dir || "");
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(c);
    }
    const keys = [...map.keys()].sort((a, b) => {
      if (a === "" && b !== "") return -1;
      if (a !== "" && b === "") return 1;
      return a.localeCompare(b);
    });
    for (const key of keys) {
      const label = cmdGroupBy === "type" ? key.replace(/_/g, " ") : (key || "Commands");
      const items = [...map.get(key)!].sort(cmdSortCompare);
      groups.push({ folder: key, label, items });
    }
    return groups;
  });
  // Whether the script name uses legacy ${VAR} substitution (no longer supported)
  const cmdScriptIsExternal = $derived(cmdScript.includes("${"));
  const cmdPhraseConflict = $derived(
    !!cmdPhrase.trim() &&
    cmdList.some(c =>
      c.phrase.toLowerCase() === cmdPhrase.trim().toLowerCase() &&
      c.file_path !== cmdSelectedFile
    )
  );
  const cmdCanSave = $derived(
    !cmdSaving &&
    cmdPhrase.trim() !== "" &&
    !cmdPhraseConflict &&
    (cmdActionType === "open_url" ? cmdUrl.trim() !== ""
      : cmdActionType === "paste_text" || cmdActionType === "copy_text" ? cmdText.trim() !== ""
      : cmdActionType === "static_list" ? cmdListName.trim() !== ""
      : cmdActionType === "dynamic_list" ? cmdScript.trim() !== ""
      : cmdActionType === "script_action" ? cmdScript.trim() !== "" && cmdResultAction !== ""
      : false)
  );
  const CMD_EDITABLE_TYPES = new Set(["open_url", "paste_text", "copy_text", "static_list", "dynamic_list", "script_action"]);
  // ── Table-view state ────────────────────────────────────────────────
  let cmdEditingMode = $state(false);  // false = table list, true = detail form
  let cmdCtxMenu = $state<{ x: number; y: number; meta: CommandFileMeta } | null>(null);
  let cmdGroupBy = $state<"none" | "folder" | "type">("none");
  let cmdSortCol = $state<"phrase" | "title" | "action" | "modified">("phrase");
  let cmdSortAsc = $state(true);
  // Bulk-select state
  let cmdBulkSelected = $state<Set<string>>(new Set());
  let cmdBulkDeleteConfirm = $state(false);
  const cmdBulkCount = $derived(cmdBulkSelected.size);
  const cmdBulkAllSelected = $derived(
    cmdFilteredList.length > 0 && cmdFilteredList.every(c => cmdBulkSelected.has(c.file_path))
  );
  // Settings sidebar nav
  let settingsActiveNav = $state("keyboard");
  // Restore Defaults confirmation
  let settingsRestoreConfirm = $state(false);
  /** Switch from table list to detail editor for a command. */
  function cmdOpenEditor(meta: CommandFileMeta) {
    cmdSelectItem(meta);
    cmdEditingMode = true;
  }
  /** Return from detail editor to the table list. */
  function cmdCloseEditor() {
    cmdEditingMode = false;
    cmdIsNew = false;
  }
  /** Start creating a new command (opens editor). */
  function cmdStartNewFromTable() {
    cmdStartNew();
    cmdEditingMode = true;
  }

  /** Icon glyph for each action type. */
  function actionIcon(type: string): string {
    switch (type) {
      case "open_url":       return "🔗";
      case "paste_text":     return "📋";
      case "copy_text":      return "⧉";
      case "static_list":    return "≣";
      case "dynamic_list":   return "⚡";
      case "script_action":  return "▶";
      default:               return "•";
    }
  }

  /** Get load-time warning for a specific command file, if any. */
  function cmdWarningForFile(meta: CommandFileMeta): string | null {
    const relPath = meta.file_path;
    const cw = commandWarnings.find(w => relPath.endsWith(w.file));
    if (cw) return cw.message;
    const sw = skippedWarnings.find(w => relPath.endsWith(w.file));
    if (sw) return sw.reason;
    return null;
  }

  /** Count disabled commands in a group. */
  function cmdDisabledCount(items: CommandFileMeta[]): number {
    return items.filter(c => !c.enabled).length;
  }

  /** Format a Unix timestamp as relative time (e.g. "2d ago", "3w ago"). */
  function relativeTime(ts: number): string {
    if (!ts) return "—";
    const now = Date.now() / 1000;
    const diff = Math.max(0, now - ts);
    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
    if (diff < 2592000) return `${Math.floor(diff / 604800)}w ago`;
    return `${Math.floor(diff / 2592000)}mo ago`;
  }

  /** Toggle sort: if same column, flip direction; otherwise sort ascending on new column. */
  function cmdToggleSort(col: "phrase" | "title" | "action" | "modified") {
    if (cmdSortCol === col) {
      cmdSortAsc = !cmdSortAsc;
    } else {
      cmdSortCol = col;
      cmdSortAsc = col === "modified" ? false : true; // modified defaults descending
    }
  }

  /** Sort comparator for the current column. */
  function cmdSortCompare(a: CommandFileMeta, b: CommandFileMeta): number {
    let cmp = 0;
    switch (cmdSortCol) {
      case "phrase": cmp = a.phrase.localeCompare(b.phrase); break;
      case "title": cmp = a.title.localeCompare(b.title); break;
      case "action": cmp = a.action_type.localeCompare(b.action_type); break;
      case "modified": cmp = a.modified - b.modified; break;
    }
    return cmdSortAsc ? cmp : -cmp;
  }
  /** Toggle a command's enabled state directly from the table row. */
  async function cmdToggleEnabled(meta: CommandFileMeta, newEnabled: boolean) {
    try {
      const result = await invoke<{ commands: import("$lib/types").Command[] }>("list_commands");
      const full = result.commands.find(c => c.phrase.toLowerCase() === meta.phrase.toLowerCase());
      if (!full) return;
      let configJson: string;
      if (full.action.type === "open_url") configJson = JSON.stringify(full.action.config);
      else if (full.action.type === "paste_text" || full.action.type === "copy_text") configJson = JSON.stringify(full.action.config);
      else if (full.action.type === "static_list") configJson = JSON.stringify(full.action.config);
      else if (full.action.type === "dynamic_list") configJson = JSON.stringify(full.action.config);
      else configJson = JSON.stringify(full.action.config);
      await invoke("save_command_file", {
        phrase: meta.phrase,
        title: meta.title,
        enabled: newEnabled,
        actionType: full.action.type,
        configJson,
        filePath: meta.file_path,
      });
      await cmdRefreshList();
    } catch (err) {
      console.error("Failed to toggle enabled:", err);
    }
  }

  /** Toggle bulk-select checkbox for a single row. */
  function cmdBulkToggle(filePath: string) {
    const next = new Set(cmdBulkSelected);
    if (next.has(filePath)) next.delete(filePath); else next.add(filePath);
    cmdBulkSelected = next;
  }

  /** Toggle select-all checkbox. */
  function cmdBulkToggleAll() {
    if (cmdBulkAllSelected) {
      cmdBulkSelected = new Set();
    } else {
      cmdBulkSelected = new Set(cmdFilteredList.map(c => c.file_path));
    }
  }

  /** Clear bulk selection. */
  function cmdBulkClear() {
    cmdBulkSelected = new Set();
    cmdBulkDeleteConfirm = false;
  }

  /** Bulk enable/disable all selected commands. */
  async function cmdBulkSetEnabled(enabled: boolean) {
    const items = cmdList.filter(c => cmdBulkSelected.has(c.file_path));
    for (const meta of items) {
      await cmdToggleEnabled(meta, enabled);
    }
    cmdBulkClear();
    await cmdRefreshList();
  }

  /** Bulk delete all selected commands (must confirm first). */
  async function cmdBulkDelete() {
    if (!cmdBulkDeleteConfirm) { cmdBulkDeleteConfirm = true; return; }
    const paths = [...cmdBulkSelected];
    for (const fp of paths) {
      try { await invoke("delete_command_file", { filePath: fp }); } catch {}
    }
    cmdBulkClear();
    cmdSelectedFile = null;
    await cmdRefreshList();
  }

  /** Scroll the settings content to a section. */
  function settingsScrollTo(sectionId: string) {
    settingsActiveNav = sectionId;
    const el = document.getElementById(`settings-section-${sectionId}`);
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  // Debug mode — toggled via /debug command, resets on app restart
  let debugMode = $state(false);
  // True when the list is showing debug log entries (special item selection)
  let showingDebugLog = $state(false);

  // Built-in /ctx commands — always present, titles reflect current activeContext
  const builtinCommands: Command[] = $derived([
    {
      phrase: "/ctx set",
      title: activeContext ? `Change context (current: "${activeContext}")` : "Set context",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "ctx_set" } },
    },
    {
      phrase: "/ctx reset",
      title: "Reset context",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "ctx_reset" } },
    },
    {
      phrase: "/docs skill",
      title: "How to deploy the Copilot skill to your project",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "docs_open", url: "https://github.com/surdy/nimble/blob/main/docs/guides/deploying-skill.md" } },
    },
    {
      phrase: "/docs commands",
      title: "How to configure YAML commands",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "docs_open", url: "https://github.com/surdy/nimble/blob/main/docs/guides/configuring-commands.md" } },
    },
    {
      phrase: "/docs scripts",
      title: "How to write scripts for dynamic lists and script actions",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "docs_open", url: "https://github.com/surdy/nimble/blob/main/docs/guides/writing-scripts.md" } },
    },
    {
      phrase: "/docs actions",
      title: "All six action types — open_url, paste_text, copy_text, static_list, dynamic_list, script_action",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "docs_open", url: "https://github.com/surdy/nimble/blob/main/docs/actions/README.md" } },
    },
    {
      phrase: "/docs contexts",
      title: "Contexts — ambient value for scripts via /ctx set and /ctx reset",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "docs_open", url: "https://github.com/surdy/nimble/blob/main/docs/guides/contexts.md" } },
    },
    {
      phrase: "/deploy copilot skill",
      title: "Deploy the nimble-authoring Copilot skill to ~/.copilot/skills/",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "deploy_skill" } },
    },
    {
      phrase: "/settings",
      title: "Open Nimble settings",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "open_settings" } },
    },
    {
      phrase: "/commands",
      title: "Open command editor",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "open_commands" } },
    },
    {
      phrase: "/debug",
      title: debugMode ? "Turn off debug mode" : "Turn on debug mode",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "toggle_debug" } },
    },
    {
      phrase: "/debug log",
      title: "View debug log",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "show_debug_log" } },
    },
    {
      phrase: "/debug log open",
      title: "Open debug log in editor",
      env: {}, source_dir: "",
      action: { type: "builtin", config: { action: "open_debug_log" } },
    },
  ]);

  // List expansion state — populated when input exactly matches a static_list phrase
  let listItems = $state<ListItem[]>([]);
  let activeListCmd = $state<Command | null>(null);
  let dynamicListLoaded = $state(false); // true once a dynamic_list invoke has resolved
  // Loading state for slow-script feedback: "idle" | "running" | "slow" | "very_slow"
  let scriptLoadingState = $state<"idle" | "running" | "slow" | "very_slow">("idle");
  // Inline error from script_action execution (shown as an error row in the results area)
  let actionError = $state<string | null>(null);
  let resultsEl: HTMLDivElement | undefined = $state();

  // Fuzzy filter state: unfiltered results from script/list, and the phrase they belong to.
  // After a list loads, further typing fuzzy-filters `fullListItems` into `listItems`.
  let fullListItems = $state<ListItem[]>([]);
  let listLoadedPhrase = $state<string | null>(null); // tracks which command phrase the list was loaded for
  let scriptInvokedArg = $state(""); // the arg passed to the script (for arg:required); fuzzy filter is relative to this
  // The active context at the time the script was invoked. Only consulted for
  // arg:context dynamic_list commands: their results depend on NIMBLE_CONTEXT,
  // so a context change (deep link, chip clear, ctx_set) invalidates the cache.
  let scriptInvokedContext = $state("");

  // For arg:required / arg:context dynamic_list: when non-null, the user has
  // pressed Tab/→ to "commit" this string as the script's arg. Subsequent
  // typing after `committedArg + " "` is treated as a client-side fuzzy filter
  // rather than a new arg (no script re-invocation). Backspacing past the
  // committed arg releases the lock and re-enables live re-invocation.
  let committedArg = $state<string | null>(null);

  // Auto-release the lock whenever we leave a dynamic_list arg:required /
  // arg:context match.
  $effect(() => {
    if (committedArg === null) return;
    if (activeListCmd && activeListCmd.action.type === "dynamic_list") {
      const mode = activeListCmd.action.config.arg ?? "none";
      if (mode === "required" || mode === "context") return;
    }
    committedArg = null;
  });

  // ── Filtering & navigation ─────────────────────────────────────────────
  const MAX_RESULTS = 8;
  const ROW_H = 56; // px per result row



  // Trimmed typed input — the sole source for command matching and params.
  // The active context never influences matching; it is ambient-only and
  // scripts read it via the NIMBLE_CONTEXT env var.
  const typedInput = $derived(input.trim());

  const filtered = $derived(filterCommands(commands, typedInput));

  // Built-in / commands filtered by the current raw input (only when input starts with "/")
  const filteredBuiltins: Command[] = $derived(
    input.trim().startsWith("/") ? filterCommands(builtinCommands, input.trim()) : []
  );

  // Combined results: built-ins first, then YAML commands
  const allFiltered = $derived([...filteredBuiltins, ...filtered]);

  // True when the typed input matches a list command and we should show list UI.
  // Includes the empty-resolved state for dynamic lists ("No results" feedback),
  // the in-progress state while the script is executing,
  // and when fuzzy filtering produces zero matches (show "No matches for filter").
  const showingList = $derived(
    activeListCmd !== null && (
      listItems.length > 0 ||
      fullListItems.length > 0 ||
      (activeListCmd.action.type === "dynamic_list" && (dynamicListLoaded || scriptLoadingState !== "idle"))
    )
  );

  let selectedIndex = $state(0);

  // Reset selection whenever the result list changes
  $effect(() => {
    void allFiltered;
    selectedIndex = 0;
    actionError = null;
  });

  // Scroll the selected row into view when navigating with arrow keys
  $effect(() => {
    if (resultsEl) {
      const row = resultsEl.children[selectedIndex] as HTMLElement | undefined;
      row?.scrollIntoView({ block: "nearest" });
    }
  });

  // Detect exact-phrase match for static_list / dynamic_list commands and load items.
  // Returns a cleanup that cancels any in-flight debounce timer.
  $effect(() => {
    const typed = typedInput.toLowerCase();

    // ── static_list: exact match or prefix (phrase + space + filter) ───
    const staticMatch = commands.find(
      cmd => cmd.action.type === "static_list" && cmd.phrase.toLowerCase() === typed
    ) ?? null;

    // Also detect prefix match for static list filtering (phrase + space + extra text)
    const staticPrefixMatch = staticMatch ? null : (
      commands.find(cmd => {
        if (cmd.action.type !== "static_list") return false;
        return typed.startsWith(cmd.phrase.toLowerCase() + " ");
      }) ?? null
    );

    const staticCmd = staticMatch ?? staticPrefixMatch;

    if (staticCmd && staticCmd.action.type === "static_list") {
      const phrase = staticCmd.phrase.toLowerCase();

      if (staticMatch) {
        // Exact match — load the list (or use cached if already loaded for this phrase)
        if (listLoadedPhrase !== phrase) {
          const listName = staticCmd.action.config.list;
          const commandDir = staticCmd.source_dir;
          activeListCmd = staticCmd;
          listLoadedPhrase = phrase;
          invoke<ListItem[]>("load_list", { commandDir, listName, inlineEnv: staticCmd.env, context: activeContext, phrase: staticCmd.phrase })
            .then(items => { fullListItems = items; listItems = items; selectedIndex = 0; })
            .catch((err) => {
              fullListItems = [];
              listItems = [{ title: "⚠️ Error loading list", subtext: String(err) }];
            });
        } else {
          // Already loaded — show unfiltered
          listItems = fullListItems;
          selectedIndex = 0;
          activeListCmd = staticCmd;
        }
      } else {
        // Prefix match — fuzzy-filter the already-loaded list
        if (listLoadedPhrase === phrase && fullListItems.length > 0) {
          const suffix = typed.slice(phrase.length + 1).trim();
          activeListCmd = staticCmd;
          listItems = fuzzyFilterListItems(fullListItems, suffix);
          selectedIndex = 0;
        } else {
          // List not loaded yet — load it first, then filter
          const listName = staticCmd.action.config.list;
          const commandDir = staticCmd.source_dir;
          activeListCmd = staticCmd;
          listLoadedPhrase = phrase;
          const suffix = typed.slice(phrase.length + 1).trim();
          invoke<ListItem[]>("load_list", { commandDir, listName, inlineEnv: staticCmd.env, context: activeContext, phrase: staticCmd.phrase })
            .then(items => { fullListItems = items; listItems = fuzzyFilterListItems(items, suffix); selectedIndex = 0; })
            .catch((err) => {
              fullListItems = [];
              listItems = [{ title: "⚠️ Error loading list", subtext: String(err) }];
            });
        }
      }
      return;
    }

    // ── dynamic_list: exact match OR phrase + space + suffix ───────────
    // Prefer exact-phrase match over prefix match so that "channels internal"
    // finds the longer command before the shorter "channels" prefix.
    // Among prefix matches, prefer the longest phrase (most specific command).
    const dynMatch = (
      commands.find(cmd => cmd.action.type === "dynamic_list" && cmd.phrase.toLowerCase() === typed) ??
      commands
        .filter(cmd => {
          if (cmd.action.type !== "dynamic_list") return false;
          return typed.startsWith(cmd.phrase.toLowerCase() + " ");
        })
        .sort((a, b) => b.phrase.length - a.phrase.length)[0]
    ) ?? null;

    if (dynMatch && dynMatch.action.type === "dynamic_list") {
      const phrase = dynMatch.phrase.toLowerCase();
      const config = dynMatch.action.config;
      const isExact = typed === phrase;
      const suffix = typed.startsWith(phrase + " ") ? typed.slice(phrase.length + 1).trim() : "";
      const argMode = config.arg ?? "none";
      // "context" is required-LIKE: a typed suffix IS the script's arg (never
      // a client-side filter), so it shares all of arg:required's guards. The
      // difference is that with no suffix an active context satisfies the
      // requirement (invoke with arg null; the script reads NIMBLE_CONTEXT).
      const requiredLike = argMode === "required" || argMode === "context";
      // For arg:context, results loaded under a different active context are
      // stale — the script's output depends on NIMBLE_CONTEXT, and the context
      // can change without the input being retyped (deep link, chip clear).
      const contextFresh = argMode !== "context" || scriptInvokedContext === activeContext;

      // When the match is via prefix (not exact), check if the typed text
      // also partially matches a longer command phrase that extends this one.
      // If so, don't trigger the list — show both as normal filtered results
      // so the user can choose between the shorter (param) and longer (phrase) command.
      if (!isExact) {
        const hasLongerPhraseMatch = commands.some(cmd => {
          if (cmd === dynMatch) return false;
          const p = cmd.phrase.toLowerCase();
          return p.startsWith(phrase + " ") && p.includes(typed);
        });
        if (hasLongerPhraseMatch) {
          activeListCmd = null;
          listItems = [];
          dynamicListLoaded = false;
          scriptLoadingState = "idle";
          return;
        }
      }

      // For arg:none and arg:optional: if results are already loaded for this
      // phrase, fuzzy-filter client-side instead of re-invoking the script.
      // For arg:required / arg:context, the suffix IS the script's arg — we
      // must re-invoke when the suffix changes (handled below).
      if (!requiredLike && listLoadedPhrase === phrase && dynamicListLoaded) {
        activeListCmd = dynMatch;
        listItems = fuzzyFilterListItems(fullListItems, suffix);
        selectedIndex = 0;
        return;
      }

      // For arg:required / arg:context, decide what is the "effective arg"
      // sent to the script vs the client-side fuzzy filter applied on top of
      // cached results. When committedArg is set, the suffix is split:
      //   <committedArg> [<filter text>]
      // and the script is NOT re-invoked while the locked prefix matches.
      let requiredArg = suffix;
      let requiredFilter = "";
      if (requiredLike && committedArg !== null) {
        if (suffix === committedArg) {
          requiredArg = committedArg;
          requiredFilter = "";
        } else if (suffix.startsWith(committedArg + " ")) {
          requiredArg = committedArg;
          requiredFilter = suffix.slice(committedArg.length + 1).trim();
        } else {
          // Backspaced past (or diverged from) the locked arg — release it
          // and treat the new suffix as a fresh script arg. Mutating state
          // here re-triggers this $effect, which will re-enter and take
          // the unlocked path below.
          committedArg = null;
          return;
        }
      }

      // For arg:required / arg:context, if the script has already been
      // invoked for the effective arg (and, for arg:context, under the
      // current active context), just fuzzy-filter the cached results.
      if (
        requiredLike &&
        listLoadedPhrase === phrase &&
        dynamicListLoaded &&
        scriptInvokedArg === requiredArg &&
        contextFresh
      ) {
        activeListCmd = dynMatch;
        listItems = requiredFilter
          ? fuzzyFilterListItems(fullListItems, requiredFilter)
          : fullListItems;
        selectedIndex = 0;
        return;
      }

      // For arg:required / arg:context, if a script is currently running for
      // the effective arg (and current context), wait for it to resolve.
      if (
        requiredLike &&
        listLoadedPhrase === phrase &&
        scriptLoadingState !== "idle" &&
        scriptInvokedArg === requiredArg &&
        contextFresh
      ) {
        activeListCmd = dynMatch;
        return;
      }

      // If a script is already running for this phrase (and the suffix hasn't
      // changed for arg:required/context), wait for the promise to resolve.
      // (runDynamic sets listLoadedPhrase and scriptLoadingState synchronously,
      // which re-triggers this $effect; without this guard, the fall-through
      // would schedule another invocation.)
      if (!requiredLike && listLoadedPhrase === phrase && scriptLoadingState !== "idle") {
        activeListCmd = dynMatch;
        return;
      }

      // Script not yet invoked for this phrase — decide whether to invoke now.
      let timer: ReturnType<typeof setTimeout> | null = null;

      const commandDir = dynMatch.source_dir;

      const runDynamic = (arg: string | null) => {
        dynamicListLoaded = false;
        listLoadedPhrase = phrase;
        scriptInvokedArg = arg ?? "";
        scriptInvokedContext = activeContext;
        scriptLoadingState = "running";
        let slowTimer = setTimeout(() => { scriptLoadingState = "slow"; }, 2000);
        let verySlowTimer = setTimeout(() => { scriptLoadingState = "very_slow"; }, 4000);
        invoke<ListItem[]>("run_dynamic_list", { commandDir, scriptName: config.script, arg, context: activeContext, phrase: dynMatch.phrase, inlineEnv: dynMatch.env })
          .then(items => {
            clearTimeout(slowTimer); clearTimeout(verySlowTimer);
            scriptLoadingState = "idle";
            fullListItems = items;
            dynamicListLoaded = true;
            // For arg:required / arg:context, the script's results are
            // already filtered by the arg — apply client-side fuzzy filtering
            // only if the user has committed an arg (locked) and is now typing
            // extra filter text after it. For arg:none/optional, always
            // fuzzy-filter using the current suffix.
            if (requiredLike) {
              const currentTyped = typedInput.toLowerCase();
              const currentSuffix = currentTyped.startsWith(phrase + " ") ? currentTyped.slice(phrase.length + 1).trim() : "";
              let filterText = "";
              if (committedArg !== null) {
                if (currentSuffix === committedArg) {
                  filterText = "";
                } else if (currentSuffix.startsWith(committedArg + " ")) {
                  filterText = currentSuffix.slice(committedArg.length + 1).trim();
                }
              }
              listItems = filterText ? fuzzyFilterListItems(items, filterText) : items;
            } else {
              const currentTyped = typedInput.toLowerCase();
              const currentSuffix = currentTyped.startsWith(phrase + " ") ? currentTyped.slice(phrase.length + 1).trim() : "";
              listItems = fuzzyFilterListItems(items, currentSuffix);
            }
            selectedIndex = 0;
          })
          .catch((err) => {
            clearTimeout(slowTimer); clearTimeout(verySlowTimer);
            scriptLoadingState = "idle";
            fullListItems = [];
            listItems = [{ title: "⚠️ Script error", subtext: String(err) }];
            dynamicListLoaded = true;
          });
      };

      if (argMode === "none") {
        if (isExact || suffix !== "") {
          activeListCmd = dynMatch;
          runDynamic(null);
        } else {
          activeListCmd = null;
          listItems = [];
          dynamicListLoaded = false;
          scriptLoadingState = "idle";
        }
      } else if (argMode === "optional") {
        activeListCmd = dynMatch;
        runDynamic(null);
      } else if (argMode === "context" && !requiredArg) {
        // context mode with no typed suffix: an active context satisfies the
        // requirement — invoke immediately (nothing is being typed, so no
        // debounce) with arg null; the script reads NIMBLE_CONTEXT. Without
        // a context the requirement is unmet: no invocation, no list.
        if (activeContext.trim() !== "") {
          activeListCmd = dynMatch;
          runDynamic(null);
        } else {
          activeListCmd = null;
          listItems = [];
          dynamicListLoaded = false;
          scriptLoadingState = "idle";
        }
      } else {
        // required (or context with a typed suffix): only invoke when the
        // effective arg is non-empty, debounced because it is being typed.
        // (When committedArg is set, requiredArg === committedArg and the
        // earlier cache/in-flight guards usually short-circuit before here.)
        if (requiredArg) {
          activeListCmd = dynMatch;
          const argToRun = requiredArg;
          timer = setTimeout(() => runDynamic(argToRun), 200);
        } else {
          activeListCmd = null;
          listItems = [];
          dynamicListLoaded = false;
          scriptLoadingState = "idle";
        }
      }

      return () => { if (timer !== null) clearTimeout(timer); };
    }

    // No list match — reset everything including fuzzy filter state
    activeListCmd = null;
    listItems = [];
    fullListItems = [];
    listLoadedPhrase = null;
    scriptInvokedArg = "";
    scriptInvokedContext = "";
    dynamicListLoaded = false;
    scriptLoadingState = "idle";
    showingDebugLog = false;
  });

  // Resize window to fit current results (skip during onboarding or settings)
  $effect(() => {
    if (onboarding || showSettings) return;
    const hasQuery = input.trim() !== "";
    const WARNING_H = 40;
    const warnExtra = warningVisible ? WARNING_H : 0;
    const hasErrorItem = (showingList && listItems.some(it => it.title.startsWith("\u26a0"))) || actionError !== null;
    const listRowCount = showingList ? (listItems.length > 0 ? Math.min(listItems.length, MAX_RESULTS) : 1) : 0;
    const contentHeight = !hasQuery ? 0
      : showingList ? listRowCount * ROW_H
      : actionError ? 44                       // action error row (measured below)
      : allFiltered.length === 0 ? 44          // "no matching commands" row
      : Math.min(allFiltered.length, MAX_RESULTS) * ROW_H;
    if (hasErrorItem) {
      // Error items wrap text, so measure actual rendered height after DOM update
      tick().then(() => {
        const measured = resultsEl?.scrollHeight ?? contentHeight;
        appWindow.setSize(new LogicalSize(640, 64 + warnExtra + measured));
      });
    } else {
      appWindow.setSize(new LogicalSize(640, 64 + warnExtra + contentHeight));
    }
  });



  const LAUNCHER_SIZE  = new LogicalSize(640, 64);
  const ONBOARDING_SIZE = new LogicalSize(480, 240);

  // ── Helpers ────────────────────────────────────────────────────────────
  // Used for blur and programmatic hides (no focus restoration needed —
  // either the OS already moved focus elsewhere, or there is no previous app).
  function dismiss() {
    input = "";
    invoke("hide_window").catch(() => appWindow.hide());
  }

  // Used for intentional user dismissal via Escape.
  // Hides the window AND restores focus to the previously active application.
  function dismissWithFocusRestore() {
    input = "";
    invoke("dismiss_launcher").catch(() => appWindow.hide());
  }

  // ── Preferences window helpers ─────────────────────────────────────────
  // Open the unified Preferences window on a specific tab, then dismiss the launcher.
  function openPreferences(tab: "commands" | "settings") {
    invoke("open_preferences_window", { tab }).catch(() => {});
    dismiss();
  }

  // ── Command editor helpers ─────────────────────────────────────────────
  async function cmdRefreshList() {
    cmdList = await invoke<CommandFileMeta[]>("list_command_files").catch(() => []);
    cmdFolders = await invoke<string[]>("list_command_folders").catch(() => []);
    // Also refresh load-time warnings so the prefs Commands tab stays in sync.
    const payload = await invoke<CommandsPayload>("list_commands").catch(() => ({ commands: [], duplicates: [], reserved: [], skipped: [], warnings: [] }));
    skippedWarnings = payload.skipped ?? [];
    commandWarnings = payload.warnings ?? [];
  }

  function cmdSelectItem(meta: CommandFileMeta) {
    cmdSelectedFile = meta.file_path;
    cmdIsNew = false;
    cmdPhrase = meta.phrase;
    cmdTitle = meta.title;
    cmdEnabled = meta.enabled;
    cmdSaveError = "";
    cmdDeleteConfirm = false;
    cmdScriptContent = "";
    cmdScriptExists = false;
    cmdScriptDirty = false;
    cmdScriptError = "";
    cmdTestResult = null;
    if (meta.action_type === "open_url" || meta.action_type === "paste_text" || meta.action_type === "copy_text" || meta.action_type === "static_list" || meta.action_type === "dynamic_list" || meta.action_type === "script_action") {
      cmdActionType = meta.action_type;
    } else {
      cmdActionType = "open_url";
    }
    cmdUrl = "";
    cmdText = "";
    cmdListName = "";
    cmdItemAction = "";
    cmdScript = "";
    cmdArgMode = "none";
    cmdResultAction = "paste_text";
    cmdPrefix = "";
    cmdSuffix = "";
    cmdTestArg = "";
    cmdTestResult = null;
    cmdInlineEnv = {};
    // Load full config from backend to populate form fields
    invoke<{ commands: import("$lib/types").Command[] }>("list_commands")
      .then(result => {
        const full = result.commands.find(c => c.phrase.toLowerCase() === meta.phrase.toLowerCase());
        if (!full) return;
        cmdInlineEnv = full.env ?? {};
        if (full.action.type === "open_url") cmdUrl = full.action.config.url;
        else if (full.action.type === "paste_text") cmdText = full.action.config.text;
        else if (full.action.type === "copy_text") cmdText = full.action.config.text;
        else if (full.action.type === "static_list") {
          cmdListName = full.action.config.list;
          cmdItemAction = full.action.config.item_action ?? "";
        } else if (full.action.type === "dynamic_list") {
          cmdScript = full.action.config.script;
          cmdArgMode = full.action.config.arg ?? "none";
          cmdItemAction = full.action.config.item_action ?? "";
          cmdLoadScript(meta.source_dir, full.action.config.script);
        } else if (full.action.type === "script_action") {
          cmdScript = full.action.config.script;
          cmdArgMode = full.action.config.arg ?? "none";
          cmdResultAction = full.action.config.result_action;
          cmdPrefix = full.action.config.prefix ?? "";
          cmdSuffix = full.action.config.suffix ?? "";
          cmdLoadScript(meta.source_dir, full.action.config.script);
        }
      })
      .catch(() => {});
  }

  /** Load a script's content for the inline editor (co-located or shared:). */
  async function cmdLoadScript(commandDir: string, scriptName: string) {
    // Skip loading for legacy ${VAR} scripts (no longer supported)
    if (scriptName.includes("${")) {
      cmdScriptContent = "";
      cmdScriptExists = false;
      cmdScriptDirty = false;
      return;
    }
    cmdScriptLoading = true;
    cmdScriptError = "";
    try {
      const content = await invoke<string>("read_script_file", {
        commandDir,
        scriptName,
      });
      cmdScriptContent = content;
      cmdScriptExists = true;
      cmdScriptDirty = false;
    } catch (err) {
      if (String(err) === "not_found") {
        cmdScriptContent = "";
        cmdScriptExists = false;
      } else {
        cmdScriptError = String(err);
      }
    } finally {
      cmdScriptLoading = false;
    }
  }

  /** Save the inline script editor content to disk. */
  async function cmdSaveScript() {
    if (!cmdScript.trim() || cmdScriptIsExternal) return;
    // Determine the command_dir from the currently selected file or target_dir
    const commandDir = cmdIsNew
      ? (cmdTargetDir || "")
      : (cmdList.find(c => c.file_path === cmdSelectedFile)?.source_dir ?? "");
    cmdScriptSaving = true;
    cmdScriptError = "";
    try {
      await invoke("write_script_file", {
        commandDir,
        scriptName: cmdScript.trim(),
        content: cmdScriptContent,
      });
      cmdScriptExists = true;
      cmdScriptDirty = false;
    } catch (err) {
      cmdScriptError = String(err);
    } finally {
      cmdScriptSaving = false;
    }
  }

  /** Run the script in test mode and capture raw stdout/stderr/exit code. */
  async function cmdRunTest() {
    if (!cmdScript.trim() || cmdScriptIsExternal) return;
    const commandDir = cmdIsNew
      ? (cmdTargetDir || "")
      : (cmdList.find(c => c.file_path === cmdSelectedFile)?.source_dir ?? "");
    cmdTestRunning = true;
    cmdTestResult = null;
    try {
      const result = await invoke<import("$lib/types").ScriptTestResult>("test_script", {
        commandDir,
        scriptName: cmdScript.trim(),
        arg: cmdTestArg.trim() || undefined,
        inlineEnv: cmdInlineEnv,
      });
      cmdTestResult = result;
    } catch (err) {
      cmdTestResult = { stdout: "", stderr: String(err), exit_code: null, duration_ms: 0, timed_out: false };
    } finally {
      cmdTestRunning = false;
    }
  }

  /** Generate a starter script template for a new command. */
  function cmdGetScriptTemplate(actionType: string): string {
    if (actionType === "dynamic_list") {
      return `#!/bin/bash
# Dynamic list script — output JSON array of { title, subtext } objects
# or plain text (one item per line)

echo '[
  { "title": "Item 1", "subtext": "Description" },
  { "title": "Item 2", "subtext": "Another item" }
]'
`;
    }
    return `#!/bin/bash
# Script action — output one value per line
# Each value is passed to the result_action (paste_text, copy_text, or open_url)

echo "Hello, world!"
`;
  }

  /** Create a new script from a template. */
  async function cmdCreateScript() {
    if (!cmdScript.trim() || cmdScriptIsExternal) return;
    cmdScriptContent = cmdGetScriptTemplate(cmdActionType);
    cmdScriptDirty = true;
  }

  function cmdStartNew() {
    cmdSelectedFile = null;
    cmdIsNew = true;
    cmdPhrase = "";
    cmdTitle = "";
    cmdEnabled = true;
    cmdActionType = "open_url";
    cmdUrl = "";
    cmdText = "";
    cmdListName = "";
    cmdItemAction = "";
    cmdScript = "";
    cmdArgMode = "none";
    cmdResultAction = "paste_text";
    cmdPrefix = "";
    cmdSuffix = "";
    cmdTestArg = "";
    cmdTestResult = null;
    cmdSaveError = "";
    cmdDeleteConfirm = false;
    cmdTargetDir = "";
    cmdNewFolderName = "";
    cmdShowNewFolder = false;
    cmdScriptContent = "";
    cmdScriptExists = false;
    cmdScriptDirty = false;
    cmdScriptError = "";
    cmdInlineEnv = {};
  }

  function cmdCancelNew() {
    cmdIsNew = false;
    cmdSelectedFile = null;
    cmdSaveError = "";
  }

  async function cmdSave() {
    if (!cmdCanSave) return;
    cmdSaving = true;
    cmdSaveError = "";
    let configJson: string;
    if (cmdActionType === "open_url") {
      configJson = JSON.stringify({ url: cmdUrl.trim() });
    } else if (cmdActionType === "paste_text" || cmdActionType === "copy_text") {
      configJson = JSON.stringify({ text: cmdText });
    } else if (cmdActionType === "static_list") {
      configJson = JSON.stringify({
        list: cmdListName.trim(),
        ...(cmdItemAction ? { item_action: cmdItemAction } : {}),
      });
    } else if (cmdActionType === "dynamic_list") {
      configJson = JSON.stringify({
        script: cmdScript.trim(),
        arg: cmdArgMode,
        ...(cmdItemAction ? { item_action: cmdItemAction } : {}),
      });
    } else {
      // script_action
      configJson = JSON.stringify({
        script: cmdScript.trim(),
        arg: cmdArgMode,
        result_action: cmdResultAction,
        ...(cmdPrefix ? { prefix: cmdPrefix } : {}),
        ...(cmdSuffix ? { suffix: cmdSuffix } : {}),
      });
    }
    try {
      // Determine target directory for new commands
      const effectiveTargetDir = cmdIsNew
        ? (cmdShowNewFolder && cmdNewFolderName.trim() ? cmdNewFolderName.trim() : cmdTargetDir)
        : undefined;
      const newPath = await invoke<string>("save_command_file", {
        phrase: cmdPhrase.trim(),
        title: cmdTitle.trim() || cmdPhrase.trim(),
        enabled: cmdEnabled,
        actionType: cmdActionType,
        configJson,
        filePath: cmdSelectedFile ?? undefined,
        targetDir: effectiveTargetDir,
      });
      // Save script content if dirty (for dynamic_list / script_action)
      if (cmdScriptDirty && cmdScript.trim() && !cmdScriptIsExternal) {
        const saveDir = effectiveTargetDir ?? (cmdList.find(c => c.file_path === newPath)?.source_dir ?? "");
        try {
          await invoke("write_script_file", {
            commandDir: saveDir,
            scriptName: cmdScript.trim(),
            content: cmdScriptContent,
          });
          cmdScriptDirty = false;
          cmdScriptExists = true;
        } catch (err) {
          cmdScriptError = String(err);
        }
      }
      cmdSelectedFile = newPath;
      cmdIsNew = false;
      await cmdRefreshList();
      // Re-select so the sidebar highlights the saved item
      const meta = cmdList.find(c => c.file_path === newPath);
      if (meta) cmdSelectItem(meta);
    } catch (err) {
      cmdSaveError = String(err);
    } finally {
      cmdSaving = false;
    }
  }

  async function cmdDelete() {
    if (!cmdSelectedFile) return;
    if (!cmdDeleteConfirm) { cmdDeleteConfirm = true; return; }
    try {
      await invoke("delete_command_file", { filePath: cmdSelectedFile });
      cmdSelectedFile = null;
      cmdIsNew = false;
      cmdDeleteConfirm = false;
      await cmdRefreshList();
    } catch (err) {
      cmdSaveError = String(err);
      cmdDeleteConfirm = false;
    }
  }

  async function closeSettings() {
    // In the preferences window: save and close the whole window.
    if (isPreferencesWindow) {
      await persistSettings();
      appWindow.close();
      return;
    }
    // Fallback (should not normally be reached after the refactor).
    showSettings = false;
    await appWindow.setSize(LAUNCHER_SIZE);
    dismiss();
  }

  async function restoreDefaults() {
    settingsShowContextChip = true;
    settingsAllowDuplicates = true;
    settingsSharedDir = "shared";
    settingsCommandsDir = "";
    await persistSettings();
  }

  function flashSaved() {
    if (settingsSavedTimer !== null) clearTimeout(settingsSavedTimer);
    settingsSaved = true;
    settingsSavedTimer = setTimeout(() => { settingsSaved = false; }, 1500);
  }

  async function browseCommandsDir() {
    const selected = await invoke<string | null>("browse_directory", {
      defaultPath: settingsCommandsDir.trim() || null,
    });
    if (selected) {
      settingsCommandsDir = selected;
      await persistSettings();
    }
  }

  function clearCommandsDir() {
    settingsCommandsDir = "";
    persistSettings();
  }



  async function persistSettings() {
    const dir = settingsCommandsDir.trim() || undefined;
    try {
      await invoke("save_settings", {
        showContextChip: settingsShowContextChip,
        allowDuplicates: settingsAllowDuplicates,
        sharedDir: settingsSharedDir,
        commandsDir: dir ?? null,
      });
      // Keep currentSettings in sync
      currentSettings = {
        ...currentSettings,
        show_context_chip: settingsShowContextChip,
        allow_duplicates: settingsAllowDuplicates,
        shared_dir: settingsSharedDir,
        commands_dir: dir,
      };
      // Apply show_context_chip immediately
      showContextChip = settingsShowContextChip;
      // Notify the launcher window so it can reload commands / refresh state.
      emit("settings-changed").catch(() => {});
      flashSaved();
    } catch {
      // Silently ignore — settings.yaml write failures are non-critical
    }
  }

  function handleSettingsKeydown(e: KeyboardEvent) {
    if (settingsChangingHotkey) {
      e.preventDefault();
      settingsHotkeyError = "";
      const shortcut = eventToShortcut(e);
      if (shortcut) settingsCapturedShortcut = shortcut;
    } else if (e.key === "Escape") {
      e.preventDefault();
      closeSettings();
    }
  }

  async function confirmSettingsHotkey() {
    if (!settingsCapturedShortcut) return;
    try {
      await invoke("register_shortcut", { shortcut: settingsCapturedShortcut });
      await invoke("save_hotkey", { hotkey: settingsCapturedShortcut });
      currentSettings = { ...currentSettings, hotkey: settingsCapturedShortcut };
      settingsChangingHotkey = false;
      flashSaved();
    } catch (err) {
      settingsHotkeyError = `Could not register shortcut: ${err}`;
      settingsCapturedShortcut = "";
    }
  }



  // ── Onboarding key capture ─────────────────────────────────────────────
  function handleOnboardingKeydown(e: KeyboardEvent) {
    e.preventDefault();
    shortcutError = "";
    const shortcut = eventToShortcut(e);
    if (shortcut) capturedShortcut = shortcut;
  }

  async function confirmShortcut() {
    if (!capturedShortcut) return;
    try {
      await invoke("register_shortcut", { shortcut: capturedShortcut });
      await invoke("save_hotkey", { hotkey: capturedShortcut }).catch(() => {});
      onboarding = false;
      await appWindow.setSize(LAUNCHER_SIZE);
      // Load commands now that onboarding is complete
      const result = await invoke<CommandsPayload>("list_commands").catch(() => ({ commands: [], duplicates: [], reserved: [], skipped: [], warnings: [] }));
      commands = result.commands;
      warnings = result.duplicates;
      reservedWarnings = result.reserved;
      skippedWarnings = result.skipped ?? [];
      commandWarnings = result.warnings ?? [];
      warningsDismissed = false;
      dismiss();
    } catch (err) {
      shortcutError = `Could not register shortcut: ${err}`;
      capturedShortcut = "";
    }
  }

  // ── Action execution ──────────────────────────────────────────────────
  async function executeListItem(item: ListItem) {
    // Error items: copy the error message to clipboard
    if (item.title.startsWith("\u26a0") && item.subtext) {
      await invoke("copy_text", { text: item.subtext });
      return;
    }
    // Debug log: first item opens the log file in the system editor
    if (showingDebugLog) {
      if (item.title.startsWith("📂")) {
        await invoke("open_debug_log").catch(() => {});
      }
      showingDebugLog = false;
      input = "";
      invoke("dismiss_launcher").catch(() => appWindow.hide());
      return;
    }
    const value = item.subtext ?? item.title;
    const itemAction =
      activeListCmd?.action.type === "static_list"
        ? activeListCmd.action.config.item_action
        : activeListCmd?.action.type === "dynamic_list"
        ? activeListCmd.action.config.item_action
        : undefined;
    input = "";
    if (itemAction === "paste_text") {
      await invoke("paste_text", { text: value });
    } else if (itemAction === "copy_text") {
      await invoke("copy_text", { text: value });
    } else if (itemAction === "open_url") {
      await invoke("open_url", { url: value, param: null, context: activeContext });
      dismiss();
    } else if (itemAction === "ctx_set") {
      activeContext = value;
      // do NOT dismiss — launcher stays open so the user sees the updated context
    } else {
      // No action configured — just dismiss
      invoke("dismiss_launcher").catch(() => appWindow.hide());
    }
  }

  async function executeCommand(cmd: Command) {
    if (cmd.action.type === "open_url") {
      // Extract any text typed after the command phrase as the param.
      // Only what the user explicitly typed becomes the param — the active
      // context is ambient-only and never injected here.
      const phrase = cmd.phrase.toLowerCase();
      const typed  = typedInput;
      const after  = typed.toLowerCase().startsWith(phrase)
        ? typed.slice(phrase.length).trim()
        : "";
      await invoke("open_url", {
        url:     cmd.action.config.url,
        param:   after !== "" ? after : null,
        context: activeContext,
      });
      dismiss();
    } else if (cmd.action.type === "paste_text") {
      // Rust command handles window hide + focus restore + clipboard + keystroke.
      // We clear input here so the bar is clean when the launcher is next shown.
      input = "";
      await invoke("paste_text", { text: cmd.action.config.text });
    } else if (cmd.action.type === "copy_text") {
      // Rust command writes to clipboard and hides the launcher.
      // No paste keystroke — the user pastes manually.
      input = "";
      await invoke("copy_text", { text: cmd.action.config.text });
    } else if (cmd.action.type === "script_action") {
      const cfg = cmd.action.config;
      const phrase = cmd.phrase.toLowerCase();
      // Script args contain only what the user explicitly typed after the
      // phrase; the active context reaches scripts via NIMBLE_CONTEXT only.
      const typed  = typedInput;
      const after  = typed.toLowerCase().startsWith(phrase)
        ? typed.slice(phrase.length).trim()
        : "";

      // Determine the argument to pass based on arg mode.
      let scriptArg: string | null = null;
      if (cfg.arg === "optional" && after !== "") {
        scriptArg = after;
      } else if (cfg.arg === "required") {
        if (after === "") return; // can't execute without a required argument
        scriptArg = after;
      } else if (cfg.arg === "context") {
        // Required, but an active context satisfies the requirement.
        // A typed suffix overrides and is passed as the arg; with no suffix
        // the command fires only when a context is set, passing arg = null so
        // the script reads NIMBLE_CONTEXT. The context value is NEVER passed
        // as the positional arg.
        if (after !== "") {
          scriptArg = after;
        } else if (activeContext.trim() === "") {
          return; // no suffix and no context: refuse
        }
        // else: no suffix, context set — scriptArg stays null
      }
      // arg === "none" (or absent): scriptArg stays null

      let values: string[];
      try {
        values = await invoke("run_script_action", {
          commandDir: cmd.source_dir,
          scriptName: cfg.script,
          arg: scriptArg,
          context: activeContext,
          phrase: cmd.phrase,
          inlineEnv: cmd.env,
        });
      } catch (err) {
        actionError = String(err);
        return;
      }

      if (cfg.result_action === "open_url") {
        for (const v of values) {
          await invoke("open_url", { url: v, param: null, context: activeContext });
        }
        dismiss();
      } else {
        // paste_text or copy_text: wrap each value with prefix/suffix and join into one string.
        const text = values
          .map(v => (cfg.prefix ?? "") + v + (cfg.suffix ?? ""))
          .join("");
        input = "";
        await invoke(cfg.result_action === "paste_text" ? "paste_text" : "copy_text", { text });
      }
    } else if (cmd.action.type === "static_list" || cmd.action.type === "dynamic_list") {
      // If the user selected this command via partial match, fill in the full
      // phrase so the reactive effect detects the exact match and loads the list.
      if (input.toLowerCase() !== cmd.phrase.toLowerCase()) {
        input = cmd.phrase;
      }
    } else if (cmd.action.type === "builtin") {
      const builtinAction = cmd.action.config.action;
      if (builtinAction === "ctx_set") {
        const suffix = input.trim().toLowerCase().startsWith("/ctx set ")
          ? input.trim().slice("/ctx set ".length).trim()
          : "";
        if (suffix) activeContext = suffix;
        input = "";
        // do NOT dismiss — launcher stays open so the user sees the updated context
      } else if (builtinAction === "ctx_reset") {
        activeContext = "";
        input = "";
        // do NOT dismiss
      } else if (builtinAction === "docs_open" && cmd.action.config.url) {
        await invoke("open_url", { url: cmd.action.config.url, param: null, context: activeContext });
        input = "";
        dismissWithFocusRestore();
      } else if (builtinAction === "deploy_skill") {
        try {
          const msg = await invoke<string>("deploy_skill");
          input = "";
          // Show the result briefly as the input placeholder, then dismiss
          if (inputEl) inputEl.placeholder = msg;
          setTimeout(() => {
            if (inputEl) inputEl.placeholder = "";
            dismissWithFocusRestore();
          }, 2000);
        } catch (err) {
          input = "";
          if (inputEl) inputEl.placeholder = `Error: ${err}`;
          setTimeout(() => { if (inputEl) inputEl.placeholder = ""; }, 3000);
        }
      } else if (builtinAction === "open_settings") {
        openPreferences("settings");
      } else if (builtinAction === "open_commands") {
        openPreferences("commands");
      } else if (builtinAction === "toggle_debug") {
        try {
          const nowOn = await invoke<boolean>("toggle_debug");
          debugMode = nowOn;
          input = "";
          if (inputEl) inputEl.placeholder = nowOn ? "Debug mode ON" : "Debug mode OFF";
          setTimeout(() => { if (inputEl) inputEl.placeholder = ""; }, 2000);
        } catch (err) {
          input = "";
          if (inputEl) inputEl.placeholder = `Error: ${err}`;
          setTimeout(() => { if (inputEl) inputEl.placeholder = ""; }, 3000);
        }
      } else if (builtinAction === "show_debug_log") {
        try {
          const log = await invoke<string>("read_debug_log");
          input = "";
          const lines = log.trim().split("\n").filter((l: string) => l.length > 0);
          if (lines.length === 0) {
            listItems = [{ title: "Debug log is empty", subtext: "Turn on debug mode with /debug, then run some commands" }];
          } else {
            const openItem: ListItem = { title: "📂 Open debug log in editor", subtext: undefined };
            const logItems: ListItem[] = lines.reverse().map((line: string) => {
              const match = line.match(/^\[[\d:.]+\]\s*(.*)/);
              return { title: match ? match[1] : line, subtext: undefined };
            });
            listItems = [openItem, ...logItems];
          }
          selectedIndex = 0;
          showingDebugLog = true;
        } catch (err) {
          listItems = [{ title: "⚠️ Error reading debug log", subtext: String(err) }];
        }
      } else if (builtinAction === "open_debug_log") {
        await invoke("open_debug_log").catch(() => {});
        input = "";
        dismissWithFocusRestore();
      }
    }
  }

  // ── Launcher key handling ──────────────────────────────────────────────

  /**
   * Commit the current suffix as the locked arg for an arg:required (or
   * arg:context) dynamic_list. Returns true when a commit happened (so the
   * caller can stop further key handling).
   *
   * Preconditions:
   *  - active match is a dynamic_list with `arg: required` or `arg: context`
   *  - the script has resolved at least once for the current suffix
   *  - no arg is currently locked
   *  - the current suffix is non-empty
   *
   * On commit, a trailing space is appended to the input so the user can
   * immediately start typing the fuzzy filter.
   */
  function tryCommitRequiredArg(): boolean {
    if (!activeListCmd) return false;
    if (activeListCmd.action.type !== "dynamic_list") return false;
    const argMode = activeListCmd.action.config.arg ?? "none";
    if (argMode !== "required" && argMode !== "context") return false;
    if (!dynamicListLoaded) return false;
    if (committedArg !== null) return false;

    const phrase = activeListCmd.phrase.toLowerCase();
    const typed = typedInput.toLowerCase();
    const suffix = typed.startsWith(phrase + " ") ? typed.slice(phrase.length + 1).trim() : "";
    if (!suffix) return false;
    // Don't commit while results are still in flight for a different arg.
    if (scriptInvokedArg !== suffix) return false;

    committedArg = suffix;
    if (!input.endsWith(" ")) input = input + " ";
    return true;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (onboarding) return; // handled by the onboarding div
    if (showSettings) { handleSettingsKeydown(e); return; }
    if (isPreferencesWindow) {
      const mod = e.metaKey || e.ctrlKey;
      if (activePreferencesTab === "commands") {
        if (mod && e.key === "n") { e.preventDefault(); cmdStartNew(); return; }
        if (mod && e.key === "s") { e.preventDefault(); cmdSave(); return; }
        if (e.key === "Escape" && cmdBulkCount > 0) { e.preventDefault(); cmdBulkClear(); return; }
      }
      if (e.key === "Escape") { e.preventDefault(); appWindow.close(); return; }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      dismissWithFocusRestore();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      const len = showingList ? listItems.length : allFiltered.length;
      if (len > 0) selectedIndex = (selectedIndex + 1) % len;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const len = showingList ? listItems.length : allFiltered.length;
      if (len > 0) selectedIndex = (selectedIndex - 1 + len) % len;
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (showingList) {
        const item = listItems[selectedIndex];
        if (item) executeListItem(item);
      } else {
        const cmd = allFiltered[selectedIndex];
        if (cmd) executeCommand(cmd);
      }
    } else if (e.key === "Tab") {
      e.preventDefault();
      // 1) Commit-arg shortcut for arg:required dynamic_list — freeze the
      //    current suffix as the script's arg and switch further typing into
      //    client-side fuzzy filtering.
      if (tryCommitRequiredArg()) return;

      // 2) Otherwise: Tab autocompletes to the longest partially-matched phrase.
      // Only considers commands whose phrase starts with what the user typed
      // but isn't fully typed yet (i.e. partial phrase matches, not param mode).
      const typed = typedInput.toLowerCase();
      if (typed === "") return;
      const candidates = allFiltered
        .filter(cmd => {
          const phrase = cmd.phrase.toLowerCase();
          return phrase.startsWith(typed) && phrase !== typed;
        })
        .sort((a, b) => b.phrase.length - a.phrase.length);
      if (candidates.length > 0) {
        input = candidates[0].phrase;
      }
    } else if (e.key === "ArrowRight") {
      // Commit-arg shortcut — only when the cursor is at the end of the input
      // so normal cursor movement is preserved when editing mid-string.
      if (
        inputEl &&
        inputEl.selectionStart === input.length &&
        inputEl.selectionEnd === input.length &&
        tryCommitRequiredArg()
      ) {
        e.preventDefault();
      }
    }
  }

  // Persist active context to state.json via the backend.
  $effect(() => {
    invoke("save_context", { context: activeContext }).catch(() => {});
  });

  // ── Lifecycle ─────────────────────────────────────────────────────────
  onMount(() => {
    let unlistenFocus: (() => void) | null = null;
    let unlistenReload: (() => void) | null = null;
    let unlistenDeepLink: (() => void) | null = null;
    let unlistenSettingsChanged: (() => void) | null = null;
    let unlistenSwitchTab: (() => void) | null = null;

    (async () => {
      // Load settings from the backend (settings.yaml)
      const appSettings = await invoke<AppSettings>("get_settings").catch(
        () => ({ hotkey: undefined, show_context_chip: true, allow_duplicates: true, shared_dir: "shared", seed_examples: false } as AppSettings)
      );
      showContextChip = appSettings.show_context_chip;
      currentSettings = appSettings;

      // ── Preferences window path ──────────────────────────────────────
      // When running as the preferences window we initialise both tabs and
      // listen for tab-switch events from the launcher, then stop.
      if (isPreferencesWindow) {
        settingsShowContextChip = appSettings.show_context_chip;
        settingsAllowDuplicates = appSettings.allow_duplicates;
        settingsSharedDir = appSettings.shared_dir ?? "shared";
        settingsCommandsDir = appSettings.commands_dir ?? "";
        defaultCommandsDir = await invoke<string>("get_default_commands_dir").catch(() => "");
        await cmdRefreshList();
        // Read the initial tab from managed state (avoids URL hash / PathBuf issues).
        const initialTab = await invoke<string>("get_preferences_initial_tab").catch(() => "commands");
        activePreferencesTab = initialTab;
        showSettings = initialTab === "settings";
        unlistenSwitchTab = await listen<string>("preferences://switch-tab", (e) => {
          activePreferencesTab = e.payload;
          showSettings = e.payload === "settings";
        });
        return;
      }

      // ── Launcher window path ─────────────────────────────────────────

      // Restore active context from state.json (persisted by the backend).
      const savedContext = await invoke<string>("load_context").catch(() => "");
      if (savedContext) activeContext = savedContext;

      // Sync debug mode state from backend (session-scoped, not persisted).
      debugMode = await invoke<boolean>("is_debug").catch(() => false);

      // One-time migration: if the backend has no hotkey saved yet, check
      // localStorage for a legacy key written by an older version of the app.
      let resolvedHotkey = appSettings.hotkey;
      if (!resolvedHotkey) {
        const legacyHotkey =
          localStorage.getItem("ctx_hotkey") ??
          localStorage.getItem("contexts_hotkey");
        if (legacyHotkey) {
          await invoke("save_hotkey", { hotkey: legacyHotkey }).catch(() => {});
          await invoke("register_shortcut", { shortcut: legacyHotkey }).catch(() => {});
          localStorage.removeItem("ctx_hotkey");
          localStorage.removeItem("contexts_hotkey");
          resolvedHotkey = legacyHotkey;
        }
      }

      if (resolvedHotkey) {
        // Hotkey already registered by Rust on startup (or just migrated above).
        // Resize to launcher bar, load commands, then hide.
        await appWindow.setSize(LAUNCHER_SIZE);
        const result = await invoke<CommandsPayload>("list_commands").catch(() => ({ commands: [], duplicates: [], reserved: [], skipped: [], warnings: [] }));
        commands = result.commands;
        warnings = result.duplicates;
        reservedWarnings = result.reserved;
        skippedWarnings = result.skipped ?? [];
        commandWarnings = result.warnings ?? [];
        warningsDismissed = false;
        dismiss();
      } else {
        // First launch (no hotkey): show onboarding at the larger size
        await appWindow.setSize(ONBOARDING_SIZE);
        onboarding = true;
        // Focus the onboarding panel so keydown events fire
        setTimeout(() => onboardingEl?.focus(), 50);
      }

      // Hide on blur, but never during onboarding
      unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
        if (!focused && !onboarding) dismiss();
        if (focused && !onboarding) setTimeout(() => inputEl?.focus(), 0);
      });

      // Live-reload: backend emits this event when a YAML file changes
      unlistenReload = await listen<CommandsPayload>("commands://reloaded", (event) => {
        commands = event.payload.commands;
        warnings = event.payload.duplicates;
        reservedWarnings = event.payload.reserved;
        skippedWarnings = event.payload.skipped ?? [];
        commandWarnings = event.payload.warnings ?? [];
        warningsDismissed = false; // always surface new warnings
        // Invalidate cached list results so the main $effect re-invokes the script/list
        fullListItems = [];
        listLoadedPhrase = null;
        scriptInvokedArg = "";
        scriptInvokedContext = "";
        // If a list is currently displayed, refresh it in case its file changed
        if (activeListCmd && activeListCmd.action.type === "static_list") {
          const listName = activeListCmd.action.config.list;
          const commandDir = activeListCmd.source_dir;
          const phrase = activeListCmd.phrase.toLowerCase();
          invoke<ListItem[]>("load_list", { commandDir, listName, inlineEnv: activeListCmd.env, context: activeContext, phrase: activeListCmd.phrase })
            .then(items => { fullListItems = items; listLoadedPhrase = phrase; listItems = items; })
            .catch((err) => { listItems = [{ title: "⚠️ Error loading list", subtext: String(err) }]; });
        } else if (activeListCmd && activeListCmd.action.type === "dynamic_list") {
          const config = activeListCmd.action.config;
          const typed = input.trim().toLowerCase();
          const phrase = activeListCmd.phrase.toLowerCase();
          const suffix = typed.startsWith(phrase + " ") ? typed.slice(phrase.length + 1).trim() : "";
          invoke<ListItem[]>("run_dynamic_list", { commandDir: activeListCmd.source_dir, scriptName: config.script, arg: suffix || null, context: activeContext, phrase: activeListCmd.phrase, inlineEnv: activeListCmd.env })
            .then(items => { fullListItems = items; listLoadedPhrase = phrase; listItems = items; dynamicListLoaded = true; })
            .catch((err) => { listItems = [{ title: "⚠️ Script error", subtext: String(err) }]; });
        }
      });

      // Deep-link: backend emits this event when nimble://ctx/... is opened
      unlistenDeepLink = await listen<string>("context://changed", (event) => {
        activeContext = event.payload;
      });

      // Settings-changed: emitted by the settings window after every save.
      // Reload settings and commands so the launcher reflects the new values.
      unlistenSettingsChanged = await listen("settings-changed", async () => {
        const s = await invoke<AppSettings>("get_settings").catch(() => currentSettings);
        currentSettings = s;
        showContextChip = s.show_context_chip;
        const result = await invoke<CommandsPayload>("list_commands").catch(() => ({ commands: [], duplicates: [], reserved: [] }));
        commands = result.commands;
        warnings = result.duplicates;
        reservedWarnings = result.reserved;
        warningsDismissed = false;
      });
    })();

    return () => {
      unlistenFocus?.();
      unlistenReload?.();
      unlistenDeepLink?.();
      unlistenSettingsChanged?.();
      unlistenSwitchTab?.();
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

{#if onboarding}
  <!-- ── Onboarding ─────────────────────────────────────────────────────── -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={onboardingEl}
    class="onboarding"
    role="dialog"
    aria-label="Set global shortcut"
    tabindex="-1"
    onkeydown={handleOnboardingKeydown}
  >
    <p class="ob-title">Welcome to Nimble</p>
    <p class="ob-sub">Press the key combination you want to use<br>to open the launcher from anywhere.</p>

    <div class="shortcut-preview" class:active={!!capturedShortcut}>
      {capturedShortcut || "Press a key combination…"}
    </div>

    {#if shortcutError}
      <p class="ob-error">{shortcutError}</p>
    {/if}

    <button class="ob-confirm" disabled={!capturedShortcut} onclick={confirmShortcut}>
      Confirm shortcut
    </button>
  </div>
{:else if isPreferencesWindow}
  <!-- ── Unified Preferences window ───────────────────────────────────── -->
  <div class="prefs-window">
    <!-- Tab bar -->
    <div class="prefs-tabs" role="tablist">
      <button
        class="prefs-tab"
        class:active={activePreferencesTab === "commands"}
        role="tab"
        aria-selected={activePreferencesTab === "commands"}
        onclick={() => { activePreferencesTab = "commands"; showSettings = false; }}
      >Commands</button>
      <button
        class="prefs-tab"
        class:active={activePreferencesTab === "settings"}
        role="tab"
        aria-selected={activePreferencesTab === "settings"}
        onclick={() => { activePreferencesTab = "settings"; showSettings = true; }}
      >Settings</button>
    </div>

    <!-- Commands tab -->
    {#if activePreferencesTab === "commands"}

    <!-- Load-time warnings: skipped files + command warnings -->
    {#if skippedWarnings.length > 0 || commandWarnings.length > 0}
      <div class="prefs-warnings">
        {#if skippedWarnings.length > 0}
          <div class="prefs-warnings-section">
            <span class="prefs-warnings-label">⚠ {skippedWarnings.length} file{skippedWarnings.length === 1 ? '' : 's'} could not be loaded</span>
            {#each skippedWarnings as w}
              <div class="prefs-warning-row">
                <span class="prefs-warning-file">{w.file}</span>
                <span class="prefs-warning-msg">{w.reason}</span>
              </div>
            {/each}
          </div>
        {/if}
        {#if commandWarnings.length > 0}
          <div class="prefs-warnings-section">
            <span class="prefs-warnings-label">⚠ {commandWarnings.length} command warning{commandWarnings.length === 1 ? '' : 's'}</span>
            {#each commandWarnings as w}
              <div class="prefs-warning-row">
                <span class="prefs-warning-file">{w.file}</span>
                <span class="prefs-warning-msg">{w.message}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <div class="cmd-editor">
      {#if !cmdEditingMode}
      <!-- ═══ TABLE VIEW ═══ -->
      <!-- Toolbar -->
      <div class="cmd-toolbar">
        <div class="cmd-toolbar-left">
          <button class="cmd-tb-btn cmd-tb-btn-primary" onclick={cmdStartNewFromTable}>
            <span class="cmd-tb-icon">+</span> New <span class="cmd-tb-kbd">⌘N</span>
          </button>
          <div class="cmd-tb-separator"></div>
          <button class="cmd-tb-btn" disabled={!cmdSelectedFile} onclick={() => { const m = cmdList.find(c => c.file_path === cmdSelectedFile); if (m) cmdOpenEditor(m); }}>
            <span class="cmd-tb-icon">✎</span> Edit <span class="cmd-tb-kbd">⏎</span>
          </button>
          <button class="cmd-tb-btn cmd-tb-btn-danger" disabled={!cmdSelectedFile} onclick={() => { const m = cmdList.find(c => c.file_path === cmdSelectedFile); if (m) { cmdSelectItem(m); cmdDeleteConfirm = true; cmdEditingMode = true; } }}>
            <span class="cmd-tb-icon">✕</span> Delete <span class="cmd-tb-kbd">⌫</span>
          </button>
          <div class="cmd-tb-separator"></div>
          <div class="cmd-seg-group">
            <span class="cmd-seg-label">Group</span>
            <button class="cmd-seg-btn" class:active={cmdGroupBy === "none"} onclick={() => cmdGroupBy = "none"}>None</button>
            <button class="cmd-seg-btn" class:active={cmdGroupBy === "folder"} onclick={() => cmdGroupBy = "folder"}>Folder</button>
            <button class="cmd-seg-btn" class:active={cmdGroupBy === "type"} onclick={() => cmdGroupBy = "type"}>Type</button>
          </div>
        </div>
        <div class="cmd-toolbar-right">
          <div class="cmd-search-wrapper">
            <input
              type="text"
              class="cmd-search-box"
              placeholder="Filter commands…"
              bind:value={cmdFilter}
              autocomplete="off"
              autocorrect="off"
              spellcheck="false"
            />
          </div>
        </div>
      </div>

      <!-- Bulk-action bar (visible when items are selected) -->
      <div class="cmd-bulk-bar" class:visible={cmdBulkCount > 0}>
        <span class="cmd-bulk-count">{cmdBulkCount} selected</span>
        <div class="cmd-bulk-actions">
          <button class="cmd-tb-btn" onclick={() => cmdBulkSetEnabled(true)}>Enable all</button>
          <button class="cmd-tb-btn" onclick={() => cmdBulkSetEnabled(false)}>Disable all</button>
          <button class="cmd-tb-btn cmd-tb-btn-danger" onclick={cmdBulkDelete}>
            {cmdBulkDeleteConfirm ? 'Confirm delete?' : 'Delete all'}
          </button>
          <button class="cmd-tb-btn" onclick={cmdBulkClear}>Clear selection</button>
        </div>
      </div>

      <!-- Column header -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="cmd-col-header">
        <span class="cmd-col-check"><input type="checkbox" class="cmd-bulk-header-cb" checked={cmdBulkAllSelected} onchange={cmdBulkToggleAll} /></span>
        <span class="cmd-col-sortable" class:sorted={cmdSortCol === 'phrase'} onclick={() => cmdToggleSort('phrase')}>
          Phrase {cmdSortCol === 'phrase' ? (cmdSortAsc ? '▲' : '▼') : ''}
        </span>
        <span class="cmd-col-sortable" class:sorted={cmdSortCol === 'title'} onclick={() => cmdToggleSort('title')}>
          Title {cmdSortCol === 'title' ? (cmdSortAsc ? '▲' : '▼') : ''}
        </span>
        <span class="cmd-col-sortable" class:sorted={cmdSortCol === 'action'} onclick={() => cmdToggleSort('action')}>
          Action {cmdSortCol === 'action' ? (cmdSortAsc ? '▲' : '▼') : ''}
        </span>
        <span class="cmd-col-sortable" class:sorted={cmdSortCol === 'modified'} onclick={() => cmdToggleSort('modified')}>
          Modified {cmdSortCol === 'modified' ? (cmdSortAsc ? '▲' : '▼') : ''}
        </span>
        <span class="cmd-col-center">On</span>
      </div>

      <!-- Table body -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="cmd-table-container" oncontextmenu={(e) => e.preventDefault()}>
        {#if cmdFilteredList.length === 0}
          <div class="cmd-table-empty">
            <div class="cmd-empty-icon">📂</div>
            <div class="cmd-empty-title">No commands yet</div>
            <div class="cmd-empty-desc">Create your first command to get started with Nimble.</div>
            <div class="cmd-empty-actions">
              <button class="cmd-tb-btn cmd-tb-btn-primary" onclick={cmdStartNewFromTable}>
                <span class="cmd-tb-icon">+</span> New Command
              </button>
            </div>
          </div>
        {:else}
          {#each cmdGroupedList() as group}
            {#if cmdGroupBy !== "none"}
            <div class="cmd-folder-group">
              <button
                class="cmd-tbl-folder-header"
                onclick={() => {
                  const next = new Set(cmdCollapsedFolders);
                  next.has(group.folder) ? next.delete(group.folder) : next.add(group.folder);
                  cmdCollapsedFolders = next;
                }}
              >
                <span class="cmd-tbl-chevron" class:collapsed={cmdCollapsedFolders.has(group.folder)}>▼</span>
                {#if cmdGroupBy === "folder"}
                  <svg class="cmd-tbl-folder-svg" width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><path d="M1.5 3a1 1 0 011-1h3.59a1 1 0 01.7.29l1 1A1 1 0 008.5 3.6h6a1 1 0 011 1v8a1 1 0 01-1 1h-13a1 1 0 01-1-1V3z"/></svg>
                {:else}
                  <span class="cmd-tbl-group-icon">{actionIcon(group.folder)}</span>
                {/if}
                <span class="cmd-tbl-folder-name">{group.label}</span>
                {#if cmdDisabledCount(group.items) > 0}
                  <span class="cmd-tbl-folder-disabled">{cmdDisabledCount(group.items)} disabled</span>
                {/if}
                <span class="cmd-tbl-folder-count">{group.items.length} command{group.items.length === 1 ? '' : 's'}</span>
              </button>
              {#if !cmdCollapsedFolders.has(group.folder)}
                {#each group.items as meta}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <div
                    class="cmd-tbl-row"
                    class:selected={meta.file_path === cmdSelectedFile}
                    class:disabled-row={!meta.enabled}
                    class:checked={cmdBulkSelected.has(meta.file_path)}
                    onclick={() => { cmdSelectedFile = meta.file_path; }}
                    ondblclick={() => cmdOpenEditor(meta)}
                    oncontextmenu={(e) => { e.preventDefault(); cmdSelectedFile = meta.file_path; cmdCtxMenu = { x: e.clientX, y: e.clientY, meta }; }}
                    onkeydown={(e) => { if (e.key === 'Enter') cmdOpenEditor(meta); }}
                    role="option"
                    tabindex="0"
                    aria-selected={meta.file_path === cmdSelectedFile}
                  >
                    <span class="cmd-tbl-check-cell"><input type="checkbox" class="cmd-row-checkbox" checked={cmdBulkSelected.has(meta.file_path)} onclick={(e) => e.stopPropagation()} onchange={() => cmdBulkToggle(meta.file_path)} /></span>
                    <span class="cmd-tbl-phrase">
                      {meta.phrase}
                      {#if cmdWarningForFile(meta)}
                        <span class="cmd-tbl-status-warn" title={cmdWarningForFile(meta)}>⚠</span>
                      {/if}
                    </span>
                    <span class="cmd-tbl-title">{meta.title}</span>
                    <span><span class="cmd-tbl-badge cmd-tbl-badge-{meta.action_type}"><span class="cmd-tbl-badge-icon">{actionIcon(meta.action_type)}</span>{meta.action_type.replace(/_/g, ' ')}</span></span>
                    <span class="cmd-tbl-modified">{relativeTime(meta.modified)}</span>
                    <span class="cmd-tbl-toggle-cell">
                      <!-- svelte-ignore a11y_label_has_associated_control -->
                      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                      <label class="cmd-tbl-toggle" onclick={(e) => e.stopPropagation()}>
                        <input type="checkbox" checked={meta.enabled} onchange={(e) => {
                          const target = e.target as HTMLInputElement;
                          cmdToggleEnabled(meta, target.checked);
                        }} />
                        <span class="cmd-tbl-toggle-track"></span>
                        <span class="cmd-tbl-toggle-knob"></span>
                      </label>
                    </span>
                  </div>
                {/each}
              {/if}
            </div>
            {:else}
              <!-- No grouping — flat rows -->
              {#each group.items as meta}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div
                  class="cmd-tbl-row"
                  class:selected={meta.file_path === cmdSelectedFile}
                  class:disabled-row={!meta.enabled}
                  class:checked={cmdBulkSelected.has(meta.file_path)}
                  onclick={() => { cmdSelectedFile = meta.file_path; }}
                  ondblclick={() => cmdOpenEditor(meta)}
                  oncontextmenu={(e) => { e.preventDefault(); cmdSelectedFile = meta.file_path; cmdCtxMenu = { x: e.clientX, y: e.clientY, meta }; }}
                  onkeydown={(e) => { if (e.key === 'Enter') cmdOpenEditor(meta); }}
                  role="option"
                  tabindex="0"
                  aria-selected={meta.file_path === cmdSelectedFile}
                >
                  <span class="cmd-tbl-check-cell"><input type="checkbox" class="cmd-row-checkbox" checked={cmdBulkSelected.has(meta.file_path)} onclick={(e) => e.stopPropagation()} onchange={() => cmdBulkToggle(meta.file_path)} /></span>
                  <span class="cmd-tbl-phrase">
                    {meta.phrase}
                    {#if cmdWarningForFile(meta)}
                      <span class="cmd-tbl-status-warn" title={cmdWarningForFile(meta)}>⚠</span>
                    {/if}
                  </span>
                  <span class="cmd-tbl-title">{meta.title}</span>
                  <span><span class="cmd-tbl-badge cmd-tbl-badge-{meta.action_type}"><span class="cmd-tbl-badge-icon">{actionIcon(meta.action_type)}</span>{meta.action_type.replace(/_/g, ' ')}</span></span>
                  <span class="cmd-tbl-modified">{relativeTime(meta.modified)}</span>
                  <span class="cmd-tbl-toggle-cell">
                    <!-- svelte-ignore a11y_label_has_associated_control -->
                    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                    <label class="cmd-tbl-toggle" onclick={(e) => e.stopPropagation()}>
                      <input type="checkbox" checked={meta.enabled} onchange={(e) => {
                        const target = e.target as HTMLInputElement;
                        cmdToggleEnabled(meta, target.checked);
                      }} />
                      <span class="cmd-tbl-toggle-track"></span>
                      <span class="cmd-tbl-toggle-knob"></span>
                    </label>
                  </span>
                </div>
              {/each}
            {/if}
          {/each}
        {/if}
      </div>

      <!-- Status bar -->
      <div class="cmd-statusbar">
        <span>{cmdFilteredList.length} command{cmdFilteredList.length === 1 ? '' : 's'}{cmdFilter ? ' matching' : ''}{cmdGroupBy !== 'none' ? ` in ${cmdGroupedList().length} ${cmdGroupBy === 'folder' ? 'folder' : 'type'}${cmdGroupedList().length === 1 ? '' : 's'}` : ''}</span>
        {#if (skippedWarnings.length + commandWarnings.length) > 0}
          <span class="cmd-statusbar-pill-warn">⚠ {skippedWarnings.length + commandWarnings.length} issue{(skippedWarnings.length + commandWarnings.length) === 1 ? '' : 's'}</span>
        {/if}
        <span class="cmd-statusbar-right">
          <kbd>↑↓</kbd> Navigate &nbsp; <kbd>⏎</kbd> Edit &nbsp;
          <kbd>⌫</kbd> Delete &nbsp; <kbd>Right-click</kbd> More
        </span>
      </div>

      <!-- Context menu -->
      {#if cmdCtxMenu}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="cmd-ctx-backdrop" onclick={() => cmdCtxMenu = null}>
          <div class="cmd-ctx-menu" style="left:{cmdCtxMenu.x}px;top:{cmdCtxMenu.y}px" onclick={(e) => e.stopPropagation()}>
            <button class="cmd-ctx-item" onclick={() => { cmdOpenEditor(cmdCtxMenu!.meta); cmdCtxMenu = null; }}>
              <span class="cmd-ctx-icon">✎</span> Edit Command <span class="cmd-ctx-shortcut">⏎</span>
            </button>
            <div class="cmd-ctx-separator"></div>
            <button class="cmd-ctx-item" onclick={() => { const m = cmdCtxMenu!.meta; cmdToggleEnabled(m, !m.enabled); cmdCtxMenu = null; }}>
              <span class="cmd-ctx-icon">◉</span> {cmdCtxMenu.meta.enabled ? 'Disable' : 'Enable'}
            </button>
            <div class="cmd-ctx-separator"></div>
            <button class="cmd-ctx-item cmd-ctx-danger" onclick={() => { const m = cmdCtxMenu!.meta; cmdCtxMenu = null; cmdSelectItem(m); cmdDeleteConfirm = true; cmdEditingMode = true; }}>
              <span class="cmd-ctx-icon">✕</span> Delete Command
            </button>
          </div>
        </div>
      {/if}

      {:else}
      <!-- ═══ DETAIL EDITOR VIEW ═══ -->
      <div class="cmd-detail-view">
        <!-- Back bar -->
        <div class="cmd-detail-bar">
          <button class="cmd-tb-btn" onclick={cmdCloseEditor}>
            ← Back to list
          </button>
          <span class="cmd-detail-bar-title">
            {#if cmdIsNew}New Command{:else}Edit: {cmdPhrase || '(untitled)'}{/if}
          </span>
        </div>

        <div class="cmd-detail-scroll">
          <div class="cmd-form">
            <!-- Phrase -->
            <div class="cmd-field">
              <label class="cmd-label" for="cmd-phrase">Phrase</label>
              <input
                id="cmd-phrase"
                class="cmd-input"
                class:error={cmdPhraseConflict}
                type="text"
                bind:value={cmdPhrase}
                placeholder="e.g. open jira"
                autocomplete="off"
                spellcheck="false"
              />
              {#if cmdPhraseConflict}
                <span class="cmd-field-error">"{cmdPhrase.trim()}" is already used by another command.</span>
              {/if}
            </div>

            <!-- Folder (new commands only) -->
            {#if cmdIsNew}
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-folder">Folder</label>
                {#if cmdShowNewFolder}
                  <div class="cmd-folder-input-row">
                    <input
                      id="cmd-folder-new"
                      class="cmd-input cmd-folder-new-input"
                      type="text"
                      bind:value={cmdNewFolderName}
                      placeholder="e.g. work/jira"
                      autocomplete="off"
                      spellcheck="false"
                    />
                    <button class="cmd-btn-tiny" onclick={() => { cmdShowNewFolder = false; cmdNewFolderName = ""; }}>Cancel</button>
                  </div>
                {:else}
                  <div class="cmd-folder-input-row">
                    <select id="cmd-folder" class="cmd-select cmd-folder-select" bind:value={cmdTargetDir}>
                      <option value="">Root (commands/)</option>
                      {#each cmdFolders as f}
                        <option value={f}>{f}/</option>
                      {/each}
                    </select>
                    <button class="cmd-btn-tiny" onclick={() => { cmdShowNewFolder = true; }}>New…</button>
                  </div>
                {/if}
                <span class="cmd-field-hint">Where to save the command YAML file.</span>
              </div>
            {:else if cmdSelectedFile}
              <div class="cmd-field">
                <span class="cmd-label">Location</span>
                <span class="cmd-field-hint cmd-location-path">{cmdList.find(c => c.file_path === cmdSelectedFile)?.source_dir || "commands/"}</span>
              </div>
            {/if}

            <!-- Title -->
            <div class="cmd-field">
              <label class="cmd-label" for="cmd-title">Title</label>
              <input
                id="cmd-title"
                class="cmd-input"
                type="text"
                bind:value={cmdTitle}
                placeholder="e.g. Open Jira"
                autocomplete="off"
                spellcheck="false"
              />
            </div>

            <!-- Action type -->
            <div class="cmd-field">
              <label class="cmd-label" for="cmd-action-type">Action</label>
              <select
                id="cmd-action-type"
                class="cmd-select"
                bind:value={cmdActionType}
                onchange={() => { cmdUrl = ""; cmdText = ""; cmdListName = ""; cmdItemAction = ""; cmdScript = ""; cmdArgMode = "none"; cmdResultAction = "paste_text"; cmdPrefix = ""; cmdSuffix = ""; cmdTestArg = ""; cmdTestResult = null; }}
              >
                <option value="open_url">Open URL</option>
                <option value="paste_text">Paste Text</option>
                <option value="copy_text">Copy Text</option>
                <option value="static_list">Static List</option>
                <option value="dynamic_list">Dynamic List</option>
                <option value="script_action">Script Action</option>
              </select>
            </div>

            <!-- Action-specific config -->
            {#if cmdActionType === "open_url"}
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-url">URL</label>
                <input
                  id="cmd-url"
                  class="cmd-input"
                  type="text"
                  bind:value={cmdUrl}
                  placeholder="https://example.com"
                  autocomplete="off"
                  spellcheck="false"
                />
                {#if cmdUrl.includes("{param}")}
                  <span class="cmd-field-hint cmd-field-hint-ok">Contains &#123;param&#125; — user input is appended after the phrase.<br>Preview: {cmdUrl.replace("{param}", "<query>")}</span>
                {:else}
                  <span class="cmd-field-hint">Use &#123;param&#125; in the URL to accept user input after the phrase.</span>
                {/if}
              </div>
            {:else if cmdActionType === "paste_text" || cmdActionType === "copy_text"}
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-text">Text</label>
                <textarea
                  id="cmd-text"
                  class="cmd-textarea"
                  bind:value={cmdText}
                  placeholder={cmdActionType === "paste_text" ? "Text to paste…" : "Text to copy…"}
                  rows="6"
                  spellcheck="false"
                ></textarea>
                <span class="cmd-field-hint">
                  {#if cmdActionType === "paste_text"}
                    Pasted into the app that had focus before the launcher.
                  {:else}
                    Copied to the clipboard without simulating a keypress.
                  {/if}
                  {cmdText.length > 0 ? `(${cmdText.length} chars)` : ""}
                </span>
              </div>
            {:else if cmdActionType === "static_list"}
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-list-name">List file name</label>
                <input
                  id="cmd-list-name"
                  class="cmd-input"
                  type="text"
                  bind:value={cmdListName}
                  placeholder="e.g. team-emails"
                  autocomplete="off"
                  spellcheck="false"
                />
                <span class="cmd-field-hint">Name of a .tsv file co-located with the command YAML (without extension).</span>
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-item-action">Item action</label>
                <select id="cmd-item-action" class="cmd-select" bind:value={cmdItemAction}>
                  <option value="">None (dismiss only)</option>
                  <option value="paste_text">Paste Text</option>
                  <option value="copy_text">Copy Text</option>
                  <option value="open_url">Open URL</option>
                  <option value="ctx_set">Set Context</option>
                </select>
                <span class="cmd-field-hint">Action applied to the selected list item's value.</span>
              </div>
            {:else if cmdActionType === "dynamic_list"}
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-script">Script</label>
                <input
                  id="cmd-script"
                  class="cmd-input"
                  type="text"
                  bind:value={cmdScript}
                  placeholder="e.g. hello.sh"
                  autocomplete="off"
                  spellcheck="false"
                />
                <span class="cmd-field-hint">Script in the command directory or scripts/ folder.</span>
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-arg-mode">Argument mode</label>
                <select id="cmd-arg-mode" class="cmd-select" bind:value={cmdArgMode}>
                  <option value="none">None</option>
                  <option value="optional">Optional</option>
                  <option value="required">Required</option>
                  <option value="context">Context</option>
                </select>
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-dl-item-action">Item action</label>
                <select id="cmd-dl-item-action" class="cmd-select" bind:value={cmdItemAction}>
                  <option value="">None (dismiss only)</option>
                  <option value="paste_text">Paste Text</option>
                  <option value="copy_text">Copy Text</option>
                  <option value="open_url">Open URL</option>
                  <option value="ctx_set">Set Context</option>
                </select>
              </div>
            {:else if cmdActionType === "script_action"}
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-sa-script">Script</label>
                <input
                  id="cmd-sa-script"
                  class="cmd-input"
                  type="text"
                  bind:value={cmdScript}
                  placeholder="e.g. emails.sh"
                  autocomplete="off"
                  spellcheck="false"
                />
                <span class="cmd-field-hint">Script in the command directory or scripts/ folder.</span>
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-sa-arg-mode">Argument mode</label>
                <select id="cmd-sa-arg-mode" class="cmd-select" bind:value={cmdArgMode}>
                  <option value="none">None</option>
                  <option value="optional">Optional</option>
                  <option value="required">Required</option>
                  <option value="context">Context</option>
                </select>
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-result-action">Result action</label>
                <select id="cmd-result-action" class="cmd-select" bind:value={cmdResultAction}>
                  <option value="paste_text">Paste Text</option>
                  <option value="copy_text">Copy Text</option>
                  <option value="open_url">Open URL</option>
                </select>
                <span class="cmd-field-hint">Built-in action applied to each value the script returns.</span>
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-prefix">Prefix (optional)</label>
                <input
                  id="cmd-prefix"
                  class="cmd-input"
                  type="text"
                  bind:value={cmdPrefix}
                  placeholder=""
                  autocomplete="off"
                  spellcheck="false"
                />
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-suffix">Suffix (optional)</label>
                <input
                  id="cmd-suffix"
                  class="cmd-input"
                  type="text"
                  bind:value={cmdSuffix}
                  placeholder=""
                  autocomplete="off"
                  spellcheck="false"
                />
              </div>
            {/if}

            <!-- Inline script editor (for dynamic_list and script_action) -->
            {#if (cmdActionType === "dynamic_list" || cmdActionType === "script_action") && cmdScript.trim()}
              <div class="cmd-field">
                <div class="cmd-script-header">
                  <label class="cmd-label" for="cmd-script-editor">Script content</label>
                  {#if cmdScriptDirty}
                    <span class="cmd-script-dirty-badge">unsaved</span>
                  {/if}
                </div>
                {#if cmdScriptIsExternal}
                  <div class="cmd-script-external-hint">
                    <span>The <code>${"${}"}</code> syntax is no longer supported. Use a plain filename for co-located scripts or the <code>shared:</code> prefix (e.g. <code>shared:script.sh</code>).</span>
                  </div>
                {:else if cmdScriptLoading}
                  <div class="cmd-script-status">Loading…</div>
                {:else if !cmdScriptExists && !cmdScriptDirty}
                  <div class="cmd-script-not-found">
                    <span>Script file not found.</span>
                    <button class="cmd-btn-tiny cmd-btn-create-script" onclick={cmdCreateScript}>Create from template</button>
                  </div>
                {:else}
                  <textarea
                    id="cmd-script-editor"
                    class="cmd-textarea cmd-script-textarea"
                    bind:value={cmdScriptContent}
                    oninput={() => { cmdScriptDirty = true; }}
                    rows="10"
                    spellcheck="false"
                    placeholder="#!/bin/bash"
                  ></textarea>
                  <div class="cmd-script-actions">
                    {#if cmdScriptError}
                      <span class="cmd-field-error">{cmdScriptError}</span>
                    {/if}
                    <button
                      class="cmd-btn-tiny"
                      disabled={!cmdScriptDirty || cmdScriptSaving}
                      onclick={cmdSaveScript}
                    >{cmdScriptSaving ? "Saving…" : "Save script"}</button>
                    {#if cmdArgMode !== "none"}
                      <input
                        class="cmd-input cmd-test-arg-input"
                        type="text"
                        placeholder="Test argument…"
                        autocomplete="off"
                        autocorrect="off"
                        autocapitalize="off"
                        spellcheck="false"
                        bind:value={cmdTestArg}
                      />
                    {/if}
                    <button
                      class="cmd-btn-tiny cmd-btn-test"
                      disabled={cmdTestRunning || !cmdScriptExists}
                      onclick={cmdRunTest}
                    >{cmdTestRunning ? "Running…" : "Test"}</button>
                  </div>
                  {#if cmdTestResult !== null}
                    <div class="cmd-test-result" class:cmd-test-error={cmdTestResult.exit_code !== 0 || cmdTestResult.timed_out}>
                      <div class="cmd-test-result-meta">
                        {#if cmdTestResult.timed_out}
                          <span class="cmd-test-badge cmd-test-badge-err">timed out</span>
                        {:else}
                          <span class="cmd-test-badge" class:cmd-test-badge-ok={cmdTestResult.exit_code === 0} class:cmd-test-badge-err={cmdTestResult.exit_code !== 0}>
                            exit {cmdTestResult.exit_code ?? "?"}
                          </span>
                        {/if}
                        <span class="cmd-test-duration">{cmdTestResult.duration_ms}ms</span>
                      </div>
                      {#if cmdTestResult.stdout}
                        <pre class="cmd-test-output">{cmdTestResult.stdout}</pre>
                      {/if}
                      {#if cmdTestResult.stderr}
                        <pre class="cmd-test-output cmd-test-stderr">{cmdTestResult.stderr}</pre>
                      {/if}
                      {#if !cmdTestResult.stdout && !cmdTestResult.stderr}
                        <span class="cmd-test-empty">(no output)</span>
                      {/if}
                    </div>
                  {/if}
                {/if}
              </div>
            {/if}

            <!-- Enabled toggle -->
            <div class="cmd-field cmd-field-inline">
              <label class="cmd-checkbox-label">
                <input type="checkbox" bind:checked={cmdEnabled} />
                <span>Enabled</span>
              </label>
            </div>

            <!-- Error -->
            {#if cmdSaveError}
              <p class="cmd-save-error">{cmdSaveError}</p>
            {/if}

            <!-- Actions row -->
            <div class="cmd-actions">
              {#if cmdIsNew}
                <button class="cmd-btn-ghost" onclick={() => { cmdCancelNew(); cmdCloseEditor(); }}>Cancel</button>
                <button class="cmd-btn-primary" disabled={!cmdCanSave} onclick={async () => { await cmdSave(); if (!cmdSaveError) cmdCloseEditor(); }}>
                  {cmdSaving ? "Creating…" : "Create"}
                </button>
              {:else}
                {#if cmdDeleteConfirm}
                  <span class="cmd-delete-confirm-text">Delete this command?</span>
                  <button class="cmd-btn-ghost" onclick={() => { cmdDeleteConfirm = false; cmdCloseEditor(); }}>Cancel</button>
                  <button class="cmd-btn-danger" onclick={async () => { await cmdDelete(); cmdCloseEditor(); }}>Delete</button>
                {:else}
                  <div class="cmd-actions-left">
                    <button class="cmd-btn-ghost cmd-btn-ghost-danger" onclick={cmdDelete}>Delete</button>
                    {#if cmdSelectedFile}
                      <button class="cmd-btn-icon" title="Reveal in file manager" onclick={() => invoke("reveal_in_file_manager", { path: cmdSelectedFile })}>
                        <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><path d="M2 3h12v10H2V3zm1 1v8h10V4H3z" fill="currentColor"/><path d="M5 7h6v1H5V7z" fill="currentColor"/></svg>
                      </button>
                      <button class="cmd-btn-icon" title="Open in default editor" onclick={() => invoke("open_in_default_editor", { path: cmdSelectedFile })}>
                        <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><path d="M11.5 1.5l3 3-9 9H2.5v-3l9-9zm-1 4l-7 7v1h1l7-7-1-1z" fill="currentColor"/></svg>
                      </button>
                    {/if}
                  </div>
                  <button class="cmd-btn-primary" disabled={!cmdCanSave} onclick={async () => { await cmdSave(); if (!cmdSaveError) cmdCloseEditor(); }}>
                    {cmdSaving ? "Saving…" : "Save"}
                  </button>
                {/if}
              {/if}
            </div>
          </div>
        </div>
      </div>
      {/if}
    </div>
    {/if}

    <!-- Settings tab -->
    {#if activePreferencesTab === "settings"}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="settings-panel" onkeydown={handleSettingsKeydown}>

      <!-- Auto-save bar -->
      <div class="settings-autosave-bar">
        <span class="settings-autosave-pill">
          {#if settingsSaved}
            <span class="settings-autosave-dot saved"></span> Changes saved
          {:else}
            <span class="settings-autosave-dot"></span> All changes saved automatically
          {/if}
        </span>
        <span class="settings-autosave-right">
          <button class="settings-restore" onclick={() => settingsRestoreConfirm = true}>Restore Defaults…</button>
        </span>
      </div>

    <div class="settings-layout">
      <!-- Sidebar nav -->
      <div class="settings-nav">
        <button class="settings-nav-item" class:active={settingsActiveNav === 'keyboard'} onclick={() => settingsScrollTo('keyboard')}>
          <svg class="settings-nav-icon" width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><rect x="2" y="6" width="3" height="3" rx="0.5"/><rect x="6.5" y="6" width="3" height="3" rx="0.5"/><rect x="11" y="6" width="3" height="3" rx="0.5"/></svg>
          Keyboard
        </button>
        <button class="settings-nav-item" class:active={settingsActiveNav === 'behavior'} onclick={() => settingsScrollTo('behavior')}>
          <svg class="settings-nav-icon" width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M8 1l1.6 4 4.4.4-3.3 3 1 4.5L8 10.7 4.3 13l1-4.5L2 5.4 6.4 5z"/></svg>
          Behavior
        </button>
        <button class="settings-nav-item" class:active={settingsActiveNav === 'files'} onclick={() => settingsScrollTo('files')}>
          <svg class="settings-nav-icon" width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M1.5 3a1 1 0 011-1h3.59a1 1 0 01.7.29l1 1A1 1 0 008.5 3.6h6a1 1 0 011 1v8a1 1 0 01-1 1h-13a1 1 0 01-1-1V3z"/></svg>
          Files & Paths
        </button>
        <button class="settings-nav-item" class:active={settingsActiveNav === 'about'} onclick={() => settingsScrollTo('about')}>
          <svg class="settings-nav-icon" width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M8 1a7 7 0 100 14A7 7 0 008 1zm0 3a1 1 0 11-2 0 1 1 0 012 0zm-1 3v4h2V7H7z"/></svg>
          About
        </button>
      </div>

    <div class="settings-body">
      <!-- Section: Keyboard -->
      <div class="settings-section-title" id="settings-section-keyboard">Keyboard</div>

      <!-- Global shortcut -->
      <div class="settings-row">
        <div class="settings-row-icon">⌨</div>
        <div class="settings-row-info">
          <span class="row-title">Activation hotkey <span class="row-help" title="The system-wide keyboard shortcut that opens Nimble from any app.">?</span></span>
          <span class="row-desc">Open the launcher from anywhere on your system.</span>
        </div>
        {#if settingsChangingHotkey}
          <div class="hotkey-capture-row">
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="shortcut-preview compact" class:active={!!settingsCapturedShortcut}>
              {settingsCapturedShortcut || "Press a key combination…"}
            </div>
            <button class="action-btn cancel" onclick={() => { settingsChangingHotkey = false; settingsCapturedShortcut = ""; settingsHotkeyError = ""; }}>Cancel</button>
            <button class="action-btn confirm" disabled={!settingsCapturedShortcut} onclick={confirmSettingsHotkey}>Confirm</button>
          </div>
          {#if settingsHotkeyError}
            <p class="row-error">{settingsHotkeyError}</p>
          {/if}
        {:else}
          <div class="row-right">
            <div class="settings-hotkey-keys">
              {#each (currentSettings.hotkey ?? "Not set").split("+") as part, i}
                {#if i > 0}<span class="settings-hotkey-plus">+</span>{/if}
                <span class="settings-hotkey-key">{part}</span>
              {/each}
            </div>
            <button class="action-btn" onclick={() => { settingsChangingHotkey = true; settingsCapturedShortcut = ""; }}>Change…</button>
          </div>
        {/if}
      </div>

      <!-- Section: Behavior -->
      <div class="settings-section-title" id="settings-section-behavior">Behavior</div>

      <!-- Show context chip -->
      <div class="settings-row">
        <div class="settings-row-icon">◐</div>
        <div class="settings-row-info">
          <span class="row-title">Show context chip <span class="row-help" title="Shows the active context (e.g. project name) inside the launcher input bar.">?</span></span>
          <span class="row-desc">Display the active context label in the launcher bar.</span>
        </div>
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="toggle"
          class:on={settingsShowContextChip}
          role="switch"
          aria-checked={settingsShowContextChip}
          tabindex="0"
          onclick={() => { settingsShowContextChip = !settingsShowContextChip; persistSettings(); }}
          onkeydown={(e) => { if (e.key === " " || e.key === "Enter") { settingsShowContextChip = !settingsShowContextChip; persistSettings(); } }}
        >
          <span class="thumb"></span>
        </div>
      </div>

      <!-- Allow duplicates -->
      <div class="settings-row">
        <div class="settings-row-icon">⚊</div>
        <div class="settings-row-info">
          <span class="row-title">Allow duplicate commands <span class="row-help" title="When enabled, multiple commands with the same phrase load together. When disabled, only the first is loaded.">?</span></span>
          <span class="row-desc">Load all commands even when phrases collide.</span>
        </div>
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="toggle"
          class:on={settingsAllowDuplicates}
          role="switch"
          aria-checked={settingsAllowDuplicates}
          tabindex="0"
          onclick={() => { settingsAllowDuplicates = !settingsAllowDuplicates; persistSettings(); }}
          onkeydown={(e) => { if (e.key === " " || e.key === "Enter") { settingsAllowDuplicates = !settingsAllowDuplicates; persistSettings(); } }}
        >
          <span class="thumb"></span>
        </div>
      </div>

      <!-- Section: Files & Paths -->
      <div class="settings-section-title" id="settings-section-files">Files & Paths</div>

      <!-- Shared scripts directory -->
      <div class="settings-row settings-row-stacked">
        <div class="settings-row-top">
          <div class="settings-row-icon">↗</div>
          <div class="settings-row-info">
            <span class="row-title">Shared directory <span class="row-help" title="Subdirectory inside commands root for shared scripts and lists referenced via 'shared:' prefix.">?</span></span>
            <span class="row-desc">Subdirectory for scripts and lists referenced via <code class="settings-code">shared:</code> prefix.</span>
          </div>
        </div>
        <input
          class="settings-text-input"
          type="text"
          bind:value={settingsSharedDir}
          placeholder="shared"
          onblur={persistSettings}
          spellcheck="false"
          autocomplete="off"
          autocorrect="off"
        />
      </div>

      <!-- Commands directory -->
      <div class="settings-row settings-row-stacked">
        <div class="settings-row-top">
          <div class="settings-row-icon">📁</div>
          <div class="settings-row-info">
            <span class="row-title">Commands directory</span>
            <span class="row-desc">Where Nimble loads command YAML files from.</span>
          </div>
        </div>
        <div class="settings-dir-row">
          {#if settingsCommandsDir.trim()}
            <div class="settings-dir-display" title={settingsCommandsDir}>
              <span class="dir-path">{shortenPath(settingsCommandsDir, defaultCommandsDir)}</span>
            </div>
            <button class="settings-dir-clear-btn" onclick={clearCommandsDir} title="Reset to default">✕</button>
          {:else}
            <div class="settings-dir-display settings-dir-default" title={defaultCommandsDir}>
              <span class="dir-path">{defaultCommandsDir ? shortenPath(defaultCommandsDir, defaultCommandsDir) : "Default"}</span>
              <span class="dir-default-badge">default</span>
            </div>
          {/if}
          <button class="settings-browse-btn" onclick={browseCommandsDir}>Browse…</button>
          <button class="settings-browse-btn" onclick={() => {
            const dir = settingsCommandsDir.trim() || defaultCommandsDir;
            if (dir) invoke("reveal_in_file_manager", { path: dir }).catch(() => {});
          }}>Reveal</button>
        </div>
      </div>

      <!-- Section: About -->
      <div class="settings-section-title" id="settings-section-about">About</div>

      <div class="settings-about-card">
        <div class="settings-about-logo">N</div>
        <div class="settings-about-info">
          <div class="settings-about-name">Nimble</div>
          <div class="settings-about-version">Version 1.1.0 · {navigator.platform.startsWith('Mac') ? 'macOS' : navigator.platform.startsWith('Linux') ? 'Linux' : 'Windows'}</div>
          <div class="settings-about-links">
            <a class="settings-about-link" href="https://github.com/surdy/nimble" target="_blank" rel="noopener">GitHub</a>
            <span class="settings-about-sep">·</span>
            <a class="settings-about-link" href="https://github.com/surdy/nimble/blob/main/docs/getting-started.md" target="_blank" rel="noopener">Documentation</a>
            <span class="settings-about-sep">·</span>
            <a class="settings-about-link" href="https://github.com/surdy/nimble/blob/main/CHANGELOG.md" target="_blank" rel="noopener">Changelog</a>
          </div>
        </div>
      </div>
    </div>
    </div>

    <!-- Restore Defaults confirmation modal -->
    {#if settingsRestoreConfirm}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="settings-modal-overlay" onclick={() => settingsRestoreConfirm = false}>
        <div class="settings-modal" onclick={(e) => e.stopPropagation()}>
          <div class="settings-modal-title">Restore default settings?</div>
          <div class="settings-modal-body">
            This will reset the global shortcut, behavior toggles, and shared directory to their defaults.
            Your commands and command directory path are not affected.
          </div>
          <div class="settings-modal-actions">
            <button class="action-btn" onclick={() => settingsRestoreConfirm = false}>Cancel</button>
            <button class="action-btn cancel" onclick={() => { settingsRestoreConfirm = false; restoreDefaults(); }}>Restore Defaults</button>
          </div>
        </div>
      </div>
    {/if}
  </div>
    {/if}
  </div>
{:else}
  <!-- ── Launcher bar ───────────────────────────────────────────────────── -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="launcher" data-tauri-drag-region>
    <div class="input-row">
      <span class="prompt-glyph">»</span>
      <input
        bind:this={inputEl}
        bind:value={input}
        onmousedown={() => appWindow.startDragging()}
        type="text"
        placeholder={activeContext && showContextChip ? "…" : "Type a command…"}
        autocomplete="off"
        autocorrect="off"
        spellcheck="false"
      />
      {#if activeContext && showContextChip}
        <div class="context-chip">
          <span class="chip-label">{activeContext}</span>
          <button
            class="chip-clear"
            onclick={() => { activeContext = ""; }}
            onmousedown={(e) => e.preventDefault()}
            aria-label="Clear context"
          >&times;</button>
        </div>
      {/if}
      {#if debugMode}
        <span class="debug-badge">DEBUG</span>
      {/if}
    </div>

    {#if warningVisible}
      <div class="warnings-bar">
        <span class="warnings-text">
          ⚠ {totalWarnings} command{totalWarnings === 1 ? '' : 's'} ignored
        </span>
        <button class="warnings-dismiss" onclick={() => (warningsDismissed = true)} aria-label="Dismiss">&times;</button>
      </div>
    {/if}

    {#if input.trim() !== ""}
      <div class="results" bind:this={resultsEl}>
        {#if showingList}
          {#if scriptLoadingState !== "idle" && !dynamicListLoaded}
            {@const loadingMsg =
              scriptLoadingState === "very_slow" ? "Script may be close to the 5s timeout…" :
              scriptLoadingState === "slow" ? "Taking longer than expected…" : "Running…"}
            <div class="no-results">{loadingMsg}</div>
          {:else if listItems.length === 0}
            {@const scriptName = activeListCmd?.action.type === "dynamic_list" ? activeListCmd.action.config.script : null}
            {@const hasFilter = (() => {
              if (!activeListCmd) return false;
              const phrase = activeListCmd.phrase.toLowerCase();
              const typed = typedInput.toLowerCase();
              return typed.startsWith(phrase + " ") && typed.slice(phrase.length + 1).trim() !== "";
            })()}
            {#if hasFilter && fullListItems.length > 0}
              <div class="no-results">No matches for filter</div>
            {:else}
              <div class="no-results">No results returned{#if scriptName}&hairsp;<span class="no-results-hint">({scriptName})</span>{/if}</div>
            {/if}
          {:else}
            {#each listItems as item, i}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                class="result-row"
                class:selected={i === selectedIndex}
                class:error-item={item.title.startsWith("\u26a0")}
                onmouseenter={() => (selectedIndex = i)}
                onmousedown={(e) => { e.preventDefault(); selectedIndex = i; }}
                onclick={() => executeListItem(item)}
              >
                <div class="result-content">
                  <span class="result-title">{item.title}</span>
                  {#if item.subtext}
                    <span class="result-subtext">{item.subtext}</span>
                  {/if}
                </div>
              </div>
            {/each}
          {/if}
        {:else if actionError}
          <div class="result-row error-item">
            <div class="result-content">
              <span class="result-title">⚠️ Action error</span>
              <span class="result-subtext">{actionError}</span>
            </div>
          </div>
        {:else if allFiltered.length === 0}
          <div class="no-results">No matching commands</div>
        {:else}
          {#each allFiltered as cmd, i}
            {@const builtinAction = cmd.action.type === "builtin" ? cmd.action.config.action : null}
            {@const ctxSetValue = builtinAction === "ctx_set" && typedInput.toLowerCase().startsWith("/ctx set ") ? typedInput.slice("/ctx set ".length).trim() : ""}
            {@const isParamMode = builtinAction === null && typedInput.toLowerCase().startsWith(cmd.phrase.toLowerCase() + " ")}
            {@const paramText  = isParamMode ? typedInput.slice(cmd.phrase.length + 1) : ""}
            {@const isContextArg = (cmd.action.type === "script_action" || cmd.action.type === "dynamic_list") && cmd.action.config.arg === "context"}
            {@const hl        = highlight(cmd.phrase, typedInput)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="result-row"
              class:selected={i === selectedIndex}
              onmouseenter={() => (selectedIndex = i)}
              onmousedown={(e) => { e.preventDefault(); selectedIndex = i; }}
              onclick={() => executeCommand(cmd)}
            >
              <div class="result-content">
                <span class="result-title">{cmd.title}</span>
                <span class="result-subtext">
                  {#if ctxSetValue}
                    → set context to "{ctxSetValue}"
                  {:else if isContextArg && paramText.trim() === ""}
                    {cmd.phrase}<span class="param-hint">{activeContext.trim() !== "" ? ` → uses context "${activeContext}"` : " → type a value or set a context"}</span>
                  {:else if isParamMode}
                    {cmd.phrase}<span class="param-hint"> → {paramText}</span>
                  {:else}
                    {hl.before}<mark>{hl.match}</mark>{hl.after}
                  {/if}
                </span>
              </div>
              {#if actionBadge(cmd)}
                <span class="action-badge">{actionBadge(cmd)}</span>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  :global(*) { box-sizing: border-box; }

  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  /* When running as the unified Preferences window give the document a solid
     background so the transparent webview doesn't show through decorated chrome. */
  :global(html.preferences-window-mode),
  :global(html.preferences-window-mode body) {
    background: #1c1c1e;
    overflow: hidden;
    height: 100%;
  }

  /* ── Preferences window shell ────────────────────────────────────────── */
  .prefs-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    color: #f5f5f7;
    background: #1c1c1e;
  }

  .prefs-tabs {
    display: flex;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255,255,255,0.08);
    background: #1c1c1e;
    padding: 10px 12px 0;
    gap: 4px;
  }

  .prefs-tab {
    padding: 6px 18px;
    font-size: 13px;
    font-weight: 500;
    color: #fff;
    opacity: 0.65;
    cursor: pointer;
    border-radius: 6px 6px 0 0;
    border: none;
    background: none;
    font-family: inherit;
    transition: opacity 0.15s;
    user-select: none;
    -webkit-user-select: none;
  }
  .prefs-tab:hover { opacity: 0.85; }
  .prefs-tab.active {
    opacity: 1;
    background: #0a84ff;
  }

  /* The settings panel inside the preferences window fills the remaining height */
  :global(html.preferences-window-mode) .settings-panel {
    border-radius: 0;
    box-shadow: none;
    flex: 1;
    height: auto;
    min-height: 0;
    overflow-y: auto;
  }

  /* The cmd-editor inside preferences fills the remaining space */
  :global(html.preferences-window-mode) .cmd-editor {
    flex: 1;
    min-height: 0;
  }

  /* ── Launcher bar ────────────────────────────────────────────────────── */
  .launcher {
    background: rgba(28, 28, 30, 0.92);
    -webkit-backdrop-filter: blur(40px) saturate(1.8);
    backdrop-filter: blur(40px) saturate(1.8);
    border: 1.5px solid rgba(255,255,255,.45);
    border-radius: 12px;
    box-shadow:
      0 8px 32px rgba(0,0,0,.5),
      0 24px 64px rgba(0,0,0,.4);
    overflow: hidden;
  }

  /* ── Input row (input + context chip) ─────────────────────────────── */
  .input-row {
    display: flex;
    align-items: center;
    padding: 0 16px 0 0;
    gap: 0;
  }

  .prompt-glyph {
    font-size: 20px;
    color: rgba(245,245,247,.3);
    flex-shrink: 0;
    margin-left: 20px;
    line-height: 1;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    color: #f5f5f7;
    font-size: 18px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    padding: 20px 12px 20px 12px;
    outline: none;
    caret-color: #0a84ff;
  }

  input::placeholder { color: rgba(245,245,247,.35); }

  /* ── Context chip ───────────────────────────────────────────────────── */
  .context-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    background: rgba(10, 132, 255, 0.18);
    border: 1px solid rgba(10, 132, 255, 0.4);
    border-radius: 20px;
    padding: 3px 6px 3px 10px;
    flex-shrink: 0;
    max-width: 180px;
  }

  .debug-badge {
    color: #ff9f0a;
    font-size: 10px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    font-weight: 700;
    letter-spacing: 0.5px;
    background: rgba(255, 159, 10, 0.15);
    border: 1px solid rgba(255, 159, 10, 0.4);
    border-radius: 4px;
    padding: 2px 6px;
    flex-shrink: 0;
  }

  .chip-label {
    color: #0a84ff;
    font-size: 12px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chip-clear {
    background: none;
    border: none;
    color: rgba(10, 132, 255, 0.6);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    padding: 0 2px;
    border-radius: 50%;
    flex-shrink: 0;
    transition: color .12s;
  }

  .chip-clear:hover { color: #0a84ff; }

  /* ── Onboarding ──────────────────────────────────────────────────────── */
  .onboarding {
    background: rgba(28, 28, 30, 0.97);
    border-radius: 12px;
    box-shadow: 0 24px 64px rgba(0,0,0,.6), 0 0 0 1px rgba(255,255,255,.08);
    padding: 28px 28px 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    outline: none;
    height: 100%;
  }

  .ob-title {
    margin: 0;
    color: #f5f5f7;
    font-size: 17px;
    font-weight: 600;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .ob-sub {
    margin: 0;
    color: rgba(245,245,247,.5);
    font-size: 13px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    text-align: center;
    line-height: 1.5;
  }

  .shortcut-preview {
    background: rgba(255,255,255,.07);
    border: 1px solid rgba(255,255,255,.12);
    border-radius: 8px;
    padding: 10px 20px;
    color: rgba(245,245,247,.35);
    font-size: 15px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    min-width: 160px;
    text-align: center;
    transition: color .15s, border-color .15s;
  }

  .shortcut-preview.active {
    color: #f5f5f7;
    border-color: rgba(10,132,255,.6);
  }

  .ob-error {
    margin: 0;
    color: #ff453a;
    font-size: 12px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .ob-confirm {
    background: #0a84ff;
    color: #fff;
    border: none;
    border-radius: 8px;
    padding: 9px 24px;
    font-size: 14px;
    font-weight: 500;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    cursor: pointer;
    transition: opacity .15s;
  }

  .ob-confirm:disabled { opacity: .35; cursor: default; }

  /* ── Results list ────────────────────────────────────────────────────── */
  .results {
    border-top: 1px solid rgba(255,255,255,.07);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: calc(8 * 56px);
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,.45) transparent;
  }

  .results::-webkit-scrollbar { width: 6px; }
  .results::-webkit-scrollbar-track { background: transparent; }
  .results::-webkit-scrollbar-thumb {
    background: rgba(255,255,255,.45);
    border-radius: 3px;
  }
  .results::-webkit-scrollbar-thumb:hover {
    background: rgba(255,255,255,.65);
  }

  .result-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 10px 16px;
    border-radius: 8px;
    cursor: default;
    transition: background .12s, transform .12s;
    border-left: 3px solid transparent;
  }

  .result-content {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }

  .result-row.selected {
    background: rgba(255,255,255,.08);
    border-left-color: #0a84ff;
    transform: scale(1.005);
  }

  .result-title {
    color: #f5f5f7;
    font-size: 14px;
    font-weight: 500;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .result-subtext {
    color: rgba(245,245,247,.4);
    font-size: 12px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .error-item .result-title,
  .error-item .result-subtext {
    white-space: normal;
    overflow: visible;
    text-overflow: unset;
    word-break: break-word;
  }

  .result-subtext mark {
    background: transparent;
    color: #0a84ff;
    font-weight: 600;
  }

  .param-hint {
    color: #0a84ff;
    font-weight: 500;
  }

  .no-results {
    padding: 12px 16px;
    color: rgba(245,245,247,.5);
    font-size: 13px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    text-align: center;
  }

  .no-results-hint {
    font-size: 11px;
    opacity: 0.7;
  }

  /* ── Preferences window load-time warnings ──────────────────────────── */
  .prefs-warnings {
    border-bottom: 1px solid rgba(255, 159, 10, 0.25);
    background: rgba(255, 159, 10, 0.07);
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .prefs-warnings-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .prefs-warnings-label {
    font-size: 11px;
    font-weight: 600;
    color: #ff9f0a;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .prefs-warning-row {
    display: flex;
    flex-direction: column;
    padding: 4px 8px;
    background: rgba(255,159,10,0.08);
    border-radius: 4px;
    gap: 2px;
  }

  .prefs-warning-file {
    font-size: 11px;
    font-weight: 600;
    color: rgba(245,245,247,0.9);
    font-family: ui-monospace, "SF Mono", "Cascadia Code", monospace;
  }

  .prefs-warning-msg {
    font-size: 11px;
    color: rgba(245,245,247,0.6);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    word-break: break-word;
  }

  /* ── Duplicate warnings bar ─────────────────────────────────────────── */
  .warnings-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    height: 40px;
    background: rgba(255, 159, 10, 0.12);
    border-top: 1px solid rgba(255, 159, 10, 0.25);
  }

  .warnings-text {
    color: #ff9f0a;
    font-size: 12px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .warnings-dismiss {
    background: none;
    border: none;
    color: rgba(255, 159, 10, 0.6);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
    transition: color .15s;
  }

  .warnings-dismiss:hover { color: #ff9f0a; }

  /* ── Action-type badge ────────────────────────────────────────────────── */
  .action-badge {
    font-size: 10px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    font-weight: 500;
    color: rgba(245,245,247,.35);
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.08);
    border-radius: 4px;
    padding: 2px 6px;
    flex-shrink: 0;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .result-row.selected .action-badge {
    color: rgba(245,245,247,.5);
    background: rgba(255,255,255,.1);
  }

  /* ── Settings panel ──────────────────────────────────────────────────── */
  .settings-panel {
    background: rgba(28, 28, 30, 0.97);
    border-radius: 12px;
    box-shadow: 0 24px 64px rgba(0,0,0,.6), 0 0 0 1px rgba(255,255,255,.08);
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  /* ─── Auto-save bar ─── */
  .settings-autosave-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 16px;
    background: rgba(255,255,255,.02);
    border-bottom: 1px solid rgba(255,255,255,.07);
    flex-shrink: 0;
  }
  .settings-autosave-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: rgba(245,245,247,.35);
  }
  .settings-autosave-dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: #3a9d5c;
  }
  .settings-autosave-dot.saved { background: #0a84ff; }
  .settings-autosave-right { margin-left: auto; }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid rgba(255,255,255,.07);
    flex-shrink: 0;
  }

  .settings-title {
    color: #f5f5f7;
    font-size: 15px;
    font-weight: 600;
  }

  .settings-header-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .settings-saved-badge {
    color: #30d158;
    font-size: 12px;
    font-weight: 500;
  }

  .settings-done {
    background: #0a84ff;
    border: none;
    border-radius: 6px;
    color: #fff;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    padding: 5px 14px;
    cursor: pointer;
    transition: opacity .15s;
  }

  .settings-done:hover { opacity: .85; }

  .settings-restore {
    background: none;
    border: 1px solid rgba(255,255,255,.12);
    border-radius: 6px;
    color: rgba(245,245,247,.5);
    font-size: 12px;
    font-weight: 500;
    font-family: inherit;
    padding: 5px 14px;
    cursor: pointer;
    transition: color .15s, border-color .15s;
  }

  .settings-restore:hover {
    color: #ff453a;
    border-color: rgba(255, 69, 58, .4);
  }

  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 0 0 16px;
    background: rgba(255,255,255,.02);
  }

  /* ─── Settings sidebar nav ─── */
  .settings-layout {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
  .settings-nav {
    width: 180px;
    flex-shrink: 0;
    background: rgba(255,255,255,.02);
    border-right: 1px solid rgba(255,255,255,.07);
    padding: 14px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .settings-nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 12px;
    border-radius: 6px;
    background: none;
    border: none;
    color: #f5f5f7;
    font-family: inherit;
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
    transition: background .1s ease;
  }
  .settings-nav-item:hover { background: rgba(255,255,255,.04); }
  .settings-nav-item.active { background: #0a84ff; color: #fff; font-weight: 500; }
  .settings-nav-icon { width: 16px; flex-shrink: 0; opacity: .8; }

  /* ─── Section dividers ─── */
  .settings-section-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: rgba(245,245,247,.35);
    padding: 18px 16px 6px;
    border-top: 1px solid rgba(255,255,255,.05);
  }
  .settings-section-title:first-child { border-top: none; }

  .settings-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid rgba(255,255,255,.05);
  }

  .settings-row:last-child { border-bottom: none; }

  .settings-row-stacked {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .settings-row-top {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
  }

  /* ─── Row icon ─── */
  .settings-row-icon {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    background: rgba(10,132,255,.12);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #0a84ff;
    font-size: 16px;
  }

  .settings-row-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .row-title {
    color: #f5f5f7;
    font-size: 13px;
    font-weight: 500;
  }

  /* ─── Help tooltip ─── */
  .row-help {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: rgba(255,255,255,.08);
    color: rgba(245,245,247,.35);
    font-size: 10px;
    font-weight: 600;
    cursor: help;
    vertical-align: middle;
    margin-left: 4px;
  }

  .row-desc {
    color: rgba(245,245,247,.4);
    font-size: 11px;
  }

  .settings-code {
    font-family: "SF Mono", Menlo, monospace;
    font-size: 11px;
    opacity: .8;
  }

  .row-error {
    margin: 0;
    color: #ff453a;
    font-size: 11px;
    padding: 0 16px 8px;
  }

  .row-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  /* ─── Polished hotkey display ─── */
  .settings-hotkey-keys {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .settings-hotkey-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 26px;
    height: 26px;
    padding: 0 6px;
    background: linear-gradient(180deg, rgba(255,255,255,.12), rgba(255,255,255,.05));
    border: 1px solid rgba(255,255,255,.18);
    border-radius: 5px;
    font-size: 12px;
    font-weight: 500;
    box-shadow: 0 1px 0 rgba(0,0,0,.2);
    font-family: "SF Mono", Menlo, monospace;
    color: rgba(245,245,247,.7);
  }
  .settings-hotkey-plus {
    color: rgba(245,245,247,.25);
    font-size: 10px;
    margin: 0 1px;
  }

  .hotkey-badge {
    background: rgba(255,255,255,.08);
    border: 1px solid rgba(255,255,255,.12);
    border-radius: 5px;
    color: rgba(245,245,247,.7);
    font-size: 12px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    padding: 3px 8px;
    white-space: nowrap;
  }

  .hotkey-capture-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .shortcut-preview.compact {
    padding: 5px 12px;
    font-size: 12px;
    min-width: 130px;
  }

  /* ─── Restore Defaults confirmation modal ─── */
  .settings-modal-overlay {
    position: absolute;
    inset: 0;
    background: rgba(0,0,0,.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    border-radius: 12px;
  }
  .settings-modal {
    background: #252526;
    border: 1px solid rgba(255,255,255,.15);
    border-radius: 10px;
    padding: 22px 24px;
    max-width: 380px;
    box-shadow: 0 24px 80px rgba(0,0,0,.6);
  }
  .settings-modal-title {
    font-size: 14px;
    font-weight: 600;
    color: #f5f5f7;
    margin-bottom: 8px;
  }
  .settings-modal-body {
    font-size: 12.5px;
    color: rgba(245,245,247,.5);
    line-height: 1.5;
    margin-bottom: 18px;
  }
  .settings-modal-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  /* ─── About section ─── */
  .settings-about-card {
    background: rgba(255,255,255,.02);
    border: 1px solid rgba(255,255,255,.07);
    border-radius: 8px;
    padding: 16px;
    display: flex;
    gap: 14px;
    align-items: center;
    margin: 8px 16px;
  }
  .settings-about-logo {
    width: 48px;
    height: 48px;
    border-radius: 10px;
    background: linear-gradient(135deg, #0a84ff, #5e5ce6);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 24px;
    font-weight: 700;
    color: #fff;
    flex-shrink: 0;
  }
  .settings-about-info { flex: 1; }
  .settings-about-name { font-size: 16px; font-weight: 600; color: #f5f5f7; }
  .settings-about-version { font-size: 12px; color: rgba(245,245,247,.5); margin-top: 2px; }
  .settings-about-links { display: flex; gap: 6px; margin-top: 8px; align-items: center; }
  .settings-about-link {
    font-size: 11px;
    color: #0a84ff;
    text-decoration: none;
    cursor: pointer;
  }
  .settings-about-link:hover { text-decoration: underline; }
  .settings-about-sep { color: rgba(245,245,247,.25); font-size: 10px; }

  .action-btn {
    background: rgba(255,255,255,.08);
    border: 1px solid rgba(255,255,255,.12);
    border-radius: 6px;
    color: #f5f5f7;
    font-size: 12px;
    font-weight: 500;
    font-family: inherit;
    padding: 5px 12px;
    cursor: pointer;
    flex-shrink: 0;
    transition: background .15s;
  }

  .action-btn:hover { background: rgba(255,255,255,.14); }
  .action-btn:disabled { opacity: .35; cursor: default; }
  .action-btn.confirm { background: rgba(10,132,255,.25); border-color: rgba(10,132,255,.4); color: #0a84ff; }
  .action-btn.confirm:hover:not(:disabled) { background: rgba(10,132,255,.35); }
  .action-btn.cancel { background: rgba(255,255,255,.06); }

  /* Toggle switch */
  .toggle {
    width: 40px;
    height: 24px;
    border-radius: 12px;
    background: rgba(255,255,255,.12);
    border: 1px solid rgba(255,255,255,.1);
    position: relative;
    cursor: pointer;
    flex-shrink: 0;
    transition: background .2s, border-color .2s;
    outline: none;
  }

  .toggle:focus-visible {
    box-shadow: 0 0 0 2px rgba(10,132,255,.6);
  }

  .toggle.on {
    background: #0a84ff;
    border-color: transparent;
  }

  .toggle .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: rgba(245,245,247,.6);
    transition: transform .2s, background .2s;
  }

  .toggle.on .thumb {
    transform: translateX(16px);
    background: #fff;
  }

  /* Commands directory text input */
  .settings-text-input {
    width: 100%;
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 7px;
    color: #f5f5f7;
    font-size: 12px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    padding: 7px 10px;
    outline: none;
    transition: border-color .15s;
  }

  .settings-text-input:focus {
    border-color: rgba(10,132,255,.5);
  }

  .settings-text-input::placeholder {
    color: rgba(245,245,247,.25);
  }

  /* Commands directory row with browse button */
  .settings-dir-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .settings-dir-display {
    flex: 1;
    min-width: 0;
    background: rgba(255,255,255,.04);
    border: 1px solid rgba(255,255,255,.08);
    border-radius: 7px;
    padding: 7px 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: default;
  }

  .settings-dir-display .dir-path {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 12px;
    color: #f5f5f7;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .settings-dir-default .dir-path {
    color: rgba(245,245,247,.45);
  }

  .dir-default-badge {
    flex-shrink: 0;
    font-size: 10px;
    font-weight: 500;
    color: rgba(245,245,247,.35);
    background: rgba(255,255,255,.06);
    border-radius: 4px;
    padding: 1px 6px;
    text-transform: uppercase;
    letter-spacing: .04em;
  }

  .settings-dir-clear-btn {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 6px;
    color: rgba(245,245,247,.5);
    font-size: 12px;
    cursor: pointer;
    transition: background .15s, color .15s;
    padding: 0;
  }

  .settings-dir-clear-btn:hover {
    background: rgba(255,80,80,.15);
    color: #ff6b6b;
    border-color: rgba(255,80,80,.25);
  }

  .settings-browse-btn {
    flex-shrink: 0;
    background: rgba(255,255,255,.08);
    border: 1px solid rgba(255,255,255,.12);
    border-radius: 7px;
    color: #f5f5f7;
    font-size: 12px;
    padding: 7px 14px;
    cursor: pointer;
    transition: background .15s, border-color .15s;
    white-space: nowrap;
  }

  .settings-browse-btn:hover {
    background: rgba(255,255,255,.14);
    border-color: rgba(255,255,255,.2);
  }

  .settings-browse-btn:active {
    background: rgba(255,255,255,.06);
  }

  /* ── Command editor ──────────────────────────────────────────────────── */
  .cmd-editor {
    display: flex;
    flex-direction: column;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    color: #f5f5f7;
    background: #1c1c1e;
    height: 100%;
  }

  /* ─── Toolbar ─── */
  .cmd-toolbar {
    display: flex;
    align-items: center;
    height: 44px;
    padding: 0 12px;
    background: rgba(255,255,255,.025);
    border-bottom: 1px solid rgba(255,255,255,.07);
    flex-shrink: 0;
    gap: 8px;
  }
  .cmd-toolbar-left { display: flex; align-items: center; gap: 6px; }
  .cmd-toolbar-right { display: flex; align-items: center; gap: 6px; margin-left: auto; }

  .cmd-tb-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 28px;
    padding: 0 10px;
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 6px;
    color: #ccc;
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
    gap: 5px;
    transition: all .12s ease;
    white-space: nowrap;
  }
  .cmd-tb-btn:hover { background: rgba(255,255,255,.1); border-color: rgba(255,255,255,.16); }
  .cmd-tb-btn:active { background: rgba(255,255,255,.04); transform: scale(0.97); }
  .cmd-tb-btn:disabled { opacity: .35; pointer-events: none; }

  .cmd-tb-btn-primary {
    background: #0a84ff;
    border-color: transparent;
    color: #fff;
    font-weight: 500;
  }
  .cmd-tb-btn-primary:hover { background: #1a8ad4; }

  .cmd-tb-btn-danger { color: #c74e4e; border-color: rgba(199,78,78,.25); }
  .cmd-tb-btn-danger:hover { background: rgba(199,78,78,.12); color: #d65f5f; }

  .cmd-tb-icon { font-size: 15px; line-height: 1; font-weight: 300; }
  .cmd-tb-kbd {
    font-size: 10px;
    opacity: .5;
    padding: 1px 4px;
    background: rgba(0,0,0,.25);
    border-radius: 3px;
    font-family: "SF Mono", Menlo, monospace;
  }
  .cmd-tb-separator { width: 1px; height: 20px; background: rgba(255,255,255,.07); margin: 0 4px; }

  /* ─── Segmented group control ─── */
  .cmd-seg-group {
    display: inline-flex;
    height: 28px;
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 6px;
    overflow: hidden;
  }
  .cmd-seg-label {
    padding: 0 10px;
    display: inline-flex;
    align-items: center;
    font-size: 11px;
    color: rgba(245,245,247,.4);
    border-right: 1px solid rgba(255,255,255,.1);
  }
  .cmd-seg-btn {
    padding: 0 10px;
    background: none;
    border: none;
    color: #ccc;
    font-size: 12px;
    cursor: pointer;
    font-family: inherit;
    border-right: 1px solid rgba(255,255,255,.06);
  }
  .cmd-seg-btn:last-child { border-right: none; }
  .cmd-seg-btn.active { background: rgba(10,132,255,.2); color: #5bb8ff; }
  .cmd-seg-btn:hover:not(.active) { background: rgba(255,255,255,.06); }

  .cmd-search-wrapper { position: relative; }
  .cmd-search-wrapper::before {
    content: "⌕";
    position: absolute;
    left: 9px; top: 50%;
    transform: translateY(-50%);
    font-size: 14px;
    color: rgba(245,245,247,.35);
    pointer-events: none;
    z-index: 1;
  }
  .cmd-search-box {
    height: 28px;
    width: 180px;
    padding: 0 8px 0 28px;
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 6px;
    color: #f5f5f7;
    font-size: 12px;
    font-family: inherit;
    outline: none;
    transition: all .15s ease;
  }
  .cmd-search-box:focus { border-color: rgba(10,132,255,.5); background: rgba(255,255,255,.08); width: 220px; }
  .cmd-search-box::placeholder { color: rgba(245,245,247,.3); }

  /* ─── Column header ─── */
  .cmd-col-header {
    display: grid;
    grid-template-columns: 28px 200px 1fr 120px 90px 60px;
    align-items: center;
    height: 28px;
    padding: 0 16px;
    background: rgba(255,255,255,.02);
    border-bottom: 1px solid rgba(255,255,255,.07);
    position: sticky;
    top: 0;
    z-index: 10;
    flex-shrink: 0;
  }
  .cmd-col-header span {
    font-size: 11px;
    font-weight: 600;
    color: rgba(245,245,247,.4);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .cmd-col-check { display: flex; align-items: center; justify-content: center; }
  .cmd-bulk-header-cb { cursor: pointer; }
  .cmd-col-sortable {
    cursor: pointer;
    user-select: none;
    transition: color .15s;
  }
  .cmd-col-sortable:hover { color: rgba(245,245,247,.6); }
  .cmd-col-sortable.sorted { color: #0a84ff; }
  .cmd-col-center { text-align: center; }

  /* ─── Table container ─── */
  .cmd-table-container {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    background: #1c1c1e;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,.15) transparent;
  }
  .cmd-table-container::-webkit-scrollbar { width: 7px; }
  .cmd-table-container::-webkit-scrollbar-track { background: transparent; }
  .cmd-table-container::-webkit-scrollbar-thumb { background: rgba(255,255,255,.15); border-radius: 4px; }
  .cmd-table-container::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,.25); }

  .cmd-table-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 48px 20px;
    color: rgba(245,245,247,.35);
  }
  .cmd-empty-icon { font-size: 48px; opacity: .3; }
  .cmd-empty-title { font-size: 16px; font-weight: 500; color: #e8e8e8; }
  .cmd-empty-desc { font-size: 12px; max-width: 320px; text-align: center; line-height: 1.5; }
  .cmd-empty-actions { display: flex; gap: 8px; margin-top: 8px; }

  /* ─── Folder group ─── */
  .cmd-tbl-folder-header {
    display: flex;
    align-items: center;
    width: 100%;
    height: 30px;
    padding: 0 12px;
    background: linear-gradient(180deg, rgba(255,255,255,.035) 0%, rgba(255,255,255,.015) 100%);
    border: none;
    border-bottom: 1px solid rgba(255,255,255,.04);
    border-top: 1px solid rgba(255,255,255,.04);
    cursor: pointer;
    font-family: inherit;
    color: inherit;
    gap: 6px;
    transition: background .1s ease;
  }
  .cmd-folder-group:first-child .cmd-tbl-folder-header { border-top: none; }
  .cmd-tbl-folder-header:hover { background: rgba(255,255,255,.05); }

  .cmd-tbl-chevron {
    font-size: 10px;
    color: rgba(245,245,247,.4);
    width: 14px;
    text-align: center;
    transition: transform .15s ease;
    display: inline-block;
  }
  .cmd-tbl-chevron.collapsed { transform: rotate(-90deg); }
  .cmd-tbl-folder-svg { color: rgba(245,245,247,.4); flex-shrink: 0; }
  .cmd-tbl-group-icon { font-size: 12px; opacity: .6; flex-shrink: 0; }
  .cmd-tbl-folder-name { font-size: 11.5px; font-weight: 600; color: rgba(245,245,247,.5); letter-spacing: 0.02em; }
  .cmd-tbl-folder-disabled {
    font-size: 9px;
    padding: 1px 5px;
    border-radius: 3px;
    background: rgba(240,185,66,.15);
    color: #f0b942;
    margin-left: auto;
  }
  .cmd-tbl-folder-count { font-size: 10px; color: rgba(245,245,247,.3); margin-left: 4px; }
  .cmd-tbl-folder-disabled + .cmd-tbl-folder-count { margin-left: 8px; }

  /* ─── Command row ─── */
  .cmd-tbl-row {
    display: grid;
    grid-template-columns: 28px 200px 1fr 120px 90px 60px;
    align-items: center;
    height: 34px;
    padding: 0 16px;
    border-bottom: 1px solid rgba(255,255,255,.03);
    cursor: default;
    transition: background .08s ease;
    position: relative;
  }
  .cmd-tbl-row:hover { background: rgba(255,255,255,.04); }
  .cmd-tbl-row.selected { background: rgba(10,132,255,.25); }
  .cmd-tbl-row.selected:hover { background: rgba(10,132,255,.3); }
  .cmd-tbl-row.disabled-row .cmd-tbl-phrase,
  .cmd-tbl-row.disabled-row .cmd-tbl-title,
  .cmd-tbl-row.disabled-row .cmd-tbl-badge { opacity: .4; }

  .cmd-tbl-phrase {
    font-family: "SF Mono", "Fira Code", "Cascadia Code", Menlo, monospace;
    font-size: 12px;
    color: #e8e8e8;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding-right: 12px;
    font-weight: 500;
    letter-spacing: -0.02em;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .cmd-tbl-status-warn {
    font-size: 11px;
    color: #f0b942;
    flex-shrink: 0;
    cursor: help;
  }
  .cmd-tbl-title {
    font-size: 12.5px;
    color: #ccc;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding-right: 12px;
  }
  .cmd-tbl-modified {
    font-size: 11px;
    color: rgba(245,245,247,.4);
    white-space: nowrap;
  }

  /* ─── Action badges ─── */
  .cmd-tbl-badge {
    display: inline-flex;
    align-items: center;
    height: 20px;
    padding: 0 7px;
    border-radius: 4px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
    gap: 5px;
  }
  .cmd-tbl-badge-icon {
    font-size: 11px;
    line-height: 1;
  }
  .cmd-tbl-badge-open_url       { background: rgba(45,125,154,.18); color: #5bb8d4; }
  .cmd-tbl-badge-paste_text     { background: rgba(124,92,191,.18); color: #b49adf; }
  .cmd-tbl-badge-copy_text      { background: rgba(92,138,191,.18); color: #8fb8e0; }
  .cmd-tbl-badge-static_list    { background: rgba(154,123,45,.18); color: #d4b95b; }
  .cmd-tbl-badge-dynamic_list   { background: rgba(45,154,110,.18); color: #5bd4a3; }
  .cmd-tbl-badge-script_action  { background: rgba(181,90,48,.18); color: #e0915b; }

  /* ─── Toggle switch ─── */
  .cmd-tbl-toggle-cell { display: flex; justify-content: center; }
  .cmd-tbl-toggle {
    position: relative;
    width: 32px; height: 18px;
    cursor: pointer;
    display: inline-block;
  }
  .cmd-tbl-toggle input { opacity: 0; width: 0; height: 0; position: absolute; }
  .cmd-tbl-toggle-track {
    position: absolute; inset: 0;
    background: #555;
    border-radius: 9px;
    transition: background .2s ease;
  }
  .cmd-tbl-toggle input:checked + .cmd-tbl-toggle-track { background: #3a9d5c; }
  .cmd-tbl-toggle-knob {
    position: absolute; top: 2px; left: 2px;
    width: 14px; height: 14px;
    background: #fff;
    border-radius: 50%;
    transition: transform .2s ease;
    box-shadow: 0 1px 3px rgba(0,0,0,.3);
  }
  .cmd-tbl-toggle input:checked ~ .cmd-tbl-toggle-knob { transform: translateX(14px); }

  /* ─── Bulk-select checkbox ─── */
  .cmd-tbl-check-cell {
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .cmd-row-checkbox {
    cursor: pointer;
    opacity: 0;
    transition: opacity .1s ease;
  }
  .cmd-tbl-row:hover .cmd-row-checkbox,
  .cmd-tbl-row.checked .cmd-row-checkbox { opacity: 1; }

  /* ─── Bulk-action bar ─── */
  .cmd-bulk-bar {
    display: none;
    align-items: center;
    height: 36px;
    padding: 0 16px;
    background: rgba(10,132,255,.15);
    border-bottom: 1px solid rgba(10,132,255,.3);
    gap: 8px;
    font-size: 12px;
    flex-shrink: 0;
  }
  .cmd-bulk-bar.visible { display: flex; }
  .cmd-bulk-count { font-weight: 500; color: #5bb8ff; }
  .cmd-bulk-actions { display: flex; gap: 6px; margin-left: auto; }

  /* ─── Status bar ─── */
  .cmd-statusbar {
    display: flex;
    align-items: center;
    height: 26px;
    padding: 0 14px;
    background: rgba(255,255,255,.025);
    border-top: 1px solid rgba(255,255,255,.07);
    font-size: 11px;
    color: rgba(245,245,247,.4);
    flex-shrink: 0;
    gap: 16px;
  }
  .cmd-statusbar-right { margin-left: auto; }
  .cmd-statusbar kbd {
    display: inline-block;
    padding: 1px 5px;
    background: rgba(255,255,255,.08);
    border: 1px solid rgba(255,255,255,.12);
    border-radius: 3px;
    font-size: 10px;
    font-family: inherit;
    color: rgba(245,245,247,.5);
  }
  .cmd-statusbar-pill-warn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 7px;
    border-radius: 9px;
    font-weight: 500;
    font-size: 11px;
    background: rgba(240,185,66,.15);
    color: #f0b942;
  }

  /* ─── Context menu ─── */
  .cmd-ctx-backdrop {
    position: fixed; inset: 0;
    z-index: 1000;
  }
  .cmd-ctx-menu {
    position: fixed;
    background: #252526;
    border: 1px solid rgba(255,255,255,.15);
    border-radius: 6px;
    padding: 4px 0;
    min-width: 160px;
    box-shadow: 0 8px 32px rgba(0,0,0,.5);
    z-index: 1001;
  }
  .cmd-ctx-item {
    display: flex;
    align-items: center;
    width: 100%;
    height: 28px;
    padding: 0 12px;
    font-size: 12.5px;
    color: #ccc;
    cursor: default;
    gap: 8px;
    border: none;
    background: none;
    font-family: inherit;
    text-align: left;
  }
  .cmd-ctx-item:hover { background: #0a84ff; color: #fff; border-radius: 3px; margin: 0 4px; padding: 0 8px; }
  .cmd-ctx-separator { height: 1px; background: rgba(255,255,255,.07); margin: 4px 8px; }
  .cmd-ctx-shortcut { margin-left: auto; font-size: 11px; color: rgba(245,245,247,.35); }
  .cmd-ctx-item:hover .cmd-ctx-shortcut { color: rgba(255,255,255,.6); }
  .cmd-ctx-icon { width: 16px; text-align: center; font-size: 13px; }
  .cmd-ctx-danger { color: #c74e4e; }
  .cmd-ctx-danger:hover { background: #c74e4e; color: #fff; }

  /* ─── Detail editor view ─── */
  .cmd-detail-view {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }
  .cmd-detail-bar {
    display: flex;
    align-items: center;
    height: 44px;
    padding: 0 12px;
    background: rgba(255,255,255,.025);
    border-bottom: 1px solid rgba(255,255,255,.07);
    flex-shrink: 0;
    gap: 12px;
  }
  .cmd-detail-bar-title {
    font-size: 13px;
    font-weight: 500;
    color: rgba(245,245,247,.6);
  }
  .cmd-detail-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,.15) transparent;
  }

  .cmd-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
    flex: 1;
  }

  .cmd-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .cmd-field-inline {
    flex-direction: row;
    align-items: center;
  }

  .cmd-label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    color: rgba(245,245,247,.45);
  }

  .cmd-input {
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 7px;
    color: #f5f5f7;
    font-size: 13px;
    font-family: inherit;
    padding: 8px 10px;
    outline: none;
    transition: border-color .15s;
  }
  .cmd-input:focus { border-color: rgba(10,132,255,.5); }
  .cmd-input::placeholder { color: rgba(245,245,247,.2); }
  .cmd-input:disabled { opacity: .5; cursor: default; }
  .cmd-input.error { border-color: rgba(255,69,58,.6); }

  .cmd-select {
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 7px;
    color: #f5f5f7;
    font-size: 13px;
    font-family: inherit;
    padding: 8px 10px;
    outline: none;
    cursor: pointer;
    appearance: auto;
  }
  .cmd-select:focus { border-color: rgba(10,132,255,.5); }

  .cmd-textarea {
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 7px;
    color: #f5f5f7;
    font-size: 13px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    padding: 8px 10px;
    outline: none;
    resize: vertical;
    min-height: 80px;
    transition: border-color .15s;
    line-height: 1.55;
  }
  .cmd-textarea:focus { border-color: rgba(10,132,255,.5); }
  .cmd-textarea::placeholder { color: rgba(245,245,247,.2); }

  .cmd-field-hint {
    font-size: 11px;
    color: rgba(245,245,247,.35);
    line-height: 1.5;
  }
  .cmd-field-hint-ok { color: #30d158; }

  .cmd-field-error {
    font-size: 11px;
    color: #ff453a;
  }

  .cmd-checkbox-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: rgba(245,245,247,.7);
    cursor: pointer;
  }

  .cmd-save-error {
    font-size: 12px;
    color: #ff453a;
    margin: 0;
    background: rgba(255,69,58,.1);
    border: 1px solid rgba(255,69,58,.25);
    border-radius: 6px;
    padding: 8px 10px;
  }

  .cmd-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    margin-top: auto;
    padding-top: 8px;
    border-top: 1px solid rgba(255,255,255,.06);
  }

  .cmd-delete-confirm-text {
    font-size: 12px;
    color: rgba(245,245,247,.5);
    margin-right: auto;
  }

  .cmd-btn-primary {
    background: #0a84ff;
    border: none;
    border-radius: 7px;
    color: #fff;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    padding: 7px 18px;
    cursor: pointer;
    transition: opacity .15s;
  }
  .cmd-btn-primary:hover:not(:disabled) { opacity: .85; }
  .cmd-btn-primary:disabled { opacity: .35; cursor: default; }

  .cmd-btn-ghost {
    background: rgba(255,255,255,.07);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 7px;
    color: rgba(245,245,247,.7);
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    padding: 7px 14px;
    cursor: pointer;
    transition: background .15s;
  }
  .cmd-btn-ghost:hover { background: rgba(255,255,255,.12); }

  .cmd-btn-ghost-danger {
    margin-right: auto;
    color: rgba(255,69,58,.7);
    border-color: rgba(255,69,58,.2);
  }
  .cmd-btn-ghost-danger:hover { color: #ff453a; background: rgba(255,69,58,.1); border-color: rgba(255,69,58,.35); }

  .cmd-btn-danger {
    background: rgba(255,69,58,.2);
    border: 1px solid rgba(255,69,58,.4);
    border-radius: 7px;
    color: #ff453a;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    padding: 7px 14px;
    cursor: pointer;
    transition: background .15s;
  }
  .cmd-btn-danger:hover { background: rgba(255,69,58,.3); }

  /* Folder selection for new commands */
  .cmd-folder-input-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .cmd-folder-select { flex: 1; min-width: 0; }
  .cmd-folder-new-input { flex: 1; min-width: 0; }

  .cmd-btn-tiny {
    background: rgba(255,255,255,.07);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 5px;
    color: rgba(245,245,247,.6);
    font-size: 11px;
    font-family: inherit;
    padding: 4px 8px;
    cursor: pointer;
    white-space: nowrap;
    transition: background .15s;
    flex-shrink: 0;
  }
  .cmd-btn-tiny:hover { background: rgba(255,255,255,.12); }
  .cmd-btn-tiny:disabled { opacity: .4; cursor: default; }

  .cmd-btn-create-script { color: #0a84ff; border-color: rgba(10,132,255,.3); }
  .cmd-btn-create-script:hover { background: rgba(10,132,255,.1); }

  .cmd-btn-test { color: rgba(245,245,247,.7); }
  .cmd-btn-test:hover:not(:disabled) { background: rgba(255,255,255,.12); }

  .cmd-test-arg-input {
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 8px;
    font-size: 11px;
    border-radius: 5px;
    border: 1px solid rgba(255,255,255,.12);
    background: rgba(255,255,255,.06);
    color: rgba(245,245,247,.9);
  }
  .cmd-test-arg-input::placeholder { color: rgba(245,245,247,.35); }

  /* Test run result panel */
  .cmd-test-result {
    margin-top: 8px;
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 7px;
    overflow: hidden;
    font-size: 11px;
  }
  .cmd-test-result.cmd-test-error { border-color: rgba(255,69,58,.35); }
  .cmd-test-result-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: rgba(255,255,255,.04);
    border-bottom: 1px solid rgba(255,255,255,.06);
  }
  .cmd-test-result.cmd-test-error .cmd-test-result-meta { background: rgba(255,69,58,.07); border-bottom-color: rgba(255,69,58,.15); }
  .cmd-test-badge {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 600;
  }
  .cmd-test-badge-ok { background: rgba(48,209,88,.18); color: #30d158; }
  .cmd-test-badge-err { background: rgba(255,69,58,.18); color: #ff453a; }
  .cmd-test-duration { color: rgba(245,245,247,.35); font-size: 10px; font-family: ui-monospace, "SF Mono", Menlo, monospace; }
  .cmd-test-output {
    margin: 0;
    padding: 8px 10px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11px;
    color: rgba(245,245,247,.75);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 140px;
    overflow-y: auto;
    background: rgba(0,0,0,.15);
  }
  .cmd-test-stderr { color: #ff6961; background: rgba(255,69,58,.06); }
  .cmd-test-empty { display: block; padding: 8px 10px; color: rgba(245,245,247,.3); font-style: italic; }

  .cmd-location-path {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11px;
  }

  /* Actions row left group */
  .cmd-actions-left {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
  }

  .cmd-btn-icon {
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.08);
    border-radius: 5px;
    color: rgba(245,245,247,.5);
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background .15s, color .15s;
    padding: 0;
  }
  .cmd-btn-icon:hover { background: rgba(255,255,255,.12); color: rgba(245,245,247,.8); }

  /* Inline script editor */
  .cmd-script-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .cmd-script-dirty-badge {
    font-size: 9px;
    font-weight: 500;
    color: #ff9f0a;
    background: rgba(255,159,10,.12);
    border-radius: 4px;
    padding: 1px 5px;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .cmd-script-textarea {
    min-height: 140px;
    font-size: 12px;
    line-height: 1.5;
    tab-size: 2;
  }

  .cmd-script-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: flex-end;
  }

  .cmd-script-status {
    font-size: 11px;
    color: rgba(245,245,247,.35);
    padding: 8px 0;
  }

  .cmd-script-not-found {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: rgba(245,245,247,.35);
    background: rgba(255,255,255,.03);
    border: 1px dashed rgba(255,255,255,.1);
    border-radius: 7px;
    padding: 10px 12px;
  }
  .cmd-script-not-found span { flex: 1; }

  .cmd-script-external-hint {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: rgba(245,245,247,.35);
    background: rgba(255,255,255,.03);
    border: 1px solid rgba(255,255,255,.06);
    border-radius: 7px;
    padding: 10px 12px;
    line-height: 1.5;
  }
  .cmd-script-external-hint span { flex: 1; }

</style>
