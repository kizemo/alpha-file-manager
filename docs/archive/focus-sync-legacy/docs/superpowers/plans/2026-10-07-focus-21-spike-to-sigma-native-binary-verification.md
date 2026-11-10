# Step 1 验证结果:Sigma FM 原生 binary 系统可行性

**日期**:2026-10-07 09:20
**目的**:验证 `2026-10-07-focus-21-spike-to-sigma-minimal-architecture.md` §6 的三个未知(U1/U2/U3)

---

## 0. 结论

| 未知 | 结论 | 依据 |
|---|---|---|
| **U1** manifest `binaries` schema | ✅ **已确认**,比预想严格 | `runtime/validation.ts:206-255` |
| **U2** JS API 绑定 | ✅ **存在**,但只有 3 个只读方法 | `api/create-binary-api.ts:46-62` |
| **U3** 启动路径限制 | ✅ **限制在 extension 目录 / shared 目录** | `extensions/processes.rs:248-262` |

**Step 3 技术上可行,但有 2 个必须解决的前提(U4/U5)。**

---

## 1. U1:manifest `binaries` schema(已确认)

`src/modules/extensions/runtime/validation.ts:206-255` + `:174-204`:

```jsonc
"binaries": {
  "focus-sync": {                    // ← ManifestBinaryDefinition
    "id": "focus-sync",              // 必填,非空字符串
    "name": "Focus Sync Sidecar",    // 必填,非空字符串
    "version": "0.5.6",              // 必填,非空字符串
    "repository": "...",             // 可选,非空字符串
    "executable": "bin/x/y.exe",     // 可选,必须是 safe relative path
    "platforms": ["windows-x86_64"], // 可选,合法平台列表
    "assets": [                      // 必填,非空数组,每个元素是 ManifestBinaryAsset
      {
        "platform": "windows-x86_64",        // 必填,合法平台
        "downloadUrl": "https://...exe",     // 必填,非空字符串
        "integrity": "...",                  // 必填,合法 integrity 格式
        "arch": ["x86_64"],                  // 可选,合法 arch 列表
        "archive": false,                    // 可选,boolean
        "executable": "..."                  // 可选,safe relative path
      }
    ]
  }
}
```

### 校验规则(硬性)

| 规则 | 位置 | 说明 |
|---|---|---|
| `assets` 必须非空数组 | `:231` | 没有 assets 的 binary 定义非法 |
| 每个 asset 必须有 `platform` + `downloadUrl` + `integrity` | `:179-188` | **`integrity` 是必填**,不能省 |
| `platforms` 与 asset.platform 必须一致 | `:240-242` | 定义里写了 platforms,asset 必须命中其中之一 |
| `(platform:arch)` 组合不能重复 | `:247-251` | 同一 binary 不能有两个相同平台的 asset |
| `executable` 必须是 safe relative path | `:199, 223` | 绝对路径非法 |

---

## 2. U2:JS API 绑定(存在,但能力有限)

`src/modules/extensions/api/create-extension-api.ts:62`:
```typescript
binary: createBinaryAPI(context),
```

`src/modules/extensions/api/create-binary-api.ts:46-62` — **只有 3 个方法**:
```typescript
return {
  getPath:    async (id) => { ensureBinaryPermission(); ... },   // 查路径
  isInstalled: async (id) => { ensureBinaryPermission(); ... }, // 查是否已装
  getInfo:    async (id) => { ensureBinaryPermission(); ... },   // 查详情
};
```

### 权限要求(`create-binary-api.ts:23-27`)
```typescript
function ensureBinaryPermission(): void {
  if (!context.hasPermission('shell')) {
    throw new Error(context.t('extensions.api.permissionDeniedBinary'));
  }
}
```

**`sigma.binary.*` 三个方法全部需要 `"shell"` 权限。** 我们在 2026-10-01 删掉的正是这个权限。

### 数据来源(`create-binary-api.ts:10, 17-21`)
```typescript
const BINARY_STORAGE_KEY = '__binaries';
const settings = await storageStore.getExtensionSettings(context.extensionId);
return settings?.customSettings?.[BINARY_STORAGE_KEY] || {};
```

**`sigma.binary.getPath()` 读的是 `customSettings.__binaries[id].path`,由 Sigma FM 安装 extension 时写入,不是 extension 自己声明的。**

---

## 3. U3:启动路径限制(确认有严格白名单)

`src-tauri/src/extensions/processes.rs:209-284` — `resolve_extension_command`:

```rust
let allowed_roots = vec![
    normalized_extension_dir.as_path(),   // extension 目录
    normalized_shared_dir.as_path(),      // shared binaries 目录
];
ensure_path_is_allowed_command(
    &normalized, &allowed_roots, &custom_binary_paths,
    "Access denied: command is outside allowed directories",
)?;
```

### 白名单范围

| 允许 | 位置 |
|---|---|
| ✅ `%APPDATA%\com.sigma-file-manager.app\extensions\<ext-id>\` 下任意文件 | `allowed_roots[0]` |
| ✅ shared binaries 目录(`get_shared_binaries_dir`)下任意文件 | `allowed_roots[1]` |
| ✅ `registry_storage` 的 `customSettings.__binaries[].path` 登记的路径 | `custom_binary_paths` |
| ❌ 其他任何绝对路径 | 被 `ensure_path_is_allowed_command` 拒绝 |

### 关键结论

**`extension_dir.join("bin").join(binary_id)`(`binaries.rs:47`)在 allowed_roots[0] 内 → 允许启动。**

**我们的 sidecar 二进制放 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync\` 完全合法。**

`start_extension_command`(`processes.rs:331-361`)行为:
```rust
Command::new(&resolved_command.command_path)
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .creation_flags(CREATE_NO_WINDOW);
```

**返回 taskId,并通过 `extension-command-progress` / `extension-command-complete` 事件回传 stdout/stderr。** 生命周期由 Sigma FM 管(`cancel_extension_command` / `cancel_all_extension_commands` / PID registry `COMMAND_PIDS`)。

---

## 4. 发现的新前提(U4 / U5)

### U4(阻塞级):binary 定义需要 HTTP 可达的 `downloadUrl` + `integrity`

`validation.ts:183-188` 强制要求:
- `downloadUrl` 非空字符串
- `integrity` 通过 `isValidIntegrity()` 校验

**且 `manifest-binaries.ts` 的下载流程走 `download_extension_binary` / `download_and_extract_extension_binary` Tauri command,那是 HTTP 下载。**

**问题**:spike 的 sidecar 是一个本地构建的 exe,**没有 HTTP 发布地址**。

可选方案:
| 方案 | 说明 | 代价 |
|---|---|---|
| A. 在本机起一个静态文件服务 | Sigma FM 从 `http://127.0.0.1:PORT/focus-sync-sidecar.exe` 下载 | 需要一个服务 |
| B. 把 sidecar 直接放进 extension 的 `bin/` 目录 | 不走 manifest binaries,靠 `customSettings.__binaries` 登记路径 | **需要 register.ps1 写 registry**(退回到当前方案) |
| C. 用 `source: 'custom'` 路径 | `registry_storage.rs:209` 支持 `customBinaries` 外部登记 | **仍需外部写 registry** |

**结论:方案 B/C 无法消除 `register.ps1`,只是换了个写法。方案 A 需要发布渠道。**

### U5:`path_exists` Tauri command 是否存在

`create-binary-api.ts:38` 调用 `invoke<boolean>('path_exists', { path })`。

**但我在 `extensions/commands.rs` 的 40+ 个 `#[tauri::command]` 里没有找到 `path_exists`。** 需要确认它是否注册在别的模块(`lib.rs` 的 `generate_handler!`)。

若未注册,`sigma.binary.getPath()` 会 catch 异常返回 `null`(`:40-42`),**导致二进制永远读不到路径**。

---

## 5. 对 Step 3 可行性的修正结论

| 步骤 | 原判断 | 修正后 |
|---|---|---|
| manifest 加回 `binaries` | ✅ 可行 | ⚠️ 可行但**需要 HTTP `downloadUrl` + `integrity`**,本地 exe 无法直接走这条路 |
| extension 改用原生 API | ✅ 可行 | ⚠️ `sigma.binary.*` 只有只读查询,**下载/启动仍需额外机制** |
| 删掉 extension-installer/ | ✅ 可省 53 KB | ❌ **无法完全删除** —— 至少 `register.ps1` 的 `__binaries` 登记仍需保留(或改成 Sigma FM UI 手动配置) |

### 更现实的最小方案

```
保留: register.ps1(但大幅瘦身,只写 __binaries 登记,不做 NSIS/Scheduled Task)
删除: installer.nsi / register-scheduled-task.ps1 / manual-install.ps1 /
      kill-sigma.ps1 / reopen-sigma.ps1 / unregister*.ps1 / resolve_user_appdata.ps1
新增: package.json 加 "shell" 权限 + binaries 定义(如果有发布地址)
改写: extension 用 sigma.shell.runWithProgress(binaryPath, ['--port','37421'])
      替代 Scheduled Task
```

**净收益:53 KB → ~13 KB(register.ps1 一份),且不再需要 Program Files 写权限、不再需要 Scheduled Task、不再有版本号 10 处字面量。**

---

## 6. 建议的下一步(修正版)

### Step 1.5(新增,必做):确认 `path_exists` 是否注册

```powershell
cd F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri
Select-String -Path src\lib.rs -Pattern "path_exists"
Select-String -Path src\**\*.rs -Pattern "fn path_exists"
```

**若未注册 → `sigma.binary.getPath()` 永远返回 null,整个原生方案不可用,必须退回当前 Scheduled Task 方案。**

### Step 2(不变):v0.5.6 gate 修复 5 行

零风险,先用实测存在的 production 路径恢复功能。

### Step 3(修正):瘦身 register.ps1 + 改用 `sigma.shell`

不做 manifest binaries(避免 U4 的 downloadUrl 问题),只:
1. `package.json` 加 `"shell"` 权限
2. `register.ps1` 瘦身:只做 extension 注册 + `__binaries` 路径登记(~100 行)
3. extension 改用 `sigma.shell.runWithProgress()` 启动 sidecar
4. 删掉其余 9 个脚本 + NSIS + Scheduled Task

### Step 4(不变):spike 瘦身

---

## 7. 元数据

| 项 | 值 |
|---|---|
| **验证方法** | 直读 Sigma FM 源码:`validation.ts` / `create-binary-api.ts` / `create-extension-api.ts` / `processes.rs` / `binaries.rs` / `registry_storage.rs` |
| **U1** | ✅ schema 确认,含 `integrity` 强制校验 |
| **U2** | ✅ `sigma.binary.*` 存在,3 个只读方法,需 `shell` 权限 |
| **U3** | ✅ 白名单 = extension 目录 + shared 目录 + custom 登记路径 |
| **新发现** | U4(`downloadUrl` + `integrity` 强制)阻塞 manifest binaries 路线;U5(`path_exists` command 存在性待确认) |
| **Step 3 修正** | 无法完全删 `register.ps1`,但可从 53 KB 瘦到 ~13 KB,并去掉 NSIS + Scheduled Task + Program Files 写权限 |

---

## 8. 一句话 prompt

```
读 docs/superpowers/plans/2026-10-07-focus-21-spike-to-sigma-native-binary-verification.md(必读)。

U1/U2/U3 已全部验证:
  U1 manifest binaries schema 确认(validation.ts:206-255),但 integrity + downloadUrl 强制必填
  U2 sigma.binary.* 存在(create-extension-api.ts:62),仅 3 个只读方法,需 "shell" 权限
  U3 启动白名单 = extension 目录 + shared 目录 + custom 登记路径(processes.rs:248-262)
     → extension/bin/focus-sync/ 完全合法

两个新阻塞:
  U4: manifest binaries 要求 HTTP downloadUrl + integrity 校验,本地构建的 exe 无法走这条路
  U5: create-binary-api.ts:38 调用 path_exists,但 commands.rs 里没找到这个 command,待确认

Step 3 修正:不能完全删 register.ps1(binary 路径登记仍需它),
但可从 53 KB 瘦到 ~13 KB,并去掉 NSIS / Scheduled Task / Program Files 写权限。

下一步必做:确认 path_exists 是否注册在 lib.rs 的 generate_handler!。
若未注册 → sigma.binary.getPath() 永远 null → 原生方案不可用。
```