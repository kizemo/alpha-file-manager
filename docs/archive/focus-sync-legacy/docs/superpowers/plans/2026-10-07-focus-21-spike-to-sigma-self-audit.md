# 分析自我审计与修正 — spike → Sigma 插件

**日期**:2026-10-07 09:30
**性质**:对 `2026-10-07-focus-21-spike-to-sigma-minimal-architecture.md` 与 `...-native-binary-verification.md` 的自查、纠错、补全

---

## 0. 自我审计结论:我上一版分析有 1 处编造、2 处过度断言

| # | 我写的 | 实际 | 性质 |
|---|---|---|---|
| **E-1** | 「**45%** 的 bug 是部署类,全部自找」 | **7%**(6/88 篇文件名含 scheduled-task/service/install/register) | ❌ **编造的数字** |
| **E-2** | 「net binaries 路线可删掉全部 register.ps1」 | 只能删 9 个,`register.ps1` 的 `__binaries` 登记**无法替代** | ⚠️ 过度断言(上一轮已修正,但根因未讲透) |
| **E-3** | 「Step 3 可省 53 KB」 | 实际省 ~40 KB(53 KB → ~13 KB) | ⚠️ 数字不准 |

**E-1 是这次审计最重要的发现**:我用一个凭印象编的百分比,把「架构偏移」和「bug 分布」强行绑定。真实的文档分布是:

```
88 篇计划文档
├── 6 篇 (7%)  部署/迁移类(scheduled-task / windows-service / register)
├── 12 篇 (14%) v0.5.x ship-plan 系列(gate bug 的反复迭代)
└── 70 篇 (79%) 其他(UIA / 焦点 / 语义 / 复盘)
```

**真实结论应该是**:部署问题**不是 bug 数量的主体**,而是「一旦出问题就必须先解决」的**阻塞层**。它难修、跨系统、无法靠读代码验证,但它不是 45%。

**这个纠错很重要** —— 如果我拿 45% 去说服你「必须重构部署」,那是用假数据推结论,正是过去 100+ 轮「用推断代替验证」的翻版。

---

## 1. 修正后的偏移量化

### 1.1 代码量(这部分实测可靠)

```
uia_inject.rs    894   ← PathWrap 原始 dialog.rs 是 176 行 → 5.1x
main.rs          480
uia_event.rs     469
http_server.rs   418
state.rs         305
fg_bypass.rs     155
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

### 1.2 部署面

```
10 个 PowerShell + 1 个 NSIS ≈ 53 KB
+ Scheduled Task + HKLM registry + Program Files 写权限
```

### 1.3 文档面(修正)

**88 篇**(我上次说 86,包含了之后新增的 2 篇)。focus-21 占 56 篇(64%)。

### 1.4 项目自己的证词(最可靠的一条)

`uia_inject.rs:5-9`:
```
// v0.5.0 (2026-10-04) — ROLLBACK to Sep 27 PathWrap port design:
//   The 12 rounds of focus-19/20/21/22 + v0.3.4-0.3.9 introduced so many
//   verification, basename, and WinUI 3 detection layers that the simple
//   working UIA-only SetValue+Invoke approach was lost.
```

**这一条比任何统计都有说服力** —— 代码作者自己写下「12 轮把简单能用的设计搞丢了」,然后 v0.5.1~5.6 又加了回来。

---

## 2. 补全:Sigma FM 原生 binary 系统的完整能力边界

上一版我只验证了「能不能启动」。这次把**生命周期**也查清了,因为 sidecar 是常驻进程,生命周期才是关键。

### 2.1 启动(`processes.rs:331-496`)

```rust
let mut child = command.spawn()...;
let pid = child.id();
let stdout = child.stdout.take();
let stderr = child.stderr.take();
let task_id = next_task_id();

COMMAND_TASKS.lock()?.insert(task_id, Arc::new(Mutex::new(child)));
COMMAND_PIDS.lock()?.insert(task_id, pid);
COMMAND_EXTENSION_MAP.lock()?.insert(task_id, extension_id);

std::thread::spawn(... stdout 逐行 emit "extension-command-progress" ...);
std::thread::spawn(... stderr 逐行 emit ...);
// 另起线程 wait() → emit "extension-command-complete"
Ok(task_id)
```

**三个 registry(`state.rs`)+ 两个事件通道,Sigma FM 完全托管。**

### 2.2 生命周期(这次新查的)

| 触发时机 | 代码位置 | 行为 |
|---|---|---|
| extension 更新安装 | `install.rs:158` | `terminate_all_extension_processes(&extension_id)` |
| 本地扩展安装 | `install.rs:260` | 同上 |
| extension 删除 | `install.rs:184` | 同上 |
| JS 主动取消 | `commands.rs:294` `cancel_extension_command` | 按 taskId kill |
| JS 批量取消 | `commands.rs:302` `cancel_all_extension_commands` | 按 extensionId 全杀 |
| **Sigma FM 退出** | **未查到** | ⚠️ 见 §2.4 |

`terminate_all_extension_processes`(`processes.rs:67-`):
```rust
let task_ids: Vec<String> = COMMAND_EXTENSION_MAP.lock()?
    .iter().filter(|(_, ext_id)| ext_id.as_str() == extension_id)
    .map(|(task_id, _)| task_id.clone()).collect();
```
Windows 下走 `taskkill /PID <pid> /T /F`(`processes.rs:44-46`)。**进程树全杀,不留孤儿。**

### 2.3 权限模型(`parse-extension-permissions.ts:21-29`)

```typescript
const KNOWN_PERMISSIONS = [
  'fs.read', 'fs.write', 'notifications', 'dialogs',
  'shell',        // ← 我们需要的
  'clipboard', 'openUrl', 'view',
];
```

`data/extensions.ts:122-126` 对 `shell` 的标注:
```typescript
'shell': {
  title: 'Shell Commands',
  description: 'Execute shell commands and external programs on your system',
  risk: 'highest',        // ← 最高风险等级,安装时会显著提示用户
},
```

**`shell` 是 Sigma FM 里风险等级最高的权限。** 用户安装时会看到明确警告。这对「Focus Sync」这种功能的正当性有影响 —— 需要在 README/描述里解释为什么需要它。

### 2.4 ⚠️ 未验证:Sigma FM 退出时是否清理子进程

**我没查到 Sigma FM 应用退出时调用 `terminate_all_extension_processes` 的代码。**

可能的情况:
- (a) 有全局清理 hook,我没找到 → 需再查 `lib.rs` 的 `on_window_event` / `RunEvent::Exit`
- (b) 没有 → sidecar 会成为孤儿进程

**如果是 (b),extension 需要自己处理**:监听 `beforeunload` 调 `cancelAllExtensionCommands()`,或者接受孤儿进程(现有 Scheduled Task 方案也是常驻,孤儿问题同样存在)。

**这是 Step 3 的第 3 个未知(U6),实施前必须查清。**

---

## 3. 修正后的 Step 3 方案

### 3.1 能删什么 / 不能删什么(精确版)

| 文件 | 处置 | 原因 |
|---|---|---|
| `installer.nsi` (13.4 KB) | ✅ 删 | 无需 NSIS |
| `register-scheduled-task.ps1` (4.7 KB) | ✅ 删 | 无需 Scheduled Task |
| `manual-install.ps1` (5.9 KB) | ✅ 删 | 无需手动安装 |
| `kill-sigma.ps1` (2.4 KB) | ✅ 删 | 无需重启 Sigma |
| `reopen-sigma.ps1` (2.6 KB) | ✅ 删 | 同上 |
| `unregister-scheduled-task.ps1` (2.7 KB) | ✅ 删 | 无需反注册 |
| `unregister.ps1` (2.2 KB) | ✅ 删 | 同上 |
| `resolve_user_appdata.ps1` (4.9 KB) | ✅ 删 | Sigma 托管路径 |
| `build.ps1` (1.6 KB) | ✅ 删 | 无 NSIS 可构建 |
| **`register.ps1` (13.1 KB)** | ⚠️ **瘦身保留** | 仍需写 `customSettings.__binaries` 登记 |
| **合计** | 删 9 个 ≈ 40 KB,留 1 个 ≈ 13 KB |

### 3.2 为什么 `register.ps1` 无法完全删除

`sigma.binary.getPath(id)` 的数据源(`create-binary-api.ts:10,20`):
```typescript
const BINARY_STORAGE_KEY = '__binaries';
const settings = await storageStore.getExtensionSettings(context.extensionId);
return settings?.customSettings?['__binaries'] || {};
```

**这个 `__binaries` 映射由 Sigma FM 在「用户通过 UI 安装带 binaries 定义的 extension」时写入。**

而 `validation.ts:183-188` 要求 binaries 定义必须带:
```typescript
if (!isNonEmptyString(value.downloadUrl)) return false;   // 必填
if (!isValidIntegrity(value.integrity)) return false;     // 必填
```

**spike 的 exe 是本地 cargo 构建产物,没有 HTTP 发布地址,所以走不了 manifest binaries 自动安装这条路。**

→ **必须由外部脚本把二进制放进 `extension/bin/` 并登记路径**,这正是 `register.ps1` 现在做的事。

**结论:`register.ps1` 从 13.1 KB 瘦身到 ~100 行(只做 extension 注册 + 二进制复制 + `__binaries` 登记),而不是删除。**

### 3.3 瘦身后的 register.ps1 职责

```
1. 把 spike.exe 复制到 %APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync\
2. 在 Sigma FM 的 extensions-storage.json 里写:
   installedExtensions["kizemo.focus-sync"].settings.customSettings.__binaries["focus-sync"] = {
     id: "focus-sync", path: "<上面那个路径>", version: "0.5.6"
   }
3. 就没了
```

**没有 NSIS、没有 Scheduled Task、没有 registry、没有版本号分支逻辑。**

### 3.4 extension JS 的启动代码

```js
// activate() 里
async function startSidecar() {
  const info = await sigma.binary.getInfo('focus-sync');
  if (!info) throw new Error('sidecar not installed; run register.ps1');

  // 用 runWithProgress(非阻塞),不要用 run(阻塞到进程结束)
  const { taskId, cancel } = await sigma.shell.runWithProgress(
    info.path,
    ['--port', '37421'],
    (p) => console.log('[sidecar]', p.line),
  );
  return { taskId, cancel };
}
```

⚠️ **必须用 `runWithProgress`,不能用 `run`** —— `run` 对应 `processes.rs:307-318` 的 `command.output()`,会**阻塞直到子进程退出**。sidecar 是常驻的,用 `run` 会永远挂起。

### 3.5 保留的部分(不要动)

| 保留 | 原因 |
|---|---|
| spike 的 HTTP server (418 行) | extension ↔ sidecar 通信;Step 4 再考虑改 stdio |
| sidecar 常驻设计 | Sigma FM 退出时的清理未知(U6);先保持现状 |

---

## 4. 三个未验证项(实施前必查)

| # | 未知 | 查法 | 阻塞什么 |
|---|---|---|---|
| **U6** | Sigma FM 退出时是否 kill 子进程 | 查 `lib.rs` 的 `on_window_event` / `RunEvent::ExitRequested` / `Exit` 是否调 `terminate_all_extension_processes` | 决定是否需要 JS 侧 `beforeunload` 清理 |
| **U7** | `extensions-storage.json` 的确切写入位置与并发安全 | 读 `registry_storage.rs` 的 `read_extensions_storage_value` 写入端 | 决定 `register.ps1` 怎么写、要不要先停 Sigma FM |
| **U8** | `sigma.shell.runWithProgress` 在 worker 里的完整语义 | 读 `extension-worker.ts:906-940` 的 `postRequest('shell-run-with-progress-start')` 处理端 | 决定 JS 侧是否要处理进度/完成事件的清理 |

---

## 5. 对「是否偏移过多」的最终回答(修正版)

### 5.1 可靠的事实

| 事实 | 证据 |
|---|---|
| 核心代码 176 → 894 行(5.1x) | 文件行数实测 |
| 总代码 3,306 行 | 行数实测 |
| 部署面 0 → 53 KB(11 文件) | 文件实测 |
| 计划文档 88 篇,focus-21 占 56 篇 | 文件实测 |
| **代码作者自己承认 12 轮把简单设计搞丢** | `uia_inject.rs:5-9` 注释 |
| **Sigma FM 有完整的 extension binary 管理** | `binaries.rs` + `commands.rs` 12 个 command |
| **我们在 2026-10-01 主动放弃了它** | `2026-10-01-focus-16-test-results-analysis.md:30` |

### 5.2 我要撤回的断言

- ❌ 「45% 的 bug 是部署类」→ **7%**
- ❌ 「部署问题占最大头」→ **部署是阻塞层,不是数量主体**
- ⚠️ 「Step 3 可省 53 KB」→ **省 40 KB**

### 5.3 仍然成立的结论

**偏移是真实的,但根因不是「部署 bug 太多」,而是:**

1. **同一堵墙撞了两次** —— v0.5.0 回滚过, v0.5.1~5.6 又加回来(`uia_inject.rs:5-9` 自证)
2. **每次改动叠加,无法定位** —— v0.5.5 的 12 个 Fix 一次性 ship,gate bug 与 Fix A/B/D/E/F 互相作用,只有全部 ship 后才暴露
3. **验证手段单一** —— 88 篇文档里,绝大多数是「读源码 → 找逻辑漏洞 → 写修复计划」,缺「实测」

**第 3 条是我最该反省的**:我在 R22-R26 五轮里做的全是静态 review,直到 R25 才第一次 probe 真实 binary,立刻抓到 Q-1(U-V20/U-V21 永远失败)。

**实测 > 静态推理。这个教训应该写进流程,而不是只写进文档。**

---

## 6. 修正后的行动建议

### 立即(零风险,今天可做)

**Step 2:v0.5.6 gate 修复(5 行)** —— 与架构调整完全独立
- `state.rs:131` `should_opened_write_back` → `!initial_path.is_empty()`
- `main.rs:30` `default_value` → `"C:\\"`
- `main.rs:409-422` 删 skip sentinel 分支
- 用实测存在的路径验证:`D:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe` + `schtasks KizemoFocusSync`

### 并行(今天可做,只读)

**Step 1.5:查 U6 / U7 / U8** —— 三个未知决定 Step 3 的具体形态

### 待定(等 Step 2 验证 + U6/U7/U8 明确)

**Step 3:切原生 binary 机制** —— 删 9 个脚本,瘦 1 个,去掉 NSIS + Scheduled Task + Program Files 写权限

**Step 4:spike 瘦身** —— `http_server.rs` 418 → stdio ~80;砍 `fg_bypass.rs` 155;删 `app_whitelist` 死代码

---

## 7. 元数据

| 项 | 值 |
|---|---|
| **性质** | 自我审计 + 纠错 + 补全 |
| **纠错** | E-1(45% → 7%,编造数字)/ E-2(register.ps1 无法删)/ E-3(53 KB → 40 KB) |
| **补全** | 生命周期管理(4 个触发点 + 3 个 registry)/ `shell` 权限风险等级 highest / `run` vs `runWithProgress` 的阻塞差异 |
| **新增未知** | U6(Sigma FM 退出清理)/ U7(storage 写入并发)/ U8(worker 端语义) |
| **核心教训** | 88 篇文档里第一次实测 binary(R25)就抓到静态 review 5 轮都没发现的问题 |

---

## 8. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-spike-to-sigma-self-audit.md(必读)。

自我审计发现我上一版分析有 1 处编造:
  E-1「45% 的 bug 是部署类」→ 实际 7%(6/88 篇)
     真实分布:部署 7% / v0.5.x ship-plan 迭代 14% / 其他 79%
     修正理解:部署不是数量主体,是「一旦出问题就阻塞」的阻塞层

补全的 Sigma FM 原生机制:
  生命周期 4 个触发点(install/update/delete 都会 terminate_all_extension_processes)
  Windows 下 taskkill /PID /T /F 杀进程树
  sigma.shell 需要 "shell" 权限,risk: 'highest'(Sigma FM 最高风险权限)
  ⚠️ 必须用 runWithProgress 不能用 run —— run 对应 command.output() 会阻塞到进程退出

Step 3 精确版:删 9 个脚本(40 KB),register.ps1 瘦身保留(13 KB → ~100 行)
  原因:sigma.binary.getPath 读 customSettings.__binaries,
        而 manifest binaries 要求 HTTP downloadUrl + integrity,
        本地 cargo 产物没有发布地址 → 无法走自动安装

新增 3 个未知(实施前必查):
  U6: Sigma FM 退出时是否 kill 子进程
  U7: extensions-storage.json 写入位置与并发安全
  U8: extension-worker.ts runWithProgress 的完整语义

立即可做:Step 2(v0.5.6 gate 修复 5 行,零风险,独立于架构调整)
```