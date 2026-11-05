// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! Backend for the "rotate / flip image" actions of the file browser.
//!
//! Layout of this module:
//! - `kernel` holds the whole pixel level implementation and does not depend on
//!   `tauri` in any way, so the unit tests at the bottom of this file never pull
//!   the tauri runtime into the test binary.
//! - The `#[tauri::command]` below is a thin wrapper around the kernel.
//!
//! Writing back is always done through a temporary file in the same directory
//! that is renamed over the original, so a failure never leaves a half written
//! image behind and no `.bak` file is produced.

use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use image::Frame;
use image::codecs::gif::{GifDecoder, GifEncoder};
use image::{
    AnimationDecoder, DynamicImage, ImageFormat, ImageReader, Rgb, RgbImage, Rgba, RgbaImage,
};
use jpeg_encoder::{ColorType as JpegColorType, Encoder as JpegEncoderFast, SamplingFactor};
use serde::Serialize;

/// Suffix of the temporary file used while the edited image is written back.
pub const TEMP_FILE_SUFFIX: &str = ".afm-img-edit.tmp";

/// JPEG quality used when a JPEG file has to be re-encoded.
pub const DEFAULT_JPEG_QUALITY: u8 = 90;

/// The pixel level operations exposed to the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageEditOperation {
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
}

impl ImageEditOperation {
    /// Parses the operation name coming from the frontend.
    pub fn parse(raw: &str) -> Result<Self, ImageEditError> {
        match raw
            .trim()
            .to_ascii_lowercase()
            .replace('_', "-")
            .replace(' ', "-")
            .as_str()
        {
            "rotate-left" | "rotateleft" => Ok(Self::RotateLeft),
            "rotate-right" | "rotateright" => Ok(Self::RotateRight),
            "flip-horizontal" | "fliphorizontal" => Ok(Self::FlipHorizontal),
            "flip-vertical" | "flipvertical" => Ok(Self::FlipVertical),
            other => Err(ImageEditError::UnsupportedOperation(other.to_string())),
        }
    }

    /// The canonical name of the operation, as understood by the frontend.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RotateLeft => "rotate-left",
            Self::RotateRight => "rotate-right",
            Self::FlipHorizontal => "flip-horizontal",
            Self::FlipVertical => "flip-vertical",
        }
    }
}

/// Every failure mode of the image edit pipeline.
///
/// The `Display` text is the message shown by the frontend toast, therefore it
/// is written in plain English and never localised on the Rust side.
#[derive(Debug)]
pub enum ImageEditError {
    UnsupportedOperation(String),
    UnsupportedFormat(String),
    NotFound(PathBuf),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Decode {
        path: PathBuf,
        message: String,
    },
    Encode {
        path: PathBuf,
        message: String,
    },
    Replace {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A panic inside the pixel pipeline.
    ///
    /// The panic is caught, so that a broken image can never take the whole
    /// application down.
    Panic {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for ImageEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedOperation(operation) => write!(
                formatter,
                "Unsupported image edit operation: {operation}. Expected one of rotate-left, rotate-right, flip-horizontal, flip-vertical."
            ),
            Self::UnsupportedFormat(format) => write!(
                formatter,
                "Unsupported image format: {format}. Supported formats are PNG, JPEG, GIF, BMP, WebP, TIFF and ICO."
            ),
            Self::NotFound(path) => {
                write!(formatter, "Image file not found: {}", path.display())
            }
            Self::Io { path, source } => write!(
                formatter,
                "Failed to access image file {}: {source}",
                path.display()
            ),
            Self::Decode { path, message } => write!(
                formatter,
                "Failed to decode image {}: {message}",
                path.display()
            ),
            Self::Encode { path, message } => write!(
                formatter,
                "Failed to encode image {}: {message}",
                path.display()
            ),
            Self::Replace { path, source } => write!(
                formatter,
                "Failed to replace image file {}: {source}",
                path.display()
            ),
            Self::Panic { path, message } => write!(
                formatter,
                "The image editor stopped unexpectedly while processing {}: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ImageEditError {}

/// Result of a successful image edit.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEditOutcome {
    /// Absolute path of the edited file.
    pub path: String,
    /// Operation that was applied.
    pub operation: String,
    /// Format the file was written back in.
    pub format: String,
    /// Width of the edited image, in pixels.
    pub width: u32,
    /// Height of the edited image, in pixels.
    pub height: u32,
    /// Size of the edited file on disk, in bytes.
    pub size_bytes: u64,
    /// Number of frames of the edited file (animated GIF keeps its frames).
    pub frame_count: u32,
}

/// The image formats this backend is able to write back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageFormatKind {
    Png,
    Jpeg,
    Gif,
    Bmp,
    WebP,
    Tiff,
    Ico,
}

impl ImageFormatKind {
    fn name(&self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::Bmp => "BMP",
            Self::WebP => "WebP",
            Self::Tiff => "TIFF",
            Self::Ico => "ICO",
        }
    }

    fn to_image_format(&self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Gif => ImageFormat::Gif,
            Self::Bmp => ImageFormat::Bmp,
            Self::WebP => ImageFormat::WebP,
            Self::Tiff => ImageFormat::Tiff,
            Self::Ico => ImageFormat::Ico,
        }
    }
}

/// Maps a file extension onto a supported format.
fn format_from_extension(extension: &str) -> Option<ImageFormatKind> {
    match extension {
        "png" | "apng" => Some(ImageFormatKind::Png),
        "jpg" | "jpeg" | "jfif" => Some(ImageFormatKind::Jpeg),
        "gif" => Some(ImageFormatKind::Gif),
        "bmp" => Some(ImageFormatKind::Bmp),
        "webp" => Some(ImageFormatKind::WebP),
        "tif" | "tiff" => Some(ImageFormatKind::Tiff),
        "ico" => Some(ImageFormatKind::Ico),
        _ => None,
    }
}

/// Sniffs the file magic, used when the extension is missing or unknown.
fn sniff_format(path: &Path) -> Option<ImageFormatKind> {
    let mut header = [0_u8; 16];
    let mut file = File::open(path).ok()?;
    let read = file.read(&mut header).ok()?;
    let bytes = &header[..read];
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(ImageFormatKind::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageFormatKind::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImageFormatKind::Gif);
    }
    if bytes.starts_with(b"BM") {
        return Some(ImageFormatKind::Bmp);
    }
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        return Some(ImageFormatKind::WebP);
    }
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return Some(ImageFormatKind::Tiff);
    }
    if bytes.starts_with(&[0, 0, 1, 0]) {
        return Some(ImageFormatKind::Ico);
    }
    None
}

/// Resolves the format of `path`, preferring the extension over the content.
fn detect_format(path: &Path) -> Result<ImageFormatKind, ImageEditError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if extension == "svg" || extension == "svgz" {
        return Err(ImageEditError::UnsupportedFormat("SVG".to_string()));
    }

    if !extension.is_empty() {
        if let Some(format) = format_from_extension(&extension) {
            return Ok(format);
        }
    }

    if let Some(format) = sniff_format(path) {
        return Ok(format);
    }

    if extension.is_empty() {
        Err(ImageEditError::UnsupportedFormat("unknown".to_string()))
    } else {
        Err(ImageEditError::UnsupportedFormat(extension))
    }
}

/// Path of the temporary file used while writing the edited image back.
fn temp_path_for(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(TEMP_FILE_SUFFIX);
    path.with_file_name(name)
}

/// Applies `operation` to the pixels of `source`.
///
/// The mapping is written out explicitly instead of relying on `imageops`, so
/// that non square images are handled exactly as expected:
///
/// - `rotate-left`  (90 degrees counter clockwise): `(x, y) -> (y, width - 1 - x)`
/// - `rotate-right` (90 degrees clockwise):        `(x, y) -> (height - 1 - y, x)`
/// - `flip-horizontal`:                           `(x, y) -> (width - 1 - x, y)`
/// - `flip-vertical`:                             `(x, y) -> (x, height - 1 - y)`
fn transform_rgba(source: &RgbaImage, operation: ImageEditOperation) -> RgbaImage {
    let (width, height) = source.dimensions();
    match operation {
        ImageEditOperation::RotateLeft => {
            let mut target = RgbaImage::new(height, width);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(y, width - 1 - x, *pixel);
            }
            target
        }
        ImageEditOperation::RotateRight => {
            let mut target = RgbaImage::new(height, width);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(height - 1 - y, x, *pixel);
            }
            target
        }
        ImageEditOperation::FlipHorizontal => {
            let mut target = RgbaImage::new(width, height);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(width - 1 - x, y, *pixel);
            }
            target
        }
        ImageEditOperation::FlipVertical => {
            let mut target = RgbaImage::new(width, height);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(x, height - 1 - y, *pixel);
            }
            target
        }
    }
}

/// The RGB flavour of [`transform_rgba`].
///
/// A JPEG is decoded as RGB anyway, so the still image path stays in RGB and
/// never pays for the round trip through four channels.
fn transform_rgb8(source: &RgbImage, operation: ImageEditOperation) -> RgbImage {
    let (width, height) = source.dimensions();
    match operation {
        ImageEditOperation::RotateLeft => {
            let mut target = RgbImage::new(height, width);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(y, width - 1 - x, *pixel);
            }
            target
        }
        ImageEditOperation::RotateRight => {
            let mut target = RgbImage::new(height, width);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(height - 1 - y, x, *pixel);
            }
            target
        }
        ImageEditOperation::FlipHorizontal => {
            let mut target = RgbImage::new(width, height);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(width - 1 - x, y, *pixel);
            }
            target
        }
        ImageEditOperation::FlipVertical => {
            let mut target = RgbImage::new(width, height);
            for (x, y, pixel) in source.enumerate_pixels() {
                target.put_pixel(x, height - 1 - y, *pixel);
            }
            target
        }
    }
}

/// Decodes a still image (any format but an animated GIF).
fn decode_still_image(path: &Path) -> Result<DynamicImage, ImageEditError> {
    let reader = ImageReader::open(path)
        .map_err(|source| ImageEditError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .with_guessed_format()
        .map_err(|source| ImageEditError::Decode {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
    reader
        .decode()
        .map_err(|error| ImageEditError::Decode {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

/// Decodes every frame of a GIF and applies `operation` to each of them.
fn transform_gif_frames(
    path: &Path,
    operation: ImageEditOperation,
) -> Result<(Vec<Frame>, u32, u32), ImageEditError> {
    let file = File::open(path).map_err(|source| ImageEditError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let decoder = GifDecoder::new(BufReader::new(file)).map_err(|error| {
        ImageEditError::Decode {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;

    let mut frames = Vec::new();
    let mut size = (0_u32, 0_u32);
    for frame in decoder.into_frames() {
        let frame = frame.map_err(|error| ImageEditError::Decode {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        let transformed = transform_rgba(frame.buffer(), operation);
        size = transformed.dimensions();
        frames.push(Frame::from_parts(transformed, 0, 0, frame.delay()));
    }
    Ok((frames, size.0, size.1))
}

/// Chroma sampling used when a JPEG is written back.
///
/// The crate names the factors by their sampling values, so `F_2_2` is the
/// classic 4:2:0: the chroma plane is halved in both directions. That is what
/// phone cameras use, the visible difference against 4:2:2 (`F_2_1`) is
/// negligible for photographs, and the encoder has less chroma data to
/// compress - which is a large part of the encoding time.
const JPEG_SAMPLING_FACTOR: SamplingFactor = SamplingFactor::F_2_2;

/// Encodes RGB pixels as JPEG.
///
/// The `jpeg-encoder` crate has an AVX2 accelerated forward DCT (feature
/// `simd`, selected at runtime) and is considerably faster than the scalar
/// encoder that ships with the `image` crate - that encoder alone used to
/// dominate the runtime of a rotation on a large photo.
fn encode_jpeg_bytes(
    rgb: &[u8],
    width: u32,
    height: u32,
    quality: u8,
) -> Result<Vec<u8>, String> {
    if width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err(format!(
            "the image is too large for the JPEG encoder: {width}x{height}"
        ));
    }

    let mut buffer = Vec::with_capacity(rgb.len() / 2 + 1024);
    let mut encoder = JpegEncoderFast::new(&mut buffer, quality);
    encoder.set_sampling_factor(JPEG_SAMPLING_FACTOR);
    encoder
        .encode(
            rgb,
            width as u16,
            height as u16,
            JpegColorType::Rgb,
        )
        .map_err(|error| error.to_string())?;

    Ok(buffer)
}

/// Writes a still image to `path` using the encoder of `format`.
fn encode_still_image(
    path: &Path,
    format: ImageFormatKind,
    image: &DynamicImage,
    quality: u8,
) -> Result<(), ImageEditError> {
    let mut file = File::create(path).map_err(|source| ImageEditError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    match format {
        ImageFormatKind::Jpeg => {
            let rgb = image.to_rgb8();
            let (width, height) = rgb.dimensions();
            let bytes =
                encode_jpeg_bytes(rgb.as_raw(), width, height, quality).map_err(|message| {
                    ImageEditError::Encode {
                        path: path.to_path_buf(),
                        message,
                    }
                })?;
            file.write_all(&bytes).map_err(|source| ImageEditError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
        _ => image
            .write_to(&mut file, format.to_image_format())
            .map_err(|error| ImageEditError::Encode {
                path: path.to_path_buf(),
                message: error.to_string(),
            }),
    }
}

/// Writes the frames of an animated GIF to `path`.
fn encode_gif_frames(path: &Path, frames: &[Frame]) -> Result<(), ImageEditError> {
    let file = File::create(path).map_err(|source| ImageEditError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut encoder = GifEncoder::new_with_speed(file, 10);
    encoder
        .encode_frames(frames.to_vec().into_iter())
        .map_err(|error| ImageEditError::Encode {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

/// Renames the temporary file over the original file.
fn replace_original(temp_path: &Path, target: &Path) -> Result<(), ImageEditError> {
    fs::rename(temp_path, target).map_err(|source| {
        let _ = fs::remove_file(temp_path);
        ImageEditError::Replace {
            path: target.to_path_buf(),
            source,
        }
    })
}

/// Rotates or flips the image at `path` and overwrites it in place.
///
/// The file keeps its original format, no `.bak` copy is created and the
/// original file stays untouched when anything goes wrong.
pub fn apply_image_edit_blocking(
    path: &Path,
    operation: &str,
    quality: Option<u8>,
) -> Result<ImageEditOutcome, ImageEditError> {
    // A panic inside the pixel pipeline (a broken file, an out of range
    // dimension, ...) must never take the application down: it is caught here
    // and turned into an ordinary error that the frontend shows as a toast.
    let path = path.to_path_buf();
    let operation = operation.to_string();
    let panic_path = path.clone();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        apply_image_edit_inner(&path, &operation, quality)
    }));

    match outcome {
        Ok(result) => result,
        Err(payload) => Err(ImageEditError::Panic {
            path: panic_path,
            message: panic_message(payload),
        }),
    }
}

/// Turns the payload of a caught panic into a readable message.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    }
    else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    }
    else {
        "unknown panic".to_string()
    }
}

/// The actual pixel work, without the panic guard.
fn apply_image_edit_inner(
    path: &Path,
    operation: &str,
    quality: Option<u8>,
) -> Result<ImageEditOutcome, ImageEditError> {
    let operation = ImageEditOperation::parse(operation)?;
    if !path.is_file() {
        return Err(ImageEditError::NotFound(path.to_path_buf()));
    }

    let format = detect_format(path)?;
    let quality = quality
        .map(|value| value.clamp(1, 100))
        .unwrap_or(DEFAULT_JPEG_QUALITY);

    if format == ImageFormatKind::Gif {
        let (frames, width, height) = transform_gif_frames(path, operation)?;
        if frames.is_empty() {
            return Err(ImageEditError::Decode {
                path: path.to_path_buf(),
                message: "the GIF does not contain any frame".to_string(),
            });
        }
        let frame_count = frames.len() as u32;
        let temp_path = temp_path_for(path);
        if let Err(error) = encode_gif_frames(&temp_path, &frames) {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        replace_original(&temp_path, path)?;
        return Ok(ImageEditOutcome {
            path: path.to_string_lossy().into_owned(),
            operation: operation.as_str().to_string(),
            format: format.name().to_string(),
            width,
            height,
            size_bytes: file_size(path),
            frame_count,
        });
    }

    let decoded = decode_still_image(path)?;
    // A JPEG decodes to RGB already, so the still image path stays in three
    // channels and never copies the whole picture into RGBA.
    let transformed = match decoded {
        DynamicImage::ImageRgb8(rgb) => DynamicImage::ImageRgb8(transform_rgb8(&rgb, operation)),
        other => DynamicImage::ImageRgba8(transform_rgba(&other.to_rgba8(), operation)),
    };
    let width = transformed.width();
    let height = transformed.height();

    let temp_path = temp_path_for(path);
    if let Err(error) = encode_still_image(&temp_path, format, &transformed, quality) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    replace_original(&temp_path, path)?;

    Ok(ImageEditOutcome {
        path: path.to_string_lossy().into_owned(),
        operation: operation.as_str().to_string(),
        format: format.name().to_string(),
        width,
        height,
        size_bytes: file_size(path),
        frame_count: 1,
    })
}

/// Size of `path` in bytes, `0` when the file cannot be read.
fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0)
}

/// The tauri free kernel, also reachable under the `apply_image_edit` name.
///
/// The Tauri command above has to keep the `apply_image_edit` name so that
/// the frontend can invoke it, therefore the kernel is additionally exposed
/// through this module.
pub mod kernel {
    use super::{apply_image_edit_blocking, ImageEditError, ImageEditOutcome};
    use std::path::Path;

    /// Rotates or flips the image at `path` and overwrites it in place.
    pub fn apply_image_edit(
        path: &Path,
        operation: &str,
        quality: Option<u8>,
    ) -> Result<ImageEditOutcome, ImageEditError> {
        apply_image_edit_blocking(path, operation, quality)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Diagnostic probe: applies an operation to the file given by the
    /// environment variable `AFM_IMAGE_EDIT_SAMPLE` and prints the outcome
    /// together with the elapsed time of every step.
    ///
    /// Run it with:
    /// `AFM_IMAGE_EDIT_SAMPLE=<file> cargo test --lib probe_external_sample -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_external_sample() {
        use image::codecs::jpeg::JpegEncoder;
        use image::{ExtendedColorType, ImageEncoder};

        let Ok(path) = std::env::var("AFM_IMAGE_EDIT_SAMPLE") else {
            println!("PROBE_SKIPPED=no AFM_IMAGE_EDIT_SAMPLE");
            return;
        };
        let operation = std::env::var("AFM_IMAGE_EDIT_OPERATION")
            .unwrap_or_else(|_| "rotate-left".to_string());
        let operation = ImageEditOperation::parse(&operation).expect("parse operation");

        // The probe only reads the sample, it never writes it.
        let sample = PathBuf::from(&path);
        let started = std::time::Instant::now();
        let decoded = decode_still_image(&sample).expect("decode");
        let decoded_at = std::time::Instant::now();
        let rgb = decoded.to_rgb8();
        let rgb_at = std::time::Instant::now();
        let transformed = transform_rgb8(&rgb, operation);
        let transform_at = std::time::Instant::now();
        let (width, height) = transformed.dimensions();

        // 1) the encoder of the `image` crate - the old implementation
        let mut old_bytes = Vec::new();
        {
            let mut encoder = JpegEncoder::new_with_quality(&mut old_bytes, 90);
            encoder
                .write_image(
                    transformed.as_raw(),
                    width,
                    height,
                    ExtendedColorType::Rgb8,
                )
                .expect("encode with the image crate");
        }
        let old_at = std::time::Instant::now();

        // 2) the `jpeg-encoder` crate - the new implementation
        let new_bytes =
            encode_jpeg_bytes(transformed.as_raw(), width, height, 90).expect("encode with jpeg-encoder");
        let new_at = std::time::Instant::now();

        println!("PROBE_SOURCE={}", sample.display());
        println!("PROBE_SIZE={}x{}", width, height);
        println!("PROBE_DECODE_MS={}", (decoded_at - started).as_millis());
        println!("PROBE_TO_RGB_MS={}", (rgb_at - decoded_at).as_millis());
        println!("PROBE_TRANSFORM_MS={}", (transform_at - rgb_at).as_millis());
        println!("PROBE_ENCODE_OLD_MS={}", (old_at - transform_at).as_millis());
        println!("PROBE_ENCODE_NEW_MS={}", (new_at - old_at).as_millis());
        println!("PROBE_TOTAL_NEW_MS={}", (new_at - started).as_millis());
        println!("PROBE_OLD_BYTES={}", old_bytes.len());
        println!("PROBE_NEW_BYTES={}", new_bytes.len());
    }

    /// A temporary directory that removes itself when the test ends.
    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "afm-image-edit-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create the test directory");
            Self { path }
        }

        fn join(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }

        fn entries(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&self.path)
                .expect("read the test directory")
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Builds a 3x2 image where every pixel carries its own coordinates.
    fn sample_image() -> RgbaImage {
        RgbaImage::from_fn(3, 2, |x, y| Rgba([(x * 40) as u8, (y * 60) as u8, 7, 255]))
    }

    fn write_image(path: &Path, image: &DynamicImage, format: ImageFormat) {
        let mut file = File::create(path).expect("create the image file");
        image
            .write_to(&mut file, format)
            .expect("write the image file");
    }

    fn write_sample_png(path: &Path) {
        write_image(path, &DynamicImage::ImageRgba8(sample_image()), ImageFormat::Png);
    }

    fn load_rgba(path: &Path) -> RgbaImage {
        let reader = ImageReader::open(path)
            .expect("open the image")
            .with_guessed_format()
            .expect("guess the image format");
        reader
            .decode()
            .expect("decode the image")
            .to_rgba8()
    }

    #[test]
    fn rotate_left_swaps_dimensions_and_moves_pixels() {
        let directory = TestDir::new("rotate-left");
        let path = directory.join("sample.png");
        write_sample_png(&path);

        let outcome = apply_image_edit_blocking(&path, "rotate-left", None).expect("rotate left");
        assert_eq!(outcome.width, 2);
        assert_eq!(outcome.height, 3);
        assert_eq!(outcome.format, "PNG");
        assert_eq!(outcome.operation, "rotate-left");
        assert_eq!(outcome.frame_count, 1);
        assert_eq!(outcome.path, path.to_string_lossy().into_owned());

        let edited = load_rgba(&path);
        assert_eq!(edited.dimensions(), (2, 3));
        // (x, y) -> (y, width - 1 - x)
        assert_eq!(edited.get_pixel(0, 2), &Rgba([0, 0, 7, 255]));
        assert_eq!(edited.get_pixel(0, 1), &Rgba([40, 0, 7, 255]));
        assert_eq!(edited.get_pixel(0, 0), &Rgba([80, 0, 7, 255]));
        assert_eq!(edited.get_pixel(1, 2), &Rgba([0, 60, 7, 255]));
        assert_eq!(edited.get_pixel(1, 1), &Rgba([40, 60, 7, 255]));
        assert_eq!(edited.get_pixel(1, 0), &Rgba([80, 60, 7, 255]));
    }

    #[test]
    fn rotate_right_swaps_dimensions_and_moves_pixels() {
        let directory = TestDir::new("rotate-right");
        let path = directory.join("sample.png");
        write_sample_png(&path);

        let outcome = apply_image_edit_blocking(&path, "rotate-right", None).expect("rotate right");
        assert_eq!(outcome.width, 2);
        assert_eq!(outcome.height, 3);

        let edited = load_rgba(&path);
        assert_eq!(edited.dimensions(), (2, 3));
        // (x, y) -> (height - 1 - y, x)
        assert_eq!(edited.get_pixel(1, 0), &Rgba([0, 0, 7, 255]));
        assert_eq!(edited.get_pixel(1, 1), &Rgba([40, 0, 7, 255]));
        assert_eq!(edited.get_pixel(1, 2), &Rgba([80, 0, 7, 255]));
        assert_eq!(edited.get_pixel(0, 0), &Rgba([0, 60, 7, 255]));
        assert_eq!(edited.get_pixel(0, 1), &Rgba([40, 60, 7, 255]));
        assert_eq!(edited.get_pixel(0, 2), &Rgba([80, 60, 7, 255]));
    }

    #[test]
    fn flip_horizontal_mirrors_the_pixels() {
        let directory = TestDir::new("flip-horizontal");
        let path = directory.join("sample.png");
        write_sample_png(&path);

        let outcome = apply_image_edit_blocking(&path, "flip-horizontal", None).expect("flip horizontal");
        assert_eq!((outcome.width, outcome.height), (3, 2));

        let edited = load_rgba(&path);
        assert_eq!(edited.get_pixel(2, 0), &Rgba([0, 0, 7, 255]));
        assert_eq!(edited.get_pixel(1, 0), &Rgba([40, 0, 7, 255]));
        assert_eq!(edited.get_pixel(0, 0), &Rgba([80, 0, 7, 255]));
        assert_eq!(edited.get_pixel(2, 1), &Rgba([0, 60, 7, 255]));
        assert_eq!(edited.get_pixel(0, 1), &Rgba([80, 60, 7, 255]));
    }

    #[test]
    fn flip_vertical_mirrors_the_pixels() {
        let directory = TestDir::new("flip-vertical");
        let path = directory.join("sample.png");
        write_sample_png(&path);

        let outcome = apply_image_edit_blocking(&path, "flip-vertical", None).expect("flip vertical");
        assert_eq!((outcome.width, outcome.height), (3, 2));

        let edited = load_rgba(&path);
        assert_eq!(edited.get_pixel(0, 1), &Rgba([0, 0, 7, 255]));
        assert_eq!(edited.get_pixel(0, 0), &Rgba([0, 60, 7, 255]));
        assert_eq!(edited.get_pixel(2, 1), &Rgba([80, 0, 7, 255]));
        assert_eq!(edited.get_pixel(2, 0), &Rgba([80, 60, 7, 255]));
    }

    #[test]
    fn rotating_left_then_right_restores_the_original_pixels() {
        let directory = TestDir::new("rotate-twice");
        let path = directory.join("sample.png");
        write_sample_png(&path);
        let original = load_rgba(&path);

        // NOTE: two rotations in the SAME direction are a 180 degree turn and
        // do NOT give the original back. Only a left/right pair cancels out.
        apply_image_edit_blocking(&path, "rotate-left", None).expect("first rotation");
        apply_image_edit_blocking(&path, "rotate-right", None).expect("second rotation");

        let edited = load_rgba(&path);
        assert_eq!(edited.dimensions(), original.dimensions());
        for (x, y, pixel) in original.enumerate_pixels() {
            assert_eq!(edited.get_pixel(x, y), pixel, "pixel {x},{y} differs");
        }
    }

    #[test]
    fn rotating_twice_in_the_same_direction_turns_the_image_by_180_degrees() {
        let directory = TestDir::new("rotate-180");
        let path = directory.join("sample.png");
        write_sample_png(&path);
        let original = load_rgba(&path);

        apply_image_edit_blocking(&path, "rotate-left", None).expect("first rotation");
        apply_image_edit_blocking(&path, "rotate-left", None).expect("second rotation");

        let edited = load_rgba(&path);
        assert_eq!(edited.dimensions(), original.dimensions());
        let (width, height) = original.dimensions();
        for (x, y, pixel) in original.enumerate_pixels() {
            assert_eq!(
                edited.get_pixel(width - 1 - x, height - 1 - y),
                pixel,
                "pixel {x},{y} differs"
            );
        }
    }

    #[test]
    fn flipping_twice_restores_the_original_pixels() {
        let directory = TestDir::new("flip-twice");
        let path = directory.join("sample.png");
        write_sample_png(&path);
        let original = load_rgba(&path);

        apply_image_edit_blocking(&path, "flip-horizontal", None).expect("first flip");
        apply_image_edit_blocking(&path, "flip-horizontal", None).expect("second flip");

        let edited = load_rgba(&path);
        for (x, y, pixel) in original.enumerate_pixels() {
            assert_eq!(edited.get_pixel(x, y), pixel, "pixel {x},{y} differs");
        }
    }

    #[test]
    fn the_edited_file_stays_decodable_and_keeps_its_format() {
        let directory = TestDir::new("write-back");
        let path = directory.join("sample.png");
        write_sample_png(&path);

        apply_image_edit_blocking(&path, "rotate-right", None).expect("rotate right");

        let bytes = fs::read(&path).expect("read the edited file");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "the PNG magic is gone");
        assert_eq!(load_rgba(&path).dimensions(), (2, 3));
    }

    #[test]
    fn jpeg_files_are_written_back_as_jpeg() {
        let directory = TestDir::new("jpeg");
        let path = directory.join("sample.jpg");
        write_image(
            &path,
            &DynamicImage::ImageRgba8(sample_image()),
            ImageFormat::Jpeg,
        );

        let outcome = apply_image_edit_blocking(&path, "rotate-left", None).expect("rotate left");
        assert_eq!(outcome.format, "JPEG");
        assert_eq!((outcome.width, outcome.height), (2, 3));

        let bytes = fs::read(&path).expect("read the edited file");
        assert!(bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "the JPEG magic is gone");
        assert_eq!(load_rgba(&path).dimensions(), (2, 3));
    }

    #[test]
    fn gif_files_are_written_back_as_gif() {
        let directory = TestDir::new("gif");
        let path = directory.join("sample.gif");
        write_image(
            &path,
            &DynamicImage::ImageRgba8(sample_image()),
            ImageFormat::Gif,
        );

        let outcome = apply_image_edit_blocking(&path, "rotate-right", None).expect("rotate right");
        assert_eq!(outcome.format, "GIF");
        assert_eq!((outcome.width, outcome.height), (2, 3));
        assert_eq!(outcome.frame_count, 1);

        let bytes = fs::read(&path).expect("read the edited file");
        assert!(
            bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
            "the GIF magic is gone"
        );
    }

    #[test]
    fn text_files_are_rejected() {
        let directory = TestDir::new("text");
        let path = directory.join("notes.txt");
        fs::write(&path, b"not an image at all").expect("write the text file");

        let error = apply_image_edit_blocking(&path, "rotate-left", None).expect_err("the text file fails");
        assert!(
            error.to_string().contains("Unsupported image format"),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read(&path).expect("read the text file"),
            b"not an image at all"
        );
    }

    #[test]
    fn svg_files_are_rejected_with_a_clear_message() {
        let directory = TestDir::new("svg");
        let path = directory.join("logo.svg");
        fs::write(&path, b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>")
            .expect("write the svg file");

        let error = apply_image_edit_blocking(&path, "rotate-left", None).expect_err("the svg file fails");
        let message = error.to_string();
        assert!(message.contains("SVG"), "unexpected error: {message}");
        assert!(
            message.contains("Unsupported image format"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn unknown_operations_are_rejected() {
        let directory = TestDir::new("operation");
        let path = directory.join("sample.png");
        write_sample_png(&path);

        let error = apply_image_edit_blocking(&path, "sharpen", None).expect_err("the operation fails");
        assert!(
            error.to_string().contains("Unsupported image edit operation"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_broken_image_is_rejected_and_the_file_is_kept() {
        let directory = TestDir::new("broken");
        let path = directory.join("broken.png");
        fs::write(&path, b"this is definitely not a png").expect("write the broken file");

        let error = apply_image_edit_blocking(&path, "rotate-left", None).expect_err("the file fails");
        assert!(
            error.to_string().contains("Failed to decode image"),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read(&path).expect("read the broken file"),
            b"this is definitely not a png"
        );
        assert_eq!(directory.entries(), vec!["broken.png".to_string()]);
    }

    #[test]
    fn no_temporary_file_is_left_behind() {
        let directory = TestDir::new("temporary");
        let path = directory.join("sample.png");
        write_sample_png(&path);

        apply_image_edit_blocking(&path, "rotate-left", None).expect("rotate left");

        assert_eq!(directory.entries(), vec!["sample.png".to_string()]);
        assert!(!directory
            .join("sample.png.afm-img-edit.tmp")
            .exists());
    }

    #[test]
    fn missing_files_are_reported() {
        let directory = TestDir::new("missing");
        let path = directory.join("absent.png");

        let error = apply_image_edit_blocking(&path, "rotate-left", None).expect_err("the file is missing");
        assert!(
            error.to_string().contains("Image file not found"),
            "unexpected error: {error}"
        );
    }
}
