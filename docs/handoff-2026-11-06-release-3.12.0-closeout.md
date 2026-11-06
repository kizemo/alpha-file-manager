# 交接文档：3.12.0 发版收尾（2026-11-06）

> 给新会话的完整上下文。先通读本文件再动手；所有结论按「已验证 / 推断 / 未验证」分级标注。

## 1. 背景

内层代码仓 `F:\soft\00selfmade\filemanager\sigma-file-manager`（git remote `https://github.com/kizemo/alpha-file-manager.git`）本轮交付三个功能，版本号 3.12.0：

1. **路径分隔符统一为 Windows 原生 `\`**：展示层与剪贴板层转换，不改内部正斜杠表示。核心实现 `src/utils/normalize-path.ts` 的 `toNativePathSeparators` / `getNativePathDisplayValue` + `src/composables/use-native-path.ts`，接入地址栏、条目复制路径、信息面板、全局搜索结果、tab、drive-card、对话框等；另修复 `navigator.vue` 快捷键通道的真机缺陷。
2. **右键「粘贴快捷方式」**：`src/composables/use-paste-shortcut.ts`，新增 `'paste-shortcut'` 动作类型 / 可见性 / 菜单项 / 两条执行通道，复用后端 `create_links`；17 个语言包全部补词条（中文 locale code 是 `ch`）。
3. **快捷方式显示目标图标**：Rust 侧 `dir_reader.rs` 的 `resolve_shortcut_target_path`、`system_icons.rs` 的 `resolve_shortcut_icon_target`（断链回退通用图标），`create_shortcut` 改 `pub(crate)`。文件列表本就走 shell 图标管线，**无需前端接线**（已定性，见 §4）。

发版闭环 = 打包 → 真机装机验收 → 站点收尾（winget/更新清单/上传）→ GitHub 发布 → 恢复用户真实数据。**本文件写作时，闭环已全部走完。**

## 2. 当前状态（全部完成）

- 三任务真机验收**全部定案通过**（打包产物 `tauri build`，非 dev 构建）。
- 站点侧：3.12.0 包 SHA256 `c3eb2c5d870a565aed87da012e9bcc0256d0fd9eb4583562c024692c9f5c302e`（45,112,805 B）；`finish-release-3120.cjs` 重写 4 个 winget yaml + `version.json` + release notes；`npm run check:winget` 全过；SFTP `UPLOAD_EXIT=0`；公网回读 `https://www.aiec.fun/alpha-file-manager/version.json` 已确认。
- GitHub：commit `526dc45e`（66 files，+769/−46）已推送 `main`；annotated tag `v3.12.0` 已推送；**Release 已创建并回读验证**：https://github.com/kizemo/alpha-file-manager/releases/tag/v3.12.0 ，资产 `Alpha_File_Manager_3.12.0_x64-setup.exe`（45,112,805 B），标题风格对齐 v3.11.0。
- 门禁（提交前全量）：vitest 289 files / 2697 tests 通过；`eslint .` 180 errors（基线 188，无回归）；`cargo test --lib` 936 passed；`npm run ts` exit 0。
- 用户数据已恢复真值：杀掉 `alpha-file-manager` 进程后依次 restore，`user-settings.json` 5645 B（`navigator.fileIconTheme` 回 `builtin:default`）、`user-workspaces.json` 2423 B（7 个 directory 标签页恢复）、`user-stats.json` 31,781 B 全程未动。
- 7 个外来窗口已按 `occluders-3120g.round4.txt` 全部挪回原位并实测 rect 核对；验收残留进程（fixture-server / fixture-writer / npx electron remote-debugging）已杀清。
- 本机已安装 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = 3.12.0；应用在收尾时被杀掉，用户可随时重开。

## 3. 下一步（仅剩可选清尾）

1. **手工删除验收夹具** `C:\Users\Duanyi\afm-accept-3120`（demo.txt / notepad.lnk / broken.lnk / demo.txt.lnk / dest 等）。权限闸门拒绝工作区外的恢复性删除（mavis-trash 与 `rm --` 均被拒），只能由用户手工删。
2. （无害，可留）`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\` 下的 `.acc3120-backup`（2423 B）与 `.acc3120-icons-backup`（5645 B）备份文件。
3. （可选）下载公网安装包逐字节算 SHA256 回验——服务端 md5 与本地包已一致，此项只是加固。
4. （可选）重核 3.12.0 新包内侧车 sha256——次要项。
5. winget PR 本轮**刻意未提交**（既定决策）；若日后要提，yaml 已在仓库内。

## 4. 结论分级

**已验证（有硬证据）**
- 任务 2：落盘 `dest\demo.txt - Shortcut.lnk` 876 B，WScript.Shell 复核目标正确（第 3 轮）。
- 任务 3：`t3-sheet.png` 五行对照夹具目视定案——notepad.lnk 显示记事本图标为决定性证据；broken.lnk 回退通用图标（第 4 轮）。
- 任务 1（第 6 轮定案）：`T1-FOCUS forced=True fg=1706060` 证明夺前台成功；`T1[demo.txt]` 剪贴板断言 `sentinel-survived=False backslashes=12 slashes=0 has-expected=True`，clip 首行 = `C:\Users\Duanyi\afm-accept-3120\demo.txt`；目录零斜杠由第 3 轮「复制当前地址」通道（slash-free=True + `f-full-2.png` 像素旁证）覆盖。
- 文件列表图标走 shell 管线、任务 3 无需前端接线（源码级定性 + 真机证据）。
- 第 4/5 轮失效系外部环境（TaskManagerWindow 抢前台），非产品缺陷。

**推断（证据间接）**
- `T1[dest]` 哨兵存活判读为 Radix 菜单枚举竞态导致的重复取证（同通道在 demo.txt 上已成功）；采信第 3 轮证据，不重跑第 7 轮。

**未验证 / 明确留白**
- 公网包逐字节 SHA256 回验（服务端 md5 已一致）。
- 3.12.0 新包内侧车 sha256 重核。
- `f-iconstrip.png`（右键图标条截图）已作废，不影响任何结论。

## 5. 证据路径

脚本与日志目录 `F:\soft\00selfmade\filemanager\.superpowers\sdd\`（不进版本库）：

- `accept-3120i-log.txt` — 第 6 轮验收日志（66 行，`RUN_DONE=0`，任务 1 定案依据）
- `occluders-3120g.round4.txt` — 外来窗口原始 rect（已全部消费复原）
- `release-3120-commit-paths.txt` — 发版提交 63 项显式路径清单
- `afm-accept-3120g.ps1` — 验收主脚本（504 行，无再跑需求）
- `set-icon-theme-3120.cjs` — 图标主题备份/恢复工具（restore 已跑，`RESTORE_OK`）
- round4 / round5 证据截图目录（`t3-sheet.png`、`f-full-2.png` 等）
- 站点 notes 源文件：`E:\办公文件\L1网站\alpha-file-manager\_release-notes-v3.12.0.md`（1599 B）
- 安装包：`sigma-file-manager\release\` 下 `Alpha File Manager_3.12.0_x64-setup.exe`（原件）与 `Alpha_File_Manager_3.12.0_x64-setup.exe`（下划线副本，为发版资产而生）；`release\` 不在版本库内

## 6. 本轮工具坑（新会话直接复用/避雷）

- **`gh release create` 含空格路径 + `=` 改名语法会被当 glob**，报 `no matches found for`。正确做法：先把资产复制成目标文件名（下划线命名），传单一路径参数，不用改名语法。本轮一次成功。
- `gh api repos/<o>/<r>/releases/tags/v3.12.0` 可回读验证 Release 与资产。
- PowerShell 字符串插值子表达式必须 `$((...))`，写 `{(...)}` 会原样输出字面文本。
- `git push` 遇 TLS 瞬断（exit 128）原样重试一次即成功。
- `AttachThreadInput` 在 `user32.dll` 不在 `kernel32`；修好后夺前台成功。
- `InvokePattern.Invoke()` 返回成功 ≠ 动作生效；剪贴板哨兵存活 = 没写成功。
- Radix 菜单 UIA 枚举/点击有竞态，必须轮询重试。
- 验收脚本任何坐标动作前必须 `SetProcessDPIAware()`；UIA rect 与 GetWindowRect 同为物理像素，不再乘 1.5。
- 工作区外的恢复性删除在本机不可达（trash 启动器与 `rm --` 均被策略拒），只能上报用户手工删。
- 外来窗口复原守卫必须**实测当前 rect**（`Left < -3000` 才挪回），清单里存的原始坐标是恢复目标、不是现状。

## 7. 项目硬约束提醒（AGENTS.md 摘要）

- 面向用户输出一律简体中文；品牌名 Alpha File Manager；受保护常量不动（Tauri `identifier` = `com.sigma-file-manager.app`、mDNS 服务名、legacy 目录名）。
- 真机验收只认 `tauri build` 打包产物，`tauri dev` 内容区全空白属构建特性。
- `.cmd` / `.ps1` 必须纯 ASCII；`.cmd` 运行期间绝不能编辑、`echo` 里不能有 `>`；后台长任务不挂 stdout 管道，脚本内部 `> logfile 2>&1`。
- 每次 bash 调用重注入 MSVC/Rust：cargo 在 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`（`pc-` 不能省）+ `vcvars64.bat`。
- 绝不用 `Get-Content -Raw`/`Set-Content` 改带中文注释的源文件。
- 工作区尚有 4 个刻意不收的侧车未提交文件：`extensions/kizemo.focus-sync/sidecar/src/{http_server,main,state,uia_event}.rs`——保持不提交，除非用户另有指示。
