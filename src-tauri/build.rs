// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::process::Command;

fn main() {
    #[cfg(target_os = "windows")]
    build_default_file_manager_launcher();

    emit_icon_rerun_guards();

    tauri_build::build();
}

/// Re-runs this build script when the bundled application icons change.
///
/// Without these guards the build script only re-ran for the launcher sources
/// listed above, so `tauri_build` reused the previously generated Windows
/// resource script (`.rc` / `.res`) and the executable kept the old taskbar
/// icon even after `icons/icon.ico` was regenerated.
fn emit_icon_rerun_guards() {
    for icon in [
        "icons/icon.ico",
        "icons/icon.png",
        "icons/icon.icns",
        "icons/32x32.png",
        "icons/128x128.png",
        "icons/128x128@2x.png",
        "icons/Square30x30Logo.png",
        "icons/Square44x44Logo.png",
        "icons/Square71x71Logo.png",
        "icons/Square89x89Logo.png",
        "icons/Square107x107Logo.png",
        "icons/Square142x142Logo.png",
        "icons/Square150x150Logo.png",
        "icons/Square284x284Logo.png",
        "icons/Square310x310Logo.png",
        "icons/StoreLogo.png",
    ] {
        println!("cargo:rerun-if-changed={icon}");
    }
}

#[cfg(target_os = "windows")]
fn build_default_file_manager_launcher() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let target = std::env::var("TARGET").expect("TARGET");
    let profile = std::env::var("PROFILE").expect("PROFILE");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let launcher_target_dir = launcher_target_directory(&manifest_dir);
    let launcher_source = launcher_source_path(&launcher_target_dir, &target, &profile);
    let launcher_destination = out_dir.join("sigma-file-manager-launcher.exe");

    println!("cargo:rerun-if-changed=default-file-manager-common/src/lib.rs");
    println!("cargo:rerun-if-changed=default-file-manager-common/Cargo.toml");
    println!("cargo:rerun-if-changed=default-file-manager-launcher/src/main.rs");
    println!("cargo:rerun-if-changed=default-file-manager-launcher/Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command
        .current_dir(&manifest_dir)
        .env("CARGO_TARGET_DIR", &launcher_target_dir)
        .args([
            "build",
            "-p",
            "sigma-file-manager-launcher",
            "--target",
            target.as_str(),
        ]);

    match profile.as_str() {
        "debug" => {}
        "release" => {
            command.arg("--release");
        }
        _ => {
            command.args(["--profile", profile.as_str()]);
        }
    }

    let status = command
        .status()
        .expect("failed to build sigma-file-manager-launcher");

    if !status.success() {
        panic!("sigma-file-manager-launcher build failed");
    }

    if !launcher_source.is_file() {
        panic!(
            "Expected launcher binary at {} after build",
            launcher_source.display()
        );
    }

    std::fs::copy(&launcher_source, &launcher_destination).unwrap_or_else(|error| {
        panic!(
            "Failed to copy launcher from {} to {}: {error}",
            launcher_source.display(),
            launcher_destination.display()
        );
    });
}

#[cfg(target_os = "windows")]
fn launcher_target_directory(manifest_dir: &Path) -> PathBuf {
    manifest_dir
        .join("target")
        .join("default-file-manager-launcher")
}

#[cfg(target_os = "windows")]
fn launcher_source_path(launcher_target_dir: &Path, target: &str, profile: &str) -> PathBuf {
    launcher_target_dir
        .join(target)
        .join(profile)
        .join("sigma-file-manager-launcher.exe")
}
