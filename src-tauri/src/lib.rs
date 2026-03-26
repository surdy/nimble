mod commands;
mod settings;
mod watcher;

use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
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

/// Return the full list of commands loaded from the user config directory,
/// along with any duplicate warnings detected during loading.
#[tauri::command]
fn list_commands(app: tauri::AppHandle) -> Result<commands::LoadResult, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    let allow_duplicates = app.state::<SettingsState>().0.lock().unwrap().allow_duplicates;
    commands::load_from_dir(&config_dir.join("commands"), allow_duplicates)
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
    let dir = config_dir.join("commands").join(&command_dir);
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
    let dir = config_dir.join("commands").join(&command_dir);
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
    let dir = config_dir.join("commands").join(&command_dir);
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
            let loaded_settings = settings::load(&config_dir);
            if let Some(ref hotkey) = loaded_settings.hotkey {
                if let Err(e) = do_register_shortcut(app.handle(), hotkey) {
                    eprintln!("[nimble] could not register hotkey from settings: {e}");
                }
            }
            let allow_duplicates = loaded_settings.allow_duplicates;
            app.manage(SettingsState(Mutex::new(loaded_settings)));

            // Manage previous-app tracking for paste_text focus restoration
            app.manage(PreviousApp(Mutex::new(None)));

            // Start watching the commands subdirectory for live command reloads
            watcher::start(app.handle().clone(), config_dir.join("commands"), allow_duplicates);

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
        .invoke_handler(tauri::generate_handler![hide_window, show_window, dismiss_launcher, register_shortcut, get_settings, save_hotkey, list_commands, load_list, run_dynamic_list, run_script_action, open_url, paste_text, copy_text])
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
}
