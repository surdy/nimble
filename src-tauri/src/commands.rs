use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use walkdir::WalkDir;

// ── Schema ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenUrlConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteTextConfig {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTextConfig {
    pub text: String,
}

/// The action performed when a list item is selected.
/// The item's `subtext` (falling back to `title`) is used as the value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemAction {
    PasteText,
    CopyText,
    OpenUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticListConfig {
    /// Name of the list file (without extension) co-located with the command YAML.
    pub list: String,
    /// Optional action to perform when an item is selected.
    /// If absent, selecting an item dismisses the launcher without any further action.
    pub item_action: Option<ItemAction>,
}

/// How a `dynamic_list` command accepts user-supplied arguments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArgMode {
    None,
    Optional,
    Required,
}

fn default_arg_mode() -> ArgMode {
    ArgMode::None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicListConfig {
    /// Name of the script file (without path) inside `config_dir/scripts/`.
    pub script: String,
    /// Controls when the script is invoked and whether a suffix argument is passed.
    #[serde(default = "default_arg_mode")]
    pub arg: ArgMode,
    /// Optional action to perform when an item is selected.
    /// If absent, selecting an item dismisses the launcher without any further action.
    pub item_action: Option<ItemAction>,
}

/// The built-in action to apply to each value returned by a `script_action` script.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResultAction {
    OpenUrl,
    PasteText,
    CopyText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptActionConfig {
    /// Name of the script file (without path) inside `config_dir/scripts/`.
    pub script: String,
    /// Controls whether the script accepts an argument from text typed after the phrase.
    #[serde(default = "default_arg_mode")]
    pub arg: ArgMode,
    /// The built-in action applied to every value the script returns.
    pub result_action: ResultAction,
    /// Text prepended to each value when `result_action` is `paste_text` or `copy_text`.
    pub prefix: Option<String>,
    /// Text appended to each value when `result_action` is `paste_text` or `copy_text`.
    pub suffix: Option<String>,
}

/// The action executed when a command is selected.
/// Serialised as `{ type: "open_url"|"paste_text"|"copy_text"|"static_list"|"dynamic_list"|"script_action", config: { … } }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum Action {
    OpenUrl(OpenUrlConfig),
    PasteText(PasteTextConfig),
    CopyText(CopyTextConfig),
    StaticList(StaticListConfig),
    DynamicList(DynamicListConfig),
    ScriptAction(ScriptActionConfig),
}

/// A single item in a named list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    pub title: String,
    pub subtext: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub phrase: String,
    pub title: String,
    /// Whether this command is active. Omitting the field in YAML is the same
    /// as `enabled: true`. Disabled commands are filtered out at load time and
    /// never sent to the frontend.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Inline user-defined environment variables. Merged last (highest
    /// precedence) into the script env. Keys must not start with `NIMBLE_`.
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub action: Action,
    /// Directory containing the command YAML file, relative to the commands
    /// root. Set at load time — not present in the YAML file itself.
    #[serde(default)]
    pub source_dir: String,
}

fn default_true() -> bool {
    true
}

/// A warning produced when two YAML files define the same command phrase.
/// The older file (by mtime) wins; the newer file is ignored.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicateWarning {
    /// The conflicting phrase (lowercased).
    pub phrase: String,
    /// Config-dir-relative path of the file whose command was kept.
    pub kept: String,
    /// Config-dir-relative path of the file whose command was ignored.
    pub ignored: String,
}

/// A warning produced when a YAML file defines a command whose phrase starts
/// with the reserved `/` sigil (built-in app commands).
#[derive(Debug, Clone, Serialize)]
pub struct ReservedPhraseWarning {
    /// The rejected phrase as written in the YAML file.
    pub phrase: String,
    /// Config-dir-relative path of the offending file.
    pub file: String,
}

/// The result of loading commands from the config directory.
#[derive(Debug, Clone, Serialize)]
pub struct LoadResult {
    pub commands: Vec<Command>,
    pub duplicates: Vec<DuplicateWarning>,
    /// Commands rejected because their phrase starts with the reserved `/` sigil.
    pub reserved: Vec<ReservedPhraseWarning>,
}

// ── Seed files written on first launch ────────────────────────────────────────
// Each entry is (relative path inside config dir, YAML content).
// Paths may include subdirectories — they mirror the kind of structure a user
// would organise their own commands into.

static SEED_FILES: &[(&str, &str)] = &[
    (
        "examples/open-google.yaml",
        r#"phrase: open google
title: Open Google
action:
  type: open_url
  config:
    url: https://www.google.com
"#,
    ),
    (
        "examples/search-google.yaml",
        r#"phrase: search google
title: Search Google for…
action:
  type: open_url
  config:
    url: https://www.google.com/search?q={param}
"#,
    ),
    (
        "examples/open-github.yaml",
        r#"phrase: open github
title: Open GitHub
action:
  type: open_url
  config:
    url: https://github.com
"#,
    ),
    (
        "examples/paste-email.yaml",
        r#"phrase: paste email
title: Paste email address
action:
  type: paste_text
  config:
    text: hello@example.com
"#,
    ),
    (
        "examples/paste-greeting.yaml",
        r#"phrase: paste greeting
title: Paste polite greeting
action:
  type: paste_text
  config:
    text: |
      Hi,

      Thank you for reaching out.

      Best regards
"#,
    ),
    (
        "examples/copy-email.yaml",
        r#"phrase: copy email
title: Copy email address
action:
  type: copy_text
  config:
    text: hello@example.com
"#,
    ),
    (
        "examples/show-team-emails/show-team-emails.yaml",
        r#"phrase: team emails
title: Team email addresses
action:
  type: static_list
  config:
    list: team-emails
"#,
    ),
    (
        "examples/show-team-emails/team-emails.tsv",
        "# Team email addresses\nAlice Smith\talice@example.com\nBob Jones\tbob@example.com\nCarol White\tcarol@example.com\n",
    ),
    (
        "examples/say-hello/say-hello.yaml",
        r#"phrase: say hello
title: Say hello (dynamic list example)
action:
  type: dynamic_list
  config:
    script: hello.sh
    arg: optional
    item_action: paste_text
"#,
    ),
    (
        "examples/paste-timestamp/paste-timestamp.yaml",
        r#"phrase: paste timestamp
title: Paste current date/time
action:
  type: script_action
  config:
    script: timestamp.sh
    arg: none
    result_action: paste_text
"#,
    ),
];

/// Seed scripts that are co-located with their command YAML files.
/// Each entry is (relative path inside config dir, content, executable flag).
static SEED_SCRIPTS: &[(&str, &str)] = &[
    (
        "examples/say-hello/hello.sh",
        "#!/bin/sh\n# Example dynamic_list script.\n# Output a JSON array of objects with \"title\" and optional \"subtext\" fields,\n# or plain text for a single-item result.\nQUERY=\"$1\"\n\nif [ -z \"$QUERY\" ]; then\n  echo '[{\"title\":\"Hello, World!\",\"subtext\":\"A classic greeting\"},{\"title\":\"Hello, Alice\",\"subtext\":\"alice@example.com\"},{\"title\":\"Hello, Bob\",\"subtext\":\"bob@example.com\"}]'\nelse\n  echo \"[{\\\"title\\\":\\\"Hello, $QUERY\\\",\\\"subtext\\\":\\\"You searched for $QUERY\\\"}]\"\nfi\n",
    ),
    (
        "examples/paste-timestamp/timestamp.sh",
        "#!/bin/sh\ndate\n",
    ),
];

// ── List loader ────────────────────────────────────────────────────────────────

/// Load a list from the path resolved from the `list:` field.
///
/// `list_ref` is the raw value from the YAML. It may be a plain name
/// (resolved to `<command_dir>/<list_ref>.tsv`) or contain `${VAR}` tokens.
///
/// The file uses **TSV format**: one item per line, tab separates `title`
/// from an optional `subtext`. Lines starting with `#` and blank lines are
/// ignored.
///
/// Returns `Err` if the path is unsafe, the file is missing, or parsing fails.
pub fn load_list(command_dir: &Path, list_ref: &str, env: &ScriptEnv<'_>) -> Result<Vec<ListItem>, String> {
    let path = resolve_list_path(list_ref, command_dir, env)?;
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read list {:?}: {e}", path.display()))?;
    parse_tsv_list(&content)
        .map_err(|e| format!("Could not parse list {:?}: {e}", path.display()))
}

/// Parse a TSV string into a list of items.
///
/// Format: one item per line. A tab character separates `title` from an
/// optional `subtext`. Lines starting with `#` (after trimming) and blank
/// lines are skipped.
fn parse_tsv_list(content: &str) -> Result<Vec<ListItem>, String> {
    let mut items = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (title, subtext) = if let Some(pos) = line.find('\t') {
            let t = line[..pos].trim().to_string();
            let s = line[pos + 1..].trim().to_string();
            (t, if s.is_empty() { None } else { Some(s) })
        } else {
            (trimmed.to_string(), None)
        };
        if title.is_empty() {
            continue;
        }
        items.push(ListItem { title, subtext });
    }
    Ok(items)
}

// ── Script environment ─────────────────────────────────────────────────────────

/// Runtime context injected as `NIMBLE_*` environment variables into every
/// script subprocess. Built by the Tauri command layer and threaded into
/// `run_script` / `run_script_values`.
pub struct ScriptEnv<'a> {
    /// Active context string (may be empty).
    pub context: &'a str,
    /// Command phrase that triggered the script.
    pub phrase: &'a str,
    /// Absolute path to the Nimble config root directory.
    pub config_dir: &'a Path,
    /// Absolute path to the directory containing the command YAML.
    pub command_dir: &'a Path,
    /// Merged user-defined environment variables (global → sidecar → inline).
    pub user_env: &'a HashMap<String, String>,
    /// Whether resolved script/list paths may point outside the command directory.
    pub allow_external_paths: bool,
}

/// Inject user-defined and `NIMBLE_*` built-in environment variables into a
/// `Command` that is about to be spawned. User-defined vars are injected first
/// so that built-in `NIMBLE_*` keys always take precedence.
fn inject_env(cmd: &mut std::process::Command, env: &ScriptEnv<'_>) {
    // User-defined variables (lowest precedence — injected first).
    for (k, v) in env.user_env {
        cmd.env(k, v);
    }
    // Built-in NIMBLE_* variables (always win).
    cmd.env("NIMBLE_CONTEXT", env.context)
        .env("NIMBLE_PHRASE", env.phrase)
        .env("NIMBLE_CONFIG_DIR", env.config_dir.to_string_lossy().as_ref())
        .env("NIMBLE_COMMAND_DIR", env.command_dir.to_string_lossy().as_ref())
        .env("NIMBLE_OS", if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        })
        .env("NIMBLE_VERSION", env!("CARGO_PKG_VERSION"));
}

// ── User-defined environment variables ────────────────────────────────────────

/// Validate that an env key uses a portable name and is not in the reserved
/// `NIMBLE_` namespace. Accepts keys matching `[A-Za-z_][A-Za-z0-9_]*`.
fn validate_env_key(key: &str, source: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err(format!("Empty environment variable key in {source}"));
    }
    if key.starts_with("NIMBLE_") {
        return Err(format!(
            "Key {key:?} in {source} uses the reserved NIMBLE_ prefix"
        ));
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(format!(
            "Key {key:?} in {source} must start with a letter or underscore"
        ));
    }
    if let Some(bad) = chars.find(|c| !c.is_ascii_alphanumeric() && *c != '_') {
        return Err(format!(
            "Key {key:?} in {source} contains invalid character {bad:?}"
        ));
    }
    Ok(())
}

/// Load an `env.yaml` file as a flat `KEY: value` map. Missing files return an
/// empty map. Non-scalar values are rejected.
fn load_env_yaml(path: &Path) -> Result<HashMap<String, String>, String> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(HashMap::new());
    }
    let mapping: serde_yaml::Mapping = serde_yaml::from_str(&content)
        .map_err(|e| format!("Could not parse {}: {e}", path.display()))?;
    let source = path.display().to_string();
    let mut env = HashMap::new();
    for (k, v) in mapping {
        let key = k
            .as_str()
            .ok_or_else(|| format!("Non-string key in {source}"))?
            .to_string();
        let value = match v {
            serde_yaml::Value::String(s) => s,
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Null => String::new(),
            _ => {
                return Err(format!(
                    "Unsupported value for key {key:?} in {source}"
                ))
            }
        };
        validate_env_key(&key, &source)?;
        env.insert(key, value);
    }
    Ok(env)
}

/// Build the merged user-defined environment by applying layers in order:
/// global `env.yaml` → command-dir sidecar `env.yaml` → inline `env:`.
/// All keys are validated; reserved `NIMBLE_*` keys are rejected.
pub fn build_user_env(
    config_dir: &Path,
    command_dir: &Path,
    inline_env: &HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    // Layer 1: global env.yaml at config root.
    let mut merged = load_env_yaml(&config_dir.join("env.yaml"))?;

    // Layer 2: sidecar env.yaml in the command directory.
    let sidecar = load_env_yaml(&command_dir.join("env.yaml"))?;
    merged.extend(sidecar);

    // Layer 3: inline env from command YAML (highest user precedence).
    for (k, v) in inline_env {
        validate_env_key(k, "inline env")?;
        merged.insert(k.clone(), v.clone());
    }

    Ok(merged)
}

// ── Variable substitution & path resolution ───────────────────────────────────

/// Replace `${VAR}` tokens in `template` using the combined set of user-defined
/// and built-in variables. Returns `Err` if any referenced variable is not
/// defined. Only `${VAR}` is supported (not `$VAR` or nested references).
fn substitute_vars(template: &str, env: &ScriptEnv<'_>) -> Result<String, String> {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}').ok_or_else(|| {
            format!("Unterminated ${{}} in {template:?}")
        })?;
        let var_name = &after[..end];
        if var_name.is_empty() {
            return Err(format!("Empty variable name in {template:?}"));
        }
        // Look up: builtins first, then user-defined.
        let value = builtin_var_value(var_name, env)
            .or_else(|| env.user_env.get(var_name).map(|s| s.as_str()))
            .ok_or_else(|| {
                format!("Undefined variable {var_name:?} in {template:?}")
            })?;
        result.push_str(value);
        rest = &after[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}

/// Return the value of a built-in `NIMBLE_*` variable by name, or `None`.
fn builtin_var_value<'a>(name: &str, env: &'a ScriptEnv<'_>) -> Option<&'a str> {
    match name {
        "NIMBLE_CONTEXT" => Some(env.context),
        "NIMBLE_PHRASE" => Some(env.phrase),
        "NIMBLE_OS" => Some(if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        }),
        // These return owned data via to_string_lossy — callers should use
        // the ScriptEnv fields directly. For substitution we need a &str,
        // so we match on the well-known names and return the path fields.
        _ => None,
    }
}

/// Resolve a `script:` field value to an absolute path. Performs `${VAR}`
/// substitution. Plain filenames (no separators after substitution) resolve
/// relative to `command_dir`. If `allow_external_paths` is false, the resolved
/// path must be inside `command_dir`.
pub fn resolve_script_path(
    raw: &str,
    command_dir: &Path,
    env: &ScriptEnv<'_>,
) -> Result<PathBuf, String> {
    // Fast path: no ${…} tokens — use legacy co-located behaviour.
    if !raw.contains("${") {
        // Security: reject names that could escape the command directory.
        if raw.contains('/') || raw.contains('\\') || raw.contains("..") {
            return Err(format!("Invalid script name: {raw:?}"));
        }
        return Ok(command_dir.join(raw));
    }

    // Substitute variables — need lossy conversions for path builtins.
    let config_str = env.config_dir.to_string_lossy();
    let cmd_str = env.command_dir.to_string_lossy();
    let version = env!("CARGO_PKG_VERSION");
    let mut resolved = raw.to_string();
    // Pre-substitute path-typed builtins that can't return &str easily.
    resolved = resolved.replace("${NIMBLE_CONFIG_DIR}", &config_str);
    resolved = resolved.replace("${NIMBLE_COMMAND_DIR}", &cmd_str);
    resolved = resolved.replace("${NIMBLE_VERSION}", version);
    // Now substitute remaining ${VAR} tokens.
    let resolved = substitute_vars(&resolved, env)?;

    let path = PathBuf::from(&resolved);
    let path = if path.is_absolute() {
        path
    } else {
        command_dir.join(&path)
    };

    // Enforce containment when external paths are disabled.
    if !env.allow_external_paths {
        let canonical_dir = command_dir.canonicalize().unwrap_or_else(|_| command_dir.to_path_buf());
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !canonical_path.starts_with(&canonical_dir) {
            return Err(format!(
                "Script path {:?} (resolved from {raw:?}) is outside the command directory. \
                 Set allow_external_paths: true in settings.yaml to allow external paths.",
                path.display()
            ));
        }
    }

    Ok(path)
}

/// Resolve a `list:` field value to an absolute path. Performs `${VAR}`
/// substitution. Plain names (no separators, no `${`) get `.tsv` appended
/// and resolve relative to `command_dir`. Paths ending in `.tsv` are
/// used as-is; otherwise `.tsv` is appended.
pub fn resolve_list_path(
    raw: &str,
    command_dir: &Path,
    env: &ScriptEnv<'_>,
) -> Result<PathBuf, String> {
    // Fast path: no ${…} tokens — use co-located behaviour.
    if !raw.contains("${") {
        if raw.contains('/') || raw.contains('\\') || raw.contains("..") {
            return Err(format!("Invalid list name: {raw:?}"));
        }
        return Ok(command_dir.join(format!("{raw}.tsv")));
    }

    let config_str = env.config_dir.to_string_lossy();
    let cmd_str = env.command_dir.to_string_lossy();
    let version = env!("CARGO_PKG_VERSION");
    let mut resolved = raw.to_string();
    resolved = resolved.replace("${NIMBLE_CONFIG_DIR}", &config_str);
    resolved = resolved.replace("${NIMBLE_COMMAND_DIR}", &cmd_str);
    resolved = resolved.replace("${NIMBLE_VERSION}", version);
    let resolved = substitute_vars(&resolved, env)?;

    // Auto-append .tsv if the resolved path doesn't already have the extension.
    let resolved = if resolved.ends_with(".tsv") {
        resolved
    } else {
        format!("{resolved}.tsv")
    };

    let path = PathBuf::from(&resolved);
    let path = if path.is_absolute() {
        path
    } else {
        command_dir.join(&path)
    };

    if !env.allow_external_paths {
        let canonical_dir = command_dir.canonicalize().unwrap_or_else(|_| command_dir.to_path_buf());
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !canonical_path.starts_with(&canonical_dir) {
            return Err(format!(
                "List path {:?} (resolved from {raw:?}) is outside the command directory. \
                 Set allow_external_paths: true in settings.yaml to allow external paths.",
                path.display()
            ));
        }
    }

    Ok(path)
}

// ── Script runner ─────────────────────────────────────────────────────────────

/// Run the script identified by `script_ref`, optionally passing `arg` as a
/// positional argument. Returns the parsed list of items on success.
///
/// `script_ref` is the raw value from the YAML `script:` field. It may be a
/// plain filename (resolved relative to `command_dir`) or contain `${VAR}`
/// tokens that are substituted first.
///
/// A 5-second timeout is enforced; the function returns `Err` on timeout.
pub fn run_script(
    command_dir: &Path,
    script_ref: &str,
    arg: Option<&str>,
    env: &ScriptEnv<'_>,
) -> Result<Vec<ListItem>, String> {
    let script_path = resolve_script_path(script_ref, command_dir, env)?;
    if !script_path.exists() {
        return Err(format!("Script not found: {}", script_path.display()));
    }

    #[cfg(windows)]
    let mut cmd = {
        let ext = script_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("ps1") {
            let mut c = std::process::Command::new("powershell");
            c.args(["-ExecutionPolicy", "Bypass", "-File", &script_path.to_string_lossy().into_owned()]);
            c
        } else {
            std::process::Command::new(&script_path)
        }
    };
    #[cfg(not(windows))]
    let mut cmd = std::process::Command::new(&script_path);
    if let Some(a) = arg {
        cmd.arg(a);
    }
    inject_env(&mut cmd, env);
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| format!("Could not spawn {:?}: {e}", script_path.display()))?;

    // Enforce a 5-second timeout using a background thread + channel.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    let output = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(format!("Script error: {e}")),
        Err(_) => return Err(format!("Script {script_ref:?} timed out after 5 seconds")),
    };

    if !output.stderr.is_empty() {
        eprintln!(
            "[nimble] script {script_ref:?} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(vec![]);
    }

    // Try JSON array first; fall back to treating the entire output as a single item title.
    if let Ok(items) = serde_json::from_str::<Vec<ListItem>>(&stdout) {
        return Ok(items);
    }
    Ok(vec![ListItem {
        title: stdout,
        subtext: None,
    }])
}

/// Run the script identified by `script_ref`, optionally passing `arg` as a
/// positional argument. Returns a list of string values on success.
///
/// `script_ref` is the raw value from the YAML `script:` field. It may be a
/// plain filename (resolved relative to `command_dir`) or contain `${VAR}`
/// tokens that are substituted first.
///
/// Script stdout is parsed as a JSON array of strings first; if that fails,
/// the entire trimmed output is returned as a single-element vec.
///
/// A 5-second timeout is enforced; the function returns `Err` on timeout.
pub fn run_script_values(
    command_dir: &Path,
    script_ref: &str,
    arg: Option<&str>,
    env: &ScriptEnv<'_>,
) -> Result<Vec<String>, String> {
    let script_path = resolve_script_path(script_ref, command_dir, env)?;
    if !script_path.exists() {
        return Err(format!("Script not found: {}", script_path.display()));
    }

    #[cfg(windows)]
    let mut cmd = {
        let ext = script_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("ps1") {
            let mut c = std::process::Command::new("powershell");
            c.args(["-ExecutionPolicy", "Bypass", "-File", &script_path.to_string_lossy().into_owned()]);
            c
        } else {
            std::process::Command::new(&script_path)
        }
    };
    #[cfg(not(windows))]
    let mut cmd = std::process::Command::new(&script_path);
    if let Some(a) = arg {
        cmd.arg(a);
    }
    inject_env(&mut cmd, env);
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| format!("Could not spawn {:?}: {e}", script_path.display()))?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    let output = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(format!("Script error: {e}")),
        Err(_) => return Err(format!("Script {script_ref:?} timed out after 5 seconds")),
    };

    if !output.stderr.is_empty() {
        eprintln!(
            "[nimble] script {script_ref:?} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(vec![]);
    }

    // Try JSON array of strings first; fall back to treating the output as a single value.
    if let Ok(values) = serde_json::from_str::<Vec<String>>(&stdout) {
        return Ok(values);
    }
    Ok(vec![stdout])
}

// ── Command loader ─────────────────────────────────────────────────────────────

/// Collect all `.yaml` / `.yml` file paths under `config_dir` recursively.
fn collect_yaml_files(config_dir: &Path) -> Vec<std::path::PathBuf> {
    WalkDir::new(config_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            matches!(
                e.path().extension().and_then(|x| x.to_str()),
                Some("yaml") | Some("yml")
            )
        })
        .map(|e| e.into_path())
        .collect()
}

/// Ensure `config_dir` exists. If no YAML files are found, seed the example
/// commands. Then walk the tree, parse every `.yaml`/`.yml` file as a single
/// `Command`, and return the collected list.
/// Files are processed oldest-first (by mtime) so that the original command
/// always wins when duplicates are present. Files that fail to parse are
/// skipped (with an eprintln warning) so one malformed file does not prevent
/// others from loading.
pub fn load_from_dir(config_dir: &Path, allow_duplicates: bool) -> Result<LoadResult, String> {
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("Could not create config directory: {e}"))?;

    // Seed examples if the directory contains no YAML files yet.
    if collect_yaml_files(config_dir).is_empty() {
        for (rel_path, content) in SEED_FILES {
            let dest = config_dir.join(rel_path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
            }
            fs::write(&dest, content)
                .map_err(|e| format!("Could not write {}: {e}", dest.display()))?;
        }
        // Seed co-located scripts and mark them executable.
        for (rel_path, content) in SEED_SCRIPTS {
            let dest = config_dir.join(rel_path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
            }
            fs::write(&dest, content)
                .map_err(|e| format!("Could not write {}: {e}", dest.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(&dest) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&dest, perms);
                }
            }
        }
    }

    // Sort files oldest-first by mtime; use path as a stable tiebreaker.
    let mut yaml_files = collect_yaml_files(config_dir);
    yaml_files.sort_by(|a, b| {
        let mtime_a = fs::metadata(a).and_then(|m| m.modified()).ok();
        let mtime_b = fs::metadata(b).and_then(|m| m.modified()).ok();
        mtime_a.cmp(&mtime_b).then_with(|| a.cmp(b))
    });

    let mut commands = Vec::new();
    let mut duplicates = Vec::new();
    let mut reserved: Vec<ReservedPhraseWarning> = Vec::new();
    // Maps lowercase phrase → relative path of the file that claimed it.
    // Only used when allow_duplicates is false.
    let mut seen: HashMap<String, String> = HashMap::new();

    for path in yaml_files {
        // Use a config-dir-relative path for human-readable warnings.
        let display = path
            .strip_prefix(config_dir)
            .unwrap_or(&path)
            .display()
            .to_string();

        match fs::read_to_string(&path) {
            Err(e) => eprintln!("[nimble] could not read {}: {e}", path.display()),
            Ok(yaml) => match serde_yaml::from_str::<Command>(&yaml) {
                Err(e) => eprintln!("[nimble] could not parse {}: {e}", path.display()),
                Ok(cmd) if !cmd.enabled => {} // disabled — silently skip
                Ok(cmd) => {
                    let key = cmd.phrase.to_lowercase();
                    // Reserved namespace: reject any phrase that starts with `/`.
                    // These are reserved for built-in app commands (e.g. `/ctx set`, `/ctx reset`).
                    if key.starts_with('/') {
                        eprintln!("[nimble] reserved phrase {:?} in {display}, skipping", cmd.phrase);
                        reserved.push(ReservedPhraseWarning {
                            phrase: cmd.phrase,
                            file: display,
                        });
                        continue;
                    }
                    if !allow_duplicates {
                        if let Some(winner) = seen.get(&key) {
                            eprintln!(
                                "[nimble] duplicate phrase {:?} in {display}, kept {winner}",
                                cmd.phrase
                            );
                            duplicates.push(DuplicateWarning {
                                phrase: cmd.phrase.clone(),
                                kept: winner.clone(),
                                ignored: display,
                            });
                            continue;
                        }
                        seen.insert(key, display);
                    }
                    // Record the directory containing this command file, relative
                    // to the commands root, so the frontend can pass it back when
                    // loading co-located list files.
                    let source_dir = path
                        .parent()
                        .and_then(|p| p.strip_prefix(config_dir).ok())
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    let mut cmd = cmd;
                    cmd.source_dir = source_dir;
                    commands.push(cmd);
                }
            },
        }
    }

    Ok(LoadResult { commands, duplicates, reserved })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_yaml(dir: &TempDir, relative: &str, content: &str) {
        let path = dir.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    // ── YAML parsing ──────────────────────────────────────────────────────────

    #[test]
    fn parses_open_url_command() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "open-google.yaml",
            "phrase: open google\ntitle: Open Google\naction:\n  type: open_url\n  config:\n    url: https://www.google.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].phrase, "open google");
        assert!(matches!(result.commands[0].action, Action::OpenUrl(_)));
    }

    #[test]
    fn parses_paste_text_command() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "paste.yaml",
            "phrase: paste email\ntitle: Paste email\naction:\n  type: paste_text\n  config:\n    text: hello@example.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        assert!(matches!(result.commands[0].action, Action::PasteText(_)));
    }

    #[test]
    fn parses_copy_text_command() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "copy.yaml",
            "phrase: copy email\ntitle: Copy email\naction:\n  type: copy_text\n  config:\n    text: hello@example.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        assert!(matches!(result.commands[0].action, Action::CopyText(_)));
    }

    // ── Deduplication ─────────────────────────────────────────────────────────

    #[test]
    fn duplicate_phrase_emits_warning_and_keeps_first() {
        let dir = TempDir::new().unwrap();
        // Write two files with identical phrases; the oldest-mtime file wins.
        // Easiest way to guarantee mtime ordering is to sleep briefly, but that
        // is fragile in CI. Instead we rely on alphabetical sort as a tiebreaker
        // by naming them a.yaml (kept) and b.yaml (ignored).
        write_yaml(
            &dir,
            "a.yaml",
            "phrase: open google\ntitle: First\naction:\n  type: open_url\n  config:\n    url: https://www.google.com\n",
        );
        write_yaml(
            &dir,
            "b.yaml",
            "phrase: open google\ntitle: Second\naction:\n  type: open_url\n  config:\n    url: https://duckduckgo.com\n",
        );
        let result = load_from_dir(dir.path(), false).unwrap();
        assert_eq!(result.commands.len(), 1, "only one command should survive");
        assert_eq!(result.duplicates.len(), 1, "one duplicate warning expected");
        assert_eq!(result.duplicates[0].phrase, "open google");
    }

    // ── Disabled commands ─────────────────────────────────────────────────────

    #[test]
    fn disabled_command_is_skipped() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "disabled.yaml",
            "phrase: hidden cmd\ntitle: Hidden\nenabled: false\naction:\n  type: open_url\n  config:\n    url: https://example.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert!(result.commands.is_empty(), "disabled command must be filtered out");
    }

    // ── Malformed YAML ────────────────────────────────────────────────────────

    #[test]
    fn malformed_yaml_is_skipped_without_panic() {
        let dir = TempDir::new().unwrap();
        write_yaml(&dir, "bad.yaml", "this: is: not: valid: yaml: ::::\n");
        // A second, valid file should still load fine
        write_yaml(
            &dir,
            "good.yaml",
            "phrase: open google\ntitle: Open Google\naction:\n  type: open_url\n  config:\n    url: https://www.google.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1, "only the valid command should load");
    }

    #[test]
    fn parses_static_list_command_without_item_action() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "sub/show.yaml",
            "phrase: team emails\ntitle: Team emails\naction:\n  type: static_list\n  config:\n    list: team-emails\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        if let Action::StaticList(cfg) = &result.commands[0].action {
            assert_eq!(cfg.list, "team-emails");
            assert!(cfg.item_action.is_none());
        } else {
            panic!("expected StaticList action");
        }
        // source_dir should reflect the subdirectory
        assert_eq!(result.commands[0].source_dir, "sub");
    }

    #[test]
    fn source_dir_is_empty_for_root_commands() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "open.yaml",
            "phrase: open google\ntitle: Open Google\naction:\n  type: open_url\n  config:\n    url: https://www.google.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands[0].source_dir, "");
    }

    #[test]
    fn parses_static_list_command_with_item_action_paste() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "show.yaml",
            "phrase: pick snippet\ntitle: Snippets\naction:\n  type: static_list\n  config:\n    list: snippets\n    item_action: paste_text\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        if let Action::StaticList(cfg) = &result.commands[0].action {
            assert_eq!(cfg.item_action, Some(ItemAction::PasteText));
        } else {
            panic!("expected StaticList action");
        }
    }

    fn write_list(dir: &TempDir, name: &str, content: &str) {
        let path = dir.path().join(format!("{name}.tsv"));
        fs::write(path, content).unwrap();
    }

    #[test]
    fn load_list_returns_items() {
        let dir = TempDir::new().unwrap();
        write_list(
            &dir,
            "emails",
            "Alice\talice@example.com\nBob\tbob@example.com\n",
        );
        let env = test_env(&dir);
        let items = load_list(dir.path(), "emails", &env).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Alice");
        assert_eq!(items[0].subtext.as_deref(), Some("alice@example.com"));
        assert_eq!(items[1].title, "Bob");
    }

    #[test]
    fn load_list_item_without_subtext() {
        let dir = TempDir::new().unwrap();
        write_list(&dir, "names", "Alice\nBob\n");
        let env = test_env(&dir);
        let items = load_list(dir.path(), "names", &env).unwrap();
        assert_eq!(items.len(), 2);
        assert!(items[0].subtext.is_none());
    }

    #[test]
    fn load_list_skips_comments_and_blank_lines() {
        let dir = TempDir::new().unwrap();
        write_list(&dir, "mixed", "# Header comment\nAlice\talice@example.com\n\n# Another comment\nBob\tbob@example.com\n");
        let env = test_env(&dir);
        let items = load_list(dir.path(), "mixed", &env).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Alice");
        assert_eq!(items[1].title, "Bob");
    }

    #[test]
    fn load_list_title_with_comma_works() {
        let dir = TempDir::new().unwrap();
        write_list(&dir, "commas", "Smith, Alice\talice@example.com\n");
        let env = test_env(&dir);
        let items = load_list(dir.path(), "commas", &env).unwrap();
        assert_eq!(items[0].title, "Smith, Alice");
        assert_eq!(items[0].subtext.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn load_list_missing_file_returns_err() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(load_list(dir.path(), "nonexistent", &env).is_err());
    }

    #[test]
    fn load_list_rejects_path_traversal_dotdot() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(load_list(dir.path(), "../secret", &env).is_err());
    }

    #[test]
    fn load_list_rejects_path_with_slash() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(load_list(dir.path(), "sub/file", &env).is_err());
    }

    // ── DynamicListConfig parsing ─────────────────────────────────────────────

    #[test]
    fn parses_dynamic_list_command_explicit_arg_none() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "dyn.yaml",
            "phrase: hello script\ntitle: Hello\naction:\n  type: dynamic_list\n  config:\n    script: hello.sh\n    arg: none\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        if let Action::DynamicList(cfg) = &result.commands[0].action {
            assert_eq!(cfg.script, "hello.sh");
            assert_eq!(cfg.arg, ArgMode::None);
            assert!(cfg.item_action.is_none());
        } else {
            panic!("expected DynamicList action");
        }
    }

    #[test]
    fn parses_dynamic_list_command_default_arg_mode() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "dyn.yaml",
            "phrase: hello script\ntitle: Hello\naction:\n  type: dynamic_list\n  config:\n    script: hello.sh\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        if let Action::DynamicList(cfg) = &result.commands[0].action {
            assert_eq!(cfg.arg, ArgMode::None, "arg should default to none");
        } else {
            panic!("expected DynamicList action");
        }
    }

    #[test]
    fn parses_dynamic_list_command_required_with_item_action() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "dyn.yaml",
            "phrase: search things\ntitle: Search\naction:\n  type: dynamic_list\n  config:\n    script: search.sh\n    arg: required\n    item_action: paste_text\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        if let Action::DynamicList(cfg) = &result.commands[0].action {
            assert_eq!(cfg.arg, ArgMode::Required);
            assert_eq!(cfg.item_action, Some(ItemAction::PasteText));
        } else {
            panic!("expected DynamicList action");
        }
    }

    // ── run_script ────────────────────────────────────────────────────────────

    fn test_env(dir: &TempDir) -> ScriptEnv<'static> {
        // Leak the path so we get a 'static lifetime — acceptable in tests.
        let config_dir: &'static Path = Box::leak(dir.path().to_path_buf().into_boxed_path());
        let command_dir: &'static Path = config_dir;
        let user_env: &'static HashMap<String, String> =
            Box::leak(Box::new(HashMap::new()));
        ScriptEnv {
            context: "test-context",
            phrase: "test phrase",
            config_dir,
            command_dir,
            user_env,
            allow_external_paths: true,
        }
    }

    #[test]
    fn run_script_rejects_path_traversal_dotdot() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(run_script(dir.path(), "../secret.sh", None, &env).is_err());
    }

    #[test]
    fn run_script_rejects_path_with_slash() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(run_script(dir.path(), "sub/file.sh", None, &env).is_err());
    }

    #[test]
    fn run_script_missing_script_returns_err() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(run_script(dir.path(), "nonexistent.sh", None, &env).is_err());
    }

    #[cfg(unix)]
    fn make_script(dir: &TempDir, name: &str, content: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_script_json_output_returns_items() {
        let dir = TempDir::new().unwrap();
        make_script(
            &dir,
            "test.sh",
            "#!/bin/sh\necho '[{\"title\":\"A\"},{\"title\":\"B\",\"subtext\":\"sub\"}]'\n",
        );
        let env = test_env(&dir);
        let items = run_script(dir.path(), "test.sh", None, &env).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "A");
        assert_eq!(items[1].subtext.as_deref(), Some("sub"));
    }

    #[cfg(unix)]
    #[test]
    fn run_script_plain_text_returns_single_item() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "plain.sh", "#!/bin/sh\necho 'hello world'\n");
        let env = test_env(&dir);
        let items = run_script(dir.path(), "plain.sh", None, &env).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "hello world");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_passes_arg_to_script() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "echo-arg.sh", "#!/bin/sh\necho \"$1\"\n");
        let env = test_env(&dir);
        let items = run_script(dir.path(), "echo-arg.sh", Some("myarg"), &env).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "myarg");
    }

    // ── run_script_values ────────────────────────────────────────────────────────────

    #[test]
    fn run_script_values_rejects_path_traversal_dotdot() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(run_script_values(dir.path(), "../secret.sh", None, &env).is_err());
    }

    #[test]
    fn run_script_values_rejects_path_with_slash() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(run_script_values(dir.path(), "sub/file.sh", None, &env).is_err());
    }

    #[test]
    fn run_script_values_missing_script_returns_err() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(run_script_values(dir.path(), "nonexistent.sh", None, &env).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn run_script_values_json_array_returns_strings() {
        let dir = TempDir::new().unwrap();
        make_script(
            &dir,
            "values.sh",
            "#!/bin/sh\necho '[\"alpha\",\"beta\",\"gamma\"]'\n",
        );
        let env = test_env(&dir);
        let values = run_script_values(dir.path(), "values.sh", None, &env).unwrap();
        assert_eq!(values, vec!["alpha", "beta", "gamma"]);
    }

    #[cfg(unix)]
    #[test]
    fn run_script_values_plain_text_returns_single_value() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "plain.sh", "#!/bin/sh\necho 'hello world'\n");
        let env = test_env(&dir);
        let values = run_script_values(dir.path(), "plain.sh", None, &env).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "hello world");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_values_passes_arg_to_script() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "echo-arg.sh", "#!/bin/sh\necho \"$1\"\n");
        let env = test_env(&dir);
        let values = run_script_values(dir.path(), "echo-arg.sh", Some("myvalue"), &env).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "myvalue");
    }

    // ── ScriptActionConfig parsing ──────────────────────────────────────────────────

    #[test]
    fn parses_script_action_command_paste() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "sa.yaml",
            "phrase: paste ts\ntitle: Paste timestamp\naction:\n  type: script_action\n  config:\n    script: ts.sh\n    result_action: paste_text\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        if let Action::ScriptAction(cfg) = &result.commands[0].action {
            assert_eq!(cfg.script, "ts.sh");
            assert_eq!(cfg.arg, ArgMode::None);
            assert_eq!(cfg.result_action, ResultAction::PasteText);
            assert!(cfg.prefix.is_none());
            assert!(cfg.suffix.is_none());
        } else {
            panic!("expected ScriptAction action");
        }
    }

    #[test]
    fn parses_script_action_command_open_url_with_arg() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "sa.yaml",
            "phrase: open urls\ntitle: Open URLs\naction:\n  type: script_action\n  config:\n    script: urls.sh\n    arg: required\n    result_action: open_url\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        if let Action::ScriptAction(cfg) = &result.commands[0].action {
            assert_eq!(cfg.arg, ArgMode::Required);
            assert_eq!(cfg.result_action, ResultAction::OpenUrl);
        } else {
            panic!("expected ScriptAction action");
        }
    }

    #[test]
    fn parses_script_action_command_copy_with_prefix_suffix() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "sa.yaml",
            "phrase: copy emails\ntitle: Copy emails\naction:\n  type: script_action\n  config:\n    script: emails.sh\n    result_action: copy_text\n    prefix: \"To: \"\n    suffix: \"\\n\"\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        if let Action::ScriptAction(cfg) = &result.commands[0].action {
            assert_eq!(cfg.result_action, ResultAction::CopyText);
            assert_eq!(cfg.prefix.as_deref(), Some("To: "));
            assert_eq!(cfg.suffix.as_deref(), Some("\n"));
        } else {
            panic!("expected ScriptAction action");
        }
    }

    // ── Reserved namespace ────────────────────────────────────────────────────

    #[test]
    fn reserved_slash_phrase_is_rejected() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "slash.yaml",
            "phrase: /ctx set foo\ntitle: Bad\naction:\n  type: open_url\n  config:\n    url: https://example.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert!(result.commands.is_empty(), "/phrase must not load as a command");
        assert_eq!(result.reserved.len(), 1);
        assert_eq!(result.reserved[0].phrase, "/ctx set foo");
    }

    #[test]
    fn reserved_slash_any_suffix_is_rejected() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "slash2.yaml",
            "phrase: /ctx reset\ntitle: Bad\naction:\n  type: open_url\n  config:\n    url: https://example.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert!(result.commands.is_empty());
        assert_eq!(result.reserved.len(), 1);
        assert_eq!(result.reserved[0].phrase, "/ctx reset");
    }

    #[test]
    fn phrase_with_slash_not_at_start_is_accepted() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "no-slash.yaml",
            "phrase: open github/issues\ntitle: Not reserved\naction:\n  type: open_url\n  config:\n    url: https://github.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1, "slash not at start is not reserved");
        assert!(result.reserved.is_empty());
    }

    #[test]
    fn normal_phrase_is_accepted() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "open-google.yaml",
            "phrase: open google\ntitle: Open Google\naction:\n  type: open_url\n  config:\n    url: https://www.google.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1, "normal phrase is not reserved");
        assert!(result.reserved.is_empty());
    }

    #[test]
    fn reserved_vec_empty_without_violations() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "good.yaml",
            "phrase: open google\ntitle: Open Google\naction:\n  type: open_url\n  config:\n    url: https://www.google.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert!(result.reserved.is_empty());
    }

    // ── allow_duplicates flag ────────────────────────────────────────────────

    #[test]
    fn allow_duplicates_true_loads_all_commands() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "a.yaml",
            "phrase: open google\ntitle: First\naction:\n  type: open_url\n  config:\n    url: https://www.google.com\n",
        );
        write_yaml(
            &dir,
            "b.yaml",
            "phrase: open google\ntitle: Second\naction:\n  type: open_url\n  config:\n    url: https://duckduckgo.com\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 2, "both commands should load when allow_duplicates=true");
        assert!(result.duplicates.is_empty(), "no warnings when allow_duplicates=true");
    }

    // ── Built-in env var injection ───────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn run_script_injects_nimble_context() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_CONTEXT\"\n");
        let env = ScriptEnv {
            context: "my-ctx",
            phrase: "test phrase",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &HashMap::new(),
            allow_external_paths: true,
        };
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, "my-ctx");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_injects_nimble_phrase() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_PHRASE\"\n");
        let env = ScriptEnv {
            context: "",
            phrase: "search contacts",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &HashMap::new(),
            allow_external_paths: true,
        };
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, "search contacts");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_injects_nimble_os() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_OS\"\n");
        let env = test_env(&dir);
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, "macos");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_injects_nimble_version() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_VERSION\"\n");
        let env = test_env(&dir);
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, env!("CARGO_PKG_VERSION"));
    }

    #[cfg(unix)]
    #[test]
    fn run_script_injects_nimble_config_dir() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_CONFIG_DIR\"\n");
        let env = ScriptEnv {
            context: "",
            phrase: "test",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &HashMap::new(),
            allow_external_paths: true,
        };
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, dir.path().to_string_lossy());
    }

    #[cfg(unix)]
    #[test]
    fn run_script_injects_nimble_command_dir() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_COMMAND_DIR\"\n");
        let env = ScriptEnv {
            context: "",
            phrase: "test",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &HashMap::new(),
            allow_external_paths: true,
        };
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, dir.path().to_string_lossy());
    }

    #[cfg(unix)]
    #[test]
    fn run_script_values_injects_nimble_context() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_CONTEXT\"\n");
        let env = ScriptEnv {
            context: "work",
            phrase: "copy uuid",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &HashMap::new(),
            allow_external_paths: true,
        };
        let values = run_script_values(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(values[0], "work");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_empty_context_injects_empty_string() {
        let dir = TempDir::new().unwrap();
        // Script outputs NIMBLE_CONTEXT surrounded by markers so we can detect empty
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"ctx=$NIMBLE_CONTEXT|\"\n");
        let env = ScriptEnv {
            context: "",
            phrase: "test",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &HashMap::new(),
            allow_external_paths: true,
        };
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, "ctx=|");
    }

    // ── validate_env_key ────────────────────────────────────────────────────

    #[test]
    fn validate_env_key_accepts_uppercase() {
        assert!(validate_env_key("MY_VAR", "test").is_ok());
    }

    #[test]
    fn validate_env_key_accepts_lowercase() {
        assert!(validate_env_key("my_var", "test").is_ok());
    }

    #[test]
    fn validate_env_key_accepts_mixed_case_with_digits() {
        assert!(validate_env_key("Var_123", "test").is_ok());
    }

    #[test]
    fn validate_env_key_accepts_underscore_start() {
        assert!(validate_env_key("_PRIVATE", "test").is_ok());
    }

    #[test]
    fn validate_env_key_rejects_empty() {
        assert!(validate_env_key("", "test").is_err());
    }

    #[test]
    fn validate_env_key_rejects_nimble_prefix() {
        assert!(validate_env_key("NIMBLE_CONTEXT", "test").is_err());
    }

    #[test]
    fn validate_env_key_rejects_nimble_custom() {
        assert!(validate_env_key("NIMBLE_MY_VAR", "test").is_err());
    }

    #[test]
    fn validate_env_key_rejects_digit_start() {
        assert!(validate_env_key("1VAR", "test").is_err());
    }

    #[test]
    fn validate_env_key_rejects_hyphen() {
        assert!(validate_env_key("MY-VAR", "test").is_err());
    }

    #[test]
    fn validate_env_key_rejects_dot() {
        assert!(validate_env_key("MY.VAR", "test").is_err());
    }

    // ── load_env_yaml ───────────────────────────────────────────────────────

    #[test]
    fn load_env_yaml_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let result = load_env_yaml(&dir.path().join("env.yaml")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_env_yaml_empty_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "").unwrap();
        let result = load_env_yaml(&dir.path().join("env.yaml")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_env_yaml_parses_string_values() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "MY_EMAIL: alice@example.com\nTEAM: engineering\n").unwrap();
        let result = load_env_yaml(&dir.path().join("env.yaml")).unwrap();
        assert_eq!(result.get("MY_EMAIL").unwrap(), "alice@example.com");
        assert_eq!(result.get("TEAM").unwrap(), "engineering");
    }

    #[test]
    fn load_env_yaml_coerces_number_to_string() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "PORT: 8080\n").unwrap();
        let result = load_env_yaml(&dir.path().join("env.yaml")).unwrap();
        assert_eq!(result.get("PORT").unwrap(), "8080");
    }

    #[test]
    fn load_env_yaml_coerces_bool_to_string() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "ENABLED: true\n").unwrap();
        let result = load_env_yaml(&dir.path().join("env.yaml")).unwrap();
        assert_eq!(result.get("ENABLED").unwrap(), "true");
    }

    #[test]
    fn load_env_yaml_rejects_nimble_prefix() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "NIMBLE_HACK: evil\n").unwrap();
        assert!(load_env_yaml(&dir.path().join("env.yaml")).is_err());
    }

    #[test]
    fn load_env_yaml_rejects_nested_map() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "NESTED:\n  a: b\n").unwrap();
        assert!(load_env_yaml(&dir.path().join("env.yaml")).is_err());
    }

    // ── build_user_env ──────────────────────────────────────────────────────

    #[test]
    fn build_user_env_empty_when_no_files() {
        let dir = TempDir::new().unwrap();
        let cmd_dir = dir.path().join("commands").join("my-cmd");
        fs::create_dir_all(&cmd_dir).unwrap();
        let result = build_user_env(dir.path(), &cmd_dir, &HashMap::new()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn build_user_env_loads_global_env() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "TEAM: ops\n").unwrap();
        let cmd_dir = dir.path().join("commands").join("my-cmd");
        fs::create_dir_all(&cmd_dir).unwrap();
        let result = build_user_env(dir.path(), &cmd_dir, &HashMap::new()).unwrap();
        assert_eq!(result.get("TEAM").unwrap(), "ops");
    }

    #[test]
    fn build_user_env_sidecar_overrides_global() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "TEAM: ops\nREGION: us\n").unwrap();
        let cmd_dir = dir.path().join("commands").join("my-cmd");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join("env.yaml"), "TEAM: dev\n").unwrap();
        let result = build_user_env(dir.path(), &cmd_dir, &HashMap::new()).unwrap();
        assert_eq!(result.get("TEAM").unwrap(), "dev");
        assert_eq!(result.get("REGION").unwrap(), "us");
    }

    #[test]
    fn build_user_env_inline_overrides_sidecar() {
        let dir = TempDir::new().unwrap();
        let cmd_dir = dir.path().join("commands").join("my-cmd");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join("env.yaml"), "TEAM: dev\n").unwrap();
        let mut inline = HashMap::new();
        inline.insert("TEAM".to_string(), "override".to_string());
        let result = build_user_env(dir.path(), &cmd_dir, &inline).unwrap();
        assert_eq!(result.get("TEAM").unwrap(), "override");
    }

    #[test]
    fn build_user_env_full_precedence_chain() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("env.yaml"), "A: global\nB: global\nC: global\n").unwrap();
        let cmd_dir = dir.path().join("commands").join("my-cmd");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join("env.yaml"), "B: sidecar\nC: sidecar\n").unwrap();
        let mut inline = HashMap::new();
        inline.insert("C".to_string(), "inline".to_string());
        let result = build_user_env(dir.path(), &cmd_dir, &inline).unwrap();
        assert_eq!(result.get("A").unwrap(), "global");
        assert_eq!(result.get("B").unwrap(), "sidecar");
        assert_eq!(result.get("C").unwrap(), "inline");
    }

    #[test]
    fn build_user_env_rejects_nimble_in_inline() {
        let dir = TempDir::new().unwrap();
        let cmd_dir = dir.path().join("commands").join("my-cmd");
        fs::create_dir_all(&cmd_dir).unwrap();
        let mut inline = HashMap::new();
        inline.insert("NIMBLE_HACK".to_string(), "evil".to_string());
        assert!(build_user_env(dir.path(), &cmd_dir, &inline).is_err());
    }

    #[test]
    fn build_user_env_no_parent_traversal() {
        // Sidecar is only in the same directory — parent env.yaml is ignored.
        let dir = TempDir::new().unwrap();
        let parent = dir.path().join("commands");
        fs::create_dir_all(&parent).unwrap();
        fs::write(parent.join("env.yaml"), "PARENT: yes\n").unwrap();
        let cmd_dir = parent.join("my-cmd");
        fs::create_dir_all(&cmd_dir).unwrap();
        let result = build_user_env(dir.path(), &cmd_dir, &HashMap::new()).unwrap();
        assert!(!result.contains_key("PARENT"));
    }

    // ── User env injection into scripts ─────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn run_script_injects_user_env() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$MY_VAR\"\n");
        let mut user_env = HashMap::new();
        user_env.insert("MY_VAR".to_string(), "hello-from-env".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "test",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, "hello-from-env");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_values_injects_user_env() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$TEAM_ID\"\n");
        let mut user_env = HashMap::new();
        user_env.insert("TEAM_ID".to_string(), "T12345".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "test",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        let values = run_script_values(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(values[0], "T12345");
    }

    #[cfg(unix)]
    #[test]
    fn run_script_builtins_override_user_nimble_prefix() {
        // Even if user_env somehow contains a NIMBLE_ key (e.g. from a
        // malformed env.yaml that bypassed validation), builtins always win.
        let dir = TempDir::new().unwrap();
        make_script(&dir, "env.sh", "#!/bin/sh\necho \"$NIMBLE_CONTEXT\"\n");
        let mut user_env = HashMap::new();
        user_env.insert("NIMBLE_CONTEXT".to_string(), "evil".to_string());
        let env = ScriptEnv {
            context: "real-context",
            phrase: "test",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        let items = run_script(dir.path(), "env.sh", None, &env).unwrap();
        assert_eq!(items[0].title, "real-context");
    }

    // ── Inline env in command YAML ──────────────────────────────────────────

    #[test]
    fn parses_command_yaml_with_inline_env() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "my-cmd.yaml",
            "phrase: test cmd\ntitle: Test\nenv:\n  MY_VAR: hello\n  OTHER: world\naction:\n  type: paste_text\n  config:\n    text: hi\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].env.get("MY_VAR").unwrap(), "hello");
        assert_eq!(result.commands[0].env.get("OTHER").unwrap(), "world");
    }

    #[test]
    fn parses_command_yaml_without_inline_env() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            &dir,
            "my-cmd.yaml",
            "phrase: test cmd\ntitle: Test\naction:\n  type: paste_text\n  config:\n    text: hi\n",
        );
        let result = load_from_dir(dir.path(), true).unwrap();
        assert_eq!(result.commands.len(), 1);
        assert!(result.commands[0].env.is_empty());
    }

    // ── substitute_vars ─────────────────────────────────────────────────────

    #[test]
    fn substitute_vars_plain_text_unchanged() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert_eq!(substitute_vars("hello world", &env).unwrap(), "hello world");
    }

    #[test]
    fn substitute_vars_single_builtin() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert_eq!(
            substitute_vars("ctx=${NIMBLE_CONTEXT}", &env).unwrap(),
            "ctx=test-context"
        );
    }

    #[test]
    fn substitute_vars_multiple_vars() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert_eq!(
            substitute_vars("${NIMBLE_CONTEXT}-${NIMBLE_PHRASE}", &env).unwrap(),
            "test-context-test phrase"
        );
    }

    #[test]
    fn substitute_vars_user_env_var() {
        let dir = TempDir::new().unwrap();
        let mut user_env = HashMap::new();
        user_env.insert("MY_DIR".to_string(), "/custom/path".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        assert_eq!(
            substitute_vars("${MY_DIR}/script.sh", &env).unwrap(),
            "/custom/path/script.sh"
        );
    }

    #[test]
    fn substitute_vars_undefined_var_returns_err() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        let result = substitute_vars("${UNDEFINED_VAR}", &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Undefined variable"));
    }

    #[test]
    fn substitute_vars_unterminated_returns_err() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        let result = substitute_vars("${NIMBLE_CONTEXT", &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unterminated"));
    }

    #[test]
    fn substitute_vars_empty_var_name_returns_err() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        let result = substitute_vars("${}", &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty variable name"));
    }

    #[test]
    fn substitute_vars_builtin_overrides_user_env() {
        let dir = TempDir::new().unwrap();
        let mut user_env = HashMap::new();
        user_env.insert("NIMBLE_CONTEXT".to_string(), "evil".to_string());
        let env = ScriptEnv {
            context: "real",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        assert_eq!(substitute_vars("${NIMBLE_CONTEXT}", &env).unwrap(), "real");
    }

    // ── resolve_script_path ─────────────────────────────────────────────────

    #[test]
    fn resolve_script_path_plain_name_co_located() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        let path = resolve_script_path("hello.sh", dir.path(), &env).unwrap();
        assert_eq!(path, dir.path().join("hello.sh"));
    }

    #[test]
    fn resolve_script_path_rejects_slash_without_vars() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(resolve_script_path("sub/hello.sh", dir.path(), &env).is_err());
    }

    #[test]
    fn resolve_script_path_rejects_dotdot_without_vars() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(resolve_script_path("../hello.sh", dir.path(), &env).is_err());
    }

    #[test]
    fn resolve_script_path_var_substitution_absolute() {
        let dir = TempDir::new().unwrap();
        let mut user_env = HashMap::new();
        user_env.insert("SCRIPTS".to_string(), "/opt/scripts".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        let path = resolve_script_path("${SCRIPTS}/run.sh", dir.path(), &env).unwrap();
        assert_eq!(path, PathBuf::from("/opt/scripts/run.sh"));
    }

    #[test]
    fn resolve_script_path_external_blocked_when_false() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "hello.sh", "#!/bin/sh\necho ok\n");
        let mut user_env = HashMap::new();
        user_env.insert("SCRIPTS".to_string(), "/opt/scripts".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: false,
        };
        let result = resolve_script_path("${SCRIPTS}/run.sh", dir.path(), &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("outside the command directory"));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_script_path_command_dir_var_works_when_external_false() {
        let dir = TempDir::new().unwrap();
        make_script(&dir, "hello.sh", "#!/bin/sh\necho ok\n");
        let env = ScriptEnv {
            context: "",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &HashMap::new(),
            allow_external_paths: false,
        };
        // ${NIMBLE_COMMAND_DIR}/hello.sh should still work — it resolves inside command_dir.
        let path = resolve_script_path("${NIMBLE_COMMAND_DIR}/hello.sh", dir.path(), &env).unwrap();
        assert_eq!(path, dir.path().join("hello.sh"));
    }

    // ── resolve_list_path ───────────────────────────────────────────────────

    #[test]
    fn resolve_list_path_plain_name_appends_tsv() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        let path = resolve_list_path("emails", dir.path(), &env).unwrap();
        assert_eq!(path, dir.path().join("emails.tsv"));
    }

    #[test]
    fn resolve_list_path_rejects_slash_without_vars() {
        let dir = TempDir::new().unwrap();
        let env = test_env(&dir);
        assert!(resolve_list_path("sub/emails", dir.path(), &env).is_err());
    }

    #[test]
    fn resolve_list_path_var_substitution_with_tsv_extension() {
        let dir = TempDir::new().unwrap();
        let mut user_env = HashMap::new();
        user_env.insert("LISTS".to_string(), "/shared/lists".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        let path = resolve_list_path("${LISTS}/team.tsv", dir.path(), &env).unwrap();
        assert_eq!(path, PathBuf::from("/shared/lists/team.tsv"));
    }

    #[test]
    fn resolve_list_path_auto_appends_tsv_after_var() {
        let dir = TempDir::new().unwrap();
        let mut user_env = HashMap::new();
        user_env.insert("LISTS".to_string(), "/shared".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: true,
        };
        let path = resolve_list_path("${LISTS}/team", dir.path(), &env).unwrap();
        assert_eq!(path, PathBuf::from("/shared/team.tsv"));
    }

    #[test]
    fn resolve_list_path_external_blocked_when_false() {
        let dir = TempDir::new().unwrap();
        let mut user_env = HashMap::new();
        user_env.insert("LISTS".to_string(), "/opt/lists".to_string());
        let env = ScriptEnv {
            context: "",
            phrase: "",
            config_dir: dir.path(),
            command_dir: dir.path(),
            user_env: &user_env,
            allow_external_paths: false,
        };
        let result = resolve_list_path("${LISTS}/team", dir.path(), &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("outside the command directory"));
    }

    // ── settings: allow_external_paths ──────────────────────────────────────

    // (settings tests for allow_external_paths are in settings.rs)
}
