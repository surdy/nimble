<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { listen, emit } from "@tauri-apps/api/event";
  import type { Action, AppSettings, Command, CommandsPayload, DuplicateWarning, ListItem, ReservedPhraseWarning } from "$lib/types";

  // ── State ──────────────────────────────────────────────────────────────
  let input = $state("");
  let inputEl: HTMLInputElement | undefined = $state();
  let onboardingEl: HTMLDivElement | undefined = $state();
  const appWindow = getCurrentWindow();

  // Detect whether we are running inside the dedicated settings window
  // (label "settings") vs the main launcher window (label "main").
  // This is synchronous so it affects initial render without any flash.
  const isSettingsWindow = appWindow.label === "settings";
  if (isSettingsWindow && typeof document !== "undefined") {
    document.documentElement.classList.add("settings-window-mode");
  }

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
    allow_external_paths: true,
  });

  // ── Settings panel state ────────────────────────────────────────────────
  // In the settings window the panel is always visible; in the launcher it is
  // toggled by the "settings" built-in command (which now opens a new window).
  let showSettings = $state(isSettingsWindow);
  let settingsShowContextChip = $state(true);
  let settingsAllowDuplicates = $state(true);
  let settingsAllowExternalPaths = $state(true);
  let settingsCommandsDir = $state("");
  let settingsChangingHotkey = $state(false);
  let settingsCapturedShortcut = $state("");
  let settingsHotkeyError = $state("");
  let settingsSavedTimer: ReturnType<typeof setTimeout> | null = null;
  let settingsSaved = $state(false);

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

  // ── Settings helpers ───────────────────────────────────────────────────
  // Opens the dedicated settings window (creates it if not already open).
  // The launcher window hides itself immediately so it stays out of the way.
  function openSettings() {
    invoke("open_settings_window").catch(() => {});
    dismiss();
  }

  async function closeSettings() {
    // In the settings window: save and close the whole window.
    if (isSettingsWindow) {
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
    settingsAllowExternalPaths = true;
    settingsCommandsDir = "";
    await persistSettings();
  }

  function flashSaved() {
    if (settingsSavedTimer !== null) clearTimeout(settingsSavedTimer);
    settingsSaved = true;
    settingsSavedTimer = setTimeout(() => { settingsSaved = false; }, 1500);
  }

  async function persistSettings() {
    const dir = settingsCommandsDir.trim() || undefined;
    try {
      await invoke("save_settings", {
        showContextChip: settingsShowContextChip,
        allowDuplicates: settingsAllowDuplicates,
        allowExternalPaths: settingsAllowExternalPaths,
        commandsDir: dir ?? null,
      });
      // Keep currentSettings in sync
      currentSettings = {
        ...currentSettings,
        show_context_chip: settingsShowContextChip,
        allow_duplicates: settingsAllowDuplicates,
        allow_external_paths: settingsAllowExternalPaths,
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
        openSettings();
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

    (async () => {
      // Load settings from the backend (settings.yaml)
      const appSettings = await invoke<AppSettings>("get_settings").catch(
        () => ({ hotkey: undefined, show_context_chip: true, allow_duplicates: true, allow_external_paths: true, seed_examples: false } as AppSettings)
      );
      showContextChip = appSettings.show_context_chip;
      currentSettings = appSettings;

      // ── Settings window path ─────────────────────────────────────────
      // When running as the settings window we only need to populate the
      // settings panel state and then stop — no launcher init required.
      if (isSettingsWindow) {
        settingsShowContextChip = appSettings.show_context_chip;
        settingsAllowDuplicates = appSettings.allow_duplicates;
        settingsAllowExternalPaths = appSettings.allow_external_paths;
        settingsCommandsDir = appSettings.commands_dir ?? "";
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
{:else if showSettings}
  <!-- ── Settings panel ────────────────────────────────────────────────── -->
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

      <!-- Allow external paths -->
      <div class="settings-row">
        <div class="settings-row-info">
          <span class="row-title">Allow external paths</span>
          <span class="row-desc">Scripts can resolve to paths outside the command directory</span>
        </div>
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="toggle"
          class:on={settingsAllowExternalPaths}
          role="switch"
          aria-checked={settingsAllowExternalPaths}
          tabindex="0"
          onclick={() => { settingsAllowExternalPaths = !settingsAllowExternalPaths; persistSettings(); }}
          onkeydown={(e) => { if (e.key === " " || e.key === "Enter") { settingsAllowExternalPaths = !settingsAllowExternalPaths; persistSettings(); } }}
        >
          <span class="thumb"></span>
        </div>
      </div>

      <!-- Commands directory -->
      <div class="settings-row settings-row-stacked">
        <div class="settings-row-info">
          <span class="row-title">Commands directory</span>
          <span class="row-desc">Custom absolute path (leave blank for default). Restart required.</span>
        </div>
        <input
          class="settings-text-input"
          type="text"
          bind:value={settingsCommandsDir}
          placeholder="Default"
          onblur={persistSettings}
          spellcheck="false"
          autocomplete="off"
          autocorrect="off"
        />
      </div>
    </div>
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

  /* When running as the standalone settings window, give the document a solid
     background so the transparent webview doesn't show through decorated chrome. */
  :global(html.settings-window-mode),
  :global(html.settings-window-mode body) {
    background: #1c1c1e;
    overflow: auto;
    height: 100%;
  }

  :global(html.settings-window-mode) .settings-panel {
    border-radius: 0;
    box-shadow: none;
    height: 100vh;
    min-height: 100vh;
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

</style>
