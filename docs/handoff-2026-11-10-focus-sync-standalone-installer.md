# 交接文档：Focus Sync 扩展独立安装器对齐新架构（2026-11-10）

> 接上一份：`docs/handoff-2026-11-10-focus-sync-installer-cleanup.md`
> （那份处理的是 **AFM 主安装器** 侧的 legacy 计划任务清理，3.13.5 已完成并通过真机 A/B 验收。）

---

## 1. 背景与本轮目标

侧车（`focus-sync-sidecar.exe`）的生命周期在 3.13.5 已经改成
**「Alpha File Manager 主进程启动时拉起、退出时结束」**：

- 主进程侧：`src-tauri/src/focus_sync_sidecar.rs::resolve_sidecar_path` 只按两个路径找二进制，
  **不读注册表**；命令行只有 `--port 37421`，**不带 `--service`**。
- 扩展侧：`extensions/kizemo.focus-sync/dist/index.js` 只通过 HTTP `127.0.0.1:37421` 与侧车通信，
  **不依赖注册表**。

但扩展的**独立安装器**（`extensions/kizemo.focus-sync/installer/installer.nsi`）当时还在
**STAGE 6.5 注册 legacy 的 `KizemoFocusSync` 计划任务**（AtLogOn + `--service`）。
一旦有人用独立安装器装扩展，legacy 任务就会被重新注册：

- 侧车的生命周期又被拖回「和应用无关」的旧模式；
- 该任务带 `-RestartCount 3 / -RestartInterval 1min`，会不断把 `--service` 侧车复活，
  和主进程拉起的侧车**抢 37421 端口**。

上一份交接文档把它列为「下一轮第一优先级」。**本轮就是做这件事**，顺带修掉调研中
发现的同一安装器上的其它缺陷（进程名、品牌、版本号漂移）。

---

## 2. 改了什么（逐文件）

### 2.1 `extensions/kizemo.focus-sync/installer/installer.nsi`（重写，17,317 B）

| 改动 | 说明 |
|---|---|
| 版本 0.5.8 → **0.5.9** | `PRODUCT_VERSION` / `PRODUCT_SIDECAR_VER` / 头部注释 |
| **删除 STAGE 6.5 的 `register-scheduled-task.ps1` 调用** | 改为调用 `remove-legacy-focus-task.ps1`（幂等清理） |
| 新增 **Stage 0** | 把 `remove-legacy-focus-task.ps1` + `stop-sidecar-processes.ps1` 打包到 `$INSTDIR\bin` |
| 新增 **Stage 1.5** | 注销 legacy 计划任务（在部署侧车二进制之前） |
| 新增 **Stage 1.6** | 结束仍在运行的侧车进程（否则 exe 被占用，Stage 2.5 覆盖会失败） |
| **Stage 2.5 只保留规范位置一处**部署 | `<appdata>\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`（与主进程 `resolve_sidecar_path` 的候选②一致）；不再打包 `register-scheduled-task.ps1` / `unregister-scheduled-task.ps1` |
| Stage 1 / `un.onInit` 进程检测 | `Get-Process -Name alpha-file-manager,sigma-file-manager`（兼容改名前后） |
| 卸载段 | 改调 `remove-legacy-focus-task.ps1` + `stop-sidecar-processes.ps1` |
| 文案 | 全部 DetailPrint / MessageBox / MUI 文案改为**简体中文**，品牌统一为 **Alpha File Manager**；删掉「Scheduled Task: KizemoFocusSync (AtLogOn, --service mode)」那行 |

### 2.2 `extensions/kizemo.focus-sync/installer/stop-sidecar-processes.ps1`（新增，3,042 B）

按 **4 条路径模式**杀 `focus-sync-sidecar` 进程，并最多等 10 秒确认进程真的退出
（释放文件句柄），退出码 0/1。纯 ASCII。

**为什么要单独一个脚本**：共享的 `remove-legacy-focus-task.ps1` 里那三条路径模式
（`*\kizemo.focus-sync\bin\focus-sync-sidecar.exe`）**匹配不到**规范的嵌套路径
`...\bin\focus-sync-sidecar\focus-sync-sidecar.exe`。为了让主安装包字节保持不变，
**没有**去改共享 helper，而是新增一个职责单一的脚本。

### 2.3 `extensions/kizemo.focus-sync/installer/register.ps1`

- **版本号不再硬编码**：原来第 135/233/270/274/288/299/300 行写死 `'0.5.5'`，
  而 `package.json` 已经是 `0.5.9` —— 漂移了好几个版本。
  现在从**已部署的** `package.json` 读版本
  （`$appDataDir\extensions\kizemo.focus-sync\package.json`；
  `installer.nsi` 的 Stage 3 先复制 package.json，Stage 5 才跑本脚本，顺序成立），
  读不到时回退 `0.5.9`。
- **manifest description** 改成
  `Listary-style global focus sync. The sidecar runs as a child process of Alpha File Manager and exits with the app.`
- **新增：就地修补过期 description**（旧安装写的是「Sidecar runs as a Windows Scheduled Task」，
  这句话描述的是**已经退休**的架构，会在「已安装」标签页里误导用户）。
  沿用既有技巧：只重写这一个字符串字段，不整体重新序列化条目（round-19c 教训）。
- 品牌：`Sigma FM` → `Alpha File Manager`（22 处）。

### 2.4 `extensions/kizemo.focus-sync/installer/unregister.ps1`

- 两处 em-dash 改成 ASCII `-`（PS 5.1 按 ANSI 读无 BOM 脚本，中文/破折号会乱码）。
- **修了一个真实的数据损坏风险**：`Get-Content $jsonPath -Raw` 在 PS 5.1 里按 ANSI（cp936）
  解码，`ConvertFrom-Json` 拿到的是乱码，然后本脚本又把它**写回去** ——
  会把别的扩展标题里的 em-dash 写成乱码。
  改成 `[System.IO.File]::ReadAllText($jsonPath, [System.Text.Encoding]::UTF8)`（与 `register.ps1` 同款修复）。

### 2.5 `extensions/kizemo.focus-sync/installer/kill-sigma.ps1` / `reopen-sigma.ps1`

- `kill-sigma.ps1`：`Get-Process -Name sigma-file-manager` → **`alpha-file-manager,sigma-file-manager`**。
  应用已改名，**原来只杀旧名 ⇒ Stage 1 的「关闭应用」检测/关闭完全失效**。
- `reopen-sigma.ps1`：只认旧路径/旧注册表 ⇒ 改成同时探测
  `Alpha File Manager` / `Alpha File Manager_is1` / 旧 key，
  二进制名同时试 `alpha-file-manager.exe` / `Alpha File Manager.exe` / 旧名，
  常见安装路径补 `D:\Program Files\Alpha File Manager\` 与 `C:\Program Files\Alpha File Manager\`。

### 2.6 `extensions/kizemo.focus-sync/installer/build.ps1` / `README.md` / `register-scheduled-task.ps1`

- `build.ps1`：输出名 `kizemo.focus-sync-0.5.8-setup.exe` → **`0.5.9`**（3 处）。
- `README.md`：**整篇改写为简体中文**，重写侧车生命周期说明、文件清单表、安装/卸载步骤、
  排障步骤、已知约束（`powershell -File` 形态、`.ps1` 纯 ASCII 约束、legacy 脚本只留作参数出处）。
- `register-scheduled-task.ps1`：**文件保留**，头部加「已停用 / 不再被执行」说明块
  （交接文档 §8 引用它作为 `RestartCount 3 / RestartInterval 1min` 的出处），正文原样保留。

### 2.7 `src-tauri/installer/__tests__/focus-sync-standalone-installer.test.ts`（新增门禁）

24 条用例，静态守上面这些事实。放在 `src-tauri/installer/__tests__/` 是因为
**`src-tauri/**` 在 `eslint.config.ts` 的 `globalIgnores` 里** ⇒ 不引入 lint 噪音。

守的断言（节选）：

- `installer.nsi` 不再引用 `register-scheduled-task.ps1`、不再出现 `--service`、
  不再出现 `schtasks /Create` / `Register-ScheduledTask`；
- 改为调用 `remove-legacy-focus-task.ps1` / `stop-sidecar-processes.ps1`，
  且**必须用 `powershell -File "$INSTDIR\bin\..."` 形态**；
- **结束侧车进程必须发生在部署侧车二进制之前**（否则覆盖会因文件锁失败）；
- 版本号三处一致（`installer.nsi` / `build.ps1` / `package.json`），
  `register.ps1` 不许再硬编码版本号；
- 进程名同时覆盖 `alpha-file-manager` 与 `sigma-file-manager`；
- 所有 `.ps1` helper **必须纯 ASCII**；
- 用户可见文案用 Alpha File Manager 品牌。

---

## 3. 真机验收记录（2026-11-10）

产物：`sigma-file-manager/extensions/kizemo.focus-sync/installer/kizemo.focus-sync-0.5.9-setup.exe`
（983,915 B，`makensis` 无 warning）。
验收脚本：`.superpowers/sdd/accept-fs-standalone.cjs`，日志：`.superpowers/sdd/accept-fs-standalone.log`。

| # | 验收项 | 结果 |
|---|---|---|
| 1 | 安装器退出码 | `0`（`/S` 静默） |
| 2 | 安装器内 legacy 任务清理是否真的执行 | **是**：helper 日志 `%TEMP%\afm-legacy-task-cleanup.log` 出现两次 `remove-legacy-focus-task START`，两次都 `RESULT=LEGACY_TASK_GONE`（Stage 1.5 与 Stage 6.5 各一次）⇒ **`powershell -File` 形态在安装器上下文确实生效** |
| 3 | 安装后 `schtasks /Query /TN KizemoFocusSync` | **不存在**（`status=1`） |
| 4 | 侧车归属 | `focus-sync-sidecar.exe` pid 91900，**父进程 144064 = `alpha-file-manager.exe`** |
| 5 | 侧车命令行 | `--port 37421`，**`has_service=false`**（无 `--service`） |
| 6 | 安装后 75 秒再看 legacy 任务 | **仍不存在**（不复活） |
| 7 | 扩展注册 | `user-extensions.json` 里 `kizemo.focus-sync` 存在，`version=0.5.9`、`manifest.version=0.5.9`、`isLocal=true`、description 为新文案 |
| 8 | 卸载 | `uninst.exe /S` 退出码 0；扩展目录被删、APPDATA 侧车二进制被删、条目从 `user-extensions.json` 移除（其余扩展 `kizemo.git` / `sigma.clipboard` 保留）、legacy 任务仍不存在、侧车进程无残留、`$INSTDIR` 被清空 |
| 9 | 卸载后重装（恢复可用状态） | 重装成功；重启应用后侧车重新成为 AFM 子进程（pid 91900 ← 144064），无 `--service`，legacy 任务仍不存在 |

**最终机器状态**：应用运行中（pid 144064），Focus Sync 已安装并注册，legacy 任务不存在。

---

## 4. 已验证 / 未验证

### 4.1 已验证

- 门禁：新增测试 24 条全绿；`src-tauri/installer/__tests__` 全量通过；
  `npm run ts`（vue-tsc）通过；`eslint .` = **177 errors / 0 warnings**（与上一轮基线一致，本轮改动路径 0 新增）；
  `node scripts/verify-changelog.mjs` → OK。
- 真机：上表 9 项全部通过。

### 4.2 未验证 / 已知限制

- **主安装包没有重建**：本轮没有动 `src-tauri/installer/hooks.nsh` 与 `kill-app-tree.ps1`，
  依据是主安装器**按镜像名**杀侧车（`HelperImages` 含 `focus-sync-sidecar.exe`），与路径无关。
  ⇒ 主安装包 3.13.5 的字节与上一轮一致，**上一轮的主安装器 A/B 验收结论继续有效**。
- **STAGE 7 失败路径仍未覆盖**（上一轮就遗留：SHA256 判据在「源与已装内容本就相同」时无法区分刷新/未刷新）。
- 静默安装时 NSIS 不写 `install.log`（本安装器没开 `LogSet on`），
  所以安装器内部阶段只能靠 helper 自己的日志取证。
- 扩展描述里的破折号仍由 `$emdash = [char]0x2014` 在运行时注入（沿用旧做法，`.ps1` 保持纯 ASCII）。

---

## 5. 本轮踩坑

1. **`.cmd` / `.ps1` 之外还要注意 `edit` 工具的并行写**：同一批并行 `edit` 里，
   后面的几条因为文件已被前面的编辑改过而报「找不到原文」。
   ⇒ 同一文件的多处改动，**要么串行，要么 old_string 互不重叠且重试**。
2. **`edit` 的 `old_string` 末尾带不带换行会改变语义**：只改一行却把换行删掉，
   会把下一行**合并**进这一行（`build.ps1` 的注释行就是这样被吞掉一行的）。
3. **PowerShell 里给 node 传带 `$` 的替换串要小心**：双引号里的 `\$extensionVersion`
   会被 PowerShell 先展开成空串。批量改文本时优先用 `edit` 工具，其次写 `.cjs` 脚本文件再跑。
4. **NSIS 注释里会「提到」被禁的字符串**：门禁断言「源码里不许出现 `--service`」时，
   头部注释为了说明历史必然提到它。⇒ 断言要**剥掉注释**（或只匹配执行部分）。
5. **`wmic` 在新版 Windows 已被移除**：验收脚本里用
   `Get-CimInstance Win32_Process` + `ConvertTo-Json` 代替 `wmic process get`。
6. **静默模式下 NSIS 不写 `install.log`**：`nsExec::ExecToLog` 的输出无处可取，
   要取证就得让 helper 自己写日志文件（本轮沿用 `%TEMP%\afm-legacy-task-cleanup.log`）。

---

## 6. 提交范围与后续建议

**本轮改动（内仓 `sigma-file-manager`，只做本地提交，不 push）**：

- `extensions/kizemo.focus-sync/installer/installer.nsi`（重写）
- `extensions/kizemo.focus-sync/installer/stop-sidecar-processes.ps1`（新）
- `extensions/kizemo.focus-sync/installer/register.ps1`
- `extensions/kizemo.focus-sync/installer/unregister.ps1`
- `extensions/kizemo.focus-sync/installer/kill-sigma.ps1`
- `extensions/kizemo.focus-sync/installer/reopen-sigma.ps1`
- `extensions/kizemo.focus-sync/installer/build.ps1`
- `extensions/kizemo.focus-sync/installer/README.md`（中文重写）
- `extensions/kizemo.focus-sync/installer/register-scheduled-task.ps1`（加停用说明）
- `src-tauri/installer/__tests__/focus-sync-standalone-installer.test.ts`（新门禁）

**不提交**：`extensions/kizemo.focus-sync/sidecar/src/bin/`（上一轮就存在的遗留未跟踪文件）。

**后续建议（下一轮第一优先级）**：

1. 主安装器 STAGE 7 失败路径的覆盖（用「源与已装内容不同」的素材做对照）。
2. 扩展独立安装器**静默安装的阶段级日志**（`LogSet on` + `LogText`），
   便于下次排障时不用只靠 helper 日志反推。
3. `manual-install.ps1` / `unregister.ps1` 与新版生命周期的对齐巡检
   （本轮只改了 `unregister.ps1`，`manual-install.ps1` **未审**）。
