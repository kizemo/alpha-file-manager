// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 远程凭据的 DPAPI 加解密与落盘。
//!
//! 本文件被 `cargo test --lib` 的测试二进制链接，**不得出现 `tauri::` 的任何路径** ——
//! 理由见 `src-tauri/src/remote/mod.rs` 顶部。
//!
//! 密文走**独立文件**（`<app_data_dir>/remote-credentials.json`），不进 storage 文件链。
//! 两条链混用会绕过前端 schema 迁移与读白名单 —— 本项目已经因为
//! 「读写两侧不同步导致重启丢数据」吃过一次亏。

use crate::remote::error::{RemoteError, RemoteErrorKind};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 凭据文件名。落在应用自己的 app-data 目录里，与 `mcp-access.json` 平级。
pub const REMOTE_CREDENTIALS_FILE: &str = "remote-credentials.json";

/// 存档格式版本。读到不认识的版本按「没有凭据」处理 ——
/// 宁可让用户重输一次密码，也不要拿一份自己读不懂的存档去猜。
///
/// ⚠️ 「按没有凭据处理」只适用于**读**；**写**之前必须先把那份读不懂的文件备份下来，
/// 否则下一次保存就会把库里其他凭据静默清空。见 [`vault_for_write`]。
const VAULT_VERSION: u32 = 1;

/// `remote-credentials.json` 的落盘形状。
///
/// 用 `BTreeMap` 而不是 `HashMap`：存档顺序稳定，写同一个键两次得到同一份文件，
/// 差异才好排查。值是 base64 编码的 DPAPI 密文，**不是密码本身**。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialVault {
    version: u32,
    #[serde(default)]
    entries: BTreeMap<String, String>,
}

impl CredentialVault {
    fn empty() -> Self {
        Self {
            version: VAULT_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

pub fn credentials_path_in(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(REMOTE_CREDENTIALS_FILE)
}

/// 读一次存档可能遇到的**三种情形**。
///
/// 把「文件不存在」和「文件在但读不出来」分开，就是这次加固的全部内容。
/// 从前两者一律返回空库，于是紧接着的那次 `save_credential` 会拿着空库写回 ——
/// 结果是**库里其他全部凭据被静默删掉，且原文件已被覆盖、无从找回**
/// （真机上已经因此丢过三条远程连接的密文）。
enum VaultRead {
    /// 文件不存在 —— 真的一个凭据都没存过，正常的干净状态。
    Absent,
    /// 读到了认得的版本。
    Loaded(CredentialVault),
    /// 文件在，但读不出来：字节读不进去 / 不是 UTF-8 / 解析失败 / 版本不认识。
    /// **这不等于「没有凭据」**，写回之前必须先把原文件留成证据。
    Unreadable { reason: String },
}

/// 只看磁盘，不写任何东西。
fn inspect_vault(path: &Path) -> VaultRead {
    let raw = match std::fs::read(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return VaultRead::Absent,
        Err(error) => {
            return VaultRead::Unreadable {
                reason: format!("读取失败: {error}"),
            }
        }
        Ok(raw) => raw,
    };

    // 用 read + from_utf8 而不是 read_to_string：后者把「不存在」和「不是 UTF-8」
    // 混成同一个错误，而这两者的处置完全不同。
    let Ok(text) = String::from_utf8(raw) else {
        return VaultRead::Unreadable {
            reason: "文件不是合法的 UTF-8 文本".to_string(),
        };
    };

    match serde_json::from_str::<CredentialVault>(&text) {
        Ok(vault) if vault.version == VAULT_VERSION => VaultRead::Loaded(vault),
        Ok(vault) => VaultRead::Unreadable {
            reason: format!(
                "格式版本是 {}，本版本只认 {VAULT_VERSION}",
                vault.version
            ),
        },
        Err(error) => VaultRead::Unreadable {
            reason: format!("无法解析: {error}"),
        },
    }
}

/// 只读路径（[`load_credential`]）用的存档读取：**文件缺失、解析失败、版本不认识，
/// 三者都交出空库** —— 与 `mcp/auth.rs` 的既有策略一致，宁可退回干净状态，
/// 也不要拿半截文件推导状态。读路径不改磁盘，所以坏文件就地留着。
///
/// 要写回请走 [`vault_for_write`]，它先把坏文件备份下来。
fn read_vault(app_data_dir: &Path) -> CredentialVault {
    let path = credentials_path_in(app_data_dir);
    match inspect_vault(&path) {
        VaultRead::Loaded(vault) => vault,
        VaultRead::Absent => CredentialVault::empty(),
        VaultRead::Unreadable { reason } => {
            log::warn!(
                "Treating unreadable credential vault {} as empty: {reason}",
                path.display()
            );
            CredentialVault::empty()
        }
    }
}

/// 写回路径用的存档读取。文件存在却读不出来时，**先把原文件按字节备份**再交出空库；
/// 备份落不下来就报错中止本次写入 —— 宁可让用户这次存不上密码，
/// 也绝不再制造一次「整库被清空且无备份」。
fn vault_for_write(app_data_dir: &Path) -> Result<CredentialVault, RemoteError> {
    let path = credentials_path_in(app_data_dir);
    match inspect_vault(&path) {
        VaultRead::Loaded(vault) => Ok(vault),
        VaultRead::Absent => Ok(CredentialVault::empty()),
        VaultRead::Unreadable { reason } => {
            archive_unreadable_vault(app_data_dir, &reason)?;
            Ok(CredentialVault::empty())
        }
    }
}

/// 进程内递增序号，与毫秒时间戳、进程号一起构成文件名的唯一后缀。
static UNIQUENESS_SEQ: AtomicU64 = AtomicU64::new(0);

/// 唯一后缀 `<毫秒>-<进程号>-<序号>`：序号防本进程内撞车，
/// 进程号防跨进程撞车（保存密码与删除凭据可能同时在跑），
/// 毫秒让文件名按时间可读、可按名字排出先后。
fn uniqueness_stamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = UNIQUENESS_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{millis}-{}-{sequence}", std::process::id())
}

/// 在 `vault_path` 旁边挑一个尚未占用的文件名。
/// `name_for` 负责把唯一后缀拼进文件名。
fn unused_path_beside(vault_path: &Path, name_for: impl Fn(String) -> String) -> PathBuf {
    let dir = vault_path.parent().unwrap_or(Path::new("."));
    for _ in 0..1_000 {
        let candidate = dir.join(name_for(uniqueness_stamp()));
        if !candidate.exists() {
            return candidate;
        }
    }
    // 走到这里说明同名被占了 1000 次，属于异常环境；交回最后一个候选，
    // 由 `create_new` 把它变成明确错误，而不是悄悄覆盖别人的文件。
    dir.join(name_for(uniqueness_stamp()))
}

/// 原子写的临时文件名：`remote-credentials.json.tmp-<唯一后缀>`。
fn temp_path_for(vault_path: &Path) -> PathBuf {
    let file_name = vault_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(REMOTE_CREDENTIALS_FILE);
    unused_path_beside(vault_path, |stamp| format!("{file_name}.tmp-{stamp}"))
}

/// 坏文件的备份名：`remote-credentials.corrupt-<唯一后缀>.json.bak`。
/// 带 `corrupt` 与 `.bak` 两头标记，一眼能看出是「读不出来的凭据库」而不是现役存档。
fn corrupt_backup_path_for(vault_path: &Path) -> PathBuf {
    let stem = vault_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(REMOTE_CREDENTIALS_FILE);
    unused_path_beside(vault_path, |stamp| {
        format!("{stem}.corrupt-{stamp}.json.bak")
    })
}

/// 是否是本模块产出的坏文件备份。
fn is_corrupt_backup_name(name: &str) -> bool {
    let stem = REMOTE_CREDENTIALS_FILE
        .split_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(REMOTE_CREDENTIALS_FILE);
    name.starts_with(&format!("{stem}.corrupt-")) && name.ends_with(".json.bak")
}

/// 把读不出来的凭据库**按字节**复制到同目录的备份文件里。
///
/// 不解析、不抢救、不删原文件：这份文件里全是 DPAPI 密文，只有本机当前用户解得开，
/// 我们能做的只有把字节留住，给用户留一条「重装/换机之前捞回来」的路。
///
/// 判重：内容逐字节相同的备份已经存在就不再新建 —— 反复失败的写回不会把目录堆成垃圾。
/// 代价是同一目录里两份**不同**的坏存档会各留一份备份，这是想要的行为，不是浪费。
fn archive_unreadable_vault(app_data_dir: &Path, reason: &str) -> Result<PathBuf, RemoteError> {
    let path = credentials_path_in(app_data_dir);
    let raw = std::fs::read(&path).map_err(|error| {
        RemoteError::unknown(format!(
            "远程凭据文件读不出来（{reason}），且备份也失败了：{error}。 \
             为免丢东西，本次保存已中止；请先手动把 {} 改名，再重新保存密码。",
            path.display()
        ))
    })?;

    for existing in list_corrupt_backups(app_data_dir) {
        let same_bytes = std::fs::read(&existing).map(|bytes| bytes == raw).unwrap_or(false);
        if same_bytes {
            log::warn!(
                "Credential vault {} is still unreadable and already archived to {}; not duplicating the backup",
                path.display(),
                existing.display()
            );
            return Ok(existing);
        }
    }

    let backup = corrupt_backup_path_for(&path);
    write_bytes_durable(&backup, &raw, "远程凭据备份").map_err(|error| {
        let _ = std::fs::remove_file(&backup);
        error
    })?;

    // 备份里同样是密文，DACL 收不留；失败只记警告，与现役存档一个口径。
    crate::mcp::auth::restrict_to_current_user(&backup, "remote credential backup");

    // 证据没落全就覆盖原件，等于什么都没加固。先核实字节数再继续。
    let written = std::fs::metadata(&backup)
        .map_err(|error| {
            RemoteError::unknown(format!("无法确认远程凭据备份是否写完: {error}"))
        })?
        .len();
    if written != raw.len() as u64 {
        return Err(RemoteError::unknown(format!(
            "远程凭据备份只落下 {written}/{} 字节，本次保存已中止",
            raw.len()
        )));
    }

    log::error!(
        "Credential vault {} is unreadable ({reason}); the original bytes were archived to {} before it is replaced",
        path.display(),
        backup.display()
    );
    Ok(backup)
}

/// 列出现有的坏文件备份。目录读不了就当没有 —— 调用方接下来会自己新建。
fn list_corrupt_backups(app_data_dir: &Path) -> Vec<PathBuf> {
    let Ok(read_dir) = std::fs::read_dir(app_data_dir) else {
        return Vec::new();
    };

    read_dir
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_corrupt_backup_name)
        })
        .collect()
}

/// 建文件、写满、`sync_all` 落盘。**用 `create_new`**：唯一名如果已被占用，
/// 报错比覆盖别人的文件诚实。
fn write_bytes_durable(path: &Path, bytes: &[u8], label: &str) -> Result<(), RemoteError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| RemoteError::unknown(format!("创建{label}文件失败: {error}")))?;

    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| RemoteError::unknown(format!("写入{label}文件失败: {error}")))?;

    Ok(())
}

/// 落盘：**同目录写临时文件 → 落盘 → rename 覆盖目标**。
///
/// 原来直接 `std::fs::write` 覆盖现役文件：进程中途被杀或磁盘满就留下半截 JSON，
/// 而半截 JSON 会让下一次保存读出一座空库 —— 截断这个诱因本身，加上
/// 「读不出来就当没有」这个放大器，一起构成了整库被清空的成因。
/// 改名是目录项级操作，读者只会看到「旧的完整文件」或「新的完整文件」，不会看到中间态。
fn write_vault(app_data_dir: &Path, vault: &CredentialVault) -> Result<(), RemoteError> {
    std::fs::create_dir_all(app_data_dir)
        .map_err(|error| RemoteError::unknown(format!("创建远程凭据目录失败: {error}")))?;

    let payload = serde_json::to_string_pretty(vault)
        .map_err(|error| RemoteError::unknown(format!("序列化远程凭据失败: {error}")))?;

    let path = credentials_path_in(app_data_dir);
    let temp = temp_path_for(&path);

    write_bytes_durable(&temp, payload.as_bytes(), "远程凭据临时文件").map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        error
    })?;

    // DPAPI 已经把同机其他账号挡住了，收 DACL 是再加一层。
    // 临时文件也要收：进程在 rename 之前崩掉，留在盘上的就是一份带密文的临时文件。
    // 这一步失败只记警告 —— 收紧权限是加固，不该因为它把「保存密码」整个挡掉。
    crate::mcp::auth::restrict_to_current_user(&temp, "remote credential");

    std::fs::rename(&temp, &path).map_err(|error| {
        // rename 失败必须清掉临时文件，否则目录里会攒出一堆带密文的孤儿。
        let _ = std::fs::remove_file(&temp);
        RemoteError::unknown(format!("提交远程凭据文件失败: {error}"))
    })?;

    crate::mcp::auth::restrict_to_current_user(&path, "remote credential");
    Ok(())
}

fn validate_key(key: &str) -> Result<(), RemoteError> {
    if key.trim().is_empty() {
        return Err(RemoteError::protocol("凭据键不能为空"));
    }
    Ok(())
}

/// 用当前用户的 DPAPI 加密。
///
/// **必须**带 `CRYPTPROTECT_UI_FORBIDDEN`：少了它，Windows 可以在任何时候弹 UI 索要凭据，
/// 而这是一条无人值守的后台路径，弹窗既没人看也挡不住。
#[cfg(windows)]
pub fn protect(plaintext: &[u8]) -> Result<Vec<u8>, RemoteError> {
    use windows::core::PCWSTR;
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let length = u32::try_from(plaintext.len())
        .map_err(|_| RemoteError::protocol("待加密的内容超过 DPAPI 的长度上限"))?;

    let input = CRYPT_INTEGER_BLOB {
        cbData: length,
        // DPAPI 只读这块内存。这里抹掉 const 纯粹是为了对上 Win32 的签名。
        pbData: plaintext.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    let result = unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    result.map_err(|error| RemoteError::auth(format!("DPAPI 加密失败: {error}")))?;
    let bytes = take_dpapi_blob(output.pbData, output.cbData);
    free_dpapi_blob(output.pbData);

    Ok(bytes)
}

#[cfg(not(windows))]
pub fn protect(_plaintext: &[u8]) -> Result<Vec<u8>, RemoteError> {
    Err(RemoteError::unsupported(
        "凭据加密目前只支持 Windows（DPAPI），当前平台无法保存密码",
    ))
}

/// 解 DPAPI 密文。
///
/// 解不开**归 `Auth` 而不是 `Unknown`**：成因几乎都是「这份密文不是当前用户写的」
/// （用户换了、数据目录被复制过来、文件被手改），用户侧的处置都是重新输入密码。
#[cfg(windows)]
pub fn unprotect(ciphertext: &[u8]) -> Result<Vec<u8>, RemoteError> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let length = u32::try_from(ciphertext.len())
        .map_err(|_| RemoteError::auth("已保存的凭据长度不合法，请重新输入密码"))?;

    let input = CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: ciphertext.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    let result = unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    result.map_err(|error| {
        RemoteError::auth(format!("已保存的凭据无法解密，请重新输入密码: {error}"))
    })?;
    let bytes = take_dpapi_blob(output.pbData, output.cbData);
    free_dpapi_blob(output.pbData);

    Ok(bytes)
}

#[cfg(not(windows))]
pub fn unprotect(_ciphertext: &[u8]) -> Result<Vec<u8>, RemoteError> {
    Err(RemoteError::unsupported(
        "凭据解密目前只支持 Windows（DPAPI），当前平台无法读取已保存的密码",
    ))
}

/// DPAPI 分配的缓冲区由 `LocalFree` 释放，不是 Rust 的 `Vec`。
#[cfg(windows)]
fn free_dpapi_blob(pointer: *mut u8) {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};

    if pointer.is_null() {
        return;
    }

    unsafe {
        let _returned = LocalFree(HLOCAL(pointer as *mut core::ffi::c_void));
    }
}

/// 拷出 DPAPI 的输出。`cbData == 0` 时**不能**调 `from_raw_parts` ——
/// 那要求指针非空且对齐，拿一个可能为 null 的指针去构造零长切片是 UB。
#[cfg(windows)]
fn take_dpapi_blob(pointer: *mut u8, length: u32) -> Vec<u8> {
    if length == 0 || pointer.is_null() {
        return Vec::new();
    }

    unsafe { std::slice::from_raw_parts(pointer, length as usize) }.to_vec()
}

/// 读一个已保存的密码。没有就是 `Ok(None)`。
///
/// **解不开时报错，不悄悄当成「没存」。** 悄悄吞掉的话，用户会用空密码去连服务器，
/// 拿回来的是一个更迷惑的远端 `Auth` 错误 —— 那时候没人查得到是这个文件坏了。
pub fn load_credential(app_data_dir: &Path, key: &str) -> Result<Option<String>, RemoteError> {
    validate_key(key)?;

    let vault = read_vault(app_data_dir);
    let Some(encoded) = vault.entries.get(key) else {
        return Ok(None);
    };

    let ciphertext = BASE64_STANDARD.decode(encoded).map_err(|error| {
        RemoteError::auth(format!(
            "已保存的凭据不是有效的密文，请重新输入密码: {error}"
        ))
    })?;

    let plaintext = unprotect(&ciphertext)?;

    String::from_utf8(plaintext)
        .map(Some)
        .map_err(|_| RemoteError::auth("已保存的凭据不是有效文本，请重新输入密码"))
}

/// 加密并保存一个密码。同键再次调用会**替换**旧值。
///
/// 走 [`vault_for_write`]：现有的存档如果读不出来，先把原文件按字节备份，
/// 这次写入才允许进行。
pub fn save_credential(app_data_dir: &Path, key: &str, secret: &str) -> Result<(), RemoteError> {
    validate_key(key)?;

    let ciphertext = protect(secret.as_bytes())?;
    let mut vault = vault_for_write(app_data_dir)?;
    vault
        .entries
        .insert(key.to_string(), BASE64_STANDARD.encode(&ciphertext));

    write_vault(app_data_dir, &vault)
}

/// 删掉一个已保存的密码。键不存在是**成功的空操作**，不是错误 ——
/// 删除动作的调用方不该因为「本来就没有」而中断。
///
/// 这里刻意走只读的 [`read_vault`] 而不是 [`vault_for_write`]，不是漏改：
/// 存档读不出来时 `read_vault` 交出空库，于是键必然不存在、直接 `Ok(())` 返回、**根本不写盘** ——
/// 坏文件原样留在原地，没有任何东西被覆盖，也就不需要备份。
/// 反过来若改成 `vault_for_write`，删一个不存在的键反而会产出备份并把空库写回去，
/// 把「删不掉」变成「顺手清空」，正是这次加固要消灭的行为。
pub fn delete_credential(app_data_dir: &Path, key: &str) -> Result<(), RemoteError> {
    validate_key(key)?;

    let mut vault = read_vault(app_data_dir);
    if vault.entries.remove(key).is_none() {
        return Ok(());
    }

    write_vault(app_data_dir, &vault)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use tempfile::TempDir;

    /// 一个 1 MiB 的确定性填充块。DPAPI 没有长度上限，
    /// 这一条防的是「实现里写了个人为的长度上限」。
    fn megabyte() -> Vec<u8> {
        (0..1024 * 1024).map(|index| (index % 251) as u8).collect()
    }

    fn temp_dir() -> TempDir {
        TempDir::new().expect("create temp dir")
    }

    // ---- protect / unprotect -------------------------------------------------

    /// 阳性对照：如果这一条不成立，下面所有「round-trip 通过」都可能只是
    /// `unprotect` 把输入原样返回了。
    #[test]
    fn protect_produces_ciphertext_that_is_not_the_plaintext() {
        let plaintext = b"correct horse battery staple";

        let ciphertext = protect(plaintext).expect("protect");

        assert_ne!(ciphertext.as_slice(), plaintext);
        assert!(
            !contains_subslice(&ciphertext, plaintext),
            "密文里不能出现明文片段"
        );
        assert!(
            ciphertext.len() >= plaintext.len(),
            "DPAPI 会带头部与签名，长度不该比明文短"
        );
    }

    #[test]
    fn protect_and_unprotect_round_trip() {
        // 中文、emoji、空格混排 —— 密码里出现非 ASCII 是常态，不是边界情况。
        let plaintext = "p@ss 中文🔐 word".as_bytes();

        let ciphertext = protect(plaintext).expect("protect");
        let recovered = unprotect(&ciphertext).expect("unprotect");

        assert_eq!(recovered.as_slice(), plaintext);
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let ciphertext = protect(b"").expect("protect");

        assert_eq!(unprotect(&ciphertext).expect("unprotect"), Vec::<u8>::new());
    }

    #[test]
    fn large_plaintext_round_trips() {
        let plaintext = megabyte();

        let ciphertext = protect(&plaintext).expect("protect");
        let recovered = unprotect(&ciphertext).expect("unprotect");

        assert_eq!(recovered.len(), plaintext.len());
        assert_eq!(recovered.as_slice(), plaintext.as_slice());
    }

    #[test]
    fn the_same_plaintext_encrypts_to_different_ciphertext_each_time() {
        // DPAPI 带随机熵。不锁住这一条的话，「密文相同就说明没加密」这种误判会一直存在。
        let plaintext = b"same input";

        let first = protect(plaintext).expect("first protect");
        let second = protect(plaintext).expect("second protect");

        assert_ne!(first, second);
        assert_eq!(unprotect(&first).expect("first unprotect"), plaintext);
        assert_eq!(unprotect(&second).expect("second unprotect"), plaintext);
    }

    #[test]
    fn random_bytes_are_rejected_as_ciphertext() {
        let garbage: Vec<u8> = (0..64).map(|index| (index * 7 + 3) as u8).collect();

        let error = unprotect(&garbage).expect_err("随机字节必须被拒");

        assert_eq!(
            error.kind,
            RemoteErrorKind::Auth,
            "解不开的密文归 Auth，让用户去重输密码，不是笼统的 Unknown：{error}"
        );
    }

    #[test]
    fn truncated_ciphertext_is_rejected() {
        let ciphertext = protect(b"a reasonably long secret value").expect("protect");
        let truncated = &ciphertext[..ciphertext.len() / 2];

        assert!(unprotect(truncated).is_err(), "半截密文必须被拒");
    }

    #[test]
    fn empty_ciphertext_is_rejected() {
        assert!(unprotect(&[]).is_err(), "空输入不可能是合法密文");
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_platform_reports_unsupported_instead_of_panicking() {
        assert_eq!(
            protect(b"x").expect_err("macOS / Linux 上没有 DPAPI").kind,
            RemoteErrorKind::Unsupported
        );
        assert_eq!(
            unprotect(b"x")
                .expect_err("macOS / Linux 上没有 DPAPI")
                .kind,
            RemoteErrorKind::Unsupported
        );
    }

    // ---- 落盘往返 -----------------------------------------------------------

    #[test]
    fn the_credential_file_sits_next_to_the_other_app_data_files() {
        let dir = temp_dir();

        let path = credentials_path_in(dir.path());

        assert_eq!(path, dir.path().join(REMOTE_CREDENTIALS_FILE));
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("remote-credentials.json")
        );
    }

    #[test]
    fn save_then_load_round_trips_through_the_real_file() {
        let dir = temp_dir();

        save_credential(dir.path(), "cred-1", "s3cr3t").expect("save");
        let loaded = load_credential(dir.path(), "cred-1").expect("load");

        assert_eq!(loaded.as_deref(), Some("s3cr3t"));
    }

    /// 阳性对照：真正证明「加密落盘」发生了，而不只是「写了个文件」。
    ///
    /// ⚠️ 第一版这条只查「文件里有没有出现明文」，**完全绕过 DPAPI 时它照样是绿的** ——
    /// 因为落盘的是 base64，明文当然搜不到。查明文证明不了任何事，
    /// 只有「密文能被 unprotect 解回原文」才真的区分了「加密」与「随便编个 base64」。
    #[test]
    fn the_credential_file_stores_real_dpapi_ciphertext() {
        let dir = temp_dir();
        let secret = "PlainTextSecret123";

        save_credential(dir.path(), "cred-1", secret).expect("save");

        let raw = std::fs::read(credentials_path_in(dir.path())).expect("read raw file");

        // 一、明文与它的 base64 都不许出现在文件里。
        assert!(
            !contains_subslice(&raw, secret.as_bytes()),
            "凭据文件里绝不能出现明文密码"
        );
        assert!(
            !String::from_utf8_lossy(&raw).contains(&BASE64_STANDARD.encode(secret)),
            "凭据文件里绝不能出现密码的 base64 —— 那等于明文落盘换个写法"
        );

        // 二、文件里那条必须能被 unprotect 解回原文。
        let stored = extract_cipher_text(&String::from_utf8_lossy(&raw));
        let ciphertext = BASE64_STANDARD
            .decode(&stored)
            .expect("存档里应当是 base64");
        assert_eq!(
            unprotect(&ciphertext).expect("存档里必须是真正的 DPAPI 密文"),
            secret.as_bytes(),
            "存档里的东西必须能被 DPAPI 解回原文"
        );
    }

    #[test]
    fn a_unicode_secret_survives_the_round_trip_byte_for_byte() {
        let dir = temp_dir();
        let secret = "密码🔐with spaces";

        save_credential(dir.path(), "cred-1", secret).expect("save");

        assert_eq!(
            load_credential(dir.path(), "cred-1")
                .expect("load")
                .as_deref(),
            Some(secret)
        );
    }

    #[test]
    fn loading_an_unknown_key_returns_none() {
        let dir = temp_dir();
        save_credential(dir.path(), "cred-1", "value").expect("save");

        assert_eq!(load_credential(dir.path(), "cred-2").expect("load"), None);
    }

    #[test]
    fn loading_from_a_missing_file_returns_none_rather_than_an_error() {
        let dir = temp_dir();

        assert_eq!(load_credential(dir.path(), "cred-1").expect("load"), None);
    }

    #[test]
    fn saving_again_replaces_the_previous_secret() {
        let dir = temp_dir();

        save_credential(dir.path(), "cred-1", "old").expect("first save");
        save_credential(dir.path(), "cred-1", "new").expect("second save");

        assert_eq!(
            load_credential(dir.path(), "cred-1")
                .expect("load")
                .as_deref(),
            Some("new")
        );
    }

    #[test]
    fn saving_one_key_keeps_the_others() {
        let dir = temp_dir();

        save_credential(dir.path(), "cred-1", "one").expect("save 1");
        save_credential(dir.path(), "cred-2", "two").expect("save 2");
        save_credential(dir.path(), "cred-3", "three").expect("save 3");

        assert_eq!(
            load_credential(dir.path(), "cred-1")
                .expect("load 1")
                .as_deref(),
            Some("one")
        );
        assert_eq!(
            load_credential(dir.path(), "cred-2")
                .expect("load 2")
                .as_deref(),
            Some("two")
        );
        assert_eq!(
            load_credential(dir.path(), "cred-3")
                .expect("load 3")
                .as_deref(),
            Some("three")
        );
    }

    #[test]
    fn delete_removes_only_the_named_key() {
        let dir = temp_dir();
        save_credential(dir.path(), "cred-1", "one").expect("save 1");
        save_credential(dir.path(), "cred-2", "two").expect("save 2");

        delete_credential(dir.path(), "cred-1").expect("delete");

        assert_eq!(load_credential(dir.path(), "cred-1").expect("load 1"), None);
        assert_eq!(
            load_credential(dir.path(), "cred-2")
                .expect("load 2")
                .as_deref(),
            Some("two")
        );
    }

    #[test]
    fn deleting_an_unknown_key_is_a_no_op() {
        let dir = temp_dir();
        save_credential(dir.path(), "cred-1", "one").expect("save");

        delete_credential(dir.path(), "cred-absent").expect("delete absent");
        delete_credential(dir.path(), "cred-absent").expect("delete twice");

        assert_eq!(
            load_credential(dir.path(), "cred-1")
                .expect("load")
                .as_deref(),
            Some("one")
        );
    }

    #[test]
    fn save_creates_the_app_data_dir_when_it_is_missing() {
        let dir = temp_dir();
        let nested = dir.path().join("not").join("created").join("yet");

        save_credential(&nested, "cred-1", "value").expect("save");

        assert_eq!(
            load_credential(&nested, "cred-1").expect("load").as_deref(),
            Some("value")
        );
    }

    #[test]
    fn blank_keys_are_rejected() {
        let dir = temp_dir();

        assert!(save_credential(dir.path(), "", "value").is_err());
        assert!(save_credential(dir.path(), "   ", "value").is_err());
        assert!(load_credential(dir.path(), "").is_err());
        assert!(delete_credential(dir.path(), "").is_err());
    }

    // ---- 损坏数据的处置 -----------------------------------------------------

    /// 关键策略：解不开的密文**必须报错**，不能悄悄当成「没存密码」。
    /// 悄悄吞掉的话，用户会用空密码去连服务器，拿回来的是一个更迷惑的 `Auth`。
    #[test]
    fn a_corrupted_entry_surfaces_an_error_instead_of_silently_dropping_the_secret() {
        let dir = temp_dir();
        save_credential(dir.path(), "cred-1", "value").expect("save");

        // 把这一条替换成「能过 base64、但不是 DPAPI 密文」的内容。
        let raw = std::fs::read_to_string(credentials_path_in(dir.path())).expect("read");
        let tampered = raw.replace(
            &extract_cipher_text(&raw),
            "bm90LWEtcmVhbC1kcGFwaS1jaXBoZXJ0ZXh0",
        );
        std::fs::write(credentials_path_in(dir.path()), tampered).expect("write tampered");

        let error = load_credential(dir.path(), "cred-1").expect_err("损坏的密文必须报错");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn an_entry_that_is_not_even_base64_surfaces_an_error() {
        let dir = temp_dir();
        std::fs::write(
            credentials_path_in(dir.path()),
            r#"{"version":1,"entries":{"cred-1":"这不是 base64!!"}}"#,
        )
        .expect("write");

        assert!(load_credential(dir.path(), "cred-1").is_err());
    }

    /// 整个文件是半截 JSON —— **只读路径**与 `mcp/auth.rs` 的既有策略一致：
    /// 宁可退回「没有凭据」，也不要拿半截文件推导出状态。
    /// 写回路径的处置完全不同，见 `saving_onto_a_corrupt_vault_backs_the_bad_file_up_before_overwriting_it`。
    #[test]
    fn a_truncated_json_file_is_treated_as_an_empty_vault() {
        let dir = temp_dir();
        std::fs::write(
            credentials_path_in(dir.path()),
            r#"{"version":1,"entries":{"cred-1":"QUJD"#,
        )
        .expect("write");

        assert_eq!(load_credential(dir.path(), "cred-1").expect("load"), None);
    }

    #[test]
    fn an_unknown_file_version_is_treated_as_an_empty_vault() {
        let dir = temp_dir();
        std::fs::write(
            credentials_path_in(dir.path()),
            r#"{"version":9999,"entries":{"cred-1":"QUJD"}}"#,
        )
        .expect("write");

        assert_eq!(load_credential(dir.path(), "cred-1").expect("load"), None);
    }

    // ---- 读不出来不再等于没有（2026-11-09 加固）-------------------------------

    /// 目录里的文件名，按名字排好序，方便断言「除了现役存档不该再有别的文件」。
    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.path().file_name().and_then(|n| n.to_str()).map(str::to_string))
            .collect();
        names.sort();
        names
    }

    /// 三态判据的直接对照：不存在 / 正常 / 读不出来，一个都不许混。
    /// 这一条是下面所有备份用例的地基 —— 分不开这三种，备份就无从谈起。
    #[test]
    fn the_three_vault_states_are_told_apart() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());

        assert!(matches!(inspect_vault(&path), VaultRead::Absent));

        save_credential(dir.path(), "cred-1", "hunter2").expect("save");
        assert!(matches!(inspect_vault(&path), VaultRead::Loaded(_)));

        std::fs::write(&path, r#"{"version":1,"entries":"#).expect("truncate");
        assert!(matches!(inspect_vault(&path), VaultRead::Unreadable { .. }));
    }

    /// 真机事故的回归用例：库里本来有凭据，文件被写成半截 JSON，
    /// 紧接着的一次保存会拿着空库写回，把其余密文一并抹掉。
    /// 现在覆盖之前必须先在旁边留下一份逐字节的原件。
    #[test]
    fn saving_onto_a_corrupt_vault_backs_the_bad_file_up_before_overwriting_it() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());
        let garbage = r#"{"version":1,"entries":{"cred-1":"QUJD"#;
        std::fs::write(&path, garbage).expect("write");

        save_credential(dir.path(), "cred-2", "hunter2").expect("save");

        let backups = list_corrupt_backups(dir.path());
        assert_eq!(backups.len(), 1, "坏文件应当留下一份备份");
        assert_eq!(
            std::fs::read(&backups[0]).expect("read backup"),
            garbage.as_bytes(),
            "备份必须与原件逐字节相同"
        );

        // 现役存档已被本次写入的新库替换，里面只有这次存的这一条。
        assert_eq!(
            load_credential(dir.path(), "cred-2").expect("load").as_deref(),
            Some("hunter2")
        );
        assert_eq!(load_credential(dir.path(), "cred-1").expect("load"), None);
    }

    /// 备份的口径必须说清楚：留住的是**读失败当场磁盘上的那些字节**。
    /// 原件若已被截断成半截，备份能留住的就是那半截 —— 它防的是「被覆盖」，
    /// 不防「已经发生的截断」；防截断靠的是原子写本身（见 `write_vault`）。
    #[test]
    fn the_backup_keeps_exactly_the_bytes_that_were_on_disk_when_the_read_failed() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());
        save_credential(dir.path(), "cred-1", "one").expect("save 1");
        save_credential(dir.path(), "cred-2", "two").expect("save 2");

        // 复现事故当场：进程在写的中途被杀，磁盘上留下半截 JSON。
        let intact = std::fs::read(&path).expect("read intact");
        let truncated = &intact[..intact.len() / 2];
        std::fs::write(&path, truncated).expect("truncate");

        save_credential(dir.path(), "cred-3", "three").expect("save 3");

        let backups = list_corrupt_backups(dir.path());
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read(&backups[0]).expect("read backup"),
            truncated,
            "备份必须逐字节等于当场那份读不出来的文件"
        );
    }

    /// 备份的全部价值在于**事后还解得开**。这一条钉的是真机丢掉的那三条：
    /// 一座「字节完好、但这个版本读不出来」的存档被覆盖之前整份留档，
    /// 里面的 DPAPI 密文照样能解回当年的密码原文。
    #[test]
    fn the_backup_of_a_vault_this_build_cannot_read_still_yields_every_secret() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());
        save_credential(dir.path(), "cred-1", "第一把钥匙").expect("save 1");
        save_credential(dir.path(), "cred-2", "second-key").expect("save 2");

        // 只把版本号换成不认识的值，条目与密文一律原样保留。
        let mut foreign = read_vault(dir.path());
        assert_eq!(foreign.entries.len(), 2, "两条凭据都应当还在库里");
        foreign.version = 9999;
        std::fs::write(&path, serde_json::to_vec_pretty(&foreign).expect("serialize"))
            .expect("write foreign");

        save_credential(dir.path(), "cred-3", "third-key").expect("save 3");

        let backups = list_corrupt_backups(dir.path());
        assert_eq!(backups.len(), 1);
        let archived: CredentialVault =
            serde_json::from_slice(&std::fs::read(&backups[0]).expect("read backup"))
                .expect("备份必须还是完整的一份存档");
        assert_eq!(archived.version, 9999, "备份不许改写原件的任何字节");

        for (key, expected) in [("cred-1", "第一把钥匙"), ("cred-2", "second-key")] {
            let encoded = archived
                .entries
                .get(key)
                .unwrap_or_else(|| panic!("{key} 应当在备份里"));
            let ciphertext = BASE64_STANDARD.decode(encoded).expect("备份里的密文应当是 base64");
            let plaintext = unprotect(&ciphertext).expect("unprotect");
            assert_eq!(String::from_utf8(plaintext).expect("utf8"), expected);
        }

        // 现役存档已被换成只含本次这一条的新库，且自身可正常读回。
        assert_eq!(
            load_credential(dir.path(), "cred-3").expect("load").as_deref(),
            Some("third-key")
        );
    }

    /// 「版本不认识」和「半截 JSON」是同一种危险：读不出来不等于没有。
    #[test]
    fn saving_onto_an_unknown_vault_version_leaves_a_backup() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());
        let future = r#"{"version":9999,"entries":{"cred-1":"QUJD"}}"#;
        std::fs::write(&path, future).expect("write");

        save_credential(dir.path(), "cred-2", "hunter2").expect("save");

        let backups = list_corrupt_backups(dir.path());
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read(&backups[0]).expect("read backup"), future.as_bytes());
    }

    /// 「不是 UTF-8」正是原来 `read_to_string` 与「文件不存在」撞成同一个错误的那一种：
    /// 二进制垃圾也必须按字节留住，而不是当成干净状态直接盖掉。
    #[test]
    fn a_vault_that_is_not_even_utf8_is_backed_up_by_bytes() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());
        let not_text: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x80, 0x81];
        std::fs::write(&path, &not_text).expect("write");

        save_credential(dir.path(), "cred-1", "hunter2").expect("save");

        let backups = list_corrupt_backups(dir.path());
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read(&backups[0]).expect("read backup"), not_text);
    }

    /// 「文件本来就不存在」是干净的初始状态，不是坏文件 —— 不该给它造备份。
    #[test]
    fn saving_the_first_credential_creates_no_backup() {
        let dir = temp_dir();
        save_credential(dir.path(), "cred-1", "hunter2").expect("save");

        assert!(list_corrupt_backups(dir.path()).is_empty());
        assert_eq!(file_names(dir.path()), vec![REMOTE_CREDENTIALS_FILE.to_string()]);
    }

    /// 备份只在写回之前发生。查密码是只读的，读一万次也不许在目录里留下任何东西 ——
    /// 否则每次启动都可能攒一堆垃圾文件。
    #[test]
    fn a_read_only_lookup_never_creates_a_backup() {
        let dir = temp_dir();
        let garbage = r#"{"version":1,"entries":{"cred-1":"QUJD"#;
        std::fs::write(credentials_path_in(dir.path()), garbage).expect("write");

        for _ in 0..3 {
            assert_eq!(load_credential(dir.path(), "cred-1").expect("load"), None);
        }

        assert!(list_corrupt_backups(dir.path()).is_empty());
        assert_eq!(file_names(dir.path()), vec![REMOTE_CREDENTIALS_FILE.to_string()]);
    }

    /// 原子写不留垃圾：改名成功之后目录里只该有现役存档，
    /// 不许留下 `.tmp-` 的孤儿（那上面带的是密文）。
    #[test]
    fn the_atomic_write_leaves_no_temp_file_behind() {
        let dir = temp_dir();
        save_credential(dir.path(), "cred-1", "one").expect("save 1");
        save_credential(dir.path(), "cred-2", "two").expect("save 2");
        delete_credential(dir.path(), "cred-1").expect("delete");

        let leftovers: Vec<String> = file_names(dir.path())
            .into_iter()
            .filter(|name| name.contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "原子写留下了临时文件: {leftovers:?}");
        assert_eq!(file_names(dir.path()), vec![REMOTE_CREDENTIALS_FILE.to_string()]);
    }

    /// 反复失败的写回不该把目录堆成垃圾：同一份坏字节只留一份备份。
    #[test]
    fn backing_up_the_same_bad_file_twice_creates_only_one_backup() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());
        let garbage = r#"{"version":1,"entries":{"cred-1":"QUJD"#;

        std::fs::write(&path, garbage).expect("write");
        save_credential(dir.path(), "cred-2", "one").expect("save 1");
        std::fs::write(&path, garbage).expect("write again");
        save_credential(dir.path(), "cred-3", "two").expect("save 2");

        assert_eq!(list_corrupt_backups(dir.path()).len(), 1);
    }

    /// 判重的边界：内容不同就必须各留一份。
    /// 反面是「只留第一份」，那样第二次事故的原件照样会被盖掉。
    #[test]
    fn a_differently_broken_vault_gets_its_own_backup() {
        let dir = temp_dir();
        let path = credentials_path_in(dir.path());
        let first = r#"{"version":1,"entries":{"cred-1":"QUJD"#;
        let second = r#"{"version":1,"entries":{"cred-9":"XYZ"#;

        std::fs::write(&path, first).expect("write");
        save_credential(dir.path(), "cred-2", "one").expect("save 1");
        std::fs::write(&path, second).expect("write again");
        save_credential(dir.path(), "cred-3", "two").expect("save 2");

        let backups = list_corrupt_backups(dir.path());
        assert_eq!(backups.len(), 2, "两份不同的坏存档应当各留一份备份");
        let bytes: Vec<Vec<u8>> = backups
            .iter()
            .map(|path| std::fs::read(path).expect("read backup"))
            .collect();
        assert!(bytes.iter().any(|b| b == first.as_bytes()));
        assert!(bytes.iter().any(|b| b == second.as_bytes()));
    }

    /// 把存档里某个 entry 的 base64 原文取出来，供篡改测试替换。
    fn extract_cipher_text(raw: &str) -> String {
        raw.split("\"cred-1\":")
            .nth(1)
            .and_then(|rest| rest.split('"').nth(1))
            .expect("存档里应当有 cred-1 这一条")
            .to_string()
    }

    /// 子串查找。`windows::core` 那套 slice API 在这个位置是多余的。
    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        haystack.windows(needle.len()).any(|w| w == needle)
    }
}
