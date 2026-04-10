use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;

const LOG_FILE: &str = "debug.log";

/// Format a timestamp as `[HH:MM:SS.mmm]`.
fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let millis = now.subsec_millis();
    let hours = (total_secs / 3600) % 24;
    let mins = (total_secs / 60) % 60;
    let secs = total_secs % 60;
    format!("[{hours:02}:{mins:02}:{secs:02}.{millis:03}]")
}

/// Append a timestamped line to `<config_dir>/debug.log`.
pub fn log(config_dir: &Path, message: &str) {
    let path = config_dir.join(LOG_FILE);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {message}", timestamp());
    }
}

/// Clear the debug log file (called when debug mode is toggled on).
pub fn clear(config_dir: &Path) {
    let path = config_dir.join(LOG_FILE);
    let _ = fs::write(&path, "");
}

/// Read the entire debug log and return its contents. Returns an empty string
/// if the file does not exist.
pub fn read(config_dir: &Path) -> String {
    let path = config_dir.join(LOG_FILE);
    fs::read_to_string(&path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn log_creates_file_and_appends() {
        let dir = TempDir::new().unwrap();
        log(dir.path(), "hello");
        log(dir.path(), "world");
        let contents = read(dir.path());
        assert!(contents.contains("hello"));
        assert!(contents.contains("world"));
        let lines: Vec<&str> = contents.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        // Each line starts with a timestamp
        assert!(lines[0].starts_with('['));
    }

    #[test]
    fn clear_truncates_file() {
        let dir = TempDir::new().unwrap();
        log(dir.path(), "before");
        assert!(!read(dir.path()).is_empty());
        clear(dir.path());
        assert!(read(dir.path()).is_empty());
    }

    #[test]
    fn read_returns_empty_when_missing() {
        let dir = TempDir::new().unwrap();
        assert!(read(dir.path()).is_empty());
    }
}
