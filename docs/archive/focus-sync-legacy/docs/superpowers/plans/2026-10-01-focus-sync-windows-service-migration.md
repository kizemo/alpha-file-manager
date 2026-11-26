# focus-sync Sidecar 迁移到 Windows Service 实施方案

**日期**: 2026-10-01
**作者**: Claude Code (systematic-debugging)
**状态**: ⏳ 待用户确认 + 执行
**承接**: `handoff-focus-sync-retrospective-2026-10-01.md` + 本会话根因分析
**目标**: 让 `focus-sync-sidecar.exe` 从 Sigma FM 的 child process tree 中**彻底脱离**,作为 Windows Service 独立运行,从根上消除 taskkill 死亡问题

---

## 1. 问题诊断(完整证据链)

### 1.1 症状
- focus-1..15 共 15 个 installer 都经历过同样失败模式
- 用户报告"反复尝试一天多,仍无效"
- 表现:安装后第一次启动时 sync 工作,但**几秒到几分钟后**突然不工作
- "Couldn't find callback id 872894121" 是**唯一一次**出现在 user 日志中的错误
- HTTP 协议层 PUSH 200 OK,get_status 200 OK,active_dialogs=0 也返回,**侧车死了但协议无感**

### 1.2 真正根因(从 Sigma FM 源码验证)

#### 死亡链路:taskkill /F 硬杀

```
reload trigger (7 类)
  ↓
loader.ts:91 unloadExtensionRuntime OR loader.ts:298 reactivateExtensionRuntime
  ↓
worker-runtime.ts:481 disposeActivationResources
  ↓
worker-runtime.ts:414 resourceDisposables[i].dispose()
  ↓
create-shell-api.ts:171 shellTask.cancel()
  ↓
invokeAsExtension('cancel_extension_command', { taskId })
  ↓
src-tauri/src/extensions/processes.rs:498-548
  ↓
processes.rs:527 terminate_process_tree(pid) [Windows 分支 line 37-65]
  ↓
taskkill /PID {pid} /T /F    ← ★ 硬杀进程树,37421 listener 瞬间断
  ↓
sidecar death
  ↓
extension 调 /set_path → connection refused
  ↓
4 次 retry 全失败 (150/300/450ms × 4)
  ↓
报 "Couldn't find callback id" 或 "push failed after 4 attempts"
```

#### 7 类 reload 触发器(全已读源码确认)

| # | 触发器 | 文件:行 | 调用的杀 sidecar 路径 |
|---|---|---|---|
| 1 | extension disable/uninstall | `sigma-file-manager/src/modules/extensions/runtime/loader.ts:91-127` | `workerHost.destroy()` → `disposable.dispose()` → cancel |
| 2 | **i18n locale 变化** ⭐ 隐藏杀手 | `sigma-file-manager/src/modules/extensions/runtime/extensions.ts:2091-2093` `watch(i18n.global.locale, () => reloadExtensionsLocale())` | `reactivateExtensionRuntime` → `disposeActivationResources` → cancel |
| 3 | binary 下载/编辑 | `sigma-file-manager/src/modules/extensions/pages/extensions.vue:185-203` `reloadExtensionsAfterBinaryEdit` | `unloadExtension` → destroy → cancel |
| 4 | install/update/refresh/delete | `extensions.ts:844, 972, 1040, 1137, 1186` (5 处) | `unloadExtension` + `cancel_all_extension_commands` → `install.rs:90-102 terminate_all_extension_processes` → taskkill |
| 5 | Sigma FM 重启 | 隐式触发 #2 #4 | worker 重建 + locale 初始化 |
| 6 | locale change (启动时) | `extensions.ts:1593-1610` `reloadExtensionsLocale` | 同 #2 |
| 7 | 主页 mount / reload window | `builtin-commands/index.ts:300-303` | 隐式 reload |

⭐ #2 是**隐藏的常驻杀手**:即使 extension 不更新,只要用户切语言(中/英/其他)或初始化时 locale watcher 触发一次,就必然 taskkill 当前所有 spawn 的 child。

#### 为什么 "manual deploy 成功" 是误导

| 维度 | manual deploy | installer |
|---|---|---|
| 触发 spawn | 用户**手动**跑 spike.exe,不经过 Sigma FM | Sigma FM 启动 → activate → spawn |
| taskkill 触发 | 无 | reload 必然触发 |
| sidecar 寿命 | 长寿 | 短寿 |

**manual deploy 绕过 Sigma FM 的 spawn 体系**,所以不经过 taskkill 路径。这与代码无关,是 spawn 机制本身不对。

### 1.3 "Couldn't find callback id" 的本质

`C:\Users\Duanyi\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\tauri-2.12.0\src\ipc\channel.rs:297, 419, 440` 显示 Tauri 2.12 通过 `webview.eval()` 执行 `window.__TAURI_INTERNALS__.runCallback(callback_id, msg)` 响应 invoke。

- callback id 在 **webview JS shim** 维护
- "Couldn't find callback id" 是 **webview 内部 console.error**,不是 Rust 后端错误
- 出现原因:invoke 响应到达时,callback 已被清理(reload 期间清理或时序竞争)

**这是症状,不是根因。根因是 sidecar 死了,4 次 retry connection refused。**

---

## 2. 方案选型

### 2.1 候选方案对比

| 方向 | 思路 | 优点 | 缺点 | 推荐度 |
|---|---|---|---|---|
| **A** ⭐ | **注册 Windows Service**,sidecar 脱离 Sigma FM 进程树 | 彻底解耦,Sigma FM 重启/卸载不影响 sidecar;taskkill /T 杀不到 | 需要修改 installer + extension 入口逻辑;Windows Service 注册有 UAC 复杂度 | ⭐⭐⭐⭐⭐ |
| B | extension 用 detached spawn wrapper | 不改 fork-keep 大文件 | `run_extension_command` 是阻塞命令,spike 不会 exit,会卡死 extension;且仍然在 Sigma FM 子进程树内(没有 CREATE_NEW_PROCESS_GROUP 也没有 DETACHED_PROCESS 的安全用法) | ⭐⭐ |
| D | 改 `processes.rs` 加白名单跳过 cancel | 改动小 | 是 fork-keep 改动,需要保留在 fork;维护负担;7 个触发器中任何一个仍可触发 cancel | ⭐⭐⭐ |

**选 A**:彻底解决,改动集中在 installer + extension 入口逻辑,不污染 fork 主干。

### 2.2 关键约束

1. **Windows 11 优先**(用户实际使用环境,见 `docs/superpowers/plans/2026-09-24-sigma-fork-and-plugin.md:30`)
2. **GPL-3.0-or-later 兼容**(项目 license,见 `sigma-file-manager/src-tauri/Cargo.toml:5`)
3. **focus-sync-sidecar.exe 已 build 通过**(见 `sigma-listary-spike/target/release/spike.exe`,2.5 MB PE32+ x86-64)
4. **不破坏 spike 的 3 个移植文件版权头**(MIT / Apache-2.0 保留)
5. **installer 走 perMachine**(已用 admin 安装,见 `sigma-file-manager/src-tauri/installer/hooks.nsh`)
6. **本地用户是 Duanyi / Administrators 组,elevated context OK**

---

## 3. 架构变更

### 3.1 Before(当前 — 失败)

```
Sigma FM process tree
├── sigma-file-manager.exe (webview + Tauri runtime)
│   └── focus-sync-sidecar.exe (spawn via std::process::Command, CREATE_NO_WINDOW)
│       └── listen 127.0.0.1:37421
└── (reload trigger) → taskkill /F /T 杀 sidecar
```

### 3.2 After(目标 — 成功)

```
[独立进程树 1: Sigma FM]
Sigma FM process tree
└── sigma-file-manager.exe
    └── (extension 不再 spawn sidecar,只连 127.0.0.1:37421)

[独立进程树 2: Windows Service]
services.exe (Windows SCM)
└── focus-sync-sidecar.exe (由 SCM 管理,无父进程是 Sigma FM)
    └── listen 127.0.0.1:37421
    └── 自动启动 + 自动重启(SCM 失败检测)
    └── 跟登录用户无关(可在 SYSTEM 账号下运行)
```

### 3.3 关键 API 替代

| 当前 | 替换为 |
|---|---|
| `sigma.shell.runWithProgress(spikePath, ['--ipc', 'http', '--port', '37421'], onProgress)` | **不调用**,只连 `http://127.0.0.1:37421` |
| `sigma.shell.cancel()` on dispose | **不调用**(没有 spawn 可取消) |
| `install` 时 copy `bin\focus-sync-sidecar.exe` 到 `%APPDATA%` | `install` 时 `sc.exe create ... binPath=...` + `sc.exe start` |
| `uninstall` 时 `RMDir /r` 删目录 | `uninstall` 时 `sc.exe stop` + `sc.exe delete` + 删目录 |

---

## 4. 实施步骤(task-by-task)

### Phase 1:Windows Service wrapper(2-3 天)

#### Task 1.1:为 sidecar 加 service-mode flag

- **File**: `sigma-listary-spike/src/main.rs`
- **改动**: 接收 `--service` flag 时,跳过 stdout 直接进入 service loop(不依赖 parent)
- **验收**: `spike.exe --service` 直接跑能 listen 37421(已在前面 round 18 验证过,只需补 flag)
- **commit**: `feat(spike): add --service flag for Windows Service deployment mode`

#### Task 1.2:加 service-mode 配置

- **File**: `sigma-listary-spike/src/config.rs`
- **改动**: 新增 `service: bool` 字段,`--service` 启用时关掉 heartbeat(SCM 会监控)、关闭孤儿清理(SCM 管)、保留 strict port
- **验收**: `cargo test --lib` 15/15 PASS,`cargo build --release` 成功
- **commit**: `feat(spike): service-mode config — disable heartbeat for SCM management`

#### Task 1.3:重新 build sidecar

- **命令**: `cd sigma-listary-spike && rustup run 1.85.0 cargo build --release`
- **产物**: `sigma-listary-spike/target/release/spike.exe`
- **验收**: 2.5 MB PE32+ x86-64,`spike.exe --service` 立即 listen 37421

### Phase 2:Installer 加 service 注册(2 天)

#### Task 2.1:创建 service 注册脚本

- **File**: `release/extension-installer/register-service.ps1`(新文件)
- **功能**:
  ```powershell
  param(
    [string]$BinaryPath,        # spike.exe 绝对路径
    [string]$ServiceName = 'KizemoFocusSync',
    [string]$DisplayName = 'Kizemo Focus Sync Sidecar'
  )
  # 1. 如果已存在,先 stop + delete
  # 2. sc.exe create ... binPath= "$BinaryPath --service" ...
  #    type= own start= auto error= normal
  # 3. sc.exe start
  # 4. sc.exe query 验证 state = RUNNING
  ```
- **验收**: PowerShell 5.1 兼容,sc.exe 退出码处理正确

#### Task 2.2:创建 service 卸载脚本

- **File**: `release/extension-installer/unregister-service.ps1`(新文件)
- **功能**:
  ```powershell
  param([string]$ServiceName = 'KizemoFocusSync')
  # 1. sc.exe stop
  # 2. sc.exe delete
  # 3. sc.exe query 验证 state = (not found)
  ```

#### Task 2.3:修改 standalone installer(Installer B)

- **File**: `release/extension-installer/installer.nsi`
- **改动**:
  - 在 `Section "Install"` 里,文件 copy 完后调 `nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\register-service.ps1" -BinaryPath "$INSTDIR\bin\focus-sync-sidecar.exe"'`
  - 在 `Section "Uninstall"` 里调 `unregister-service.ps1`
- **验收**: focus-16 installer build 通过,装机后 `Get-Service KizemoFocusSync` 显示 Running

#### Task 2.4:修改 bundled installer(Installer A)

- **File**: `sigma-file-manager/src-tauri/installer/hooks.nsh`
- **改动**:
  - POSTINSTALL 阶段 6 后,新增阶段 7:注册 service
  - POSTUNINSTALL 同步新增:卸载 service
- **验收**: `release/Sigma-File-Manager-2.2.0-focus-16-setup.exe` build 通过,装机后 service 自动 running

### Phase 3:Extension 入口改造(1 天)

#### Task 3.1:移除 sidecar spawn,只连 37421

- **File**: `release/extension/dist/index.js`
- **改动**:
  - 移除 `sigma.shell.runWithProgress` 调用
  - 移除 `activate()` 中的 spawn 逻辑
  - 改为 `activate()` 只 set up path listener,**不** spawn sidecar
  - `/set_path` / `/get_status` 直接 fetch 127.0.0.1:37421
  - 加 sidecar ping 检查,失败时弹 notification("Focus Sync service not running, please check Windows Services")
- **关键**: extension 不再持有 sidecar 的 lifetime,**完全无状态**

#### Task 3.2:移除 manifest binaries 段

- **File**: `release/extension/package.json`
- **改动**: 移除 `binaries` 段(Sigma FM 不再 download/run sidecar),`permissions` 保留 `http` + `127.0.0.1:37421`
- **保留**: `commands` + `toolbar` + `notifications`

#### Task 3.3:extension 改为无 spawn-error notification

- **`:activate`** 失败处理:
  ```js
  const reachable = await ping('http://127.0.0.1:37421/get_status');
  if (!reachable) {
    sigma.ui.showNotification({
      title: 'Focus Sync',
      description: 'Service not running. Open Windows Services and start "Kizemo Focus Sync".',
      type: 'error',
      duration: 15000
    });
  }
  ```

### Phase 4:验证(2-3 天)

#### Task 4.1:Installer A 装机测试

- [ ] 卸载旧 focus-N
- [ ] 装 focus-16
- [ ] `Get-Service KizemoFocusSync` 显示 Running
- [ ] 启动 Sigma FM
- [ ] 切到 Edge → Save As → Sigma FM tree 切 → dialog 自动跳转 ✓
- [ ] **关键**: 切 Sigma FM 语言 zhs → en → zh-tw,看 sync 是否仍工作 ✓(这是修复 1 的核心验证)
- [ ] 重启 Sigma FM 5 次,每次切路径,看 sync 是否仍工作 ✓

#### Task 4.2:Installer B 装机测试

- [ ] 在另一台机器(或 VM)装 Installer B(已有任意 Sigma FM)
- [ ] service 自动注册
- [ ] sync 正常工作

#### Task 4.3:卸载流程验证

- [ ] 卸载 Installer B
- [ ] `Get-Service KizemoFocusSync` 显示 not found
- [ ] `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\` 被删

#### Task 4.4:服务持久化验证(终极验证)

- [ ] Sigma FM 在跑,sidecar service 在跑
- [ ] **杀 Sigma FM**(`taskkill /F /IM sigma-file-manager.exe`)
- [ ] sidecar service 仍然 RUNNING ✓(这就是修复的核心价值)
- [ ] 重启 Sigma FM,extension re-activate,**不 spawn**,直接连 service → sync 仍工作 ✓

### Phase 5:Release & 文档(1 天)

#### Task 5.1:打 tag v0.3.0

- 在 `kizemo/focus-sync` repo 打 v0.3.0
- release body: 列出 service-mode 改动 + 装机说明 + 验证证据
- assets: 新的 sidecar binary + package.json + manifest

#### Task 5.2:更新 FORK-KEEP-LIST.md

- 在 `docs/FORK-KEEP-LIST.md` 加新章节"Windows Service 化"
- 记录:
  - installer changes(POSTINSTALL 阶段 7)
  - extension 入口改造
  - 移除 manifest binaries 段

#### Task 5.3:更新 handoff

- 写 `handoff-2026-10-XX-windows-service-migration.md`
- 列出 commit 链 + SHA + 测试证据 + 用户验收步骤

---

## 5. 验证清单(acceptance criteria)

### 5.1 必须满足(否则不算完成)

- [ ] Service 启动后,**Sigma FM 重启 5 次,sync 持续工作**
- [ ] **Sigma FM 完全卸载后,sidecar 仍 RUNNING**(standalone verification)
- [ ] **Sigma FM 切语言(zhs ↔ en ↔ zh-tw)多次,sync 仍工作**(原 reload 杀手场景)
- [ ] Sidecar binary crash 后,SCM 在 5 秒内自动重启 ✓(Windows 默认)
- [ ] 卸载 Installer 后,service 被清理 ✓
- [ ] 7-app 矩阵(Chrome/Edge/Word/VS Code/钉钉/飞书/微信)≥ 5 个 dialog 视觉跳转成功

### 5.2 应满足(理想状态)

- [ ] Service 在 SYSTEM 账号下运行(不需要用户登录就启动)
- [ ] Service 启动延迟 < 5 秒
- [ ] Service 占内存 < 30 MB
- [ ] 网络断开/重连后 sidecar 自动恢复

### 5.3 测试用例

| ID | 场景 | 预期 |
|---|---|---|
| TC-01 | clean install → 启动 Sigma FM → sync 工作 | ✓ |
| TC-02 | sync 工作 → 切语言 zhs→en → sync 仍工作 | ✓ (原 reload 杀手场景) |
| TC-03 | sync 工作 → 重启 Sigma FM 5 次 → 每次 sync 工作 | ✓ |
| TC-04 | sync 工作 → 杀 Sigma FM → 重启 → sync 工作 | ✓ |
| TC-05 | sync 工作 → 杀 spike.exe → 5s 内重启 → sync 工作 | ✓ |
| TC-06 | 卸载 Installer B → service 不存在 | ✓ |
| TC-07 | Edge Save As → Sigma FM tree 切 → dialog 跳转 | ✓ |
| TC-08 | Chrome download → Sigma FM tree 切 → dialog 跳转 | ✓ |
| TC-09 | Word 另存为 → Sigma FM tree 切 → dialog 跳转 | ✓ |
| TC-10 | VS Code Open Folder → Sigma FM tree 切 → dialog 跳转 | ✓ |

---

## 6. 风险与缓解

### 6.1 高风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| Windows Service 注册需要 admin,普通用户装机失败 | 中 | 高 | Installer 已是 perMachine,UAC 会触发;manifest 加 `<requestedExecutionLevel level="requireAdministrator"/>` |
| Service 在 SYSTEM 账号下跑,可能无法访问 user-context 注册表/文件 | 中 | 中 | 测试 spike.exe 在 SYSTEM 下的行为;若需要 user context,用 `sc.exe ... type= own obj= "DOMAIN\User" password= ...` |
| 用户已有 focus-N 装过,旧 focus-N install 用的是 sidecar-binary-in-APPDATA 路径,新版要 uninstall 旧的 | 高 | 中 | Installer B 在 uninstall 时检测 service 是否存在,有则清理;Installer A POSTUNINSTALL 同步 |
| Sidecar 跑在 SYSTEM 账号,user-extensions.json 可能被 Sigma FM 重写导致 binary 路径失效 | 低 | 低 | service 不依赖 binary 路径(安装时固定),Sigma FM 不会动 |

### 6.2 中风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| PowerShell 5.1 上 `sc.exe` 调用兼容性 | 低 | 中 | 测试所有退出码;不依赖 PS 6+ 特性 |
| Service 注册后,SIGTERM 走不同路径,spike.exe 的 graceful shutdown 路径未覆盖 | 低 | 低 | 测试 `Stop-Service KizemoFocusSync`,spike 应在 5s 内退出 |
| 多用户切换时,service 在 SYSTEM 跑,但 sidecar 监听 127.0.0.1,只对安装时的 user 可见 | 中 | 中 | 在多用户机器上,每个用户的 Sigma FM 看到不同 service instance;需要 service name 加 username 后缀,或一个 service 支持所有 client(单实例 OK) |
| Network profile 切换(public/private)影响 127.0.0.1 binding | 低 | 低 | 不影响,127.0.0.1 是 loopback |

### 6.3 低风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| service 启动失败,SCM 自动重启 storm | 低 | 低 | SCM 默认 3 次失败后停;用户手动干预 |
| antivirus 把 spike.exe 当 malware | 中 | 高 | spike 已 build 通过,代码签名可选;先 release unsigned,等用户报告问题再说 |

---

## 7. 回滚方案

### 7.1 如果 service 注册失败(装机阶段)

- Installer POSTINSTALL 失败 → service 不注册 → 用户回退到 focus-15(原 spawn 模式,可能有 taskkill 问题但能跑)
- 用户操作: `sc.exe delete KizemoFocusSync`(手动清理),然后重装 focus-15

### 7.2 如果 sync 工作不稳定(service 模式有 bug)

- 回退到 focus-15 installer 即可
- service 不影响旧 installer 的 spawn 路径

### 7.3 如果 commit 链有问题

- 整个 PR 没合,只针对本地
- 用户继续用 focus-15 + kizemo/focus-sync v0.2.0 release

---

## 8. 引用与依赖

### 8.1 关键文件位置(都要改)

```
F:\soft\00selfmade\filemanager\
├── sigma-listary-spike\
│   ├── src\
│   │   ├── main.rs                    # Task 1.1: --service flag
│   │   └── config.rs                  # Task 1.2: service-mode config
│   └── target\release\spike.exe       # Task 1.3: build 产物
├── release\
│   ├── extension\
│   │   ├── package.json               # Task 3.2: 移除 binaries 段
│   │   └── dist\index.js              # Task 3.1, 3.3: 入口改造
│   └── extension-installer\
│       ├── installer.nsi              # Task 2.3: 加 service 注册调用
│       ├── register-service.ps1       # Task 2.1: 新文件
│       ├── unregister-service.ps1     # Task 2.2: 新文件
│       └── build.ps1                  # 可能需要改(纳入新文件)
└── sigma-file-manager\
    └── src-tauri\installer\hooks.nsh  # Task 2.4: bundled installer POSTINSTALL 阶段 7
```

### 8.2 必读的前置文档

- `handoff-focus-sync-retrospective-2026-10-01.md`(本次根因分析)
- `docs/superpowers/plans/2026-09-24-sigma-fork-and-plugin.md`(fork 工作流总览)
- `docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md`(spec 上下文)
- `docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`(PASS 决策)
- `docs/FORK-KEEP-LIST.md`(同步策略)

### 8.3 必须保留不变的

- `sigma-listary-spike/src/uia_inject.rs` 版权头(MIT, PathWrap)
- `sigma-listary-spike/src/uia_event.rs` 版权头(MIT, PathWrap)
- 任何已 commit 的 spike 移植文件版权头

---

## 9. 时间线估算

| 阶段 | 工作量 | 累计 |
|---|---|---|
| Phase 1: sidecar 加 service flag | 0.5 天 | 0.5 天 |
| Phase 2: installer 加 service 注册 | 1.5 天 | 2 天 |
| Phase 3: extension 入口改造 | 1 天 | 3 天 |
| Phase 4: 验证 | 2-3 天(用户机器手动) | 5-6 天 |
| Phase 5: release & 文档 | 0.5 天 | 5.5-6.5 天 |

实际可能更长,**因为用户机器手动验证无法压缩**。

---

## 10. 待确认问题

需要在 Phase 1 开始前由用户确认:

1. **Service 账号**:SYSTEM(无需登录)还是当前用户(标准生命周期)?**推荐 SYSTEM**
2. **多用户场景**:多用户切换时,service 在 SYSTEM 跑是 OK 的吗?
3. **service name**:用 `KizemoFocusSync` 还是 `KizemoFocusSyncSidecar` 或 `SigmaFMFocusSync`?
4. **是否保留 spawn 模式作为 fallback**:有些用户可能有特殊环境(企业管控)不让注册 service,是否需要 installer 检测并 fallback 到 spawn 模式?

---

**结束。下次 session 接手时,从 Phase 1 Task 1.1 开始。**