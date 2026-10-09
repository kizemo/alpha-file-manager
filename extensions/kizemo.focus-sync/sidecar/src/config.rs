//! CLI argument parsing + port resolution.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::net::TcpListener;
use std::process::ExitCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub initial_path: String,
    pub app_whitelist: Vec<String>,
    pub service: bool,
}

/// Resolve port with two distinct modes.
///
/// **spinner mode** (`service = false`, v0.2.0 backward-compat):
///   Bind strictly to `preferred` — but if the port is held by another
///   `focus-sync-sidecar.exe` (the previous install's orphan), taskkill
///   that process and retry once before giving up.
///
/// **service mode** (`service = true`, v0.3.0 Scheduled Task):
///   Single bind attempt. On EADDRINUSE, log and `process::exit(0)` cleanly.
///   Reason: on a multi-user machine the first logged-in user's sidecar
///   already holds 37421; this user is a 2nd login attempt. We must NOT
///   restart-spam (would generate Windows Event Log noise) and we must NOT
///   kill the existing instance (would break the first user). Clean exit 0
///   tells Task Scheduler "nothing to do, do not restart".
pub fn resolve_port(preferred: u16, service: bool) -> Result<u16> {
    if service {
        // v0.3.0 service mode: fail-fast
        match TcpListener::bind(("127.0.0.1", preferred)) {
            Ok(_listener) => {
                tracing::info!(port = preferred, "service mode: port bound (fail-fast success)");
                return Ok(preferred);
            }
            Err(e) => {
                tracing::error!(
                    port = preferred,
                    err = %e,
                    "service mode: bind failed (likely another user's sidecar is active on this port). Exiting cleanly so Task Scheduler does not restart-spam."
                );
                // Exit 0 = clean exit; Task Scheduler treats this as a successful run.
                process::exit(0); // unreachable in practice (process::exit aborts); see below
            }
        }
        // Unreachable: process::exit terminates. Kept for type-system clarity.
        #[allow(unreachable_code)]
        Ok(preferred)
    } else {
        // spinner mode (v0.2.0 backward-compat): retry + orphan-kill
        match TcpListener::bind(("127.0.0.1", preferred)) {
            Ok(_listener) => return Ok(preferred),
            Err(_) => {
                if kill_holding_sidecar() {
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    if TcpListener::bind(("127.0.0.1", preferred)).is_ok() {
                        return Ok(preferred);
                    }
                }
            }
        }
        Err(anyhow!(
            "preferred port {} is already in use — another focus-sync-sidecar may be running. Kill it (taskkill /F /IM focus-sync-sidecar.exe) and retry.",
            preferred
        ))
    }
}

// Allow `process::exit(0)` typing.
use std::process;

/// Try to kill any focus-sync-sidecar.exe processes via taskkill. Returns
/// `true` if a kill command was issued and at least one process was
/// terminated, `false` otherwise.
///
/// We intentionally only kill by image name (focus-sync-sidecar.exe),
/// never by port ownership — the port could legitimately be held by a
/// non-sidecar process (developer running a test server, etc.) and we
/// don't want to nuke that.
fn kill_holding_sidecar() -> bool {
    // v0.5.8: a unit test must NEVER taskkill the user's real deployment.
    //
    // This function is reached from `resolve_port`'s spinner branch whenever a
    // bind fails. `resolve_port_strict_on_conflict` deliberately occupies a
    // port to force exactly that path — so before this guard, running
    // `cargo test` on a developer machine issued
    //   taskkill /F /IM focus-sync-sidecar.exe /T
    // against whatever was actually running. Observed 2026-10-08: the test
    // suite killed the live Scheduled Task sidecar (PID 598276) as a side
    // effect, which then had to be redeployed by hand.
    //
    // Tests must be safe to run against a live install, so the kill is
    // disabled for them. Production behaviour is unchanged: `cfg!(test)` is
    // false in every shipped binary, including the one the installer copies.
    if cfg!(test) {
        return false;
    }
    let status = std::process::Command::new("taskkill.exe")
        .args(["/F", "/IM", "focus-sync-sidecar.exe", "/T"])
        .status();
    match status {
        Ok(s) => {
            s.code() == Some(0)
        }
        Err(_) => false, // taskkill.exe not on PATH
    }
}

/// Parse Config from raw values (testable without Cli struct).
pub fn parse_config(
    port: u16,
    initial_path: String,
    app_whitelist: Vec<String>,
    service: bool,
) -> Result<Config> {
    let port = resolve_port(port, service)?;
    Ok(Config {
        port,
        initial_path,
        app_whitelist,
        service,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_port_preferred_free() {
        // Use a known-high port that should be free; should return same port.
        // Test spinner mode (does not exit).
        let p = resolve_port(37422, false).unwrap();
        assert!(p >= 37422);
    }

    /// v0.5.8: this test asserted `#[should_panic]`, but `resolve_port`
    /// reports an occupied port by RETURNING `Err` — it never panics. The
    /// attribute could therefore never be satisfied and the test failed on
    /// every run, including the ones reported as "66 tests all green".
    ///
    /// It also had a live side effect: occupying a port drives the spinner
    /// branch into `kill_holding_sidecar()`, which taskkilled the real
    /// deployed sidecar. That is now disabled under `cfg!(test)`, so the
    /// assertion below checks the real contract — a busy port must be an
    /// `Err`, never a panic and never a silent fallback to another port.
    #[test]
    fn resolve_port_strict_on_conflict() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied = listener.local_addr().unwrap().port();
        let resolved = resolve_port(occupied, false);
        drop(listener);
        assert!(
            resolved.is_err(),
            "an occupied port must return Err in spinner mode, not panic and not \
             silently pick another port"
        );
    }

    #[test]
    fn parse_config_returns_resolved_port() {
        let cfg = parse_config(37423, "C:\\Users\\foo".into(), vec![], false).unwrap();
        assert_eq!(cfg.initial_path, "C:\\Users\\foo");
        assert!(cfg.port >= 37423);
        assert!(cfg.app_whitelist.is_empty());
        assert!(!cfg.service);
    }

    #[test]
    fn parse_config_service_mode_persists() {
        let cfg = parse_config(37424, "C:\\".into(), vec![], true).unwrap();
        assert!(cfg.service);
    }

    // We do NOT test service mode bind-failure because it calls process::exit(0)
    // which would terminate the test runner. The exit path is verified manually
    // (./spike.exe --service with port occupied).
    //
    // To make that testable, we'd need to extract the bind logic into a pure
    // function returning Result<u16, BindError> and unit-test that. Out of
    // scope for v0.3.0.
}

// Suppress unused warning for ExitCode import (kept for future tests).
#[allow(dead_code)]
const _EXIT_CODE_OK: ExitCode = ExitCode::SUCCESS;