use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_true() -> bool {
    true
}

/// Application-level settings persisted to `config_dir/settings.yaml`.
/// Every field has a sensible default so a missing or partial file is safe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Global hotkey accelerator string (e.g. `"Super+Space"`).
    /// When absent the onboarding screen is shown at first launch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,

    /// Whether to show the context chip in the launcher bar when a context is
    /// active. Defaults to `true`.
    #[serde(default = "default_true")]
    pub show_context_chip: bool,

    /// Whether to allow duplicate command phrases.
    /// `true` (default) — all matching files are loaded; no deduplication or
    /// warnings.
    /// `false` — first-encountered command wins; a `DuplicateWarning` is
    /// surfaced in the UI for every collision.
    #[serde(default = "default_true")]
    pub allow_duplicates: bool,

    /// Whether `script:` and `list:` fields can resolve to paths outside the
    /// command directory via `${VAR}` substitution.
    /// `true` (default) — external paths are allowed.
    /// `false` — resolved paths must stay inside the command directory.
    #[serde(default = "default_true")]
    pub allow_external_paths: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: None,
            show_context_chip: true,
            allow_duplicates: true,
            allow_external_paths: true,
        }
    }
}

/// Load settings from `config_dir/settings.yaml`.
/// Returns `AppSettings::default()` if the file is absent or unparseable.
pub fn load(config_dir: &Path) -> AppSettings {
    let path = config_dir.join("settings.yaml");
    if !path.exists() {
        return AppSettings::default();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[nimble] could not read settings.yaml: {e}");
            return AppSettings::default();
        }
    };
    match serde_yaml::from_str::<AppSettings>(&content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[nimble] could not parse settings.yaml: {e}");
            AppSettings::default()
        }
    }
}

/// Save `settings` to `config_dir/settings.yaml`.
pub fn save(config_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let path = config_dir.join("settings.yaml");
    let yaml = serde_yaml::to_string(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, yaml).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_when_file_absent() {
        let dir = TempDir::new().unwrap();
        let s = load(dir.path());
        assert_eq!(s.hotkey, None);
        assert!(s.show_context_chip);
        assert!(s.allow_duplicates);
        assert!(s.allow_external_paths);
    }

    #[test]
    fn parses_hotkey() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("settings.yaml"), "hotkey: Super+Space\n").unwrap();
        let s = load(dir.path());
        assert_eq!(s.hotkey, Some("Super+Space".to_string()));
    }

    #[test]
    fn omitted_fields_use_defaults() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("settings.yaml"), "hotkey: Super+Space\n").unwrap();
        let s = load(dir.path());
        assert!(s.show_context_chip, "show_context_chip should default to true");
        assert!(s.allow_duplicates, "allow_duplicates should default to true");
    }

    #[test]
    fn can_disable_context_chip() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("settings.yaml"), "show_context_chip: false\n").unwrap();
        let s = load(dir.path());
        assert!(!s.show_context_chip);
    }

    #[test]
    fn can_disable_allow_duplicates() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("settings.yaml"), "allow_duplicates: false\n").unwrap();
        let s = load(dir.path());
        assert!(!s.allow_duplicates);
    }

    #[test]
    fn save_and_reload_round_trip() {
        let dir = TempDir::new().unwrap();
        let original = AppSettings {
            hotkey: Some("Super+Space".to_string()),
            show_context_chip: false,
            allow_duplicates: false,
            allow_external_paths: false,
        };
        save(dir.path(), &original).unwrap();
        let reloaded = load(dir.path());
        assert_eq!(reloaded.hotkey, original.hotkey);
        assert_eq!(reloaded.show_context_chip, original.show_context_chip);
        assert_eq!(reloaded.allow_duplicates, original.allow_duplicates);
        assert_eq!(reloaded.allow_external_paths, original.allow_external_paths);
    }

    #[test]
    fn save_none_hotkey_omits_field() {
        let dir = TempDir::new().unwrap();
        let s = AppSettings::default();
        save(dir.path(), &s).unwrap();
        let yaml = std::fs::read_to_string(dir.path().join("settings.yaml")).unwrap();
        assert!(!yaml.contains("hotkey"), "None hotkey should be omitted from YAML");
    }

    #[test]
    fn defaults_on_malformed_yaml() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("settings.yaml"), "{ not: [ valid yaml }").unwrap();
        let s = load(dir.path());
        assert_eq!(s.hotkey, None);
        assert!(s.show_context_chip);
        assert!(s.allow_duplicates);
        assert!(s.allow_external_paths);
    }

    #[test]
    fn can_disable_allow_external_paths() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("settings.yaml"),
            "allow_external_paths: false\n",
        )
        .unwrap();
        let s = load(dir.path());
        assert!(!s.allow_external_paths);
    }
}
