mod commands;
mod settings;
mod watcher;

use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Listener, Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

// ── Previous-app focus tracking ────────────────────────────────────────────────

/// Holds a platform-specific identifier for the application that had focus
/// before the launcher appeared.
/// - macOS: process ID as a decimal string
/// - Linux: X11 window ID as a decimal string (via `xdo` crate / libxdo)
struct PreviousApp(Mutex<Option<String>>);

/// macOS: captures the frontmost application's PID via NSWorkspace.
/// Called in the global-shortcut handler and tray show/hide before the launcher
/// is shown, so focus can be restored when paste_text is executed.
#[cfg(target_os = "macos")]
fn capture_previous_app(state: &PreviousApp) {
    use objc2_app_kit::NSWorkspace;
    let workspace = NSWorkspace::sharedWorkspace();
    if let Some(app) = workspace.frontmostApplication() {
        let pid = app.processIdentifier();
        // Don't record ourselves
        if pid != std::process::id() as i32 {
            *state.0.lock().unwrap() = Some(pid.to_string());
        }
    }
}

/// Linux: captures the active X11 window ID via `libxdo-sys`.
/// No-op under pure Wayland (libxdo requires an X11 DISPLAY).
#[cfg(target_os = "linux")]
fn capture_previous_app(state: &PreviousApp) {
    // libxdo requires an X11 DISPLAY; skip silently under pure Wayland.
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_none()
    {
        return;
    }
    use libxdo_sys::{xdo_free, xdo_get_active_window, xdo_new};
    use std::ptr;
    unsafe {
        let xdo = xdo_new(ptr::null());
        if xdo.is_null() {
            return;
        }
        let mut win: std::os::raw::c_ulong = 0;
        if xdo_get_active_window(xdo, &mut win) == 0 && win != 0 {
            *state.0.lock().unwrap() = Some(win.to_string());
        }
        xdo_free(xdo);
    }
}

/// Windows: captures the foreground window handle via `GetForegroundWindow`.
/// Stores the HWND cast to `usize` as a decimal string.
#[cfg(target_os = "windows")]
fn capture_previous_app(state: &PreviousApp) {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let hwnd = unsafe { GetForegroundWindow() };
    if !hwnd.is_null() {
        *state.0.lock().unwrap() = Some((hwnd as usize).to_string());
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn capture_previous_app(_state: &PreviousApp) {}

/// macOS: activates the application identified by its PID string.
#[cfg(target_os = "macos")]
fn restore_previous_app(id: String) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    if let Ok(pid) = id.parse::<i32>() {
        if let Some(app) =
            NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        {
            // ActivateIgnoringOtherApps is deprecated in macOS 14 but still works;
            // it has no replacement on NSRunningApplication in objc2-app-kit 0.3.
            #[allow(deprecated)]
            app.activateWithOptions(
                NSApplicationActivationOptions::ActivateIgnoringOtherApps,
            );
        }
    }
}

/// Linux: focuses the X11 window identified by its window ID string via `libxdo-sys`.
/// Gracefully skips under pure Wayland (libxdo unavailable there).
#[cfg(target_os = "linux")]
fn restore_previous_app(win_id: String) {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_none()
    {
        eprintln!("[nimble] focus restore skipped: Wayland without XWayland bridge");
        return;
    }
    use libxdo_sys::{xdo_focus_window, xdo_free, xdo_new, xdo_raise_window};
    use std::ptr;
    if let Ok(win) = win_id.parse::<std::os::raw::c_ulong>() {
        unsafe {
            let xdo = xdo_new(ptr::null());
            if !xdo.is_null() {
                let _ = xdo_focus_window(xdo, win);
                let _ = xdo_raise_window(xdo, win);
                xdo_free(xdo);
            }
        }
    }
}

/// Windows: restores foreground focus to the window identified by its HWND string.
/// Uses `SetForegroundWindow` + `BringWindowToTop`.
/// Note: Windows focus-stealing prevention may silently block `SetForegroundWindow`
/// if this process is not currently the foreground process; call immediately
/// after the launcher window is hidden to minimise the blocking window.
#[cfg(target_os = "windows")]
fn restore_previous_app(id: String) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{BringWindowToTop, SetForegroundWindow};
    if let Ok(hwnd_val) = id.parse::<usize>() {
        let hwnd = hwnd_val as *mut std::ffi::c_void;
        unsafe {
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn restore_previous_app(_id: String) {}

// ── Pure helpers (no Tauri runtime needed — fully testable) ──────────────────

/// URL-encode `param` and substitute it for every `{param}` token in `url`,
/// then validate the resulting URL has a well-formed scheme (RFC 3986).
/// Returns the resolved URL string on success.
pub(crate) fn resolve_url(url: String, param: Option<String>) -> Result<String, String> {
    let resolved = if let Some(p) = param {
        let encoded: String = p
            .bytes()
            .flat_map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
                | b'-' | b'_' | b'.' | b'~' => vec![b as char],
                b' ' => vec!['+'],
                _ => format!("%{:02X}", b).chars().collect(),
            })
            .collect();
        url.replace("{param}", &encoded)
    } else {
        url
    };

    let has_valid_scheme = resolved
        .find(':')
        .map(|colon| {
            let scheme = &resolved[..colon];
            !scheme.is_empty()
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        })
        .unwrap_or(false);

    if !has_valid_scheme {
        return Err(format!("Rejected URL with missing or invalid scheme: {resolved}"));
    }

    Ok(resolved)
}

/// Validate that `text` is safe to place on the clipboard or simulate as a paste.
/// Currently rejects text containing NUL bytes.
pub(crate) fn validate_text(text: &str) -> Result<(), String> {
    if text.contains('\0') {
        return Err("Text must not contain NUL bytes".to_string());
    }
    Ok(())
}

// ── State file helpers ─────────────────────────────────────────────────────────

/// Read the active context from `state.json` inside `config_dir`.
/// Returns an empty string when the file is absent or unreadable.
pub(crate) fn read_context_from_state(config_dir: &std::path::Path) -> String {
    let path = config_dir.join("state.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    val.get("context")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Write the active context to `state.json` inside `config_dir`.
/// Creates the file if absent; overwrites if present.
pub(crate) fn write_context_to_state(
    config_dir: &std::path::Path,
    context: &str,
) -> Result<(), String> {
    let path = config_dir.join("state.json");
    let json = serde_json::json!({ "context": context });
    let content = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ── Deep-link URL parsing ──────────────────────────────────────────────────────

/// Parsed deep-link action from a `nimble://` URL.
#[derive(Debug, PartialEq)]
pub(crate) enum DeepLinkAction {
    /// Set context to the given value.
    CtxSet(String),
    /// Reset (clear) the active context.
    CtxReset,
}

/// Parse a `nimble://` URL into a `DeepLinkAction`.
/// Returns `None` for unrecognised paths.
///
/// Supported routes:
///   nimble://ctx/set/<value>
///   nimble://ctx/reset
pub(crate) fn parse_deep_link(url: &str) -> Option<DeepLinkAction> {
    // Strip the scheme. Accept both nimble:// and nimble:/// (some OS launchers add triple slash).
    let path = url
        .strip_prefix("nimble:///")
        .or_else(|| url.strip_prefix("nimble://"))?;

    if let Some(value) = path.strip_prefix("ctx/set/") {
        let decoded = percent_decode(value);
        let trimmed = decoded.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(DeepLinkAction::CtxSet(trimmed.to_string()))
    } else if path == "ctx/reset" || path == "ctx/reset/" {
        Some(DeepLinkAction::CtxReset)
    } else {
        None
    }
}

/// Minimal percent-decoding for deep-link values.
/// Handles `%XX` sequences and `+` as space.
fn percent_decode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        match b {
            b'%' => {
                let hi = chars.next();
                let lo = chars.next();
                if let (Some(h), Some(l)) = (hi, lo) {
                    if let Ok(byte) = u8::from_str_radix(
                        &format!("{}{}", h as char, l as char),
                        16,
                    ) {
                        out.push(byte as char);
                    } else {
                        // Malformed: pass through literally
                        out.push('%');
                        out.push(h as char);
                        out.push(l as char);
                    }
                } else {
                    out.push('%');
                }
            }
            b'+' => out.push(' '),
            _ => out.push(b as char),
        }
    }
    out
}

// ── One-time config dir migration ───────────────────────────────────────────

/// Migrate the config directory from the old reverse-DNS identifier
/// (`io.switchpanel.nimble`) to the new short identifier (`nimble`).
/// If the old directory exists and the new one does not, all contents
/// are moved. If both exist the migration is skipped (user may have
/// manually created the new dir).
fn migrate_config_dir(new_dir: &std::path::Path) {
    let old_name = "io.switchpanel.nimble";
    // The new config dir's parent is the platform app-support root.
    let Some(parent) = new_dir.parent() else { return };
    let old_dir = parent.join(old_name);
    if old_dir.exists() && !new_dir.exists() {
        if let Err(e) = std::fs::rename(&old_dir, new_dir) {
            eprintln!(
                "[nimble] config migration failed ({} → {}): {e}",
                old_dir.display(),
                new_dir.display()
            );
        } else {
            eprintln!(
                "[nimble] migrated config dir: {} → {}",
                old_dir.display(),
                new_dir.display()
            );
        }
    }
}

// ── Bundled skills ─────────────────────────────────────────────────────────────

/// Content of the nimble-authoring SKILL.md, embedded at compile time.
const SKILL_MD: &str = include_str!("../../.github/skills/nimble-authoring/SKILL.md");

/// Content of the nimble-authoring nimble-spec.yaml, embedded at compile time.
const SKILL_SPEC: &str = include_str!("../../.github/skills/nimble-authoring/nimble-spec.yaml");

/// Copy the bundled nimble-authoring skill files into
/// `<config_dir>/skills/nimble-authoring/`. Always overwrites so the skill
/// stays current with the installed app version.
fn install_bundled_skills(config_dir: &std::path::Path) {
    let dest = config_dir.join("skills").join("nimble-authoring");
    if let Err(e) = std::fs::create_dir_all(&dest) {
        eprintln!("[nimble] could not create skills dir: {e}");
        return;
    }
    for (name, content) in [("SKILL.md", SKILL_MD), ("nimble-spec.yaml", SKILL_SPEC)] {
        if let Err(e) = std::fs::write(dest.join(name), content) {
            eprintln!("[nimble] could not write {name}: {e}");
        }
    }
}

/// Deploy the nimble-authoring skill to `~/.copilot/skills/nimble-authoring/`
/// by creating a symbolic link (macOS/Linux) or copying the files (Windows).
/// Returns Ok with a human-readable status message, or Err on failure.
#[tauri::command]
fn deploy_skill(app: tauri::AppHandle) -> Result<String, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let source = config_dir.join("skills").join("nimble-authoring");
    if !source.exists() {
        return Err("Skill files not found in config directory. Please restart Nimble.".into());
    }

    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let target = home.join(".copilot").join("skills").join("nimble-authoring");

    // If target already exists, check if it's a symlink pointing to our source.
    if target.exists() || target.symlink_metadata().is_ok() {
        if target.is_symlink() {
            let link_dest = std::fs::read_link(&target).map_err(|e| e.to_string())?;
            if link_dest == source {
                return Ok("Already deployed — symlink is up to date.".into());
            }
        }
        // Remove existing target (file, dir, or stale symlink) so we can recreate.
        if target.is_dir() && !target.is_symlink() {
            std::fs::remove_dir_all(&target).map_err(|e| format!("Could not remove existing directory: {e}"))?;
        } else {
            std::fs::remove_file(&target).map_err(|e| format!("Could not remove existing file: {e}"))?;
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Could not create .copilot/skills/: {e}"))?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&source, &target)
            .map_err(|e| format!("Could not create symlink: {e}"))?;
        Ok(format!("Deployed — symlinked to {}", source.display()))
    }

    #[cfg(windows)]
    {
        // Windows symlinks require developer mode or admin privileges.
        // Fall back to a directory junction which works without elevation.
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J",
                   &target.to_string_lossy(),
                   &source.to_string_lossy()])
            .output()
            .map_err(|e| format!("Could not create junction: {e}"))?;
        if target.exists() {
            Ok(format!("Deployed — junction created at {}", target.display()))
        } else {
            // Junction failed; fall back to copy.
            copy_dir_recursive(&source, &target)?;
            Ok(format!("Deployed — copied to {}", target.display()))
        }
    }
}

/// Recursively copy a directory. Used as a Windows fallback when junctions fail.
#[cfg(windows)]
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ── Clipboard helper ───────────────────────────────────────────────────────────

/// Write `text` to the system clipboard.
/// macOS: delegates to the `pbcopy` subprocess (avoids NSPasteboard threading
///        constraints with the main thread).
/// Linux / Windows: uses `arboard`, a pure-Rust cross-platform clipboard crate.
fn write_clipboard_text(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start pbcopy: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Could not write to pbcopy: {e}"))?;
        }
        child
            .wait()
            .map_err(|e| format!("pbcopy failed: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| format!("Could not open clipboard: {e}"))?;
        clipboard
            .set_text(text)
            .map_err(|e| format!("Could not write to clipboard: {e}"))?;
        Ok(())
    }
}

/// Open a URL in the default browser or the registered handler for its scheme.
#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String, param: Option<String>) -> Result<(), String> {
    let resolved = resolve_url(url, param)?;
    app.opener()
        .open_url(resolved, None::<&str>)
        .map_err(|e| e.to_string())
}

struct TrayMenuState {
    show_hide_item: Arc<MenuItem<tauri::Wry>>,
}

/// Persisted application settings, loaded once at startup.
struct SettingsState(Mutex<settings::AppSettings>);

/// Resolved commands root directory, computed once at startup from settings.
struct CommandsRoot(std::path::PathBuf);

/// Update the tray Show/Hide item text to reflect current window visibility.
fn sync_tray(app: &tauri::AppHandle, visible: bool) {
    let text = if visible { "Hide" } else { "Show" };
    // Clone the Arc inside a block so `state` (and its borrow) is dropped before
    // we call set_text, avoiding any lifetime entanglement with State<'_,...>.
    let item = {
        let state = app.state::<TrayMenuState>();
        Arc::clone(&state.show_hide_item)
    };
    item.set_text(text).ok();
}

/// Hide the launcher window (keeps app running in background).
#[tauri::command]
fn hide_window(app: tauri::AppHandle, window: tauri::Window) {
    window.hide().ok();
    sync_tray(&app, false);
}

/// Show and focus the launcher window.
///
/// Resets to the base 640×64 size and re-centers on the active monitor before
/// making the window visible. This corrects drift caused by macOS anchoring
/// `setSize` calls from the bottom-left while the window is hidden: shrinking
/// from a tall (results-visible) state moves the window top downward, so we
/// must correct the position on every show.
#[tauri::command]
fn show_window(app: tauri::AppHandle, window: tauri::Window) {
    window.set_size(tauri::LogicalSize::new(640_f64, 64_f64)).ok();
    window.center().ok();
    window.show().ok();
    window.set_focus().ok();
    sync_tray(&app, true);
}

/// Return the current application settings.
#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> settings::AppSettings {
    app.state::<SettingsState>().0.lock().unwrap().clone()
}

/// Persist a new hotkey to `settings.yaml` and update the in-memory settings.
/// The caller is responsible for also calling `register_shortcut` to activate
/// the shortcut for the current session.
#[tauri::command]
fn save_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let binding = app.state::<SettingsState>();
    let mut state = binding.0.lock().unwrap();
    state.hotkey = Some(hotkey);
    settings::save(&config_dir, &state)
}

/// Save general settings to `settings.yaml` and update the in-memory state.
/// Hotkey changes are handled separately via `save_hotkey` + `register_shortcut`.
#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    show_context_chip: bool,
    allow_duplicates: bool,
    allow_external_paths: bool,
    commands_dir: Option<String>,
) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let binding = app.state::<SettingsState>();
    let mut state = binding.0.lock().unwrap();
    state.show_context_chip = show_context_chip;
    state.allow_duplicates = allow_duplicates;
    state.allow_external_paths = allow_external_paths;
    state.commands_dir = commands_dir;
    settings::save(&config_dir, &state)
}

/// Open (or focus) the dedicated Settings window.
/// The window loads the same SPA as the main launcher; it recognises it is
/// the settings window via its Tauri window label ("settings") and renders
/// the settings UI instead of the launcher bar.
#[tauri::command]
fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    // Re-use an existing settings window if one is already open.
    if let Some(win) = app.get_webview_window("settings") {
        win.show().ok();
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App(std::path::PathBuf::from("index.html")),
    )
    .title("Nimble Settings")
    .inner_size(520.0, 460.0)
    .resizable(false)
    .always_on_top(false)
    .decorations(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Save the active context to `state.json` in the config directory.
#[tauri::command]
fn save_context(app: tauri::AppHandle, context: String) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    write_context_to_state(&config_dir, &context)
}

/// Load the active context from `state.json` in the config directory.
/// Returns an empty string if no context has been persisted.
#[tauri::command]
fn load_context(app: tauri::AppHandle) -> String {
    let config_dir = app.path().app_config_dir().unwrap_or_default();
    read_context_from_state(&config_dir)
}

/// Return the full list of commands loaded from the user config directory,
/// along with any duplicate warnings detected during loading.
#[tauri::command]
fn list_commands(app: tauri::AppHandle) -> Result<commands::LoadResult, String> {
    let commands_root = &app.state::<CommandsRoot>().0;
    let state = app.state::<SettingsState>();
    let settings = state.0.lock().unwrap();
    commands::load_from_dir(commands_root, settings.allow_duplicates, settings.seed_examples)
}

/// Load a named list from `<commands_dir>/<command_dir>/<list_name>.yaml`.
/// The list file is co-located with the command YAML that references it,
/// unless `${VAR}` substitution resolves it to an external path.
#[tauri::command]
fn load_list(
    app: tauri::AppHandle,
    command_dir: String,
    list_name: String,
    inline_env: std::collections::HashMap<String, String>,
    context: String,
    phrase: String,
) -> Result<Vec<commands::ListItem>, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    let dir = app.state::<CommandsRoot>().0.join(&command_dir);
    let allow_external = app.state::<SettingsState>().0.lock().unwrap().allow_external_paths;
    let user_env = commands::build_user_env(&config_dir, &dir, &inline_env)?;
    let env = commands::ScriptEnv {
        context: &context,
        phrase: &phrase,
        config_dir: &config_dir,
        command_dir: &dir,
        user_env: &user_env,
        allow_external_paths: allow_external,
    };
    commands::load_list(&dir, &list_name, &env)
}

/// Run a script co-located with the command YAML and return the items it produces.
/// The optional `arg` is passed as a positional argument to the script.
#[tauri::command]
fn run_dynamic_list(
    app: tauri::AppHandle,
    command_dir: String,
    script_name: String,
    arg: Option<String>,
    context: String,
    phrase: String,
    inline_env: std::collections::HashMap<String, String>,
) -> Result<Vec<commands::ListItem>, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    let dir = app.state::<CommandsRoot>().0.join(&command_dir);
    let allow_external = app.state::<SettingsState>().0.lock().unwrap().allow_external_paths;
    let user_env = commands::build_user_env(&config_dir, &dir, &inline_env)?;
    let env = commands::ScriptEnv {
        context: &context,
        phrase: &phrase,
        config_dir: &config_dir,
        command_dir: &dir,
        user_env: &user_env,
        allow_external_paths: allow_external,
    };
    commands::run_script(&dir, &script_name, arg.as_deref(), &env)
}

/// Run a script co-located with the command YAML and return its output as a list of
/// string values. Used by `script_action` commands — the launcher applies the returned values
/// directly via its built-in open_url / paste_text / copy_text actions.
#[tauri::command]
fn run_script_action(
    app: tauri::AppHandle,
    command_dir: String,
    script_name: String,
    arg: Option<String>,
    context: String,
    phrase: String,
    inline_env: std::collections::HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    let dir = app.state::<CommandsRoot>().0.join(&command_dir);
    let allow_external = app.state::<SettingsState>().0.lock().unwrap().allow_external_paths;
    let user_env = commands::build_user_env(&config_dir, &dir, &inline_env)?;
    let env = commands::ScriptEnv {
        context: &context,
        phrase: &phrase,
        config_dir: &config_dir,
        command_dir: &dir,
        user_env: &user_env,
        allow_external_paths: allow_external,
    };
    commands::run_script_values(&dir, &script_name, arg.as_deref(), &env)
}

/// Dismiss the launcher intentionally (Escape key, hotkey while visible, tray Hide).
/// Hides the window, updates the tray, and restores focus to the previously
/// active application. Distinct from `hide_window` which is used for blur
/// dismissal where the OS already transferred focus to the new frontmost app.
#[tauri::command]
fn dismiss_launcher(app: tauri::AppHandle, window: tauri::Window) {
    window.hide().ok();
    sync_tray(&app, false);
    let prev_pid = app.state::<PreviousApp>().0.lock().unwrap().take();
    if let Some(pid) = prev_pid {
        restore_previous_app(pid);
    }
}

/// Register (or replace) the global hotkey — shared logic used by both the
/// Tauri command (onboarding) and the startup path (settings.yaml).
fn do_register_shortcut(app: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;

    let shortcut_str = shortcut.to_string();
    app.global_shortcut()
        .on_shortcut(shortcut_str.as_str(), move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        window.hide().ok();
                        sync_tray(app, false);
                        // Restore focus to the app that was active before we appear
                        let prev_pid = app.state::<PreviousApp>().0.lock().unwrap().take();
                        if let Some(pid) = prev_pid {
                            restore_previous_app(pid);
                        }
                    } else {
                        // Capture the frontmost app before we steal focus
                        let prev = app.state::<PreviousApp>();
                        capture_previous_app(&prev);
                        window.set_size(tauri::LogicalSize::new(640_f64, 64_f64)).ok();
                        window.center().ok();
                        window.show().ok();
                        window.set_focus().ok();
                        sync_tray(app, true);
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Register (or replace) the global hotkey that summons the launcher.
#[tauri::command]
fn register_shortcut(app: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    do_register_shortcut(&app, &shortcut)
}

/// Paste pre-defined plain text into the application that had focus before
/// the launcher was invoked.
///
/// Flow:
///   1. Validate the text (plain text only; reject NUL bytes).
///   2. Hide the launcher window and update the tray label.
///   3. Restore focus to the previously active application.
///   4. Write the text to the clipboard.
///   5. Simulate Cmd+V (macOS) / Ctrl+V (other) to trigger a paste.
///
/// Requires macOS Accessibility permission for the key-simulation step.
#[tauri::command]
fn paste_text(app: tauri::AppHandle, window: tauri::Window, text: String) -> Result<(), String> {
    validate_text(&text)?;

    // 1. Hide launcher
    window.hide().ok();
    sync_tray(&app, false);

    // 2. Restore focus to the previous app
    let prev_pid = app.state::<PreviousApp>().0.lock().unwrap().take();
    if let Some(pid) = prev_pid {
        restore_previous_app(pid);
    }

    // Pause so focus transfer completes before we touch the clipboard.
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 3. Write text to clipboard
    write_clipboard_text(&text)?;

    // Small gap so the clipboard write is visible to the target app.
    std::thread::sleep(std::time::Duration::from_millis(30));

    // 4. Simulate paste keystroke
    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        const VK_V: CGKeyCode = 0x09;
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "Failed to create CGEventSource")?;
        let key_down = CGEvent::new_keyboard_event(source.clone(), VK_V, true)
            .map_err(|_| "Failed to create key-down event")?;
        key_down.set_flags(CGEventFlags::CGEventFlagCommand);
        key_down.post(core_graphics::event::CGEventTapLocation::HID);
        let key_up = CGEvent::new_keyboard_event(source, VK_V, false)
            .map_err(|_| "Failed to create key-up event")?;
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);
        key_up.post(core_graphics::event::CGEventTapLocation::HID);
    }
    #[cfg(not(target_os = "macos"))]
    {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
        enigo.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Copy text to the clipboard without pasting it.
/// The launcher is hidden and the text is written to the clipboard;
/// no focus restoration or keystroke simulation is performed.
#[tauri::command]
fn copy_text(window: tauri::Window, app: tauri::AppHandle, text: String) -> Result<(), String> {
    validate_text(&text)?
;

    window.hide().ok();
    sync_tray(&app, false);

    write_clipboard_text(&text)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            // macOS: hide from Dock and Cmd+Tab app switcher
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // macOS: prompt for Accessibility permission if not already granted.
            // CGEvent keystroke simulation (used by paste_text) silently fails
            // without this.  The check is per code-signature, so a new version
            // installed via brew may need the user to re-grant permission.
            #[cfg(target_os = "macos")]
            {
                use core_foundation::base::TCFType;
                use core_foundation::boolean::CFBoolean;
                use core_foundation::dictionary::CFDictionary;
                use core_foundation::string::CFString;
                extern "C" {
                    static kAXTrustedCheckOptionPrompt: core_foundation::string::CFStringRef;
                    fn AXIsProcessTrustedWithOptions(
                        options: core_foundation::base::CFTypeRef,
                    ) -> bool;
                }
                unsafe {
                    let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt as _);
                    let value = CFBoolean::true_value();
                    let options = CFDictionary::from_CFType_pairs(&[(
                        key.as_CFType(),
                        value.as_CFType(),
                    )]);
                    AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as _);
                }
            }

            // Build system tray menu
            let version = app.package_info().version.to_string();
            let app_info = MenuItem::new(
                app,
                format!("Nimble v{}", version),
                false,
                None::<&str>,
            )?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let show_hide = MenuItem::with_id(app, "show_hide", "Show", true, None::<&str>)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Nimble", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&app_info, &sep1, &show_hide, &sep2, &quit])?;

            // Manage state for updating the Show/Hide item label
            app.manage(TrayMenuState {
                show_hide_item: Arc::new(show_hide),
            });

            // Load settings from config dir and manage in app state.
            // The hotkey (if set) is registered here so it is active immediately
            // on startup without waiting for the frontend to load.
            let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
            migrate_config_dir(&config_dir);

            // Ship bundled skill files to <config_dir>/skills/nimble-authoring/
            install_bundled_skills(&config_dir);

            let loaded_settings = settings::load(&config_dir);
            if let Some(ref hotkey) = loaded_settings.hotkey {
                if let Err(e) = do_register_shortcut(app.handle(), hotkey) {
                    eprintln!("[nimble] could not register hotkey from settings: {e}");
                }
            }
            let allow_duplicates = loaded_settings.allow_duplicates;
            let commands_root = loaded_settings.commands_root(&config_dir);
            app.manage(SettingsState(Mutex::new(loaded_settings)));
            app.manage(CommandsRoot(commands_root.clone()));

            // Manage previous-app tracking for paste_text focus restoration
            app.manage(PreviousApp(Mutex::new(None)));

            // Start watching the commands subdirectory for live command reloads
            watcher::start(app.handle().clone(), commands_root, allow_duplicates);

            // Listen for incoming deep-link URLs (nimble://...) and route them.
            // Currently supports: nimble://ctx/set/<value> and nimble://ctx/reset.
            // When a recognised link arrives we update state.json and emit a
            // frontend event so the UI reflects the change immediately.
            let dl_handle = app.handle().clone();
            app.listen("deep-link://new-url", move |event| {
                let raw = event.payload();
                if let Ok(urls) = serde_json::from_str::<Vec<String>>(raw) {
                    for url in urls {
                        if let Some(action) = parse_deep_link(&url) {
                            let cfg = dl_handle.path().app_config_dir().ok();
                            match action {
                                DeepLinkAction::CtxSet(ref value) => {
                                    if let Some(ref dir) = cfg {
                                        let _ = write_context_to_state(dir, value);
                                    }
                                    dl_handle
                                        .emit("context://changed", value.clone())
                                        .ok();
                                }
                                DeepLinkAction::CtxReset => {
                                    if let Some(ref dir) = cfg {
                                        let _ = write_context_to_state(dir, "");
                                    }
                                    dl_handle
                                        .emit("context://changed", "".to_string())
                                        .ok();
                                }
                            }
                        }
                    }
                }
            });

            // Load monochrome tray icon for macOS template rendering.
            // The @2x variant is embedded at compile time for crisp Retina display.
            // Falls back to the default app icon if decoding fails.
            let tray_icon = {
                let png_bytes = include_bytes!("../icons/tray-icon@2x.png");
                tauri::image::Image::from_bytes(png_bytes)
                    .unwrap_or_else(|_| {
                        app.default_window_icon()
                            .cloned()
                            .expect("no default window icon configured")
                    })
            };

            TrayIconBuilder::new()
                .icon(tray_icon)
                .icon_as_template(true)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show_hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                window.hide().ok();
                                sync_tray(app, false);
                                // Restore focus to the app that was active before we appeared
                                let prev_pid = app.state::<PreviousApp>().0.lock().unwrap().take();
                                if let Some(pid) = prev_pid {
                                    restore_previous_app(pid);
                                }
                            } else {
                                // Capture previous app before we steal focus
                                let prev = app.state::<PreviousApp>();
                                capture_previous_app(&prev);
                                window.set_size(tauri::LogicalSize::new(640_f64, 64_f64)).ok();
                                window.center().ok();
                                window.show().ok();
                                window.set_focus().ok();
                                sync_tray(app, true);
                            }
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![hide_window, show_window, dismiss_launcher, register_shortcut, get_settings, save_hotkey, save_settings, open_settings_window, save_context, load_context, list_commands, load_list, run_dynamic_list, run_script_action, open_url, paste_text, copy_text, deploy_skill])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_url ────────────────────────────────────────────────────────────

    #[test]
    fn bare_string_rejected() {
        assert!(resolve_url("google.com".into(), None).is_err());
    }

    #[test]
    fn accepts_https() {
        assert!(resolve_url("https://example.com".into(), None).is_ok());
    }

    #[test]
    fn accepts_http() {
        assert!(resolve_url("http://example.com".into(), None).is_ok());
    }

    #[test]
    fn accepts_deep_link() {
        assert!(resolve_url("slack://open".into(), None).is_ok());
    }

    #[test]
    fn accepts_mailto() {
        assert!(resolve_url("mailto:a@b.com".into(), None).is_ok());
    }

    #[test]
    fn param_substitution_encodes_spaces() {
        let r = resolve_url(
            "https://g.com/search?q={param}".into(),
            Some("hello world".into()),
        )
        .unwrap();
        assert_eq!(r, "https://g.com/search?q=hello+world");
    }

    #[test]
    fn param_substitution_encodes_special_chars() {
        let r = resolve_url(
            "https://g.com/search?q={param}".into(),
            Some("a&b".into()),
        )
        .unwrap();
        assert!(r.contains("%26"), "expected %26 in {r}");
    }

    #[test]
    fn url_without_placeholder_ignores_param() {
        let r = resolve_url(
            "https://example.com".into(),
            Some("ignored".into()),
        )
        .unwrap();
        assert_eq!(r, "https://example.com");
    }

    // ── validate_text ──────────────────────────────────────────────────────────

    // clipboard_roundtrip: requires a live display server; skipped in headless CI.
    // Run manually: cargo test -- --ignored clipboard_roundtrip
    #[test]
    #[ignore = "requires a display server / desktop session"]
    fn clipboard_roundtrip() {
        write_clipboard_text("nimble clipboard test")
            .expect("clipboard write should succeed");
    }

    #[test]
    fn nul_byte_rejected() {
        assert!(validate_text("hello\0world").is_err());
    }

    #[test]
    fn plain_text_accepted() {
        assert!(validate_text("Hello, world!").is_ok());
    }

    #[test]
    fn empty_string_accepted() {
        assert!(validate_text("").is_ok());
    }

    // ── read_context_from_state / write_context_to_state ───────────────────────

    #[test]
    fn state_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        write_context_to_state(dir.path(), "reddit").unwrap();
        assert_eq!(read_context_from_state(dir.path()), "reddit");
    }

    #[test]
    fn state_missing_file_returns_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        assert_eq!(read_context_from_state(dir.path()), "");
    }

    #[test]
    fn state_empty_context_persists() {
        let dir = tempfile::TempDir::new().unwrap();
        write_context_to_state(dir.path(), "foo").unwrap();
        write_context_to_state(dir.path(), "").unwrap();
        assert_eq!(read_context_from_state(dir.path()), "");
    }

    #[test]
    fn state_malformed_json_returns_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("state.json"), "not json").unwrap();
        assert_eq!(read_context_from_state(dir.path()), "");
    }

    #[test]
    fn state_missing_context_key_returns_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("state.json"), r#"{"other":"val"}"#).unwrap();
        assert_eq!(read_context_from_state(dir.path()), "");
    }

    // ── parse_deep_link ────────────────────────────────────────────────────────

    #[test]
    fn deep_link_ctx_set() {
        assert_eq!(
            parse_deep_link("nimble://ctx/set/reddit"),
            Some(DeepLinkAction::CtxSet("reddit".into()))
        );
    }

    #[test]
    fn deep_link_ctx_set_with_spaces() {
        assert_eq!(
            parse_deep_link("nimble://ctx/set/my%20project"),
            Some(DeepLinkAction::CtxSet("my project".into()))
        );
    }

    #[test]
    fn deep_link_ctx_set_plus_as_space() {
        assert_eq!(
            parse_deep_link("nimble://ctx/set/hello+world"),
            Some(DeepLinkAction::CtxSet("hello world".into()))
        );
    }

    #[test]
    fn deep_link_ctx_set_empty_value_returns_none() {
        assert_eq!(parse_deep_link("nimble://ctx/set/"), None);
    }

    #[test]
    fn deep_link_ctx_reset() {
        assert_eq!(
            parse_deep_link("nimble://ctx/reset"),
            Some(DeepLinkAction::CtxReset)
        );
    }

    #[test]
    fn deep_link_ctx_reset_trailing_slash() {
        assert_eq!(
            parse_deep_link("nimble://ctx/reset/"),
            Some(DeepLinkAction::CtxReset)
        );
    }

    #[test]
    fn deep_link_triple_slash() {
        assert_eq!(
            parse_deep_link("nimble:///ctx/set/work"),
            Some(DeepLinkAction::CtxSet("work".into()))
        );
    }

    #[test]
    fn deep_link_unknown_route_returns_none() {
        assert_eq!(parse_deep_link("nimble://unknown/path"), None);
    }

    #[test]
    fn deep_link_wrong_scheme_returns_none() {
        assert_eq!(parse_deep_link("https://ctx/set/reddit"), None);
    }

    // ── percent_decode ─────────────────────────────────────────────────────────

    #[test]
    fn percent_decode_basic() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
    }

    #[test]
    fn percent_decode_plus_to_space() {
        assert_eq!(percent_decode("a+b"), "a b");
    }

    #[test]
    fn percent_decode_passthrough() {
        assert_eq!(percent_decode("plain"), "plain");
    }

    // ── install_bundled_skills ──────────────────────────────────────────────

    #[test]
    fn install_bundled_skills_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        install_bundled_skills(tmp.path());
        let skill_md = tmp.path().join("skills/nimble-authoring/SKILL.md");
        let spec = tmp.path().join("skills/nimble-authoring/nimble-spec.yaml");
        assert!(skill_md.exists(), "SKILL.md should exist");
        assert!(spec.exists(), "nimble-spec.yaml should exist");
        // Content should be non-empty and match the embedded strings
        let md_content = std::fs::read_to_string(&skill_md).unwrap();
        assert!(md_content.contains("Nimble Authoring"), "SKILL.md should contain heading");
        let spec_content = std::fs::read_to_string(&spec).unwrap();
        assert!(spec_content.contains("spec_version"), "spec should contain spec_version");
    }

    #[test]
    fn install_bundled_skills_overwrites_stale_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("skills/nimble-authoring");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("SKILL.md"), "old content").unwrap();
        install_bundled_skills(tmp.path());
        let content = std::fs::read_to_string(dest.join("SKILL.md")).unwrap();
        assert!(content.contains("Nimble Authoring"), "should overwrite stale file");
    }

    #[test]
    fn install_bundled_skills_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        install_bundled_skills(tmp.path());
        install_bundled_skills(tmp.path()); // second call should not fail
        let skill_md = tmp.path().join("skills/nimble-authoring/SKILL.md");
        assert!(skill_md.exists());
    }

    // ── SKILL_MD / SKILL_SPEC constants ────────────────────────────────────

    #[test]
    fn embedded_skill_md_is_not_empty() {
        assert!(!SKILL_MD.is_empty());
        assert!(SKILL_MD.contains("nimble-spec.yaml"));
    }

    #[test]
    fn embedded_spec_is_not_empty() {
        assert!(!SKILL_SPEC.is_empty());
        assert!(SKILL_SPEC.contains("spec_version"));
    }
}
