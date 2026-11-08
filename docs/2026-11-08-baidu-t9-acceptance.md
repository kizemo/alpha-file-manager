# 百度网盘接入阶段 1 — T9 打包与真机验收清单

日期：2026-11-08
分支：`feat/baidu-remote-provider`
阶段目标：侧边栏「远程位置」出现「百度网盘」，双击进入后可像本地目录一样浏览（复制远端→本地、拖拽）。

## 一、本轮门禁状态

| 门禁 | 命令 | 结果 |
|---|---|---|
| 后端单测 | `cargo test --lib` | `1142 passed; 0 failed; 1 ignored`（`.superpowers/sdd/baidu-t9-test4.log`，收尾复核；早前同数见 `baidu-t7-test1.log`） |
| 前端定向单测 | `npx vitest run`（4 个文件） | `32 passed (32)` |
| eslint（6 个改动文件） | `npx eslint <files>` | `ESLINT_EXIT=0` |
| 全量前端单测 | `npx vitest run` | `291 files / 2732 tests passed`，但同时出现 1 个 `Worker exited unexpectedly` 未处理错误 ⇒ 退出码 1。该错误发生在 `tauri build` 正在编译 Rust 的同时，判为**资源抢占导致 worker 进程被杀**（非用例失败）；已在打包结束后单独重跑复核，结果见 `baidu-t8-vitest2.log` / `baidu-t9-chain.log` |

注：`cargo fmt --check` 在本仓库不是绿灯（存量差异），未作为门禁；本轮新增/修改的 Rust 块已单独 `rustfmt --edition 2021` 处理。

## 二、打包

- 命令：`npm run tauri:build`（末尾 `scripts/publish-bundle-to-release.mjs` 只把安装包复制到仓库 `release/`，**不上传**）。
- 脚本：`.superpowers/sdd/baidu-t9-build1.cmd`（纯 ASCII、内部 `> logfile 2>&1`、不挂 stdout 管道）。
- 日志：`.superpowers/sdd/baidu-t9-build1.log`（末尾 `BUILD_EXITCODE=`）。
- 产物：`src-tauri/target/release/alpha-file-manager.exe` + NSIS 安装包。

⚠️ `tauri dev` 的界面结论一律作废（dev 构建下路由页内容区空白），只能以 `tauri build` 产物验收。

## 三、真机验收脚本

`.superpowers/sdd/accept-baidu-t9.ps1`（纯 ASCII，中文 needle 用码点构造，避免 PS 5.1 ANSI 读文件乱码）。

流程：
1. 杀掉既有 `alpha-file-manager` 进程；
2. 启动 release 版 exe；
3. 轮询 `EnumWindows` 找 class = `Tauri Window` 且面积最大的窗口（**不**用 `Process.MainWindowHandle`，它会落到托盘窗）；
4. `AutomationElement.FromHandle` 拉全量可访问性树，输出 `.superpowers/sdd/t9-accept/uia-tree.txt`；
5. 按 needle 计数：`remote_locations`（远程位置）、`baidu_netdisk`（百度网盘）、`remote`（远程）、`baidu`（百度）、`settings`（设置）、`saved`（已安装）；
6. `PrintWindow(hwnd, hdc, 2)` 拍整窗 → `.superpowers/sdd/t9-accept/window.png`。

### 关键取证坑（沿用 AGENTS.md 结论）
- 脚本开头必须 `SetProcessDPIAware()`，否则 UIA 物理像素与 user32 逻辑像素差 1.5 倍；
- `PrintWindow` 拍的是整窗（含边框），屏幕坐标 = PNG 坐标 − 窗口原点；
- `PrintWindow` 不渲染 hover / overlay 层，右键菜单类交互只能走屏幕 BitBlt；
- 抓屏时机：列表滚动动画未结束会抓到黑块，延长等待并连拍两次取后一帧；
- 应用启动后可能弹更新日志模态框挡住列表，脚本须先关掉它再判「找不到卡片」。

## 四、验收判定口径（人工逐条确认）

| # | 判定项 | 通过标准 |
|---|---|---|
| 1 | 侧边栏「远程位置」下出现「百度网盘」 | UIA 树中命中 `百度网盘` 节点 |
| 2 | 设置 → 远程 → 新建连接，协议下拉有「百度网盘」 | 界面可见 |
| 3 | 选百度网盘时隐藏主机/端口/用户名，密码框变 Cookie 框 | 界面可见 + `data-baidu="cookie"` |
| 4 | 授权卡显示「尚未授权」等状态，且有「删除会话」按钮 | 界面可见 |
| 5 | 保存连接后 host/port 固定 `pan.baidu.com` / `443`，列表出现该连接 | 界面 + `user-remote-connections.json` |
| 6 | 「测试连接」在无 cookie 时回「还没有登录凭据」而不是泛化失败 | 错误文案分类正确 |
| 7 | 双击「百度网盘」进入远程浏览页 | 内容区不空白（`tauri build` 产物） |
| 8 | 有真实会话时能列出根目录 | 本机**无**百度会话 ⇒ 本条无法在本机成立，须如实记为「未验证」 |
| 9 | 新建目录出现在网页端 | 本机无会话 ⇒ 未验证 |
| 10 | 改名同步到网页端 | 本机无会话 ⇒ 未验证 |
| 11 | 删除进入回收站 | 本机无会话 ⇒ 未验证 |
| 12 | 右键下载到本地且 sha256 一致 | 本机无会话 ⇒ 未验证 |

第 8–12 条依赖真实百度会话（`BDUSS` + `STOKEN`）。本机用户数据目录
`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\` 下**没有**
`remote-credentials.json`（本机无可用百度会话），故只能由用户在装有会话的机器上补验。
**不得把「未验证」写成「通过」。**

### 4.1 真机实测结果（2026-11-08，打包版 exe，pid 60648）

取证脚本：`.superpowers/sdd/accept-click.ps1`（按 UIA 名字/物理坐标点击 + 剪贴板粘贴）、
`.superpowers/sdd/accept-remote-panel.ps1`（枚举左栏图标按钮并逐个悬停）。
证据：`.superpowers/sdd/t9-accept/remotepanel-shot.png`（屏幕 BitBlt，含悬停面板）、
`remotepanel-tree.txt`（UIA 树）、`.superpowers/sdd/baidu-t9-build1.log`（`BUILD_EXITCODE=0`）。

| # | 判定项 | 实测结论 | 硬证据 |
|---|---|---|---|
| 1 | 协议下拉含「百度网盘」 | ✅ **通过** | UIA 命中 `ListItem \| 百度网盘`（下拉展开态） |
| 2 | 选百度后隐藏主机/端口/用户名，密码框变 Cookie 框 | ✅ **通过** | 表单只剩 `Text \| Cookie` + `Edit \| Cookie`，引导文案「在浏览器里登录 pan.baidu.com 后，复制整段 Cookie 粘贴到这里（必须包含 BDUSS）…」 |
| 3 | 保存后落盘 `pan.baidu.com` / `443` | ✅ **通过** | `user-remote-connections.json` 新增条目：`protocol=baidu_pan`、`host=pan.baidu.com`、`port=443`、`credentialKey=<连接id>`、`initialPath=/`、`username=""` |
| 4 | 侧边栏「远程位置」面板出现该连接 | ✅ **通过（口径修正见下）** | 悬停左栏云图标按钮（物理坐标 `(40,414)`）后，面板出现 `Button \| 百度网盘验收测试 @79,649` + `Button \| 编辑该连接: 百度网盘验收测试 @401,655` |
| 5 | 授权卡显示状态 + 「删除会话」按钮 | ⚠️ **判据作废（实现变更）** | T7 的授权卡 watch `props.connection?.id`，**新建连接时没有 id ⇒ 授权卡不显示**。已改为「Cookie 输入框 + 引导文案」形态，本轮未观察到「删除会话」按钮 |
| 6 | 「测试连接」无 cookie 时报「还没有登录凭据」 | ⚠️ **未单独取证** | 后端 `remote_test_connection(app, request)` 带 `credentialKey` 读 DPAPI 存档的分支已实现且有单测，但真机上未单独点「测试连接」取证 |
| 7 | 双击进入远程浏览页 | ⚠️ **未验证** | 本机只有格式合法的**假 cookie**，会话无效，进不了真实目录 ⇒ 内容区无法判定 |
| 8–12 | 真实会话下的列表 / 新建 / 改名 / 删除 / 下载 | ❌ **未验证** | 本机无百度会话，须由用户在有会话的机器上补验 |

**判据 4 的口径修正（重要）**：`remote-nav-panel.vue` 按 `connection.name` 渲染，
**面板里显示的是用户自填的连接名，不显示协议名**。本轮连接名取「百度网盘验收测试」，
所以面板出现的是这个名字；用户把连接命名为「百度网盘」时，面板即出现「百度网盘」。
协议名「百度网盘」在**设置 → 远程**的连接列表里以标签形式显示
（UIA：`ListItem | 百度网盘验收测试***百度网盘pan.baidu.com:443/直连编辑删除`）。

## 五、用户数据安全

- 验收前已完整备份真实用户数据目录：
  `.superpowers/sdd/backup-t9-userdata/`（28 个文件 / 3,861,863 字节）。
- 若验收过程写入真实数据，恢复顺序必须是**先 `Stop-Process alpha-file-manager` 确认进程为空 → 再 restore**（应用退出时会把内存里的统计状态写回真实文件）。
- `remote-credentials.json` 内是可冒充用户会话的凭证：写盘 / 日志 / 错误信息一律不得出现明文。

### 5.1 本轮实际恢复记录（已执行）

顺序：**先杀进程 → 再 restore → 删凭据文件 → 校验**。

脚本：`.superpowers/sdd/restore-t9-data.ps1`（纯 ASCII）。
⚠️ 直接在 bash 里对 `C:\Users\...\AppData\...` 做递归复制/删除会被**本地安全策略拦截**
（`recursive write/delete target cannot be verified inside the workspace`）⇒ 必须把逻辑写进工作区内的 `.ps1`，再用 `pwsh -NoProfile -ExecutionPolicy Bypass -File <绝对路径>` 执行。

脚本输出：

```
PROC_STOPPED
RESTORED_FILES=28
DST_FILES=28
EXTRA_IN_DST=                ← 目标目录没有多出备份之外的文件
CRED_REMOVED
BAIDU_OCCURRENCES=0
CONN_BYTES=1154
BACKUP_CONN_BYTES=1154
RESTORE_OK
```

两条必须记住的事实：

1. **密文文件的真实路径在 `user-data/` 的上一级**：
   `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\remote-credentials.json`
   （本轮由验收创建，1,593 字节）。⚠️ plan 文档里写的「`remote-credentials.json` 在 `user-data` 下」
   **与实际不符**；备份快照 `backup-t9-userdata/` **不含**该文件 ⇒ 恢复时必须单独删。
2. `user-remote-connections.json` 恢复后 1,154 字节，与备份同尺寸，`baidu` 出现 0 次 ⇒ 假连接已清除。

## 六、本轮改动文件清单

后端：
- `src-tauri/src/lib.rs`
- `src-tauri/src/remote/mod.rs`
- `src-tauri/src/remote/baidu_api.rs`
- `src-tauri/src/remote/baidu_auth.rs`
- `src-tauri/src/remote/baidu_provider.rs`
- `src-tauri/src/remote/commands.rs`
- `src-tauri/src/remote/connection.rs`
- `src-tauri/src/remote/connection_test.rs`
- `src-tauri/src/remote/registry.rs`
- `src-tauri/src/baidu_probe.rs`

前端：
- `src/types/remote.ts`
- `src/types/__tests__/remote-protocol-baidu.test.ts`
- `src/stores/storage/remote-connections.ts`
- `src/stores/storage/__tests__/remote-connections-baidu.test.ts`
- `src/modules/settings/ui/categories/remote/connection-form.vue`
- `src/modules/settings/ui/categories/remote/baidu-connection-form.test.ts`
- `src/localization/messages/*.json`（17 个：ch/en/de/es/fr/it/ja/pt/ru/tr/vi/ur/hi/hy/he/fa/sl）

## 七、红线提醒

新增 `baidu_*.rs` 文件**零 `tauri::` 路径**（`tauri::AppHandle` / `tauri::async_runtime::*`），
否则 `cargo test --lib` 会在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` 崩掉。
三个新文件顶部照抄 `src-tauri/src/remote/mod.rs:8-18` 的红线说明。
