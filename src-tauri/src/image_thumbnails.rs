// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::SystemTime;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use image::codecs::gif::GifDecoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::{imageops::FilterType, ImageDecoder, ImageReader, Limits};
use once_cell::sync::Lazy;
use serde::Serialize;
use sha2::{Digest, Sha256};

const IMAGE_THUMBNAIL_CACHE_DIR: &str = "image-thumbnails";
const THUMBNAIL_MAX_DIMENSION: u32 = 384;
/// 缩略图缓存的条目数上限。
///
/// 2026-10-28 从 5000 收紧到 2000：实测增长约 265 张/天，5000 的上限意味着
/// 「某一刻全部缩略图被清空、全部重生成」。超限现在按最旧优先淘汰，
/// 不再整体清空，所以这个数字只决定缓存能长到多大，不再决定体验。
const MAX_THUMBNAIL_CACHE_ITEM_COUNT: usize = 2000;
/// 缩略图缓存的字节数上限（256 MB）。
///
/// 2026-10-28 从 1 GB 收紧到 256 MB：实测 4249 张就占了 434 MB，
/// 单张中位数 121 KB，1 GB 迟早会撞上。
const MAX_THUMBNAIL_CACHE_SIZE_BYTES: u64 = 256 * 1024 * 1024;
const THUMBNAIL_CACHE_LIMIT_CHECK_INTERVAL: usize = 100;
const MAX_THUMBNAIL_SOURCE_FILE_SIZE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_THUMBNAIL_SOURCE_DIMENSION: u32 = 32_768;
/// 单张源图允许的最大像素数（16 MP = 2^24 像素 ≈ 64 MiB RGBA）。
///
/// 2026-10-28 从 50 MP 收紧到 16 MP：与下面的解码分配上限精确对齐
/// （16 MP × 4 字节 = 64 MiB），超限的图在解码之前就被挡掉，
/// 不会先分配再失败。
const MAX_THUMBNAIL_SOURCE_PIXELS: u64 = 16_777_216;
/// 单张源图解码允许的最大分配字节数（64 MB）。
///
/// 2026-10-28 从 192 MB 收紧到 64 MB：一张大图解码的瞬时占用
/// 从「接近 200 MB」降到「不超过 64 MB」。
const MAX_THUMBNAIL_DECODE_ALLOCATION_BYTES: u64 = 64 * 1024 * 1024;
/// 同时解码的缩略图张数上限。
///
/// 解码是内存尖峰来源，多张大图同时解码峰值会叠加。
/// 3 是「够用又不打爆」的折中：2 太保守，4 以上并发峰值又会叠加上去。
const MAX_CONCURRENT_THUMBNAIL_DECODES: usize = 3;
const MAX_ORIGINAL_IMAGE_THUMBNAIL_SOURCE_SIZE_BYTES: u64 = 512 * 1024;
const MAX_VIDEO_THUMBNAIL_CACHE_BYTES: usize = 2 * 1024 * 1024;
const IMAGE_THUMBNAIL_RESIZE_FILTER: FilterType = FilterType::Triangle;
const IMAGE_THUMBNAIL_PNG_COMPRESSION: CompressionType = CompressionType::Fast;
const IMAGE_THUMBNAIL_PNG_FILTER: PngFilterType = PngFilterType::Adaptive;

static IMAGE_THUMBNAIL_CACHE_FILE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static IMAGE_THUMBNAIL_CACHE_MAINTENANCE: Lazy<Mutex<ThumbnailCacheMaintenanceState>> =
    Lazy::new(|| Mutex::new(ThumbnailCacheMaintenanceState::default()));
static TEMPORARY_THUMBNAIL_COUNTER: AtomicU64 = AtomicU64::new(0);
static THUMBNAIL_DECODE_GATE: Lazy<ThumbnailDecodeGate> = Lazy::new(ThumbnailDecodeGate::new);

/// 解码并发闸门。
///
/// 解码一张大图会瞬时占用几十 MB（上限见 `MAX_THUMBNAIL_DECODE_ALLOCATION_BYTES`），
/// 批量生成缩略图时多个解码并发跑，峰值直接叠加。这里把并发解码数压在
/// `MAX_CONCURRENT_THUMBNAIL_DECODES` 以内。
///
/// 刻意用 `std::sync` 实现而不是 `tokio::sync::Semaphore`：解码发生在
/// `spawn_blocking` 的阻塞线程里，同步内核不能碰异步运行时
/// （`tauri::async_runtime` 一旦被链接进测试二进制，就会把整条 tao 窗口链拖进来，
/// 测试二进制会在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND` 崩掉）。
struct ThumbnailDecodeGate {
    active_decodes: Mutex<usize>,
    slot_available: Condvar,
}

impl ThumbnailDecodeGate {
    fn new() -> Self {
        Self {
            active_decodes: Mutex::new(0),
            slot_available: Condvar::new(),
        }
    }

    /// 占一个解码名额；名额用完时阻塞等待。
    fn acquire(&self) -> ThumbnailDecodeSlot<'_> {
        let mut active_decodes = self
            .active_decodes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        while *active_decodes >= MAX_CONCURRENT_THUMBNAIL_DECODES {
            active_decodes = self
                .slot_available
                .wait(active_decodes)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }

        *active_decodes += 1;

        ThumbnailDecodeSlot { gate: self }
    }
}

/// 闸门的占用凭证：离开作用域就还回去一个名额。
struct ThumbnailDecodeSlot<'a> {
    gate: &'a ThumbnailDecodeGate,
}

impl Drop for ThumbnailDecodeSlot<'_> {
    fn drop(&mut self) {
        let mut active_decodes = self
            .gate
            .active_decodes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        *active_decodes = active_decodes.saturating_sub(1);
        self.gate.slot_available.notify_one();
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailCacheStats {
    pub item_count: usize,
    pub size_bytes: u64,
}

#[derive(Default)]
struct ThumbnailCacheMaintenanceState {
    checked_once: bool,
    generated_since_check: usize,
}

fn normalize_thumbnail_max_dimension(max_dimension: Option<u32>) -> u32 {
    match max_dimension {
        Some(dimension) if dimension > 0 => dimension.min(THUMBNAIL_MAX_DIMENSION),
        _ => THUMBNAIL_MAX_DIMENSION,
    }
}

fn hash_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn thumbnail_cache_key(path: &str, modified_time: u64, size: u64, max_dimension: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update([0]);
    hasher.update(modified_time.to_le_bytes());
    hasher.update(size.to_le_bytes());
    hasher.update(max_dimension.to_le_bytes());
    hash_to_hex(&hasher.finalize())
}

fn video_thumbnail_cache_key(
    path: &str,
    modified_time: u64,
    size: u64,
    width: u32,
    height: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"video");
    hasher.update([0]);
    hasher.update(path.as_bytes());
    hasher.update([0]);
    hasher.update(modified_time.to_le_bytes());
    hasher.update(size.to_le_bytes());
    hasher.update(width.to_le_bytes());
    hasher.update(height.to_le_bytes());
    hash_to_hex(&hasher.finalize())
}

fn thumbnail_cache_path(
    cache_dir: &Path,
    path: &str,
    modified_time: u64,
    size: u64,
    max_dimension: u32,
) -> PathBuf {
    cache_dir.join(format!(
        "{}.png",
        thumbnail_cache_key(path, modified_time, size, max_dimension)
    ))
}

fn video_thumbnail_cache_path(
    cache_dir: &Path,
    path: &str,
    modified_time: u64,
    size: u64,
    width: u32,
    height: u32,
) -> PathBuf {
    cache_dir.join(format!(
        "{}.jpg",
        video_thumbnail_cache_key(path, modified_time, size, width, height)
    ))
}

/// 缩略图缓存目录名的**唯一**拼接点。
///
/// 生成、统计、清理三条路径都必须经过这里。这三者如果各自 `join`，
/// 将来有人只改其中一处（例如改成分层布局 `image-thumbnails/v2`），
/// 生成会写到新目录、统计与清理还盯着旧目录，而单测全部照常通过——
/// 测试守的是 helper 自己算出来的路径，不是彼此是否一致。
/// 收敛成单一函数后，"改目录名"只剩一个可改的地方。
fn thumbnail_cache_dir_in(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(IMAGE_THUMBNAIL_CACHE_DIR)
}

fn thumbnail_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(thumbnail_cache_dir_in(
        &crate::app_dirs::app_data_dir(app)
            .map_err(|error| format!("Failed to resolve app data directory: {error}"))?,
    ))
}

/// 解析缩略图缓存目录并统计。
///
/// 单独抽出来是为了让「只统计 cache_dir、不统计 app_data_dir 下的其他文件」
/// 这条能被 `--lib` 单元测试覆盖：command 需要 `tauri::AppHandle`，而本项目
/// 的 tauri feature 不含 `test`，单元测试构造不出 AppHandle。command 只负责
/// AppHandle → app_data_dir 的转换。
///
/// 入参刻意是 app_data_dir（父目录）而不是 cache_dir：只有拿到父目录，
/// 下面的测试才能证明「即使 app_data_dir 下摆着 settings.json 和扩展目录，
/// 统计也不会把它们算进来」。若改成直接收 cache_dir，越界就无从构造，
/// 测试会变成恒真。
fn thumbnail_cache_stats_in_app_data_dir(
    app_data_dir: &Path,
) -> Result<ThumbnailCacheStats, String> {
    let cache_dir = thumbnail_cache_dir_in(app_data_dir);

    if !cache_dir.exists() {
        return Ok(ThumbnailCacheStats::default());
    }

    thumbnail_cache_stats(&cache_dir)
}

/// 解析缩略图缓存目录并清理。
///
/// 抽出来的理由同 `thumbnail_cache_stats_in_app_data_dir`。这条尤其关键：
/// 一旦把 app_data_dir（父目录）而不是 cache_dir 交给清理函数，
/// 用户的设置文件、扩展数据等都会被一起删掉。
///
/// 这里刻意不重置 `IMAGE_THUMBNAIL_CACHE_MAINTENANCE` 的
/// `checked_once` / `generated_since_check`：手动清理后，缓存必然小于限额，
/// 下一次自动限额检查无论如何都会判定"无需处理"。不重置的唯一后果是自动
/// 检查最多推迟 `THUMBNAIL_CACHE_LIMIT_CHECK_INTERVAL` 次生成，不影响正确性；
/// 换来的是清理路径不必去碰维护状态的内部字段。
fn clear_thumbnail_cache_in_app_data_dir(app_data_dir: &Path) -> Result<(), String> {
    let cache_dir = thumbnail_cache_dir_in(app_data_dir);

    if !cache_dir.exists() {
        return Ok(());
    }

    let _cache_file_lock = IMAGE_THUMBNAIL_CACHE_FILE_LOCK
        .lock()
        .map_err(|error| format!("Failed to lock thumbnail cache files: {error}"))?;

    clear_thumbnail_cache_dir(&cache_dir)
}

fn path_is_same_or_descendant(path: &Path, parent: &Path) -> bool {
    path == parent || path.starts_with(parent)
}

fn add_thumbnail_cache_path_stats(
    path: &Path,
    stats: &mut ThumbnailCacheStats,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Failed to read cache item: {error}"))?;

    stats.item_count += 1;

    if metadata.is_file() {
        stats.size_bytes = stats.size_bytes.saturating_add(metadata.len());
        return Ok(());
    }

    if metadata.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|error| format!("Failed to read thumbnail cache directory: {error}"))?
        {
            let entry =
                entry.map_err(|error| format!("Failed to read thumbnail cache item: {error}"))?;
            add_thumbnail_cache_path_stats(&entry.path(), stats)?;
        }
    }

    Ok(())
}

fn thumbnail_cache_stats(cache_dir: &Path) -> Result<ThumbnailCacheStats, String> {
    let mut stats = ThumbnailCacheStats::default();

    for entry in fs::read_dir(cache_dir)
        .map_err(|error| format!("Failed to read thumbnail cache directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("Failed to read thumbnail cache item: {error}"))?;
        add_thumbnail_cache_path_stats(&entry.path(), &mut stats)?;
    }

    Ok(stats)
}

fn clear_thumbnail_cache_dir(cache_dir: &Path) -> Result<(), String> {
    for entry in fs::read_dir(cache_dir)
        .map_err(|error| format!("Failed to read thumbnail cache directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("Failed to read thumbnail cache item: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Failed to read thumbnail cache item: {error}"))?;

        if metadata.is_dir() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("Failed to clear thumbnail cache directory: {error}"))?;
        } else {
            fs::remove_file(&path)
                .map_err(|error| format!("Failed to clear thumbnail cache file: {error}"))?;
        }
    }

    Ok(())
}

/// 缓存目录里的一个文件，以及淘汰排序需要的元信息。
struct ThumbnailCacheEntry {
    path: PathBuf,
    modified_time: SystemTime,
    size_bytes: u64,
}

/// 递归收集缓存目录里的所有文件。
///
/// 缓存目录是平铺的（只有生成出来的缩略图文件），这里仍然递归一层，
/// 以防目录里出现子目录时统计与淘汰的口径不一致。
fn collect_thumbnail_cache_entries(
    cache_dir: &Path,
    entries: &mut Vec<ThumbnailCacheEntry>,
) -> Result<(), String> {
    for entry in fs::read_dir(cache_dir)
        .map_err(|error| format!("Failed to read thumbnail cache directory: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Failed to read thumbnail cache item: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Failed to read cache item: {error}"))?;

        if metadata.is_dir() {
            collect_thumbnail_cache_entries(&path, entries)?;
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        // 读不到修改时间就按「最旧」处理：宁可先淘汰它，
        // 也不要因为排序依据缺失而让整个缓存被清空。
        let modified_time = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        entries.push(ThumbnailCacheEntry {
            path,
            modified_time,
            size_bytes: metadata.len(),
        });
    }

    Ok(())
}

/// 缓存超限时**按最旧优先淘汰**，直到回到限额内。
///
/// 2026-10-28 之前这里是「超限就清空整个目录」：缓存涨到上限的那一刻，
/// 用户会看到所有缩略图集体消失、全部重新生成。现在只淘汰最旧的文件，
/// 新生成的那批总是最新、因此总是留下。
///
/// 限额做成参数是为了让单测能用小数字精确验证淘汰顺序；
/// 生产入口见 `enforce_thumbnail_cache_limits`。
fn enforce_thumbnail_cache_limits_with_limits(
    cache_dir: &Path,
    max_item_count: usize,
    max_size_bytes: u64,
) -> Result<(), String> {
    let mut entries = Vec::new();
    collect_thumbnail_cache_entries(cache_dir, &mut entries)?;

    let mut item_count = entries.len();
    let mut size_bytes = entries
        .iter()
        .map(|entry| entry.size_bytes)
        .fold(0u64, |total, size| total.saturating_add(size));

    if item_count <= max_item_count && size_bytes <= max_size_bytes {
        return Ok(());
    }

    // 最旧的先淘汰。
    entries.sort_by(|left, right| left.modified_time.cmp(&right.modified_time));

    for entry in entries {
        if item_count <= max_item_count && size_bytes <= max_size_bytes {
            break;
        }

        // 单个文件删不掉（被占用等）就跳过它，不应该让整轮淘汰失败。
        if fs::remove_file(&entry.path).is_ok() {
            item_count -= 1;
            size_bytes = size_bytes.saturating_sub(entry.size_bytes);
        }
    }

    Ok(())
}

/// 缓存超限时按最旧优先淘汰，直到回到限额内（不再整体清空）。
fn enforce_thumbnail_cache_limits(cache_dir: &Path) -> Result<(), String> {
    enforce_thumbnail_cache_limits_with_limits(
        cache_dir,
        MAX_THUMBNAIL_CACHE_ITEM_COUNT,
        MAX_THUMBNAIL_CACHE_SIZE_BYTES,
    )
}

fn should_enforce_thumbnail_cache_limits() -> Result<bool, String> {
    let mut state = IMAGE_THUMBNAIL_CACHE_MAINTENANCE
        .lock()
        .map_err(|error| format!("Failed to lock thumbnail cache maintenance: {error}"))?;

    if !state.checked_once {
        state.checked_once = true;
        state.generated_since_check = 0;
        return Ok(true);
    }

    if state.generated_since_check >= THUMBNAIL_CACHE_LIMIT_CHECK_INTERVAL {
        state.generated_since_check = 0;
        return Ok(true);
    }

    Ok(false)
}

fn record_generated_thumbnail() -> Result<(), String> {
    let mut state = IMAGE_THUMBNAIL_CACHE_MAINTENANCE
        .lock()
        .map_err(|error| format!("Failed to lock thumbnail cache maintenance: {error}"))?;

    state.generated_since_check += 1;

    Ok(())
}

fn temporary_thumbnail_path(thumbnail_path: &Path) -> PathBuf {
    let temporary_id = TEMPORARY_THUMBNAIL_COUNTER.fetch_add(1, Ordering::Relaxed);
    thumbnail_path.with_extension(format!("tmp-{temporary_id}"))
}

fn thumbnail_decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_THUMBNAIL_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_THUMBNAIL_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_THUMBNAIL_DECODE_ALLOCATION_BYTES);
    limits
}

fn validate_image_thumbnail_source(
    source_size: u64,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if source_size > MAX_THUMBNAIL_SOURCE_FILE_SIZE_BYTES {
        return Err("Image is too large to thumbnail".to_string());
    }

    let pixel_count = u64::from(width).saturating_mul(u64::from(height));

    if width == 0 || height == 0 || pixel_count > MAX_THUMBNAIL_SOURCE_PIXELS {
        return Err("Image dimensions are too large to thumbnail".to_string());
    }

    Ok(())
}

fn is_browser_renderable_image_path(source_path: &Path) -> bool {
    let Some(extension) = source_path
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return false;
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico"
    )
}

fn is_gif_path(source_path: &Path) -> bool {
    source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
}

fn should_use_original_image_thumbnail(
    source_path: &Path,
    source_size: u64,
    width: u32,
    height: u32,
    max_dimension: u32,
) -> bool {
    if is_gif_path(source_path) {
        return false;
    }

    source_size <= MAX_ORIGINAL_IMAGE_THUMBNAIL_SOURCE_SIZE_BYTES
        && width <= max_dimension
        && height <= max_dimension
        && is_browser_renderable_image_path(source_path)
}

fn image_dimensions(source_path: &Path) -> Result<(u32, u32), String> {
    let mut reader = ImageReader::open(source_path)
        .map_err(|error| format!("Failed to open image thumbnail source: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("Failed to detect image thumbnail format: {error}"))?;
    reader.limits(thumbnail_decode_limits());

    reader
        .into_dimensions()
        .map_err(|error| format!("Failed to read image thumbnail dimensions: {error}"))
}

fn decode_gif_first_frame(source_path: &Path) -> Result<image::DynamicImage, String> {
    let file = File::open(source_path)
        .map_err(|error| format!("Failed to open GIF thumbnail source: {error}"))?;
    let mut decoder = GifDecoder::new(BufReader::new(file))
        .map_err(|error| format!("Failed to read GIF thumbnail source: {error}"))?;

    decoder
        .set_limits(thumbnail_decode_limits())
        .map_err(|error| format!("Failed to set GIF thumbnail limits: {error}"))?;

    let (screen_width, screen_height) = decoder.dimensions();

    if screen_width == 0 || screen_height == 0 {
        return Err("GIF dimensions are invalid".to_string());
    }

    let mut buffer = vec![0u8; decoder.total_bytes() as usize];

    decoder
        .read_image(&mut buffer)
        .map_err(|error| format!("Failed to decode GIF thumbnail frame: {error}"))?;

    let rgba_image = image::RgbaImage::from_raw(screen_width, screen_height, buffer)
        .ok_or_else(|| "GIF frame canvas size is invalid".to_string())?;

    Ok(image::DynamicImage::ImageRgba8(rgba_image))
}

fn decode_image_thumbnail_source(source_path: &Path) -> Result<image::DynamicImage, String> {
    if is_gif_path(source_path) {
        return decode_gif_first_frame(source_path);
    }

    let mut reader = ImageReader::open(source_path)
        .map_err(|error| format!("Failed to open image thumbnail source: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("Failed to detect image thumbnail format: {error}"))?;
    reader.limits(thumbnail_decode_limits());

    reader
        .decode()
        .map_err(|error| format!("Failed to decode image thumbnail source: {error}"))
}

fn write_image_thumbnail(
    thumbnail: &image::DynamicImage,
    thumbnail_path: &Path,
) -> Result<(), String> {
    let file = File::create(thumbnail_path)
        .map_err(|error| format!("Failed to create image thumbnail: {error}"))?;
    let writer = BufWriter::new(file);
    let encoder = PngEncoder::new_with_quality(
        writer,
        IMAGE_THUMBNAIL_PNG_COMPRESSION,
        IMAGE_THUMBNAIL_PNG_FILTER,
    );

    thumbnail
        .write_with_encoder(encoder)
        .map_err(|error| format!("Failed to write image thumbnail: {error}"))
}

fn validate_video_thumbnail_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("Video thumbnail dimensions are invalid".to_string());
    }

    if width > THUMBNAIL_MAX_DIMENSION || height > THUMBNAIL_MAX_DIMENSION {
        return Err("Video thumbnail dimensions are too large".to_string());
    }

    Ok(())
}

fn decode_video_thumbnail_data_url(thumbnail_data_url: &str) -> Result<Vec<u8>, String> {
    let (metadata, encoded_thumbnail) = thumbnail_data_url
        .split_once(',')
        .ok_or_else(|| "Video thumbnail data URL is invalid".to_string())?;

    if metadata != "data:image/jpeg;base64" {
        return Err("Video thumbnail format is unsupported".to_string());
    }

    let thumbnail = BASE64_STANDARD
        .decode(encoded_thumbnail)
        .map_err(|error| format!("Failed to decode video thumbnail: {error}"))?;

    if thumbnail.is_empty() {
        return Err("Video thumbnail is empty".to_string());
    }

    if thumbnail.len() > MAX_VIDEO_THUMBNAIL_CACHE_BYTES {
        return Err("Video thumbnail is too large".to_string());
    }

    Ok(thumbnail)
}

fn get_cached_video_thumbnail_file(
    cache_dir: PathBuf,
    path: String,
    modified_time: u64,
    size: u64,
    width: u32,
    height: u32,
) -> Result<Option<String>, String> {
    validate_video_thumbnail_size(width, height)?;

    let thumbnail_path =
        video_thumbnail_cache_path(&cache_dir, &path, modified_time, size, width, height);

    if thumbnail_path.exists() {
        return Ok(Some(thumbnail_path.to_string_lossy().to_string()));
    }

    Ok(None)
}

fn cache_video_thumbnail_file(
    cache_dir: PathBuf,
    path: String,
    modified_time: u64,
    size: u64,
    width: u32,
    height: u32,
    thumbnail_data_url: String,
) -> Result<String, String> {
    validate_video_thumbnail_size(width, height)?;

    let source_metadata = fs::metadata(Path::new(&path))
        .map_err(|error| format!("Failed to read video metadata: {error}"))?;

    if !source_metadata.is_file() {
        return Err("Video thumbnail source is not a file".to_string());
    }

    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("Failed to create thumbnail cache directory: {error}"))?;

    let thumbnail_path =
        video_thumbnail_cache_path(&cache_dir, &path, modified_time, size, width, height);

    {
        let _cache_file_lock = IMAGE_THUMBNAIL_CACHE_FILE_LOCK
            .lock()
            .map_err(|error| format!("Failed to lock thumbnail cache files: {error}"))?;

        if should_enforce_thumbnail_cache_limits()? {
            enforce_thumbnail_cache_limits(&cache_dir)?;
        }

        if thumbnail_path.exists() {
            return Ok(thumbnail_path.to_string_lossy().to_string());
        }
    }

    let thumbnail = decode_video_thumbnail_data_url(&thumbnail_data_url)?;
    let temporary_path = temporary_thumbnail_path(&thumbnail_path);

    if let Err(error) = fs::write(&temporary_path, thumbnail) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!("Failed to write video thumbnail: {error}"));
    }

    {
        let _cache_file_lock = IMAGE_THUMBNAIL_CACHE_FILE_LOCK
            .lock()
            .map_err(|error| format!("Failed to lock thumbnail cache files: {error}"))?;

        if thumbnail_path.exists() {
            let _ = fs::remove_file(&temporary_path);
            return Ok(thumbnail_path.to_string_lossy().to_string());
        }

        if let Err(error) = fs::rename(&temporary_path, &thumbnail_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(format!("Failed to finalize video thumbnail: {error}"));
        }

        record_generated_thumbnail()?;
    }

    Ok(thumbnail_path.to_string_lossy().to_string())
}

fn generate_image_thumbnail_file(
    cache_dir: PathBuf,
    path: String,
    modified_time: u64,
    size: u64,
    max_dimension: u32,
) -> Result<String, String> {
    let source_path = Path::new(&path);
    let source_metadata = fs::metadata(source_path)
        .map_err(|error| format!("Failed to read image metadata: {error}"))?;

    if !source_metadata.is_file() {
        return Err("Thumbnail source is not a file".to_string());
    }

    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("Failed to create thumbnail cache directory: {error}"))?;

    let canonical_source_path = source_path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve image thumbnail source: {error}"))?;
    let canonical_cache_dir = cache_dir
        .canonicalize()
        .map_err(|error| format!("Failed to resolve thumbnail cache directory: {error}"))?;

    if path_is_same_or_descendant(&canonical_source_path, &canonical_cache_dir) {
        return Ok(canonical_source_path.to_string_lossy().to_string());
    }

    let thumbnail_path =
        thumbnail_cache_path(&cache_dir, &path, modified_time, size, max_dimension);

    {
        let _cache_file_lock = IMAGE_THUMBNAIL_CACHE_FILE_LOCK
            .lock()
            .map_err(|error| format!("Failed to lock thumbnail cache files: {error}"))?;

        if should_enforce_thumbnail_cache_limits()? {
            enforce_thumbnail_cache_limits(&cache_dir)?;
        }

        if thumbnail_path.exists() {
            return Ok(thumbnail_path.to_string_lossy().to_string());
        }
    }

    let (source_width, source_height) = image_dimensions(source_path)?;
    validate_image_thumbnail_source(source_metadata.len(), source_width, source_height)?;

    if should_use_original_image_thumbnail(
        source_path,
        source_metadata.len(),
        source_width,
        source_height,
        max_dimension,
    ) {
        return Ok(canonical_source_path.to_string_lossy().to_string());
    }

    // 解码大图是内存尖峰来源，多张同时解码峰值会叠加 ⇒ 走并发闸门。
    let image = {
        let _decode_slot = THUMBNAIL_DECODE_GATE.acquire();
        decode_image_thumbnail_source(source_path)?
    };
    let thumbnail = image.resize(max_dimension, max_dimension, IMAGE_THUMBNAIL_RESIZE_FILTER);
    let temporary_path = temporary_thumbnail_path(&thumbnail_path);

    if let Err(error) = write_image_thumbnail(&thumbnail, &temporary_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }

    {
        let _cache_file_lock = IMAGE_THUMBNAIL_CACHE_FILE_LOCK
            .lock()
            .map_err(|error| format!("Failed to lock thumbnail cache files: {error}"))?;

        if thumbnail_path.exists() {
            let _ = fs::remove_file(&temporary_path);
            return Ok(thumbnail_path.to_string_lossy().to_string());
        }

        if let Err(error) = fs::rename(&temporary_path, &thumbnail_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(format!("Failed to finalize image thumbnail: {error}"));
        }

        record_generated_thumbnail()?;
    }

    Ok(thumbnail_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn generate_image_thumbnail(
    app: tauri::AppHandle,
    path: String,
    modified_time: u64,
    size: u64,
    max_dimension: Option<u32>,
) -> Result<String, String> {
    let cache_dir = thumbnail_cache_dir(&app)?;
    let max_dimension = normalize_thumbnail_max_dimension(max_dimension);

    tauri::async_runtime::spawn_blocking(move || {
        generate_image_thumbnail_file(cache_dir, path, modified_time, size, max_dimension)
    })
    .await
    .map_err(|error| format!("Failed to generate image thumbnail: {error}"))?
}

#[tauri::command]
pub async fn get_thumbnail_cache_stats(
    app: tauri::AppHandle,
) -> Result<ThumbnailCacheStats, String> {
    let app_data_dir = crate::app_dirs::app_data_dir(&app)
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;

    tauri::async_runtime::spawn_blocking(move || {
        thumbnail_cache_stats_in_app_data_dir(&app_data_dir)
    })
    .await
    .map_err(|error| format!("Failed to read thumbnail cache stats: {error}"))?
}

#[tauri::command]
pub async fn clear_thumbnail_cache(app: tauri::AppHandle) -> Result<(), String> {
    let app_data_dir = crate::app_dirs::app_data_dir(&app)
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;

    tauri::async_runtime::spawn_blocking(move || {
        clear_thumbnail_cache_in_app_data_dir(&app_data_dir)
    })
    .await
    .map_err(|error| format!("Failed to clear thumbnail cache: {error}"))?
}

#[tauri::command]
pub async fn get_cached_video_thumbnail(
    app: tauri::AppHandle,
    path: String,
    modified_time: u64,
    size: u64,
    width: u32,
    height: u32,
) -> Result<Option<String>, String> {
    let cache_dir = thumbnail_cache_dir(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        get_cached_video_thumbnail_file(cache_dir, path, modified_time, size, width, height)
    })
    .await
    .map_err(|error| format!("Failed to get cached video thumbnail: {error}"))?
}

#[tauri::command]
pub async fn cache_video_thumbnail(
    app: tauri::AppHandle,
    path: String,
    modified_time: u64,
    size: u64,
    width: u32,
    height: u32,
    thumbnail_data_url: String,
) -> Result<String, String> {
    let cache_dir = thumbnail_cache_dir(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        cache_video_thumbnail_file(
            cache_dir,
            path,
            modified_time,
            size,
            width,
            height,
            thumbnail_data_url,
        )
    })
    .await
    .map_err(|error| format!("Failed to cache video thumbnail: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::{
        clear_thumbnail_cache_dir, clear_thumbnail_cache_in_app_data_dir,
        enforce_thumbnail_cache_limits, enforce_thumbnail_cache_limits_with_limits,
        generate_image_thumbnail_file, normalize_thumbnail_max_dimension,
        path_is_same_or_descendant, thumbnail_cache_key, thumbnail_cache_stats,
        thumbnail_cache_stats_in_app_data_dir, validate_image_thumbnail_source,
        ThumbnailDecodeGate, IMAGE_THUMBNAIL_CACHE_DIR,
        MAX_CONCURRENT_THUMBNAIL_DECODES, MAX_ORIGINAL_IMAGE_THUMBNAIL_SOURCE_SIZE_BYTES,
        MAX_THUMBNAIL_CACHE_ITEM_COUNT, MAX_THUMBNAIL_CACHE_SIZE_BYTES,
        MAX_THUMBNAIL_DECODE_ALLOCATION_BYTES, MAX_THUMBNAIL_SOURCE_FILE_SIZE_BYTES,
        MAX_THUMBNAIL_SOURCE_PIXELS, THUMBNAIL_MAX_DIMENSION,
    };
    use std::fs;
    use std::fs::File;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime};

    fn write_test_png(path: &Path, width: u32, height: u32) {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([32, 64, 96, 255]));
        image
            .save_with_format(path, image::ImageFormat::Png)
            .unwrap();
    }

    fn write_test_bmp(path: &Path, width: u32, height: u32) {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([32, 64, 96, 255]));
        image
            .save_with_format(path, image::ImageFormat::Bmp)
            .unwrap();
    }

    fn write_test_tiff(path: &Path, width: u32, height: u32) {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([32, 64, 96, 255]));
        image
            .save_with_format(path, image::ImageFormat::Tiff)
            .unwrap();
    }

    fn write_test_animated_gif(path: &Path, width: u32, height: u32, frame_count: u16) {
        use image::codecs::gif::{GifEncoder, Repeat};
        use image::ExtendedColorType;
        use std::io::BufWriter;

        let file = File::create(path).unwrap();
        let mut encoder = GifEncoder::new(BufWriter::new(file));
        encoder.set_repeat(Repeat::Infinite).unwrap();

        for frame_index in 0..frame_count {
            let red = frame_index.saturating_mul(40).min(255) as u8;
            let pixel_count = (width as usize) * (height as usize);
            let mut pixels = Vec::with_capacity(pixel_count * 3);

            for _ in 0..pixel_count {
                pixels.extend_from_slice(&[red, 0, 0]);
            }

            encoder
                .encode(&pixels, width, height, ExtendedColorType::Rgb8)
                .unwrap();
        }
    }

    #[test]
    fn thumbnail_cache_key_changes_when_metadata_changes() {
        let first_key = thumbnail_cache_key("C:/photos/image.jpg", 100, 200, 384);
        let second_key = thumbnail_cache_key("C:/photos/image.jpg", 101, 200, 384);
        let third_key = thumbnail_cache_key("C:/photos/image.jpg", 100, 201, 384);

        assert_ne!(first_key, second_key);
        assert_ne!(first_key, third_key);
    }

    #[test]
    fn thumbnail_dimension_is_clamped_to_backend_cap() {
        assert_eq!(
            normalize_thumbnail_max_dimension(None),
            THUMBNAIL_MAX_DIMENSION
        );
        assert_eq!(normalize_thumbnail_max_dimension(Some(1)), 1);
        assert_eq!(
            normalize_thumbnail_max_dimension(Some(0)),
            THUMBNAIL_MAX_DIMENSION
        );
        assert_eq!(
            normalize_thumbnail_max_dimension(Some(10_000)),
            THUMBNAIL_MAX_DIMENSION
        );
    }

    #[test]
    fn thumbnail_cache_descendant_paths_are_detected() {
        assert!(path_is_same_or_descendant(
            Path::new("/tmp/sigma/image-thumbnails/thumb.png"),
            Path::new("/tmp/sigma/image-thumbnails")
        ));
        assert!(path_is_same_or_descendant(
            Path::new("/tmp/sigma/image-thumbnails"),
            Path::new("/tmp/sigma/image-thumbnails")
        ));
        assert!(!path_is_same_or_descendant(
            Path::new("/tmp/sigma/photo.png"),
            Path::new("/tmp/sigma/image-thumbnails")
        ));
    }

    #[test]
    fn thumbnail_cache_images_return_source_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let thumbnail_path = temp_dir.path().join("thumb.png");
        fs::write(&thumbnail_path, b"not decoded").unwrap();

        let result = generate_image_thumbnail_file(
            temp_dir.path().to_path_buf(),
            thumbnail_path.to_string_lossy().to_string(),
            100,
            11,
            384,
        )
        .unwrap();

        assert_eq!(
            result,
            thumbnail_path.canonicalize().unwrap().to_string_lossy()
        );
    }

    #[test]
    fn small_thumbnail_sources_return_source_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_path = temp_dir.path().join("source.png");
        let cache_dir = temp_dir.path().join("cache");
        write_test_png(&source_path, 128, 128);

        let result = generate_image_thumbnail_file(
            cache_dir.clone(),
            source_path.to_string_lossy().to_string(),
            100,
            source_path.metadata().unwrap().len(),
            THUMBNAIL_MAX_DIMENSION,
        )
        .unwrap();

        assert_eq!(
            result,
            source_path.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(thumbnail_cache_stats(&cache_dir).unwrap().item_count, 0);
    }

    #[test]
    fn small_tiff_sources_generate_cached_png() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_path = temp_dir.path().join("source.tiff");
        let cache_dir = temp_dir.path().join("cache");
        write_test_tiff(&source_path, 128, 128);

        let result = generate_image_thumbnail_file(
            cache_dir.clone(),
            source_path.to_string_lossy().to_string(),
            100,
            source_path.metadata().unwrap().len(),
            THUMBNAIL_MAX_DIMENSION,
        )
        .unwrap();

        assert_ne!(
            result,
            source_path.canonicalize().unwrap().to_string_lossy()
        );
        assert!(result.ends_with(".png"));
        assert_eq!(thumbnail_cache_stats(&cache_dir).unwrap().item_count, 1);
    }

    #[test]
    fn small_dimension_large_sources_generate_cached_png() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_path = temp_dir.path().join("source.bmp");
        let cache_dir = temp_dir.path().join("cache");
        write_test_bmp(&source_path, 384, 384);
        assert!(
            source_path.metadata().unwrap().len() > MAX_ORIGINAL_IMAGE_THUMBNAIL_SOURCE_SIZE_BYTES
        );

        let result = generate_image_thumbnail_file(
            cache_dir.clone(),
            source_path.to_string_lossy().to_string(),
            100,
            source_path.metadata().unwrap().len(),
            THUMBNAIL_MAX_DIMENSION,
        )
        .unwrap();

        assert_ne!(
            result,
            source_path.canonicalize().unwrap().to_string_lossy()
        );
        assert!(result.ends_with(".png"));
        assert_eq!(thumbnail_cache_stats(&cache_dir).unwrap().item_count, 1);
    }

    #[test]
    fn small_gif_sources_generate_cached_png() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_path = temp_dir.path().join("source.gif");
        let cache_dir = temp_dir.path().join("cache");
        write_test_animated_gif(&source_path, 128, 128, 4);
        assert!(
            source_path.metadata().unwrap().len() <= MAX_ORIGINAL_IMAGE_THUMBNAIL_SOURCE_SIZE_BYTES
        );

        let result = generate_image_thumbnail_file(
            cache_dir.clone(),
            source_path.to_string_lossy().to_string(),
            100,
            source_path.metadata().unwrap().len(),
            THUMBNAIL_MAX_DIMENSION,
        )
        .unwrap();

        assert_ne!(
            result,
            source_path.canonicalize().unwrap().to_string_lossy()
        );
        assert!(result.ends_with(".png"));
        assert_eq!(thumbnail_cache_stats(&cache_dir).unwrap().item_count, 1);
    }

    #[test]
    fn thumbnail_source_size_is_limited() {
        let result =
            validate_image_thumbnail_source(MAX_THUMBNAIL_SOURCE_FILE_SIZE_BYTES + 1, 100, 100);

        assert!(result.is_err());
    }

    #[test]
    fn thumbnail_source_dimensions_are_limited() {
        let oversized_height = (MAX_THUMBNAIL_SOURCE_PIXELS / 100) + 1;
        let result = validate_image_thumbnail_source(1024, 100, oversized_height as u32);

        assert!(result.is_err());
    }

    /// 写一个带指定修改时间的缓存文件。
    ///
    /// 淘汰按 mtime 排序，所以测试必须能控制每个文件的「新旧」，
    /// 否则只能靠 sleep 排序，既慢又不稳定。
    fn write_cache_file_with_modified_time(path: &Path, contents: &[u8], modified_seconds: u64) {
        fs::write(path, contents).unwrap();

        let file = fs::OpenOptions::new().write(true).open(path).unwrap();
        let modified_time = SystemTime::UNIX_EPOCH + Duration::from_secs(modified_seconds);
        file.set_times(fs::FileTimes::new().set_modified(modified_time))
            .unwrap();
    }

    /// 超限时**淘汰最旧的**，而不是清空整个目录。
    ///
    /// 这条是 2026-10-28 那次「超限全清」改动的守门测试：
    /// 如果哪天又退回整体清空，这里会直接红。
    #[test]
    fn thumbnail_cache_evicts_oldest_items_instead_of_clearing() {
        let temp_dir = tempfile::tempdir().unwrap();

        for item_index in 0..5u64 {
            write_cache_file_with_modified_time(
                &temp_dir.path().join(format!("{item_index}.png")),
                b"thumb",
                1_600_000_000 + item_index,
            );
        }

        enforce_thumbnail_cache_limits_with_limits(temp_dir.path(), 3, u64::MAX).unwrap();

        let stats = thumbnail_cache_stats(temp_dir.path()).unwrap();
        assert_eq!(
            stats.item_count, 3,
            "应只淘汰到限额内，而不是清空整个缓存目录"
        );
        assert!(
            !temp_dir.path().join("0.png").exists(),
            "最旧的条目应被淘汰"
        );
        assert!(
            !temp_dir.path().join("1.png").exists(),
            "次旧的条目应被淘汰"
        );
        assert!(
            temp_dir.path().join("2.png").exists(),
            "较新的条目应被保留"
        );
        assert!(
            temp_dir.path().join("3.png").exists(),
            "较新的条目应被保留"
        );
        assert!(
            temp_dir.path().join("4.png").exists(),
            "最新的条目应被保留"
        );
    }

    /// 字节数超限时同样按最旧优先淘汰。
    #[test]
    fn thumbnail_cache_evicts_oldest_items_above_size_limit() {
        let temp_dir = tempfile::tempdir().unwrap();

        for item_index in 0..4u64 {
            write_cache_file_with_modified_time(
                &temp_dir.path().join(format!("{item_index}.png")),
                &[0u8; 100],
                1_600_000_000 + item_index,
            );
        }

        // 4 × 100 B = 400 B，限额 250 B ⇒ 必须淘汰掉最旧的两个。
        enforce_thumbnail_cache_limits_with_limits(temp_dir.path(), usize::MAX, 250).unwrap();

        let stats = thumbnail_cache_stats(temp_dir.path()).unwrap();
        assert_eq!(stats.item_count, 2, "应淘汰到字节限额内");
        assert_eq!(stats.size_bytes, 200, "应淘汰到字节限额内");
        assert!(
            !temp_dir.path().join("0.png").exists(),
            "最旧的条目应被淘汰"
        );
        assert!(
            !temp_dir.path().join("1.png").exists(),
            "次旧的条目应被淘汰"
        );
        assert!(
            temp_dir.path().join("2.png").exists(),
            "较新的条目应被保留"
        );
        assert!(
            temp_dir.path().join("3.png").exists(),
            "较新的条目应被保留"
        );
    }

    /// 没超限时一个文件都不能动。
    #[test]
    fn thumbnail_cache_keeps_everything_within_limits() {
        let temp_dir = tempfile::tempdir().unwrap();

        for item_index in 0..3u64 {
            write_cache_file_with_modified_time(
                &temp_dir.path().join(format!("{item_index}.png")),
                b"thumb",
                1_600_000_000 + item_index,
            );
        }

        enforce_thumbnail_cache_limits_with_limits(temp_dir.path(), 3, 300).unwrap();

        let stats = thumbnail_cache_stats(temp_dir.path()).unwrap();
        assert_eq!(stats.item_count, 3, "限额内的条目不应被淘汰");
        for item_index in 0..3u64 {
            assert!(
                temp_dir.path().join(format!("{item_index}.png")).exists(),
                "限额内的条目不应被淘汰"
            );
        }
    }

    /// 生产入口（走真实限额）在没超限时不应动任何文件。
    #[test]
    fn thumbnail_cache_production_limits_keep_small_caches_intact() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_cache_file_with_modified_time(
            &temp_dir.path().join("a.png"),
            b"thumb",
            1_600_000_000,
        );

        enforce_thumbnail_cache_limits(temp_dir.path()).unwrap();

        let stats = thumbnail_cache_stats(temp_dir.path()).unwrap();
        assert_eq!(stats.item_count, 1);
        assert!(
            temp_dir.path().join("a.png").exists(),
            "限额内的条目不应被淘汰"
        );
    }

    /// 限额常量不得被悄悄放宽。
    #[test]
    fn thumbnail_cache_limits_stay_tightened() {
        assert_eq!(MAX_THUMBNAIL_CACHE_ITEM_COUNT, 2000);
        assert_eq!(MAX_THUMBNAIL_CACHE_SIZE_BYTES, 256 * 1024 * 1024);
        assert_eq!(MAX_THUMBNAIL_DECODE_ALLOCATION_BYTES, 64 * 1024 * 1024);
        assert_eq!(
            MAX_THUMBNAIL_SOURCE_PIXELS * 4,
            MAX_THUMBNAIL_DECODE_ALLOCATION_BYTES,
            "像素上限应与解码分配上限对齐（每像素 4 字节 RGBA）"
        );
    }

    /// 解码并发闸门：同时解码的张数不得超过上限。
    #[test]
    fn thumbnail_decode_gate_caps_concurrent_decodes() {
        let gate = Arc::new(ThumbnailDecodeGate::new());
        let active_decodes = Arc::new(AtomicUsize::new(0));
        let peak_decodes = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..MAX_CONCURRENT_THUMBNAIL_DECODES * 4)
            .map(|_| {
                let gate = Arc::clone(&gate);
                let active_decodes = Arc::clone(&active_decodes);
                let peak_decodes = Arc::clone(&peak_decodes);

                thread::spawn(move || {
                    let _decode_slot = gate.acquire();

                    let current = active_decodes.fetch_add(1, Ordering::SeqCst) + 1;
                    peak_decodes.fetch_max(current, Ordering::SeqCst);

                    thread::sleep(Duration::from_millis(50));

                    active_decodes.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let peak = peak_decodes.load(Ordering::SeqCst);
        assert!(
            peak <= MAX_CONCURRENT_THUMBNAIL_DECODES,
            "并发解码峰值 {peak} 超过了上限 {MAX_CONCURRENT_THUMBNAIL_DECODES}"
        );
        assert!(
            peak > 1,
            "闸门应当允许多张同时解码，实际峰值 {peak}"
        );
    }

    #[test]
    fn thumbnail_cache_stats_counts_files_and_bytes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.png"), b"12345").unwrap();
        std::fs::write(dir.path().join("b.png"), b"678").unwrap();

        let stats = thumbnail_cache_stats(dir.path()).unwrap();
        assert_eq!(stats.item_count, 2);
        assert_eq!(stats.size_bytes, 8);
    }

    #[test]
    fn clear_thumbnail_cache_dir_empties_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.png"), b"12345").unwrap();

        clear_thumbnail_cache_dir(dir.path()).unwrap();

        let remaining = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(remaining, 0, "清理后目录应为空");
    }

    #[test]
    fn clear_thumbnail_cache_dir_only_touches_cache_dir() {
        let app_data_dir = tempfile::tempdir().unwrap();
        let cache_dir = app_data_dir.path().join(IMAGE_THUMBNAIL_CACHE_DIR);
        let user_file = app_data_dir.path().join("user-notes.txt");
        let settings_file = app_data_dir.path().join("settings.json");

        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("thumb.png"), b"12345").unwrap();
        fs::write(&user_file, b"do not touch").unwrap();
        fs::write(&settings_file, b"{}").unwrap();

        clear_thumbnail_cache_dir(&cache_dir).unwrap();

        assert!(
            app_data_dir.path().exists(),
            "缓存目录之外的父目录不应被清理，实际已被删除"
        );
        assert!(
            user_file.exists(),
            "缓存目录之外的同级文件不应被清理，实际已被删除"
        );
        assert!(
            settings_file.exists(),
            "缓存目录之外的同级文件不应被清理，实际已被删除"
        );
        assert_eq!(
            fs::read(&user_file).unwrap(),
            b"do not touch",
            "缓存目录之外的文件内容不应被改动"
        );
        assert_eq!(
            fs::read_dir(&cache_dir).unwrap().count(),
            0,
            "缓存目录应被清空"
        );
    }

    /// 设置页的清理按钮最终会调用 `clear_thumbnail_cache` command，
    /// 它转发到 `clear_thumbnail_cache_in_app_data_dir`。
    /// 这条测试盯的就是那次转发：**必须**只把 cache_dir 交给清理函数。
    /// 如果哪天改成把 app_data_dir（父目录）传下去，这个测试会红。
    #[test]
    fn clear_thumbnail_cache_in_app_data_dir_only_clears_cache_subdirectory() {
        let app_data_dir = tempfile::tempdir().unwrap();
        let cache_dir = app_data_dir.path().join(IMAGE_THUMBNAIL_CACHE_DIR);
        let settings_file = app_data_dir.path().join("settings.json");
        let extension_dir = app_data_dir.path().join("extensions");

        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("thumb.png"), b"12345").unwrap();
        fs::write(cache_dir.join("video-thumb.jpg"), b"12345").unwrap();
        fs::write(&settings_file, b"{}").unwrap();
        fs::create_dir_all(&extension_dir).unwrap();
        fs::write(extension_dir.join("ext.marker"), b"keep me").unwrap();

        clear_thumbnail_cache_in_app_data_dir(app_data_dir.path()).unwrap();

        // 先断言「不该被删的东西」，再断言「该被删的东西」。
        // 顺序反过来的话，缺陷注入时第一个失败会是一条 unwrap 的 NotFound，
        // 看不出到底损坏成什么样。
        assert!(
            settings_file.exists(),
            "command 转发时若把父目录交给清理函数，settings.json 会被删掉"
        );
        assert!(
            extension_dir.exists(),
            "command 转发时若把父目录交给清理函数，extensions 目录会被删掉"
        );
        assert_eq!(
            fs::read(extension_dir.join("ext.marker")).unwrap(),
            b"keep me",
            "缓存目录之外的兄弟目录内容不应被改动"
        );
        assert_eq!(
            fs::read_dir(&cache_dir).unwrap().count(),
            0,
            "缓存目录应被清空"
        );
    }

    /// 设置页显示的占用大小来自 `get_thumbnail_cache_stats` command。
    /// 同样的转发风险：统计范围必须限定在 cache_dir 内，
    /// 否则设置页会把用户设置文件的大小算进「缩略图缓存占用」。
    #[test]
    fn thumbnail_cache_stats_in_app_data_dir_counts_only_cache_subdirectory() {
        let app_data_dir = tempfile::tempdir().unwrap();
        let cache_dir = app_data_dir.path().join(IMAGE_THUMBNAIL_CACHE_DIR);

        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("thumb.png"), b"12345").unwrap();
        fs::write(cache_dir.join("video-thumb.jpg"), b"12345").unwrap();
        fs::write(app_data_dir.path().join("settings.json"), b"x").unwrap();

        let stats = thumbnail_cache_stats_in_app_data_dir(app_data_dir.path()).unwrap();

        assert_eq!(stats.item_count, 2, "统计只应包含缓存目录内的两个缩略图");
        assert_eq!(
            stats.size_bytes, 10,
            "统计只应包含缓存目录内的字节数，不应包含 settings.json"
        );
    }

    /// 缓存目录尚未创建时（用户从没浏览过图片）command 不应报错。
    #[test]
    fn thumbnail_cache_in_app_data_dir_handles_missing_cache_dir() {
        let app_data_dir = tempfile::tempdir().unwrap();
        let settings_file = app_data_dir.path().join("settings.json");
        fs::write(&settings_file, b"{}").unwrap();

        let stats = thumbnail_cache_stats_in_app_data_dir(app_data_dir.path()).unwrap();
        assert_eq!(stats.item_count, 0);
        assert_eq!(stats.size_bytes, 0);

        // 缓存目录不存在时清理应当是安全的空操作。
        clear_thumbnail_cache_in_app_data_dir(app_data_dir.path()).unwrap();
        assert!(
            settings_file.exists(),
            "缓存目录不存在时清理不应删除任何东西"
        );
    }
}
