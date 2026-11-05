// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! Tauri command layer of the image edit actions (rotate / flip).
//!
//! The pixel work lives in [`crate::image_edit`] and is deliberately free of
//! any tauri dependency, so the unit tests of that module never pull the tauri
//! runtime into the test binary. This module is the only place that talks to
//! tauri.
//!
//! ## Why the command is asynchronous
//!
//! A **synchronous** tauri command runs on the **main thread**. Decoding,
//! rotating and re-encoding a 7 megapixel JPEG takes seconds, so a synchronous
//! command freezes the whole application: the webview stops receiving messages,
//! the window looks dead and the built in webview guard
//! (`webview_guard_runtime.rs`) can mistake the renderer for a crashed one and
//! restart the application - which is exactly what a user perceives as
//! "Alpha File Manager crashed".
//!
//! Running the very same work on a blocking worker thread keeps the main
//! thread (and therefore the webview) responsive, no matter how big the image
//! is. `tokio::task::spawn_blocking` is used on purpose: it is the executor
//! the rest of the crate already uses for its blocking commands and, unlike
//! `tauri::async_runtime`, it does not drag the windowing stack into the
//! `cargo test --lib` test binary.
//!
//! A panic inside the task is turned into a `JoinError` and therefore into an
//! error message: a broken image can never take the application down.

use std::path::PathBuf;

use crate::image_edit::ImageEditOutcome;

/// Rotates or flips the image at `path` and overwrites it in place.
///
/// The file keeps its original format, no `.bak` copy is created and the
/// original file stays untouched when anything goes wrong.
#[tauri::command]
pub async fn apply_image_edit(
    path: String,
    operation: String,
    quality: Option<u8>,
) -> Result<ImageEditOutcome, String> {
    let path = PathBuf::from(path);

    tokio::task::spawn_blocking(move || {
        crate::image_edit::kernel::apply_image_edit(&path, &operation, quality)
    })
    .await
    .map_err(|error| format!("The image edit task did not finish: {error}"))?
    .map_err(|error| error.to_string())
}
