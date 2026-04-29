// ── Command schema ─────────────────────────────────────────────────────────────
// These types mirror the Rust structs in src-tauri/src/commands.rs exactly.
// Keep them in sync when the schema changes.

export interface OpenUrlConfig {
  url: string;
}

export interface PasteTextConfig {
  text: string;
}

export interface CopyTextConfig {
  text: string;
}

export type ItemAction = "paste_text" | "copy_text" | "open_url";

export interface StaticListConfig {
  list: string;
  /** If absent, selecting an item only dismisses the launcher. */
  item_action?: ItemAction;
}

/** Controls when a dynamic_list script is invoked and whether a suffix arg is passed. */
export type ArgMode = "none" | "optional" | "required";

export interface DynamicListConfig {
  /** Name of the script file (without path) inside `config_dir/scripts/`. */
  script: string;
  /** Defaults to "none" if absent. */
  arg?: ArgMode;
  /** If absent, selecting an item only dismisses the launcher. */
  item_action?: ItemAction;
}

/** The built-in action applied to each value returned by a script_action script. */
export type ResultAction = "open_url" | "paste_text" | "copy_text";

export interface ScriptActionConfig {
  /** Name of the script file (without path) inside `config_dir/scripts/`. */
  script: string;
  /** Controls whether a suffix typed after the phrase is passed as an argument. Defaults to "none". */
  arg?: ArgMode;
  /** Built-in action to apply to every value the script returns. */
  result_action: ResultAction;
  /** Text prepended to each value when result_action is paste_text or copy_text. */
  prefix?: string;
  /** Text appended to each value when result_action is paste_text or copy_text. */
  suffix?: string;
}

export interface ListItem {
  title: string;
  subtext?: string;
}

export interface BuiltinConfig {
  action: "ctx_set" | "ctx_reset" | "docs_open" | "deploy_skill" | "open_settings" | "open_commands" | "toggle_debug" | "show_debug_log" | "open_debug_log";
  /** URL to open when action is "docs_open". */
  url?: string;
}

export type Action =
  | { type: "open_url"; config: OpenUrlConfig }
  | { type: "paste_text"; config: PasteTextConfig }
  | { type: "copy_text"; config: CopyTextConfig }
  | { type: "static_list"; config: StaticListConfig }
  | { type: "dynamic_list"; config: DynamicListConfig }
  | { type: "script_action"; config: ScriptActionConfig }
  | { type: "builtin"; config: BuiltinConfig };

export interface Command {
  phrase: string;
  title: string;
  /** Inline user-defined environment variables. Merged last into script env. */
  env: Record<string, string>;
  action: Action;
  /** Directory containing the command YAML, relative to commands root. Set at load time. */
  source_dir: string;
  /** Path to the YAML file, relative to commands root. Set at load time. */
  source_file: string;
}

export interface DuplicateWarning {
  phrase: string;
  /** Config-dir-relative path of the file whose command was kept. */
  kept: string;
  /** Config-dir-relative path of the file whose command was ignored. */
  ignored: string;
}

export interface ReservedPhraseWarning {
  /** The rejected phrase as written in the YAML file. */
  phrase: string;
  /** Config-dir-relative path of the offending file. */
  file: string;
}

/** A YAML command file that could not be read or parsed (skipped during loading). */
export interface SkippedFileWarning {
  /** Config-dir-relative path of the file that was skipped. */
  file: string;
  /** Human-readable reason (e.g. serde error with line/column info). */
  reason: string;
}

/** A load-time validation warning for a successfully-parsed command. */
export interface CommandWarning {
  /** Config-dir-relative path of the file whose command triggered the warning. */
  file: string;
  /** Human-readable warning message. */
  message: string;
}

/** Payload returned by list_commands and emitted as commands://reloaded. */
export interface CommandsPayload {
  commands: Command[];
  duplicates: DuplicateWarning[];
  /** Commands rejected because their phrase starts with the reserved `ctx` prefix. */
  reserved: ReservedPhraseWarning[];
  /** YAML files that could not be read or parsed. */
  skipped: SkippedFileWarning[];
  /** Load-time validation warnings for successfully-parsed commands. */
  warnings: CommandWarning[];
}

/** Raw result of running a script in test mode (used by the preferences Test button). */
export interface ScriptTestResult {
  stdout: string;
  stderr: string;
  /** null if the process was killed or timed out */
  exit_code: number | null;
  duration_ms: number;
  timed_out: boolean;
}

/** Application settings loaded from `settings.yaml` in the config directory. */
export interface AppSettings {
  hotkey?: string;
  show_context_chip: boolean;
  allow_duplicates: boolean;
  shared_dir: string;
  seed_examples: boolean;
  commands_dir?: string;
}

/** Metadata returned by list_command_files — one entry per YAML command file. */
export interface CommandFileMeta {
  phrase: string;
  title: string;
  enabled: boolean;
  /** e.g. "open_url" | "paste_text" | "copy_text" | "static_list" | … */
  action_type: string;
  /** Absolute path to the YAML file on disk. */
  file_path: string;
  /** Relative directory containing the YAML file (empty string for root-level commands). */
  source_dir: string;
}
