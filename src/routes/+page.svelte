<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { listen, emit } from "@tauri-apps/api/event";
  import type { Action, AppSettings, Command, CommandFileMeta, CommandsPayload, DuplicateWarning, ListItem, ReservedPhraseWarning } from "$lib/types";

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
  let warningsDismissed = $state(false);
  const totalWarnings = $derived(warnings.length + reservedWarnings.length);
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
  let cmdScriptLoading = $state(false);
  let cmdScriptDirty = $state(false);
  let cmdScriptSaving = $state(false);
  let cmdScriptError = $state("");
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
  // Group filtered commands by source_dir for sidebar folder view
  const cmdGroupedList = $derived(() => {
    const groups: { folder: string; label: string; items: CommandFileMeta[] }[] = [];
    const map = new Map<string, CommandFileMeta[]>();
    for (const c of cmdFilteredList) {
      const key = c.source_dir || "";
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(c);
    }
    // Sort folders: root first, then alphabetically
    const keys = [...map.keys()].sort((a, b) => {
      if (a === "" && b !== "") return -1;
      if (a !== "" && b === "") return 1;
      return a.localeCompare(b);
    });
    for (const key of keys) {
      groups.push({
        folder: key,
        label: key || "Commands",
        items: map.get(key)!,
      });
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
      title: "Contexts — scoped matching with /ctx set and /ctx reset",
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
  // Inline error from script_action execution (shown as an error row in the results area)
  let actionError = $state<string | null>(null);
  let resultsEl: HTMLDivElement | undefined = $state();

  // ── Filtering & navigation ─────────────────────────────────────────────
  const MAX_RESULTS = 8;
  const ROW_H = 56; // px per result row

  // Human-readable badge label for each action type
  function actionBadge(cmd: { action: Action }): string {
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

  // When a context is active and the user is not typing a / command, append
  // the context to raw input so commands are matched against the full phrase.
  // However, if the raw input already places us in "param mode" for a known
  // command (i.e. the user typed the full phrase + extra text), do NOT append
  // context — the trailing text is a user-supplied parameter, not a phrase
  // fragment.  Scripts can still read the context via NIMBLE_CONTEXT env var.
  const rawInParamMode = $derived(
    (() => {
      const raw = input.trim().toLowerCase();
      if (raw === "" || raw.startsWith("/")) return false;
      return commands.some(cmd => raw.startsWith(cmd.phrase.toLowerCase() + " "));
    })()
  );

  const effectiveInput = $derived(
    activeContext && input.trim() !== "" && !input.trim().startsWith("/") && !rawInParamMode
      ? input.trim() + " " + activeContext
      : input.trim()
  );

  const filtered = $derived(
    effectiveInput === ""
      ? []
      : (() => {
          const typed = effectiveInput.toLowerCase();
          const matches = commands.filter(cmd => {
            const phrase = cmd.phrase.toLowerCase();
            // Standard partial/substring match (discovery while typing)
            // OR param mode: user has typed the full phrase + space + param text
            return phrase.includes(typed) || typed.startsWith(phrase + " ");
          });
          // Longest-phrase-wins: when multiple commands match in param mode,
          // sort the longer phrase first so it is the default Enter target.
          return matches.slice().sort((a, b) => {
            const ap = a.phrase.toLowerCase();
            const bp = b.phrase.toLowerCase();
            const aParam = typed.startsWith(ap + " ");
            const bParam = typed.startsWith(bp + " ");
            if (aParam && bParam) return bp.length - ap.length;
            return 0;
          });
        })()
  );

  // Built-in / commands filtered by the current raw input (only when input starts with "/")
  const filteredBuiltins: Command[] = $derived(
    input.trim().startsWith("/")
      ? (() => {
          const typed = input.trim().toLowerCase();
          const matches = builtinCommands.filter(cmd => {
            const phrase = cmd.phrase.toLowerCase();
            return phrase.includes(typed) || typed.startsWith(phrase + " ");
          });
          return matches.slice().sort((a, b) => {
            const ap = a.phrase.toLowerCase();
            const bp = b.phrase.toLowerCase();
            const aParam = typed.startsWith(ap + " ");
            const bParam = typed.startsWith(bp + " ");
            if (aParam && bParam) return bp.length - ap.length;
            return 0;
          });
        })()
      : []
  );

  // Combined results: built-ins first, then YAML commands
  const allFiltered = $derived([...filteredBuiltins, ...filtered]);

  // True when the typed input matches a list command and we should show list UI.
  // Includes the empty-resolved state for dynamic lists ("No results" feedback).
  const showingList = $derived(
    activeListCmd !== null && (
      listItems.length > 0 ||
      (activeListCmd.action.type === "dynamic_list" && dynamicListLoaded)
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
  // Uses effectiveInput so context-suffixed phrases are matched correctly.
  // Returns a cleanup that cancels any in-flight debounce timer.
  $effect(() => {
    const typed = effectiveInput.toLowerCase();

    // ── static_list: exact match only ─────────────────────────────────
    const staticMatch = commands.find(
      cmd => cmd.action.type === "static_list" && cmd.phrase.toLowerCase() === typed
    ) ?? null;

    if (staticMatch && staticMatch.action.type === "static_list") {
      const listName = staticMatch.action.config.list;
      const commandDir = staticMatch.source_dir;
      activeListCmd = staticMatch;
      invoke<ListItem[]>("load_list", { commandDir, listName, inlineEnv: staticMatch.env, context: activeContext, phrase: staticMatch.phrase })
        .then(items => { listItems = items; selectedIndex = 0; })
        .catch((err) => {
          listItems = [{ title: "⚠️ Error loading list", subtext: String(err) }];
        });
      return;
    }

    // ── dynamic_list: exact match OR phrase + space + suffix ───────────
    const dynMatch = commands.find(cmd => {
      if (cmd.action.type !== "dynamic_list") return false;
      const phrase = cmd.phrase.toLowerCase();
      return typed === phrase || typed.startsWith(phrase + " ");
    }) ?? null;

    if (dynMatch && dynMatch.action.type === "dynamic_list") {
      const phrase = dynMatch.phrase.toLowerCase();
      const config = dynMatch.action.config;
      const isExact = typed === phrase;
      const suffix = typed.startsWith(phrase + " ") ? typed.slice(phrase.length + 1).trim() : "";
      const argMode = config.arg ?? "none";

      let timer: ReturnType<typeof setTimeout> | null = null;

      const commandDir = dynMatch.source_dir;

      const runDynamic = (arg: string | null) => {
        dynamicListLoaded = false;
        invoke<ListItem[]>("run_dynamic_list", { commandDir, scriptName: config.script, arg, context: activeContext, phrase: dynMatch.phrase, inlineEnv: dynMatch.env })
          .then(items => { listItems = items; selectedIndex = 0; dynamicListLoaded = true; })
          .catch((err) => {
            listItems = [{ title: "⚠️ Script error", subtext: String(err) }];
            dynamicListLoaded = true;
          });
      };

      if (argMode === "none") {
        if (isExact) {
          activeListCmd = dynMatch;
          runDynamic(null);
        } else {
          activeListCmd = null;
          listItems = [];
          dynamicListLoaded = false;
        }
      } else if (argMode === "optional") {
        activeListCmd = dynMatch;
        if (isExact) {
          runDynamic(null);
        } else {
          timer = setTimeout(() => runDynamic(suffix), 200);
        }
      } else {
        // required: only invoke when suffix is non-empty
        if (suffix) {
          activeListCmd = dynMatch;
          timer = setTimeout(() => runDynamic(suffix), 200);
        } else {
          activeListCmd = null;
          listItems = [];
          dynamicListLoaded = false;
        }
      }

      return () => { if (timer !== null) clearTimeout(timer); };
    }

    // No list match
    activeListCmd = null;
    listItems = [];
    dynamicListLoaded = false;
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

  // ── Highlight helper ──────────────────────────────────────────────────
  function highlight(phrase: string, query: string) {
    const q = query.trim().toLowerCase();
    const idx = phrase.toLowerCase().indexOf(q);
    if (idx === -1 || q === "") return { before: phrase, match: "", after: "" };
    return {
      before: phrase.slice(0, idx),
      match:  phrase.slice(idx, idx + q.length),
      after:  phrase.slice(idx + q.length),
    };
  }

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
    // Load full config from backend to populate form fields
    invoke<{ commands: import("$lib/types").Command[] }>("list_commands")
      .then(result => {
        const full = result.commands.find(c => c.phrase.toLowerCase() === meta.phrase.toLowerCase());
        if (!full) return;
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
    cmdSaveError = "";
    cmdDeleteConfirm = false;
    cmdTargetDir = "";
    cmdNewFolderName = "";
    cmdShowNewFolder = false;
    cmdScriptContent = "";
    cmdScriptExists = false;
    cmdScriptDirty = false;
    cmdScriptError = "";
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

  // Build a Tauri-compatible accelerator string from a KeyboardEvent
  function eventToShortcut(e: KeyboardEvent): string | null {
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
      const result = await invoke<CommandsPayload>("list_commands").catch(() => ({ commands: [], duplicates: [], reserved: [] }));
      commands = result.commands;
      warnings = result.duplicates;
      reservedWarnings = result.reserved;
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
      await invoke("open_url", { url: value, param: null });
      dismiss();
    } else {
      // No action configured — just dismiss
      invoke("dismiss_launcher").catch(() => appWindow.hide());
    }
  }

  async function executeCommand(cmd: Command) {
    if (cmd.action.type === "open_url") {
      // Extract any text typed after the command phrase as the param.
      // effectiveInput includes context only when no param is present,
      // so params are never polluted by the active context.
      const phrase = cmd.phrase.toLowerCase();
      const typed  = effectiveInput;
      const after  = typed.toLowerCase().startsWith(phrase)
        ? typed.slice(phrase.length).trim()
        : "";
      await invoke("open_url", {
        url:   cmd.action.config.url,
        param: after !== "" ? after : null,
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
      // effectiveInput excludes context when a param is present,
      // so script args contain only what the user explicitly typed.
      const typed  = effectiveInput;
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
          await invoke("open_url", { url: v, param: null });
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
        await invoke("open_url", { url: cmd.action.config.url, param: null });
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
  function handleKeydown(e: KeyboardEvent) {
    if (onboarding) return; // handled by the onboarding div
    if (showSettings) { handleSettingsKeydown(e); return; }
    if (isPreferencesWindow) {
      const mod = e.metaKey || e.ctrlKey;
      if (activePreferencesTab === "commands") {
        if (mod && e.key === "n") { e.preventDefault(); cmdStartNew(); return; }
        if (mod && e.key === "s") { e.preventDefault(); cmdSave(); return; }
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
        const result = await invoke<CommandsPayload>("list_commands").catch(() => ({ commands: [], duplicates: [], reserved: [] }));
        commands = result.commands;
        warnings = result.duplicates;
        reservedWarnings = result.reserved;
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
        warningsDismissed = false; // always surface new warnings
        // If a list is currently displayed, refresh it in case its file changed
        if (activeListCmd && activeListCmd.action.type === "static_list") {
          const listName = activeListCmd.action.config.list;
          const commandDir = activeListCmd.source_dir;
          invoke<ListItem[]>("load_list", { commandDir, listName, inlineEnv: activeListCmd.env, context: activeContext, phrase: activeListCmd.phrase })
            .then(items => { listItems = items; })
            .catch((err) => { listItems = [{ title: "⚠️ Error loading list", subtext: String(err) }]; });
        } else if (activeListCmd && activeListCmd.action.type === "dynamic_list") {
          const config = activeListCmd.action.config;
          const typed = input.trim().toLowerCase();
          const phrase = activeListCmd.phrase.toLowerCase();
          const suffix = typed.startsWith(phrase + " ") ? typed.slice(phrase.length + 1).trim() : "";
          invoke<ListItem[]>("run_dynamic_list", { commandDir: activeListCmd.source_dir, scriptName: config.script, arg: suffix || null, context: activeContext, phrase: activeListCmd.phrase, inlineEnv: activeListCmd.env })
            .then(items => { listItems = items; })
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
    <div class="cmd-editor">
      <!-- sidebar -->
      <div class="cmd-sidebar">
        <div class="cmd-sidebar-header">
          <input
            class="cmd-filter"
            type="text"
            placeholder="Filter…"
            bind:value={cmdFilter}
            autocomplete="off"
            autocorrect="off"
            spellcheck="false"
          />
          <!-- svelte-ignore a11y_consider_explicit_label -->
          <button class="cmd-new-btn" title="New command (⌘N)" onclick={cmdStartNew}>＋</button>
        </div>
        <div class="cmd-list" role="list">
          {#if cmdFilteredList.length === 0}
            <div class="cmd-list-empty">No commands yet.<br>Click ＋ to create one.</div>
          {:else}
            {#each cmdGroupedList() as group}
              <button
                class="cmd-folder-header"
                onclick={() => {
                  const next = new Set(cmdCollapsedFolders);
                  next.has(group.folder) ? next.delete(group.folder) : next.add(group.folder);
                  cmdCollapsedFolders = next;
                }}
              >
                <span class="cmd-folder-toggle">{cmdCollapsedFolders.has(group.folder) ? '+' : '−'}</span>
                <span class="cmd-folder-name">{group.label}</span>
                <span class="cmd-folder-count">{group.items.length}</span>
              </button>
              {#if !cmdCollapsedFolders.has(group.folder)}
                {#each group.items as meta}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <div
                    class="cmd-list-item"
                    class:selected={meta.file_path === cmdSelectedFile}
                    onclick={() => cmdSelectItem(meta)}
                    role="option"
                    tabindex="0"
                    aria-selected={meta.file_path === cmdSelectedFile}
                  >
                    <span class="cmd-item-phrase">{meta.phrase}</span>
                    <span class="cmd-item-badge cmd-badge-{meta.action_type.replace('_', '-')}">{meta.action_type.replace('_', ' ')}</span>
                  </div>
                {/each}
              {/if}
            {/each}
          {/if}
        </div>
      </div>

      <!-- detail panel -->
      <div class="cmd-detail">
        {#if !cmdSelectedFile && !cmdIsNew}
          <div class="cmd-empty-state">
            <p>No command selected.</p>
            <p>Pick one from the list, or click <strong>＋</strong> to create a new command.</p>
          </div>
        {:else}
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
                onchange={() => { cmdUrl = ""; cmdText = ""; cmdListName = ""; cmdItemAction = ""; cmdScript = ""; cmdArgMode = "none"; cmdResultAction = "paste_text"; cmdPrefix = ""; cmdSuffix = ""; }}
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
                </select>
              </div>
              <div class="cmd-field">
                <label class="cmd-label" for="cmd-dl-item-action">Item action</label>
                <select id="cmd-dl-item-action" class="cmd-select" bind:value={cmdItemAction}>
                  <option value="">None (dismiss only)</option>
                  <option value="paste_text">Paste Text</option>
                  <option value="copy_text">Copy Text</option>
                  <option value="open_url">Open URL</option>
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
                  </div>
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
                <button class="cmd-btn-ghost" onclick={cmdCancelNew}>Cancel</button>
                <button class="cmd-btn-primary" disabled={!cmdCanSave} onclick={cmdSave}>
                  {cmdSaving ? "Creating…" : "Create"}
                </button>
              {:else}
                {#if cmdDeleteConfirm}
                  <span class="cmd-delete-confirm-text">Delete this command?</span>
                  <button class="cmd-btn-ghost" onclick={() => cmdDeleteConfirm = false}>Cancel</button>
                  <button class="cmd-btn-danger" onclick={cmdDelete}>Delete</button>
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
                  <button class="cmd-btn-primary" disabled={!cmdCanSave} onclick={cmdSave}>
                    {cmdSaving ? "Saving…" : "Save"}
                  </button>
                {/if}
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
    {/if}

    <!-- Settings tab -->
    {#if activePreferencesTab === "settings"}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="settings-panel">
      <div class="settings-header">
        <span class="settings-title">Settings</span>
        <div class="settings-header-right">
          <button class="settings-restore" onclick={restoreDefaults}>Restore Defaults</button>
          {#if settingsSaved}
            <span class="settings-saved-badge">Saved</span>
          {/if}
          <button class="settings-done" onclick={closeSettings}>Save</button>
        </div>
      </div>

    <div class="settings-body">
      <!-- Global shortcut -->
      <div class="settings-row">
        <div class="settings-row-info">
          <span class="row-title">Global shortcut</span>
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
            <code class="hotkey-badge">{currentSettings.hotkey ?? "Not set"}</code>
            <button class="action-btn" onclick={() => { settingsChangingHotkey = true; settingsCapturedShortcut = ""; }}>Change</button>
          </div>
        {/if}
      </div>

      <!-- Show context chip -->
      <div class="settings-row">
        <div class="settings-row-info">
          <span class="row-title">Show context chip</span>
          <span class="row-desc">Display the active context label in the launcher bar</span>
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
        <div class="settings-row-info">
          <span class="row-title">Allow duplicate commands</span>
          <span class="row-desc">Load all commands even when phrases collide</span>
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

      <!-- Shared scripts directory -->
      <div class="settings-row settings-row-stacked">
        <div class="settings-row-info">
          <span class="row-title">Shared directory</span>
          <span class="row-desc">Subdirectory inside commands root for shared scripts and lists (default: shared)</span>
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
        <div class="settings-row-info">
          <span class="row-title">Commands directory</span>
          <span class="row-desc">Custom absolute path (leave blank for default). Restart required.</span>
        </div>
        <div class="settings-dir-row">
          <input
            class="settings-text-input settings-dir-input"
            type="text"
            bind:value={settingsCommandsDir}
            placeholder="Default"
            onblur={persistSettings}
            spellcheck="false"
            autocomplete="off"
            autocorrect="off"
          />
          <button class="settings-browse-btn" onclick={browseCommandsDir}>Browse…</button>
        </div>
      </div>
    </div>
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
          {#if listItems.length === 0}
            <div class="no-results">No results</div>
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
            {@const rawTyped   = input.trim()}
            {@const builtinAction = cmd.action.type === "builtin" ? cmd.action.config.action : null}
            {@const ctxSetValue = builtinAction === "ctx_set" && rawTyped.toLowerCase().startsWith("/ctx set ") ? rawTyped.slice("/ctx set ".length).trim() : ""}
            {@const isParamMode = builtinAction === null && effectiveInput.toLowerCase().startsWith(cmd.phrase.toLowerCase() + " ")}
            {@const paramText  = isParamMode ? effectiveInput.slice(cmd.phrase.length + 1) : ""}
            {@const hl        = highlight(cmd.phrase, effectiveInput)}
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
    font-size: 13px;
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
    padding: 4px 0;
  }

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

  .row-desc {
    color: rgba(245,245,247,.4);
    font-size: 11px;
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

  .settings-dir-input {
    flex: 1;
    min-width: 0;
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
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    color: #f5f5f7;
    background: #1c1c1e;
    height: 100%;
  }

  /* Sidebar */
  .cmd-sidebar {
    width: 210px;
    min-width: 160px;
    max-width: 280px;
    display: flex;
    flex-direction: column;
    border-right: 1px solid rgba(255,255,255,.07);
    background: rgba(255,255,255,.025);
    flex-shrink: 0;
  }

  .cmd-sidebar-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 10px 8px;
    border-bottom: 1px solid rgba(255,255,255,.06);
    flex-shrink: 0;
  }

  .cmd-filter {
    flex: 1;
    background: rgba(255,255,255,.06);
    border: 1px solid rgba(255,255,255,.1);
    border-radius: 6px;
    color: #f5f5f7;
    font-size: 12px;
    font-family: inherit;
    padding: 5px 8px;
    outline: none;
    transition: border-color .15s;
  }
  .cmd-filter:focus { border-color: rgba(10,132,255,.5); }
  .cmd-filter::placeholder { color: rgba(245,245,247,.3); }

  .cmd-new-btn {
    background: #0a84ff;
    border: none;
    border-radius: 6px;
    color: #fff;
    font-size: 18px;
    line-height: 1;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    flex-shrink: 0;
    transition: opacity .15s;
  }
  .cmd-new-btn:hover { opacity: .85; }

  .cmd-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,.2) transparent;
  }

  .cmd-list-empty {
    padding: 20px 12px;
    color: rgba(245,245,247,.35);
    font-size: 12px;
    text-align: center;
    line-height: 1.6;
  }

  .cmd-list-item {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 7px 8px 7px 22px;
    border-radius: 7px;
    cursor: default;
    transition: background .1s;
    border-left: 2px solid transparent;
  }
  .cmd-list-item:hover { background: rgba(255,255,255,.06); }
  .cmd-list-item.selected {
    background: rgba(255,255,255,.09);
    border-left-color: #0a84ff;
  }

  .cmd-item-phrase {
    font-size: 12px;
    font-weight: 500;
    color: #f5f5f7;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cmd-item-badge {
    font-size: 9px;
    font-weight: 500;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    color: rgba(245,245,247,.4);
  }
  .cmd-badge-open-url      { color: #0a84ff; }
  .cmd-badge-paste-text    { color: #30d158; }
  .cmd-badge-copy-text     { color: #ff9f0a; }
  .cmd-badge-static-list   { color: #bf5af2; }
  .cmd-badge-dynamic-list  { color: #64d2ff; }
  .cmd-badge-script-action { color: #ff6482; }

  /* Detail panel */
  .cmd-detail {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .cmd-empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: rgba(245,245,247,.3);
    font-size: 13px;
    text-align: center;
    padding: 40px;
  }
  .cmd-empty-state p { margin: 0; }

  .cmd-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px 24px 24px;
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

  /* Folder headers in sidebar */
  .cmd-folder-header {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 7px 8px 6px;
    margin-top: 6px;
    cursor: default;
    user-select: none;
    background: rgba(255,255,255,.04);
    border: none;
    border-top: 1px solid rgba(255,255,255,.12);
    border-left: 2px solid rgba(10,132,255,.35);
    border-radius: 0;
    width: 100%;
    text-align: left;
    color: inherit;
    font-family: inherit;
  }
  .cmd-folder-header:first-child { margin-top: 0; border-top: none; }
  .cmd-folder-header:hover { background: rgba(255,255,255,.07); }

  .cmd-folder-toggle {
    font-size: 12px;
    font-weight: 700;
    color: rgba(245,245,247,.45);
    width: 14px;
    text-align: center;
    flex-shrink: 0;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    line-height: 1;
  }

  .cmd-folder-name {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.3px;
    color: rgba(245,245,247,.55);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cmd-folder-count {
    font-size: 9px;
    color: rgba(245,245,247,.3);
    background: rgba(255,255,255,.08);
    border-radius: 8px;
    padding: 1px 6px;
    min-width: 16px;
    text-align: center;
    font-weight: 500;
  }

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
