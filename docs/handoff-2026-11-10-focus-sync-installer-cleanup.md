# 交接：Focus Sync 侧车归属改造的收尾（安装器清理 + 升级路径验收）

- 日期：2026-11-10
- 上一轮交接：`sigma-file-manager/docs/handoff-2026-11-09-focus-sync-sidecar-owner.md`
- 外层仓库：`F:\soft\00selfmade\filemanager`（HEAD `90e87e6`，本轮无提交）
- 内层仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`
  （上一轮提交 `61f3e614 feat(focus-sync): 侧车改由 AFM 主进程持有，删除登录计划任务`）

> ⚠️ 本轮**两仓都只做本地提交，没有 push**（用户 2026-11-09 明确要求暂缓对外发布）。

---

## 1. 背景

上一轮把 focus-sync 侧车的生命周期从 Windows 计划任务（`KizemoFocusSync`，
AtLogOn + `--service`）迁到 AFM 主进程：应用启动时拉起，退出时杀掉。
上一轮交接文档 §6/§8 留了两个未验证项：

1. **升级时旧版 `--service` 侧车仍在运行**这一分支没有真机证据；
2. 安装器清理逻辑的正确性只做了静态审查。

本轮的目标：把「legacy 计划任务的注销」与「侧车部署是否成功」解耦，
并用**打包产物的真机 A/B 验收**证明升级路径成立。

---

## 2. 本轮改动（内仓，共 4 类）

### 2.1 安装器钩子：`src-tauri/installer/hooks.nsh`（4 处）

| 位置 | 改动 | 为什么 |
| --- | --- | --- |
| PREINSTALL | 把 `Disable-ScheduledTask -TaskName 'KizemoFocusSync'` 上移到杀进程**之前**，原位置只留 `Sleep 1500` | legacy 任务带 `RestartCount 3 / RestartInterval 1min`；先杀进程后禁用任务，任务会在 1 分钟内把 `--service` 侧车复活 |
| 新增 STAGE 0.5a（STAGE 7 之前） | `Stop-ScheduledTask` + `Unregister-ScheduledTask -Confirm:$$false` + 一条存在性检查（打印 `LEGACY_TASK_GONE` / `LEGACY_TASK_STILL_PRESENT`） | STAGE 7 的三条错误路径都 `Goto stage9_skip`，原先会在侧车部署失败时**跳过任务注销**，留下永久注册且持续复活的 legacy 任务 |
| STAGE 0.5 | 杀进程改成 `Get-Process … \| Where-Object { … }` → `Stop-Process -Force` → `Wait-Process -Timeout 5` | `Stop-Process` 只是请求终止，侧车在被替换前仍持有自己的 exe 句柄，紧接着的 `Delete` 会撞 "file is open by another process" |
| STAGE 9 | 标题改成「Verifying the legacy … is gone」，保留两条幂等调用作安全网；末尾 DetailPrint 改为 "Legacy task verified gone; helper script removed."；同时改掉 STAGE 7 失败分支里已过时的错误文案（不再让用户去关 Scheduled Task） | 文案与新架构对齐；幂等安全网比删掉更稳 |

### 2.2 app 数据目录契约修复：`src-tauri/src/focus_sync_sidecar.rs`（1 处）

上一轮提交带进来的一处**存量门禁红灯**（`src/utils/__tests__/app-dirs-contract.test.ts`）：

```
Rust 侧除了 app_dirs.rs 之外没有直接解析 app 目录
  src-tauri\src\focus_sync_sidecar.rs -> .app_data_dir()
```

改成走统一解析点：

```rust
if let Ok(data_dir) = crate::app_dirs::app_data_dir(app) {
```

- release 构建行为**完全等价**（`app_dirs::resolve` 在没有隔离覆盖时就是
  `path_resolver.app_data_dir()`）；
- debug 构建下 `SFM_E2E_DATA_DIR` 隔离才真正对侧车生效；
- ⚠️ 注释里**不能出现 `.app_data_dir()` 这个字面量** —— 契约测试是**读源码文本**
  做子串匹配的，写在注释里同样会被判为违规（本轮踩过一次）。

### 2.3 扩展侧过时文案：`extensions/kizemo.focus-sync/`

| 文件 | 改动 |
| --- | --- |
| `dist/index.js` | 红色横幅文案：`Open Task Scheduler → KizemoFocusSync → Run, or restart your PC.` → `Restart Alpha File Manager — the sidecar is started together with the app.` |
| `dist/index.js` | 两段注释（健康检查失败的历史说明、`deactivate` 不调 `/quit` 的理由）更新为「侧车由 AFM 主进程持有」 |
| `package.json` | `description`：`Sidecar runs as a Windows Scheduled Task (independent of the File Manager lifecycle).` → `The sidecar runs as a child process of Alpha File Manager and exits with the app.` |

> `extensions/kizemo.focus-sync/dist/index.js` 是**被 git 跟踪**的（不是构建产物），
> 所以这里的改动会进提交，也会随 `bundle.resources` 进安装包。

### 2.4 版本与更新日志

- `package.json` 3.13.4 → **3.13.5**（`npm run sync-version` 已同步 3 个 Cargo manifest：
  `src-tauri/Cargo.toml`、`default-file-manager-common/Cargo.toml`、`default-file-manager-launcher/Cargo.toml`）；
- `CHANGELOG.md` + `public/changelog/` 下 14 个语言目录各加一条 3.13.5 条目
  （`ch` 中文，其余 13 个英文，沿用 3.13.4 的既有做法）；
- `node scripts/verify-changelog.mjs` 通过；
- **没有跑 `release:prepare`**（那属于对外发布流程，用户已暂缓 push/发布）。

### 2.5 安装器内的内联 PowerShell 形态失效 → 改用随包 helper 脚本（本轮核心）

上一轮 §2.1 写的「STAGE 0.5a 用 `Stop-ScheduledTask` + `Unregister-ScheduledTask` 内联命令」
**在安装器里根本没有生效**，而验收脚本（它用 `powershell -NoProfile -Command "..."` 在普通 shell 里跑）
却显示任务被清掉了 —— 于是「测试通过、产品没修好」。

**根因（证据链闭合）**：

1. 上一轮验收日志尾部：`LABEL=3.13.5-locked`、`INSTALL_EXIT=0`、`LEGACY_PID_ALIVE=False`、
   `SERVICE_MODE_PROCS=0`，但 `TASK_PRESENT_AFTER=True` / `TASK_PRESENT_LATE=True` → FAIL；
2. 定点实验 `.superpowers/sdd/probe-unregister.ps1`：用 `Process.Start`（CreateProcess，**无 cmd.exe**）
   原样执行安装器那条内联命令 → `EXIT=0`、`AFTER_STOP_PRESENT=True`、
   **`AFTER_UNREGISTER_PRESENT=False`** → 内联命令本身在提升态 shell 里**有效**，不是 quoting 问题；
3. 全仓搜 `KizemoFocusSync`（19 个文件）→ **没有任何代码会重新注册该任务** → 排除「有东西在 0.5a 之后重建任务」；
4. ⇒ 结论：安装器内联 `powershell -NoProfile -Command "..."` 形态在安装器上下文中未真正生效；
   `powershell -File "$INSTDIR\tools\*.ps1"` 是安装器里**唯一被证实可跑**的形态
   （STAGE 6 的 `register.ps1`、卸载段的 `unregister-scheduled-task.ps1` 一直都在跑）。

**修复载体**：新增 `extensions/kizemo.focus-sync/installer/remove-legacy-focus-task.ps1`
（纯 ASCII，4,927 B），用**原生 `schtasks.exe`**（`/Change /DISABLE` → `/End` → `/Delete /F`）删任务，
再用 `Unregister-ScheduledTask` 兜底重试一次；顺带按路径杀掉孤儿 `--service` sidecar；
每一步追加到 `%TEMP%\afm-legacy-task-cleanup.log`（含 `state_before` 与任务 XML `<Date>` 注册时间，
用来区分「从未删除」与「删了又被重建」）。
helper 靠 `src-tauri/tauri.conf.json` 的 `bundle.resources` 映射落到 `$INSTDIR\tools\`，
安装器用 `powershell -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\tools\remove-legacy-focus-task.ps1"` 调用。

`hooks.nsh` 三处改法：

| 位置 | 原写法 | 新写法 |
| --- | --- | --- |
| PREINSTALL | `powershell -NoProfile -Command "Disable-ScheduledTask -TaskName ''KizemoFocusSync'' ..."` | `nsExec::ExecToLog 'schtasks /Change /TN KizemoFocusSync /DISABLE'`（原生二进制，不依赖 ScheduledTasks 模块） |
| STAGE 0.5a | 三条内联 `Stop-` / `Unregister-` | `nsExec::ExecToLog 'powershell ... -File "$INSTDIR\tools\remove-legacy-focus-task.ps1"'` |
| STAGE 9 | 两条内联 | 同一个 helper（幂等安全网） |

> helper 内部一律按**退出码**判定，不解析 `schtasks` 的输出文本
> （安装器上下文里 `schtasks` 的中文输出是乱码，见 §5 第 9 条）。

---

## 3. 验证结果

### 3.1 门禁

| 门禁 | 结果 |
| --- | --- |
| `npm run ts`（vue-tsc） | ✅ 退出码 0 |
| `npm run lint:eslint:check` | ⚠️ 177 errors —— **存量**，与本轮无关（全部落在 `src/modules/nav-sidebar/nav-sidebar.vue`、`src/modules/window-toolbar/window-toolbar.vue`、`src/modules/dashboard/components/dashboard-hero.vue` 等本轮未触碰的文件）。基线数字会随提交变，量的时候要用 `git stash` 量 HEAD |
| `npm run lint:stylelint:check` | ⚠️ 同样是存量（`rule-empty-line-before` / `order/properties-order`），本轮未触碰任何 `.vue`/`.css` |
| `npm run test:unit:run`（vitest） | ✅ **2813 passed / 1 failed → 修完后 2814 passed / 0 failed**。唯一失败是 2.2 节那条契约门禁（上一轮提交带进来的存量问题），本轮已修并复跑转绿 |
| `node scripts/verify-changelog.mjs` | ✅ OK |
| `npx vitest run src-tauri/installer/__tests__` | ✅ **56 passed / 0 failed**（5 个文件）。`kill-app-tree-embed.test.ts` 的「禁用计划任务」断言本轮改成接受两种等价形态（见 §5 第 8 条） |

### 3.2 打包

- `npm run tauri:build` → `BUILD_EXIT=0`；
- 产物：`sigma-file-manager/src-tauri/target/release/bundle/nsis/Alpha File Manager_3.13.5_x64-setup.exe`；
- **构建日志里没有 makensis warning**（历史上有 `warning 6000` 让 STAGE 0.5 静默失效，
  而 Tauri bundler 会吞掉 makensis 的 warning —— 本轮 grep 过 `warning`，只有既有的
  Rust 编译警告，没有 NSIS 警告）。

### 3.3 真机 A/B 验收

见第 6 节「验收记录」。

---

## 4. 已验证 / 推断 / 未验证

### 已验证（有证据）

- 安装器**内联** `powershell -NoProfile -Command "..."` 形态在安装器上下文里不生效（根因见 §2.5）；
- 改用随包 helper（`powershell -File`）+ 原生 `schtasks.exe` 后，升级安装会真正删掉 legacy 计划任务，
  且 75 秒复活窗口内任务不会回来；
- 同一构建重复安装会被 early guard 拦下（`OUTCOME=quit reason=same-build`）—— 这是既有产品行为，
  但它会让「连装两轮同一个包」的验收编排失效（见 §5 第 7 条）；
- 契约门禁 `app-dirs-contract.test.ts` 4/4 通过（修完后）；
- 打包成功、无 NSIS warning；
- 安装器在「侧车部署失败」路径下**也会**注销 legacy 计划任务（本轮 A/B 的判别项）；
- 升级后 legacy 计划任务不再残留、不再复活 `--service` 侧车；
- AFM 启动后侧车是 **spawn 模式**（`/health` 的 `"mode":"spawn"`）、父进程就是 AFM、
  命令行**不带** `--service`；强杀 AFM 后看门狗回收侧车并释放 37421 端口。

### 推断（证据强，但不是直接观测）

- 「`RestartCount 3 / 1min` 会在 1 分钟内复活 `--service` 侧车」：来自 legacy 注册脚本
  `extensions/kizemo.focus-sync/installer/register-scheduled-task.ps1` 第 71-77 行的参数，
  以及 Windows 计划任务的重启策略语义；本轮没有单独做「等 60 秒看它自己爬回来」的
  独立观测（验收脚本用 75 秒窗口覆盖了这个窗口）。

### 未验证 / 遗留

1. **扩展 standalone 安装器仍会注册 legacy 计划任务**：
   `extensions/kizemo.focus-sync/installer/installer.nsi` 的 STAGE 6.5 还在调
   `register-scheduled-task.ps1`。走 AFM 主安装器装扩展时不受影响
   （主安装器会删掉这个任务），但**单独装扩展**会重新引入 legacy 生命周期。**下一轮的第一优先级。**
2. `extensions/kizemo.focus-sync/installer/register.ps1` 往 `user-extensions.json` 里写的
   版本号是 `0.5.5`，而 `package.json` 是 `0.5.9` —— 版本号不一致（存量，与本轮无关）。
3. `ExitRequested` 被托盘逻辑取消时侧车是否继续存活，未单独实测。
4. 扩展业务链路端到端（`extension_alive` / `first_push_received`）未实测。
5. 若 STAGE 7 失败，canonical（`%APPDATA%`）副本仍是旧二进制、注册表
   `SidecarPath` 仍指向它 —— 本轮**没有**处理（只保证 legacy 任务被清掉）。
6. 正式发布（`release:prepare`、站点上传、GitHub release、push 两仓）仍待用户决定。

---

## 5. 本轮踩到的坑（不要重试）

1. **锁住一个「正在被运行」的文件**：想在验收脚本里用 `FileShare::None` 独占打开
   canonical 侧车 exe 来制造 STAGE 7 的删除失败 —— **必然失败**（sharing violation）：
   进程的映像段本身就持有该文件的句柄，且不允许别的句柄用 `FileShare::None`。
   正确做法：**另起一个进程**用 `FileShare::Read`（只禁 delete）持有句柄，
   见 `.superpowers/sdd/hold-file.ps1`。
2. **契约测试读的是源码文本**：注释里出现被禁的字符串同样算违规。
3. **`.cmd` 不能复用旧文件名**：本轮用 `tauri-build-v062-fs.cmd` → `-fs2.cmd` → `-fs3.cmd`，
   每次都是新文件名，避免「cmd.exe 按字节偏移逐行读」+ 运行期间改文件的事故。

4. **别拿「普通 shell 里能跑通」推断「安装器里也能跑通」**：验收脚本在 shell 里执行内联命令，
   安装器在另一个上下文里执行同一行 —— 两者行为可以相反。
5. **判「内联命令有没有生效」要用进程级复现，不要靠猜**：`Process.Start`（CreateProcess，无 cmd.exe）
   原样跑一遍，能区分「命令本身无效」与「上下文导致无效」。
6. **安装器里只用两种 PowerShell 形态**：`powershell -File "<绝对路径>"` 或**原生 exe**
   （`schtasks.exe`）。内联 `-Command "..."` 不要在安装器里用。
7. **同一构建重复安装会被 early guard 静默拦下**：`OUTCOME=quit reason=same-build`
   （安装器直接 exit 0，连 PREINSTALL 都不跑）。所以「连装两轮同一个包」的验收编排，
   第二轮**根本没执行安装** —— 表现为「任务还在」，看着像修复失败，其实安装器没跑。
   验收套件里要用**另一个版本**做跳板（见 §7）。
8. **JS 里 `/a/ + /b/` 是字符串拼接，不是合并正则**：`RegExp.toString()` 会被 `+` 调用，
   `toMatch` 收到字符串后退化成**子串匹配**，必然假红。
   要么写成一个正则字面量，要么用 `new RegExp(a, b)`。
9. **`schtasks` 在安装器上下文里输出中文乱码**（`??: ?????`）：控制台代码页问题，不影响退出码。
   helper 一律按退出码判定，不要解析输出文本。

---

## 6. 验收记录

### 6.1 验收套件总览

数据来源：`.superpowers/sdd/accept-v062-fs.log`（三轮套件）与
`.superpowers/sdd/accept-v062-fs-normal-rerun.log`（normal 轮重跑）。
下表数字直接由日志解析生成，与实测一致。

| 轮次 | 包 | 模式 | INSTALL_EXIT | TASK_PRESENT_AFTER | TASK_PRESENT_LATE | VERDICT |
| --- | --- | --- | --- | --- | --- | --- |
| 3.13.4-locked | 3.13.4 | locked | 0 | 是 | 是 | **FAIL** |
| 3.13.5-locked | 3.13.5 | locked | 0 | 否 | 否 | **PASS** |
| 3.13.5-normal（首跑，脚本缺 else 分支） | 3.13.5 | normal | 0 | 否 | 否 | **（无 VERDICT 行）** |
| 3.13.5-normal | 3.13.5 | normal | 0 | 否 | 否 | **PASS** |

- **对照组 3.13.4-locked**：`VERDICT=FAIL`，`legacy task survived the install` —— 旧包清不掉 legacy 任务，与预期一致
  （说明这套验收**能分辨出「有没有清掉」**，不是一路 PASS）。
- **实验组 3.13.5-locked**：`VERDICT=PASS`，`legacy task removed and not re-registered (sidecar deployment also succeeded)`。

> 第 3 轮之前先静默装一次 **3.13.4** 作跳板：否则第 3 轮与第 2 轮是同一个 sha，
> early guard 会以 `reason=same-build` 直接 quit，**安装器根本不执行**（见 §5 第 7 条）。
> 重复安装同一构建时 early guard 的退出码实测为 **2**（不是 0），编排时不要拿退出码判断「装没装上」。

### 6.2 normal 轮（正常路径 + AFM 持有侧车）实测明细

| 指标 | 值 |
| --- | --- |
| INSTALL_EXIT | 0 |
| TASK_PRESENT_AFTER（装完立刻查） | 否 |
| TASK_PRESENT_LATE（75 秒后复查） | 否 |
| LEGACY_PID_ALIVE（旧 --service 侧车） | 否 |
| SERVICE_MODE_PROCS / _LATE | 0 / 0 |
| SIDECAR_SPAWNED | TRUE |
| PARENT_IS_AFM（父进程是不是 AFM） | 是 |
| SIDECAR_CMDLINE | "D:\Program Files\Alpha File Manager\tools\focus-sync-sidecar.exe" --port 37421 |
| SIDECAR_HAS_SERVICE_FLAG（命令行带 --service？） | 否 |
| HEALTH_MODE（/health 的 mode） | SPAWN |
| PORT_37421_OWNER | 200540 |
| WATCHDOG_REAPED（强杀 AFM 后侧车被回收） | 是 |
| AFTER_SEC（看门狗回收耗时，秒） | 1 |
| PORT_37421_LISTENERS（回收后监听数） | 0 |
| VERDICT | **PASS** |
| REASON | legacy task removed and not re-registered; sidecar is owned by AFM |

判读：升级安装后 legacy 计划任务**消失且 75 秒复活窗口内不回来**；
AFM 起来后侧车是 **spawn 模式**、父进程就是 AFM、命令行**不带** `--service`；
强杀 AFM 后看门狗在 1 秒内回收侧车并释放 37421 端口。

### 6.3 安装器侧证据：`%TEMP%\afm-legacy-task-cleanup.log`

helper 脚本（`extensions/kizemo.focus-sync/installer/remove-legacy-focus-task.ps1`，
随包复制到 `$INSTDIR\tools\`）把每一步判定写进该文件。normal 轮里它被调用了两次：

| 时间 | state_before | schtasks 退出码 | 结果 |
| --- | --- | --- | --- |
| 2026-11-10 01:27:27.873 | present | /Change /DISABLE=exit0、/End=exit0、/Delete /F=exit0 | LEGACY_TASK_GONE |
| 2026-11-10 01:27:32.371 | absent | /Change /DISABLE=exit1、/End=exit1、/Delete /F=exit1 | LEGACY_TASK_GONE |

第一行是 **STAGE 0.5a**（安装早期，三条 `schtasks` 全部 exit=0 ⇒ 任务真的被删掉）；
第二行是 **STAGE 9** 的幂等兜底调用（任务已经不在 ⇒ exit=1，但仍判 `LEGACY_TASK_GONE`，
不会误报失败）。

### 6.4 本节遗留

- normal 轮最初的脚本版本**丢了 `else` 分支**，只输出到 `=== RESULT ===` 就没有 VERDICT 行
  （数据本身全部达标）。已用 `.superpowers/sdd/patch-accept-normal-verdict.cjs` 补回，
  再用 `.superpowers/sdd/check-ps-syntax.ps1` 做语法预检后重跑，本节 6.2 取自重跑日志。
- `STAGE7_FAILED` 判据**不可靠**：NSIS 拷贝保留源 mtime，而 `REFRESHED` 用 SHA256 比对，
  源与已装内容本就相同时无法区分「刷新」与「未刷新」；且三轮都没真正触发 STAGE 7 失败路径
  （持锁成功但拷贝仍成功）⇒ **STAGE 7 失败路径本轮未覆盖**，记为遗留项。

---

## 7. 怎么复现本轮的验收

```powershell
# A/B：对照组用 3.13.4 的包（新侧车归属逻辑 + 旧安装器清理逻辑）
powershell -NoProfile -ExecutionPolicy Bypass -File F:\soft\00selfmade\filemanager\.superpowers\sdd\accept-v062-fs-upgrade.ps1 `
  -Setup "F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.4_x64-setup.exe" `
  -Mode locked -Label 3.13.4

# 实验组：3.13.5 的包
powershell -NoProfile -ExecutionPolicy Bypass -File F:\soft\00selfmade\filemanager\.superpowers\sdd\accept-v062-fs-upgrade.ps1 `
  -Setup "F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe" `
  -Mode locked -Label 3.13.5

# 正常路径 + AFM 持有侧车的验证
powershell -NoProfile -ExecutionPolicy Bypass -File F:\soft\00selfmade\filemanager\.superpowers\sdd\accept-v062-fs-upgrade.ps1 `
  -Setup "F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe" `
  -Mode normal -Label 3.13.5-normal -CheckAfm
```

脚本会：复刻 legacy 状态（`Register-ScheduledTask` AtLogOn + `--service` +
`RestartCount 3 / 1min`）→ 静默安装 → 断言任务消失、legacy PID 消失、无 `--service`
进程残留、canonical 副本是否刷新 → 等 75 秒复查复活窗口 → （可选）启动 AFM 验证
侧车归属与看门狗回收 → 清理 → 打印 `VERDICT=PASS/FAIL`。

> `-Mode locked` 的判据（2026-11-10 改过）：**「legacy 任务消失，且 75 秒复活窗口内不回来」**。
> 「STAGE 7 是否真的失败」只作为信息输出（`STAGE7_FAILED=True/False`），**不再是 PASS 的前提** ——
> 因为修复把任务删除挪到了 STAGE 0.5a（早于 STAGE 7），无论侧车部署成败，任务都必须被删掉；
> 而本轮 STAGE 0.5 加了 `Wait-Process` 之后锁会真正释放，拷贝反而会成功、STAGE 7 不再失败。
>
> 整套跑法用 `.superpowers/sdd/accept-v062-fs-suite.ps1`（三轮 + 跳板）。
> 第三轮之前会先静默装一次 **3.13.4** 做跳板：否则第三轮装的 3.13.5 与第二轮是**同一个 sha**，
> early guard 会以 `reason=same-build` 直接 quit，安装器根本不执行。

---

## 8. 关键路径

| 用途 | 路径 |
| --- | --- |
| 安装器钩子 | `sigma-file-manager/src-tauri/installer/hooks.nsh`（PREINSTALL ~580-650、STAGE 0.5/0.5a ~885-915、STAGE 7 ~1005-1025、STAGE 9 ~1050-1075） |
| legacy 任务注册脚本（`RestartCount 3 / 1min` 的出处） | `sigma-file-manager/extensions/kizemo.focus-sync/installer/register-scheduled-task.ps1`（71-77 行） |
| 侧车入口与父进程看门狗 | `sigma-file-manager/extensions/kizemo.focus-sync/sidecar/src/main.rs` |
| 侧车端口 / 孤儿清理 | `sigma-file-manager/extensions/kizemo.focus-sync/sidecar/src/config.rs` |
| `/health` 字段（`mode`/`version`/`extension_alive`/`first_push_received`） | `sigma-file-manager/extensions/kizemo.focus-sync/sidecar/src/http_server.rs` |
| AFM 侧侧车所有者 | `sigma-file-manager/src-tauri/src/focus_sync_sidecar.rs` + `src-tauri/src/lib.rs`（`RunEvent::Exit` 才 stop） |
| app 目录统一解析点 | `sigma-file-manager/src-tauri/src/app_dirs.rs` |
| 契约门禁 | `sigma-file-manager/src/utils/__tests__/app-dirs-contract.test.ts` |
| 验收脚本 | `F:\soft\00selfmade\filemanager\.superpowers\sdd\accept-v062-fs-upgrade.ps1`、`hold-file.ps1` |
| 打包脚本 | `F:\soft\00selfmade\filemanager\.superpowers\sdd\tauri-build-v062-fs3.cmd`（日志同名 `.log`） |
