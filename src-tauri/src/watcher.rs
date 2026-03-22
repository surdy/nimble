use std::{path::PathBuf, sync::mpsc, thread, time::Duration};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::commands;

/// The Tauri event name emitted to the frontend when commands are reloaded.
pub const COMMANDS_RELOADED_EVENT: &str = "commands://reloaded";

/// Start a background thread that watches `commands_dir` for file changes.
/// On any relevant event the command list is reloaded and emitted to all
/// windows as `commands://reloaded`.
///
/// `commands_dir` is `config_dir/commands/`. Scripts are co-located with
/// their command YAML files inside this tree, so a single recursive watch
/// covers both commands and scripts.
///
/// The watcher runs for the lifetime of the app — Tauri will clean up the
/// thread when the process exits.
pub fn start(app: AppHandle, commands_dir: PathBuf, allow_duplicates: bool) {
    thread::spawn(move || {
        // Channel for raw notify events
        let (tx, rx) = mpsc::channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        ) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[nimble] could not create file watcher: {e}");
                return;
            }
        };

        if let Err(e) = watcher.watch(&commands_dir, RecursiveMode::Recursive) {
            eprintln!("[nimble] could not watch commands dir: {e}");
            return;
        }

        eprintln!(
            "[nimble] watching for changes: {}",
            commands_dir.display()
        );

        // Debounce: after a relevant event, wait this long before reloading
        // so that rapid saves / multiple renames don't trigger multiple loads.
        const DEBOUNCE: Duration = Duration::from_millis(300);

        loop {
            // Block until first event
            let event = match rx.recv() {
                Ok(e) => e,
                Err(_) => break, // channel closed, app is exiting
            };

            if !is_relevant_event(&event) {
                continue;
            }

            // Drain any additional events that arrive within the debounce window
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(_) => continue, // discard, keep draining
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }

            // Reload commands and emit to frontend
            match commands::load_from_dir(&commands_dir, allow_duplicates) {
                Ok(result) => {
                    if let Err(e) = app.emit(COMMANDS_RELOADED_EVENT, &result) {
                        eprintln!("[nimble] could not emit reload event: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("[nimble] reload failed: {e}");
                }
            }
        }
    });
}

/// Returns true if the event is one we should react to:
/// - Create / Modify: when a .yaml, .yml, or script file is affected.
/// - Remove: any removal inside the watched config dir. On macOS, FSEvents can
///   report a deletion with the *parent directory path* rather than the deleted
///   file's path, so the extension check would incorrectly filter it out.
///   The config dir is low-churn, so reloading on any removal is harmless.
fn is_relevant_event(event: &notify::Event) -> bool {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            event.paths.iter().any(|p| {
                matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("yaml") | Some("yml") | Some("sh") | Some("ps1") | Some("py") | Some("js") | Some("bat")
                )
            })
        }
        EventKind::Remove(_) => !event.paths.is_empty(),
        _ => false,
    }
}
