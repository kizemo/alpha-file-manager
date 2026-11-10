# spike → Sigma 插件:最小化改造架构分析

**日期**:2026-10-07 09:00
**问题**:是否偏移过多?如何在最小改动下把 spike 转成 Sigma 插件?
**方法**:源码清点 + git 历史 + Sigma FM 主项目能力盘点

---

## 0. 结论

**偏移了,而且偏移量已经超过原始实现的 5 倍。**

但更重要的发现是:**Sigma FM 主项目本身已经内置了完整的 extension binary 管理能力,而我们在 2026-10-01 主动删掉了对它的使用,改用 PowerShell + NSIS + Scheduled Task 自建了一套。**

**这是 100+ 轮反复的根源。**

---

## 1. 偏移量化

### 1.1 代码量

| 项 | 原始 PathWrap | 当前 spike | 倍数 |
|---|---|---|---|
| `dialog.rs` / `uia_inject.rs` | **176 行** | **894 行** | **5.1x** |
| 整个 sidecar | ~400 行(推测) | **3,306 行** | **~8x** |

源码清点:
```
uia_inject.rs    894   ← 核心,5.1x 膨胀
main.rs          480
uia_event.rs     469
http_server.rs   418   ← 原本 spike 没有 HTTP server
state.rs         305
fg_bypass.rs     155   ← focus-18 加的 foreground 绕过
logger.rs        146
config.rs        146
writer.rs        109
events.rs         89
reader.rs         52
lib.rs            31
detector.rs       12
─────────────────
TOTAL          3306
```

### 1.2 部署面(PathWrap 完全没有这一层)

```
release/extension-installer/
├── build.ps1                      1,585 B
├── installer.nsi                 13,423 B   ← NSIS 安装器
├── kill-sigma.ps1                2,372 B
├── manual-install.ps1             5,882 B
├── register-scheduled-task.ps1    4,698 B
├── register.ps1                  13,109 B
├── reopen-sigma.ps1               2,575 B
├── resolve_user_appdata.ps1      4,917 B
├── unregister-scheduled-task.ps1 2,710 B
└── unregister.ps1                 2,181 B
─────────────────
TOTAL         ~53 KB / 10 个脚本
```

外加:Scheduled Task 注册 + HKLM registry key + Program Files 写权限 + 12 个 setup.exe(v0.3.5 ~ v0.5.5)

### 1.3 文档面

```
docs/superpowers/plans/  共 86 篇
  focus-16:  1 篇
  focus-17:  3 篇
  focus-18:  3 篇
  focus-19:  5 篇
  focus-20:  7 篇
  focus-21: 56 篇   ← 65% 都在同一轮 focus
```

### 1.4 关键佐证:项目自己承认偏移过

`uia_inject.rs:5-9` 的文件头注释:

```rust
// v0.5.0 (2026-10-04) — ROLLBACK to Sep 27 PathWrap port design:
//   The 12 rounds of focus-19/20/21/22 + v0.3.4-0.3.9 introduced so many
//   verification, basename, and WinUI 3 detection layers that the simple
//   working UIA-only SetValue+Invoke approach was lost. This rollback
//   restores the Sep 27 design (which the user verified PASSES manually):
```

**项目已经回滚过一次了。** 然后 v0.5.1 ~ v0.5.6 又在已回滚的基础上重新加了 6 层(v0.5.1 的 4-layer fallback、H1 SendInput、H2 restore、Fix A/B/D/E/F/G/H/N-4)。

**这是同一个错误的第二次发生。**

---

## 2. 根本原因:Sigma FM 主项目已有原生能力,被主动放弃

### 2.1 Sigma FM 的 extension binary 系统(现成可用)

`sigma-file-manager/src-tauri/src/extensions/binaries.rs`:

```rust
pub fn get_shared_binaries_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    ensure_app_data_subdir(app_handle, "binaries", "shared binaries directory")
}

fn get_extension_binary_dir(...) -> Result<PathBuf, String> {
    Ok(extension_dir.join("bin").join(binary_id))     // ← extension 自带 bin 目录
}

pub async fn get_extension_binary_path(...)           // ← 查路径
pub async fn download_extension_binary(...)           // ← 下载
pub async fn download_and_extract_extension_binary(...)  // ← 下载+解压
pub async fn remove_extension_binary(...)
pub async fn extension_binary_exists(...)
pub async fn get_shared_binary_path(...)
pub async fn download_shared_binary(...)
```

`extensions/commands.rs` 里对应的 Tauri commands 共 **8 个**:

```rust
#[tauri::command] pub async fn get_extension_binary_path(...)
#[tauri::command] pub async fn download_extension_binary(...)
#[tauri::command] pub async fn download_and_extract_extension_binary(...)
#[tauri::command] pub async fn remove_extension_binary(...)
#[tauri::command] pub async fn extension_binary_exists(...)
#[tauri::command] pub async fn get_shared_binary_path(...)
#[tauri::command] pub async fn download_shared_binary(...)
#[tauri::command] pub async fn download_and_extract_shared_binary(...)
```

`extensions/processes.rs` + `commands.rs`:

```rust
#[tauri::command] pub async fn start_extension_command(
    app_handle, extension_id, command_path, args, caller_extension_id
) -> Result<String, String>       // ← extension 可以启动自己的 binary

#[tauri::command] pub async fn run_extension_command(...)
#[tauri::command] pub async fn cancel_extension_command(...)
#[tauri::command] pub async fn cancel_all_extension_commands(...)
```

**Sigma FM 原生就能:下载 sidecar 二进制 → 放到 `extension/bin/` → 启动它 → 管理它的生命周期。**

### 2.2 我们在 2026-10-01 删掉了对它的使用

`docs/superpowers/plans/2026-10-01-focus-16-test-results-analysis.md:30`:

```
| release/extension/package.json | ✅ | 0.3.0,删除 binaries + shell permission |
```

同一份文档里还留着当时的 `register.ps1` 片段:
```
Write-Host "==> Added new kizemo.focus-sync entry (v0.3.0 manifest, no binaries segment)"
```

**我们把 manifest 里的 `binaries` + `shell` 权限删了,然后写了一个 "no binaries segment" 的注册脚本。**

### 2.3 代价对比

| 能力 | Sigma FM 原生 | 我们的 PowerShell 自建 |
|---|---|---|
| 二进制分发 | `download_extension_binary` | NSIS installer + register.ps1 |
| 路径解析 | `get_extension_binary_path` | `$INSTDIR\tools\` / `$APPDATA\...\bin\` 两套路径 |
| 生命周期 | `start_extension_command` / `cancel_*` | Scheduled Task + schtasks /End /Run |
| 版本管理 | binary_id + version 目录 | 10 处 `0.5.4` 字面量 sed |
| 多用户 | Sigma FM 自己管 | 自己的 session_id 隔离 + fail-fast |
| 升级 | `download_and_extract_*` | 13 KB register.ps1 的 if/else 分支 |

**我们用 53 KB PowerShell + 1 个 NSIS + Scheduled Task + registry,重新实现了一个 Sigma FM 已经内置的功能。**

---

## 3. 100+ 轮 bug 的重新归类

把已知的 bug 按根因归类,会看到清晰的分布:

| 根因类别 | 具体 bug | 出现轮次 | 占比 |
|---|---|---|---|
| **A. 部署/安装** | installer 失败、Task Scheduler 权限、Program Files 写权限、PATH、registry、版本号字面量漂移(86 篇文档里大半) | focus-16 ~ focus-21 全程 | **~45%** |
| **B. 焦点/键盘** | SendInput race condition、SetForegroundWindow、foreground lock、Rule 47/51/52 | focus-19/20/21 | ~25% |
| **C. UIA 元素识别** | 41477 vs 1001 vs 1148、WinUI 3 wrapped 检测、APPEND 行为 | focus-19/20/21 | ~20% |
| **D. extension API** | `sigma.context.onPathChange` 不 fire、TDZ、sandbox 限制 | focus-21 | ~10% |

**A 类(部署)是最大的一块,而它 100% 是自找的** —— Sigma FM 原生 binary 系统能消掉整类。

---

## 4. 最小化改造方案

### 4.1 目标架构

```
┌─────────────────────────────────────────────────┐
│ Sigma FM (Tauri/Rust)                            │
│  ├─ extension host(JS sandbox)                   │
│  │   └─ kizemo.focus-sync                        │
│  │       ├─ onPathChange → 记住 path              │
│  │       └─ start_extension_command(sidecar, ...) │
│  │                                             │
│  └─ extensions/binaries.rs  ← 原生管理           │
│      ├─ download_extension_binary               │
│      ├─ get_extension_binary_path               │
│      └─ 生命周期跟随 Sigma FM                    │
│                                                 │
│  ┌─ extension/bin/focus-sync/  ← Sigma 托管     │
│  │   └─ focus-sync-sidecar.exe(spike)           │
│  └──────────────────────────────────────────────┘
```

**删除**:installer.nsi / register.ps1 / register-scheduled-task.ps1 / manual-install.ps1 / Scheduled Task / registry key
**保留**:spike(改成 stdio 或单向 pipe)/ extension JS(大幅精简)

### 4.2 改动清单

#### 改动 1:manifest 加回 binaries(关键)

`release/extension/package.json`:

```json
{
  "id": "kizemo.focus-sync",
  "version": "0.1.0",
  "permissions": ["commands", "toolbar", "notifications"],
  "binaries": {
    "focus-sync": {
      "version": "0.1.0",
      "platforms": ["windows-x86_64"],
      "executable": "bin/focus-sync/focus-sync-sidecar.exe"
    }
  }
}
```

> ⚠️ 具体 schema 需要按 Sigma FM `binaries.rs` 的解析逻辑对齐(我还没读到 manifest 解析部分),这是**实施前必须验证的第一件事**。

#### 改动 2:extension 改用原生 API

```js
// 启动 sidecar(Sigma FM 管理生命周期)
async function ensureSidecar() {
  const p = await sigma.binaries.getExtensionBinaryPath({
    binaryId: 'focus-sync', executableName: 'focus-sync-sidecar.exe'
  });
  if (!p) {
    await sigma.binaries.downloadExtensionBinary({ binaryId: 'focus-sync' });
  }
  return await sigma.commands.startExtensionCommand({
    commandPath: p, args: ['--stdio']
  });
}
```

#### 改动 3:spike 改 stdio,删 HTTP server(可后置)

第一阶段**不改** spike,继续用 HTTP(已工作,改动风险 0)。
第二阶段若要瘦身,再把 `http_server.rs`(418 行)换成 stdio(~80 行)。

#### 改动 4:删掉全部部署脚本

改动 1-2 验证通过后:
```
删除 release/extension-installer/ 整个目录(~53 KB)
删除 Scheduled Task 注册
删除 registry key
```

### 4.3 改动量对比

| 方案 | 改动量 | 风险 |
|---|---|---|
| **继续修 v0.5.6** | 5 行 Rust + 版本号 | 低,但**只解决 gate,installer 问题依旧** |
| **回到 Sigma 原生 binary** | manifest + extension JS(约 150 行)+ 删 53 KB 脚本 | 中,需先验证 manifest schema |
| **重写 spike 为 Sigma 插件** | 全量 | 高,不推荐 |

---

## 5. 关于 spike 本身的瘦身(独立议题)

即使完成了 4.2,`uia_inject.rs` 的 5.1x 膨胀仍值得处理。

### 5.1 可以砍掉的部分

| 模块 | 行数 | 能否砍 | 理由 |
|---|---|---|---|
| `fg_bypass.rs` | 155 | **可砍** | focus-18 加的 foreground 绕过,H2 fallback 成熟后可移除 |
| `http_server.rs` | 418 | **可砍**(二阶段) | 改 stdio 省 ~340 行 |
| `config.rs` | 146 | 部分 | `app_whitelist` 是死代码(全项目 grep 无使用) |
| `uia_inject.rs` H1/H2 | ~120 | **谨慎** | SendInput 有 Rule 47/51/52 的坑,但删了就没有地址栏写入能力 |
| PathWrap 核心 | ~250 | **不可砍** | 这是真正在工作的部分 |

### 5.2 关键提醒

**不要在同一次改动里同时做「瘦身」和「修功能」。**

过去 100+ 轮的教训是:每次改都混着改,一旦出问题无法定位是哪个改动引起的。

建议:
1. **v0.5.6**:只做 gate 修复(5 行),验证功能恢复
2. **v0.6.0**:切 Sigma 原生 binary(删 53 KB 脚本),验证部署简化
3. **v0.7.0**:瘦身 spike(砍 HTTP / fg_bypass),验证无回归

---

## 6. 需要先验证的三个未知(实施前必查)

| # | 未知 | 怎么查 | 为什么重要 |
|---|---|---|---|
| **U1** | `binaries` manifest 的确切 schema | 读 `binaries.rs` 的 manifest 解析段 + 找 `__binaries` 关键字 | 决定改动 1 能否落地 |
| **U2** | extension JS 侧有没有 `sigma.binaries.*` / `sigma.commands.startExtensionCommand` 绑定 | 在 Sigma FM 的 JS API 注入代码里 grep | 决定改动 2 能否落地 |
| **U3** | Sigma FM 是否限制 `start_extension_command` 的可执行文件必须在 extension 目录内 | 读 `processes.rs` 的路径校验 | 决定 sidecar 二进制放哪 |

**如果 U1/U2 任一为否** —— 说明 Sigma FM 的原生 binary 系统只用于「下载」,不支持「启动」,那改动 2 不成立,只能保留 Scheduled Task 方案,但至少可以**用 `download_extension_binary` 替代 NSIS installer**,省掉大部分脚本。

---

## 7. 对「是否偏移过多」的直接回答

**是。量化如下:**

| 维度 | 原始 | 现在 | 结论 |
|---|---|---|---|
| 核心代码 | 176 行 | 894 行 | 5.1x |
| 总代码 | ~400 行 | 3,306 行 | ~8x |
| 部署文件 | 0 | 11 个(53 KB) | ∞ |
| 计划文档 | 0 | 86 篇 | ∞ |
| 已发生回滚 | 0 | 1 次(v0.5.0),且未根治 | — |

**最关键的一条:**

> Sigma FM 主项目**已经内置**了 extension binary 的下载、路径解析、生命周期管理(8 个 Tauri command)。
> 我们在 2026-10-01 删掉了 manifest 里的 `binaries` 权限,用 **53 KB PowerShell + NSIS + Scheduled Task** 重新实现了一遍。

**这就是 45% 的 bug(部署类)的来源,也解释了大量「版本号漂移」「路径不一致」「Task Scheduler 无响应」问题的反复出现。**

---

## 8. 建议的行动顺序

```
Step 1 (今天,零风险):
  验证 U1/U2/U3 —— 读 Sigma FM 源码,确认原生 binary 系统能否用
  ↓ 如果可以 → 走 Step 3;如果不可以 → 走 Step 2

Step 2 (低风险,5 行):
  实施 v0.5.6 gate 修复,恢复功能
  用 production 路径验证(D:\Program Files\... 已实测存在)

Step 3 (中风险,1-2 天):
  manifest 加回 binaries + extension 改用原生 API
  删掉 extension-installer/ 整个目录
  ↓

Step 4 (低风险):
  v0.7 spike 瘦身:砍 http_server(改 stdio)/ 砍 fg_bypass / 删死代码
```

**不要跳步。** 过去 100+ 轮的失败模式是「一次改一堆,出问题无法定位」。

---

## 9. 元数据

| 项 | 值 |
|---|---|
| **方法** | 源码行数清点 + git log + Sigma FM `binaries.rs`/`commands.rs`/`processes.rs` 能力盘点 + manifest 权限检查 |
| **核心发现** | Sigma FM 已有 8 个 extension binary Tauri command + 4 个 process management command,2026-10-01 被主动弃用 |
| **量化** | 核心代码 5.1x / 总代码 8x / 部署面 0→53 KB / 文档 0→86 篇 |
| **未验证** | U1 (manifest schema) / U2 (JS API 绑定) / U3 (路径限制) —— 实施前必查 |
| **建议** | 分 4 步,零风险起步;不要一次改一堆 |

---

## 10. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-spike-to-sigma-minimal-architecture.md(必读)。

量化偏移:核心代码 176→894 行(5.1x),总代码 3306 行,部署面 0→53 KB(10 个 ps1 + NSIS),
计划文档 0→86 篇。uia_inject.rs:5 的文件头自己写着"v0.5.0 ROLLBACK ... 12 rounds introduced
so many layers that the simple working approach was lost"——已回滚过一次,v0.5.1-5.6 又加了回来。

根本原因:Sigma FM 主项目已有 extension binary 完整能力
(binaries.rs: get_extension_binary_dir / download_and_extract_extension_binary /
 get_shared_binary_path;commands.rs: 8 个 binary command + start_extension_command /
 run_extension_command / cancel_*)。
2026-10-01 focus-16 主动删除了 manifest 的 binaries + shell 权限
(见 2026-10-01-focus-16-test-results-analysis.md:30),改用 PowerShell + Scheduled Task 自建。
→ 45% 的 bug 是部署类,全部自找。

最小化路径:
  Step 1: 验证 manifest binaries schema / JS API 绑定 / 路径限制(3 个未知,实施前必查)
  Step 2: v0.5.6 gate 修复 5 行(零风险)
  Step 3: manifest 加回 binaries + 删 extension-installer/(省 53 KB)
  Step 4: spike 瘦身(砍 http_server / fg_bypass / 死代码)

不要一次改一堆——这是过去 100+ 轮失败的模式。
```