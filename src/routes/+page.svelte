<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import type { Action, AppSettings, Command, CommandsPayload, DuplicateWarning, ListItem, ReservedPhraseWarning } from "$lib/types";

  // ── State ──────────────────────────────────────────────────────────────
  let input = $state("");
  let inputEl: HTMLInputElement | undefined = $state();
  let onboardingEl: HTMLDivElement | undefined = $state();
  const appWindow = getCurrentWindow();

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
  ]);

  // List expansion state — populated when input exactly matches a static_list phrase
  let listItems = $state<ListItem[]>([]);
  let activeListCmd = $state<Command | null>(null);
  let resultsEl: HTMLDivElement | undefined = $state();

  // ── Filtering & navigation ─────────────────────────────────────────────
  const MAX_RESULTS = 8;
  const ROW_H = 56; // px per result row

  // Human-readable badge label for each action type
  function actionBadge(cmd: { action: Action }): string {
    const type = cmd.action.type;
    if (type === "builtin" && cmd.action.config.action === "docs_open") return "Docs";
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
  // Requires non-empty raw input: an empty input must always produce no results,
  // regardless of context, to avoid unexpected matches on launcher open.
  const effectiveInput = $derived(
    activeContext && input.trim() !== "" && !input.trim().startsWith("/")
      ? input.trim() + " " + activeContext
      : input.trim()
  );

  const filtered = $derived(
    effectiveInput === ""
      ? []
      : commands
          .filter(cmd => {
            const phrase = cmd.phrase.toLowerCase();
            const typed  = effectiveInput.toLowerCase();
            // Standard partial/substring match (discovery while typing)
            // OR param mode: user has typed the full phrase + space + param text
            return phrase.includes(typed) || typed.startsWith(phrase + " ");
          })
  );

  // Built-in / commands filtered by the current raw input (only when input starts with "/")
  const filteredBuiltins: Command[] = $derived(
    input.trim().startsWith("/")
      ? builtinCommands.filter(cmd => {
          const phrase = cmd.phrase.toLowerCase();
          const typed  = input.trim().toLowerCase();
          return phrase.includes(typed) || typed.startsWith(phrase + " ");
        })
      : []
  );

  // Combined results: built-ins first, then YAML commands
  const allFiltered = $derived([...filteredBuiltins, ...filtered]);

  // True when the typed input exactly equals a static_list command phrase.
  // In this mode we show list items instead of the normal results list.
  const showingList = $derived(activeListCmd !== null && listItems.length > 0);

  let selectedIndex = $state(0);

  // Reset selection whenever the result list changes
  $effect(() => {
    void allFiltered;
    selectedIndex = 0;
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
        .catch(() => { listItems = []; });
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

      if (argMode === "none") {
        if (isExact) {
          activeListCmd = dynMatch;
          invoke<ListItem[]>("run_dynamic_list", { commandDir, scriptName: config.script, arg: null, context: activeContext, phrase: dynMatch.phrase, inlineEnv: dynMatch.env })
            .then(items => { listItems = items; selectedIndex = 0; })
            .catch(() => { listItems = []; });
        } else {
          activeListCmd = null;
          listItems = [];
        }
      } else if (argMode === "optional") {
        activeListCmd = dynMatch;
        if (isExact) {
          // Immediate invocation — no suffix
          invoke<ListItem[]>("run_dynamic_list", { commandDir, scriptName: config.script, arg: null, context: activeContext, phrase: dynMatch.phrase, inlineEnv: dynMatch.env })
            .then(items => { listItems = items; selectedIndex = 0; })
            .catch(() => { listItems = []; });
        } else {
          // Suffix present — debounce re-invocation
          timer = setTimeout(() => {
            invoke<ListItem[]>("run_dynamic_list", { commandDir, scriptName: config.script, arg: suffix, context: activeContext, phrase: dynMatch.phrase, inlineEnv: dynMatch.env })
              .then(items => { listItems = items; selectedIndex = 0; })
              .catch(() => { listItems = []; });
          }, 200);
        }
      } else {
        // required: only invoke when suffix is non-empty
        if (suffix) {
          activeListCmd = dynMatch;
          timer = setTimeout(() => {
            invoke<ListItem[]>("run_dynamic_list", { commandDir, scriptName: config.script, arg: suffix, context: activeContext, phrase: dynMatch.phrase, inlineEnv: dynMatch.env })
              .then(items => { listItems = items; selectedIndex = 0; })
              .catch(() => { listItems = []; });
          }, 200);
        } else {
          activeListCmd = null;
          listItems = [];
        }
      }

      return () => { if (timer !== null) clearTimeout(timer); };
    }

    // No list match
    activeListCmd = null;
    listItems = [];
  });

  // Resize window to fit current results (skip during onboarding)
  $effect(() => {
    if (onboarding) return;
    const hasQuery = input.trim() !== "";
    const WARNING_H = 40;
    const warnExtra = warningVisible ? WARNING_H : 0;
    const contentHeight = !hasQuery ? 0
      : showingList ? Math.min(listItems.length, MAX_RESULTS) * ROW_H
      : allFiltered.length === 0 ? 44          // "no results" row
      : Math.min(allFiltered.length, MAX_RESULTS) * ROW_H;
    appWindow.setSize(new LogicalSize(640, 64 + warnExtra + contentHeight));
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
      " ": "Space", "ArrowUp": "Up", "ArrowDown": "Down",
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
      // Use effectiveInput so the context suffix can serve as the {param} value.
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
      // Use effectiveInput so the context suffix is included in the suffix arg.
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

      const values: string[] = await invoke("run_script_action", {
        commandDir: cmd.source_dir,
        scriptName: cfg.script,
        arg: scriptArg,
        context: activeContext,
        phrase: cmd.phrase,
        inlineEnv: cmd.env,
      });

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
      }
    }
  }

  // ── Launcher key handling ──────────────────────────────────────────────
  function handleKeydown(e: KeyboardEvent) {
    if (onboarding) return; // handled by the onboarding div
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

  // Persist active context to localStorage.
  $effect(() => {
    localStorage.setItem("ctx_active_context", activeContext);
  });

  // ── Lifecycle ─────────────────────────────────────────────────────────
  onMount(() => {
    let unlistenFocus: (() => void) | null = null;
    let unlistenReload: (() => void) | null = null;

    (async () => {
      // Load settings from the backend (settings.yaml)
      const appSettings = await invoke<AppSettings>("get_settings").catch(
        () => ({ hotkey: undefined, show_context_chip: true, allow_duplicates: true } as AppSettings)
      );
      showContextChip = appSettings.show_context_chip;

      // Restore active context from the previous session.
      const savedContext = localStorage.getItem("ctx_active_context");
      if (savedContext) activeContext = savedContext;

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
            .catch(() => { listItems = []; });
        } else if (activeListCmd && activeListCmd.action.type === "dynamic_list") {
          const config = activeListCmd.action.config;
          const typed = input.trim().toLowerCase();
          const phrase = activeListCmd.phrase.toLowerCase();
          const suffix = typed.startsWith(phrase + " ") ? typed.slice(phrase.length + 1).trim() : "";
          invoke<ListItem[]>("run_dynamic_list", { commandDir: activeListCmd.source_dir, scriptName: config.script, arg: suffix || null, context: activeContext, phrase: activeListCmd.phrase, inlineEnv: activeListCmd.env })
            .then(items => { listItems = items; })
            .catch(() => { listItems = []; });
        }
      });
    })();

    return () => {
      unlistenFocus?.();
      unlistenReload?.();
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
          {#each listItems as item, i}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="result-row"
              class:selected={i === selectedIndex}
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
        {:else if allFiltered.length === 0}
          <div class="no-results">No results</div>
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

  /* ── Launcher bar ────────────────────────────────────────────────────── */
  .launcher {
    background: rgba(28, 28, 30, 0.82);
    -webkit-backdrop-filter: blur(40px) saturate(1.8);
    backdrop-filter: blur(40px) saturate(1.8);
    border-radius: 12px;
    box-shadow:
      0 0 0 1px rgba(255,255,255,.1),
      0 1px 3px rgba(0,0,0,.2),
      0 8px 24px rgba(0,0,0,.35),
      0 24px 64px rgba(0,0,0,.5);
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
    scrollbar-color: rgba(255,255,255,.2) transparent;
  }

  .results::-webkit-scrollbar { width: 6px; }
  .results::-webkit-scrollbar-track { background: transparent; }
  .results::-webkit-scrollbar-thumb {
    background: rgba(255,255,255,.2);
    border-radius: 3px;
  }
  .results::-webkit-scrollbar-thumb:hover {
    background: rgba(255,255,255,.35);
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
    color: rgba(245,245,247,.3);
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

</style>
