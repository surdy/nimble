use std::{path::PathBuf, sync::mpsc, thread, time::Duration};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::commands;

/// The Tauri event name emitted to the frontend when commands are reloaded.
pub const COMMANDS_RELOADED_EVENT: &str = "commands://reloaded";

/// A command sent to the watcher thread to reconfigure it at runtime.
pub enum WatcherCommand {
    /// Switch to a (possibly new) commands directory and/or update the
    /// `allow_duplicates` or `shared_dir` settings. The watcher unwatches
    /// the old directory, watches the new one, reloads commands, and emits
    /// to the frontend.
    Reconfigure {
        commands_dir: PathBuf,
        allow_duplicates: bool,
        shared_dir: String,
    },
}

/// Start a background thread that watches `commands_dir` for file changes.
/// On any relevant event the command list is reloaded and emitted to all
/// windows as `commands://reloaded`.
///
/// Returns a `Sender` that can be used to reconfigure the watcher at runtime
/// (e.g. when the user changes `commands_dir` in settings).
///
/// `commands_dir` is `config_dir/commands/`. Scripts are co-located with
/// their command YAML files inside this tree, so a single recursive watch
/// covers both commands and scripts.
///
/// The watcher runs for the lifetime of the app — Tauri will clean up the
/// thread when the process exits.
pub fn start(
    app: AppHandle,
    commands_dir: PathBuf,
    allow_duplicates: bool,
    shared_dir: String,
) -> mpsc::Sender<WatcherCommand> {
    let (ctrl_tx, ctrl_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut current_dir = commands_dir;
        let mut current_allow_dupes = allow_duplicates;
        let mut current_shared_dir = shared_dir;

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

        if let Err(e) = watcher.watch(&current_dir, RecursiveMode::Recursive) {
            eprintln!("[nimble] could not watch commands dir: {e}");
            return;
        }

        eprintln!(
            "[nimble] watching for changes: {}",
            current_dir.display()
        );

        // Debounce: after a relevant event, wait this long before reloading
        // so that rapid saves / multiple renames don't trigger multiple loads.
        const DEBOUNCE: Duration = Duration::from_millis(300);

        // How often to check the control channel when no file events arrive.
        const POLL_INTERVAL: Duration = Duration::from_secs(1);

        loop {
            // Check for reconfigure commands from the main thread.
            while let Ok(cmd) = ctrl_rx.try_recv() {
                match cmd {
                    WatcherCommand::Reconfigure {
                        commands_dir: new_dir,
                        allow_duplicates: new_dupes,
                        shared_dir: new_shared,
                    } => {
                        if new_dir != current_dir {
                            watcher.unwatch(&current_dir).ok();
                            if let Err(e) =
                                watcher.watch(&new_dir, RecursiveMode::Recursive)
                            {
                                eprintln!(
                                    "[nimble] could not watch new commands dir: {e}"
                                );
                            }
                            eprintln!(
                                "[nimble] watcher switched: {} → {}",
                                current_dir.display(),
                                new_dir.display()
                            );
                            current_dir = new_dir;
                        }
                        current_allow_dupes = new_dupes;
                        current_shared_dir = new_shared;

                        // Reload immediately with the new configuration.
                        emit_reload(&app, &current_dir, current_allow_dupes, &current_shared_dir);
                    }
                }
            }

            // Wait for a file event, with a timeout so we periodically
            // re-check the control channel above.
            let event = match rx.recv_timeout(POLL_INTERVAL) {
                Ok(e) => e,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
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
            emit_reload(&app, &current_dir, current_allow_dupes, &current_shared_dir);
        }
    });

    ctrl_tx
}

/// Reload commands from `dir` and emit the result to the frontend.
fn emit_reload(app: &AppHandle, dir: &PathBuf, allow_duplicates: bool, shared_dir: &str) {
    match commands::load_from_dir(dir, allow_duplicates, false, shared_dir) {
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
