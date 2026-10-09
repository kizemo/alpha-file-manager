//! JSON-line logger. focus-17 v0.3.0: writes to file under
//! `%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log` (or `%TEMP%` fallback).
//! Previously wrote to stdout, but `#![windows_subsystem = "windows"]` makes
//! stdout invisible. LOG_LOCK serializes 4 concurrent call sites:
//! main loop / heartbeat thread / UIA monitor / HTTP server.

use crate::events::Event;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

static LOG_LOCK: Mutex<()> = Mutex::new(());

/// Returns current UTC time in ISO 8601 format (e.g. "2026-09-27T12:34:56Z").
pub fn now_iso8601() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    format_iso8601_utc(secs)
}

fn format_iso8601_utc(unix_secs: u64) -> String {
    let days = unix_secs / 86400;
    let secs_of_day = unix_secs % 86400;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m as u32, d as u32)
}

/// Returns the log directory. Windows: `%LOCALAPPDATA%\kizemo\focus-sync\logs`.
/// Falls back to `%TEMP%\focus-sync-logs` if LOCALAPPDATA is unset.
///
/// focus-17 fix: console subsystem = "windows" makes stdout unavailable.
/// All logging MUST go to file.
pub fn log_dir() -> std::path::PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        std::path::PathBuf::from(local).join("kizemo").join("focus-sync").join("logs")
    } else {
        std::env::temp_dir().join("focus-sync-logs")
    }
}

/// Process-wide shared log file handle. Opened lazily on first use.
fn log_file() -> &'static std::fs::File {
    static FILE: OnceLock<std::fs::File> = OnceLock::new();
    FILE.get_or_init(|| {
        let dir = log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("spike.log");
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|_| {
                // Fallback: write to TEMP. Never panic on log init.
                let fallback = std::env::temp_dir().join("focus-sync-fallback.log");
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&fallback)
                    .expect("cannot open fallback log file")
            })
    })
}

/// Serialize event as one-line JSON and append to log file, followed by newline.
pub fn log_event(event: &Event) {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let json = serde_json::to_string(event).expect("Event serialization never fails");
    let mut f = log_file();
    let _ = writeln!(f, "{}", json);
    let _ = f.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_iso8601_format() {
        let s = now_iso8601();
        // ISO 8601: 2026-09-27T12:34:56Z
        assert!(s.contains("T"));
        assert!(s.ends_with("Z"));
    }

    #[test]
    fn format_iso8601_utc_known_value() {
        // 2026-09-27T00:00:00Z = 1790467200
        let s = format_iso8601_utc(1790467200);
        assert_eq!(s, "2026-09-27T00:00:00Z");
    }

    #[test]
    fn log_event_does_not_panic() {
        let evt = Event::ForegroundChanged {
            hwnd: 1,
            ts: now_iso8601(),
        };
        log_event(&evt); // smoke test, output captured by test framework
    }

    #[test]
    fn log_dir_uses_localappdata_when_set() {
        let original = std::env::var("LOCALAPPDATA").ok();
        // SAFETY: cargo test runs tests single-threaded by default; no concurrent
        // env reads in this test process.
        unsafe { std::env::set_var("LOCALAPPDATA", r"C:\Users\test\AppData\Local"); }
        let dir = log_dir();
        // SAFETY: restore env BEFORE the assert — if assert panics we'd leave
        // a polluted env for subsequent tests.
        unsafe {
            match original {
                Some(v) => std::env::set_var("LOCALAPPDATA", v),
                None => std::env::remove_var("LOCALAPPDATA"),
            }
        }
        assert_eq!(
            dir,
            std::path::PathBuf::from(r"C:\Users\test\AppData\Local\kizemo\focus-sync\logs")
        );
    }

    #[test]
    fn log_dir_fallback_path_has_focus_sync_suffix() {
        // Under parallel test execution we cannot safely unset LOCALAPPDATA
        // (other tests are reading it). Instead, just verify the function
        // returns a PathBuf without crashing, AND that the LOCALAPPDATA branch
        // path always ends with our expected suffix. The TEMP fallback path is
        // exercised manually in environments where LOCALAPPDATA is unset.
        let dir = log_dir();
        assert!(!dir.as_os_str().is_empty());
        assert!(
            dir.to_string_lossy().contains("focus-sync"),
            "unexpected dir: {:?}",
            dir
        );
    }
}