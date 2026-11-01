# Handoff · Alpha File Manager 发版 3.9.0 + UI 改动介绍与重截图（2026-11-01）

> 本文档是**本轮（r15 → r17）的交接终点**，也是**下一会话的发版作业书**。
> 下一会话请按第 2 节的顺序读文档，读完后用 10 行以内复述背景/目标/进度/下一步，再动手。
>
> 本文档中的所有路径都是**绝对路径**，可直接复制使用。

---

## 0. 进度快照（读完本文档后应能复述的内容）

| 项 | 值 |
|---|---|
| 仓库（代码在**内层**） | `F:\soft\00selfmade\filemanager\sigma-file-manager` |
| 仓库 remote | `kizemo/alpha-file-manager` |
| 外层目录 | `F:\soft\00selfmade\filemanager` —— 只放 `AGENTS.md` 与历史档，**不跟踪内层代码** |
| 当前版本号 | **3.8.0**（`package.json` 与 `src-tauri/Cargo.toml` 一致；`tauri.conf.json` 的 version 仍是 `"../package.json"`） |
| **线上已发布版本** | **3.8.0** —— aiec `pub_date=2026-10-30T10:22:54Z`，GitHub release `2026-10-30T10:36:56Z` |
| 线上 3.8.0 安装包 sha256 | `ee1c5f0f98394ad28d90c9a3ace5587d516d35ec246e83f4d203a8e0794497c3` |
| 本地 r17 重打的 3.8.0 | 44,771,400 B，sha256 `5208C19B8D1AF9723422A1793F477D2E2436F80DC9A1A2B740627AC9EFF92588` |
| 工作树状态 | `git status --porcelain` = **115 行**，全部未 commit（本轮约定：**不 commit、不 push**；发版本身含 GitHub 发布，属用户明确授权范围） |
| 本轮已完成并闭环 | r15 族色 token / r16 暖纸低饱和红 + 目录树 tooltip / r17 图标方案 A + 字符下移（落地、打包、覆盖装机、取证齐全） |

### 0.1 ⚠️ 最重要的一条结论：**下一版必须是 3.9.0，不能用 `--bump none` 重发 3.8.0**

理由（三条独立证据）：

1. **3.8.0 已经对外发布**（aiec `version.json` 的 `version` 字段 = `3.8.0`，GitHub 有 `v3.8.0` release）。
2. r16 / r17 的改动（暖纸低饱和红、目录树 tooltip、图标重设计）**全都不在已发布的 3.8.0 里** —— 线上 3.8.0 的 sha256 与本地 r17 重打的 3.8.0 **不同**（`ee1c5f0f…` vs `5208C19B…`），证明本地代码已领先于线上。
3. `--bump none` 会用**新二进制覆盖线上同版本号的包**，sha256 变化而版本号不变 ⇒ 用户本地校验与更新提示行为全乱（`app_updater` 用 sha256 比对）。

⇒ **下一版 = `3.9.0`（minor）**。本轮含 UI 特性改动（图标重设计 + 新的 tooltip 本地化），minor 是合适粒度；若你判断 patch 更合适，可改 `--bump patch` 得到 3.8.1，但**不要用 `none`**。

---

## 1. 背景与已锁定的决策（本轮不可推翻）

### 1.1 r17 图标最终参数（已落地并装机验证）

- 方案 **A**（风格库 DNA 演绎）+ 用户追加要求「**字符下移**」
- 最终参数：`family=Georgia, 'Times New Roman', serif`，`weight=700`，`style=''`，`a=1.50`，`sx=0.97`，`sy=1`，`dx=-0.10`，`dy=0.7683`
- 容器几何**一字未改**（只动字形变换）
- 居中解算：文件夹长方形部分 `bodyTop=262.31` / `bodyBot=897.88` / `bodyCenter=580.10 px`；`pxPerUnit=56.7467`；dy=0 时字形中心 536.50 ⇒ `dy=0.7683`
- **实测残差 −0.10 px，越界（overhang）= 0**
- 风险（已向用户披露并获知情选定）：16 px 下封闭字腔由 6 px 降到 3 px，Georgia Bold 笔画重，收紧后更易糊

### 1.2 r16 两项修复

1. 暖纸主题右上角「信息面板」「目录树」两图标改为**低饱和度红**
2. 悬停 hide folder tree 图标，**软件语言为中文时显示「目录树」**（运行时 UIA 铁证已取得；`navigator.showFolderTree` / `navigator.hideFolderTree` 两个 key 在 **17 个语言包**均已本地化，中文 locale code 是 `ch` 不是 `zh`）

### 1.3 生产资产现状（r17 改写后）

- `src/assets/icons/logo-1024x1024.png`（31,145 B，sha `530063AD…`）
- `src/assets/icons/logo-1024x1024-dark.png`（30,914 B，sha `4EC4D252…`）
- `src/assets/icons/logo-32x32.png`（1,677 B，sha `9ADF091C…`，**保持 64×64 契约**）
- `src-tauri/icons/**` 57 个文件已由 `npm run tauri:gen-icons` 同步（含 android/ios）
- r15 原资产备份：`.superpowers\sdd\r17-icon-backup\assets\`

---

## 2. 必读顺序（按此顺序读，读完再动手）

| # | 路径 | 读它拿什么 |
|---|---|---|
| 1 | `F:\soft\00selfmade\filemanager\AGENTS.md` | **本项目全部硬约束**（运行时注入的版本末尾被截断，需要时直接 `read` 源文件） |
| 2 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-release-3.9.0-and-ui-screenshots.md` | 本文档 |
| 3 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md` | **发版五条命令**、五处状态一致性、缓存/回读/回滚的全部细节 |
| 4 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\version-info.md` | 版本号规则与三处版本号同步关系 |
| 5 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\release-notes-3.8.0.md` | 升级说明文案的**格式模板**（中文，`### 新增 / ### 修复 / ### 备注`） |
| 6 | `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` | 更新日志**英文格式模板**（`## [x.y.z] - Month 2026` + `**Summary:**` + `### What changed` + `**Improved:/Changed:/Fixed:**` 段落式） |
| 7 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\r17-acceptance-report.md` | r17 的完整取证（截图在哪、怎么抓的、数字是多少） |
| 8 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\design-demos\icon-direction-approved.md` | 图标方向拍板记录（§8 是 r17 的参数与实测） |

**读完后**先在对话里用 10 行以内复述：项目背景、本次任务目标、当前进度（3.8.0 已发布、本地领先、r15–r17 已闭环）、下一步计划（3.9.0 发版 + 截图），然后开始执行。

---

## 3. 任务清单（下一会话的作业）

### T1 · 重截图（8 张：4 个页面 × 2 种界面语言）

**必须拍的 4 个页面**

| 页面 | 英文文件名建议 | 中文文件名建议 |
|---|---|---|
| 目录页（文件浏览器主界面） | `docs/screenshots/afm-browser-en.png` | `docs/screenshots/afm-browser-zh.png` |
| 设置页（总览） | `docs/screenshots/afm-settings-en.png` | `docs/screenshots/afm-settings-zh.png` |
| UI 外观设置页（设置 → 外观） | `docs/screenshots/afm-appearance-en.png` | `docs/screenshots/afm-appearance-zh.png` |
| 搜索管理页（设置 → 搜索管理） | `docs/screenshots/afm-search-manage-en.png` | `docs/screenshots/afm-search-manage-zh.png` |

**⚠️ 现状陷阱（必须处理）**

`docs/screenshots/afm-main-window.png` 目前**被 `README.md` 和 `README.zh-CN.md` 同时引用**（两处都是第 21 行）。用户要求「README 用英文界面截图，中文说明用中文界面截图」⇒ 这一个文件不能一稿两用。必须：

1. 新增两套带 `-en` / `-zh` 后缀的文件；
2. `README.md` 第 21 行改为引用**英文**那张；
3. `README.zh-CN.md` 第 21 行改为引用**中文**那张；
4. `README.md` 第 119–120 行与 `README.zh-CN.md` 第 110–111 行引用的 `afm-dashboard-hero-light.png` / `afm-dashboard-card-enrichment.png` **同样是两文件共用**，同样要拆成 `-en` / `-zh` 两套（或者明确论证这两张与语言无关，但用户明确要求「中文说明截图中文界面」，保守做法是拆）。

**截图方法（沿用已验证的路径，不要另起炉灶）**

- 真机界面**只能用 `tauri build` 的打包产物**。`tauri dev` 的所有路由页内容区**全空白**（`RouterView` 挂载却不渲染子组件），在它上面得到的界面结论**一律作废**。
- 抓图链路：`PrintWindow(hwnd, hdc, 2)`；已验证的脚本在 `.superpowers\sdd\` 下，优先复用：
  - `.superpowers\sdd\accept-r10-sandbox.ps1`（13,387 B，隔离桌面抓图主脚本）
  - `.superpowers\sdd\afm-shot-full.ps1`（2,506 B，整窗口抓图）
  - `.superpowers\sdd\sandbox-theme-accept.ps1`（15,515 B，主题区抓图）
  - `.superpowers\sdd\r14-capture-probe.ps1`（11,948 B，抓图探针）
  - `.superpowers\sdd\shot-quant.ps1` / `shot-quant-sum.cjs`（量像素/取样）
- 隔离桌面：自建 `CreateDesktop`，掩码用 `DESKTOP_ALL = 0x1FF`。
- **本机 DPI 144（150% 缩放）** ⇒ 物理像素 ÷ 1.5 = CSS 逻辑像素。量尺寸时别忘了换算。
- 应用启动会弹「新功能」更新日志模态框，把整个文件列表挡住 ⇒ 抓图前**先关掉它**（关闭按钮可访问名「关闭」）。
- `PrintWindow` 抓不到 hover / overlay 层；验这类交互只能走屏幕 DC BitBlt，并先用 `WindowFromPoint` + `GetAncestor(h,2)` 校验实际绘制者（白名单**必须**包含应用自己的 `msedgewebview2` 子进程）。

**语言切换（下一会话自行确认，本轮未核实）**

- 本轮**没有**核实「设置页里切换语言的控件路径」与「语言持久化字段名」。请在设置页里实地确认后再写脚本，**不要猜字段名**。
- 切换语言通常需要**重启应用**才全局生效 —— 这一点请实测确认。
- 已知可用的事实：locale code 是 `ch`（不是 `zh`）；语言包在 `src/localization/messages/**`；应用数据目录在 `%APPDATA%\com.sigma-file-manager.app\user-data\`。

**数据安全（release 构建下的铁律）**

- release 构建会把 `SFM_E2E_DATA_DIR` 覆盖**编译掉** ⇒ 真机验收只能「**备份真实文件 → 灌夹具 → 验收 → 恢复**」。
- **恢复顺序必须是「先杀进程 → 再 restore」**：应用退出时会把内存里的统计状态写回真实 `user-stats.json` ⇒ 夹具灌进去后只要进程还活着，磁盘上的真实文件就变成夹具残留。
- 恢复后**必须核对字节数与条目数**，别把夹具留给用户。
- 真实数据路径：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\`
- `user-settings.json` / `user-stats.json` / `user-workspaces.json` 三项都要比对 sha256。
- ⚠️ 改 `user-settings.json` 的 `navigator.defaultDirectory` **不等于**应用会打开那个目录：`user-workspaces.json` 里保存的当前标签页优先，会把启动目录钉死。要量指定目录，必须**同时**改 `workspaces[].tabGroups[][]` 中 `type === 'directory'` 的 `path`。
- r17 的备份参考：`.superpowers\sdd\r17-userdata-backup\user-data\`

### T2 · 改 `README.md`（**英文界面**截图 + 介绍 UI 改动）

路径：`F:\soft\00selfmade\filemanager\sigma-file-manager\README.md`（17,442 B）

- 换掉第 21 行与第 119–120 行的截图引用为 `-en` 版本；
- 增补一段 **UI 改动介绍**，内容覆盖本轮三条：图标重设计（方案 A + 字符下移）、暖纸主题右上角两枚图标改低饱和度红、目录树 tooltip 本地化；
- 全文保持**英文**（README.md 是英文文档）。

### T3 · 改 `README.zh-CN.md`（**中文界面**截图 + 介绍 UI 改动）

路径：`F:\soft\00selfmade\filemanager\sigma-file-manager\README.zh-CN.md`（16,088 B）

- 换掉第 21 行与第 110–111 行的截图引用为 `-zh` 版本；
- 同样增补 UI 改动介绍，**全文用简体中文**（与 README.md 的英文版一一对应）。

### T4 · 写升级说明

路径（新建）：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\release-notes-3.9.0.md`

- 格式照抄 `docs\release-notes-3.8.0.md`：`## Alpha File Manager 3.9.0` + `### 新增` / `### 修复` / `### 备注`；
- **正文用简体中文**；
- **不要手写大小和 sha256** —— `release:prepare --write-notes` 会自动把构建信息追加进 GitHub 说明里，`version.json` 的 `notes` 则不需要这两项；
- 建议同时把这一版的中文说明作为 `--notes-file` 的输入（放 `.superpowers\sdd\_notes-3.9.0.md`，中间产物一律写 `.superpowers\sdd\`）。

### T5 · 更新 CHANGELOG（**人工，脚本只校验**）

- `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` —— 加 `## [3.9.0] - November 2026` 段落，**英文**（照第 3–21 行 3.8.0 条目的格式：`**Summary:**` + `### What changed` + `**Improved:/Changed:/Fixed:**` 段落）；
- `F:\soft\00selfmade\filemanager\sigma-file-manager\public\changelog\ch\changelog.md` —— **中文**那一份，**必须补**（首启更新弹窗读的是它；locale code 是 `ch`）；
- 其余 16 个语言副本在 `public\changelog\<locale>\changelog.md` —— **不必全部翻译**：中文那一份没写时，其它语言会退回英文条目。但脚本会检查「无 BOM / 无 U+FFFD 乱码」，所有 17 份都要过这一关。
- ⚠️ `verify-changelog` 的查找逻辑是 `find(版本) || releases[0]`，找不到**不报错、不告警**，弹窗会静默回退到最旧条目 —— 所以第 5 处状态（CHANGELOG）是最容易被忘、后果最直观的一处。

### T6 · 升版本号 + 打包

**顺序（`release-workflow.md` 第 0 步必须在 bump 之前，否则误报）**

```powershell
cd F:\soft\00selfmade\filemanager\sigma-file-manager

# 0) 先补 CHANGELOG.md（见 T5），再校验 —— 必须早于升版本号
npm run verify-changelog

# 1) 升版本号到 3.9.0 + 打包 + 写站点源（version.json 与安装包副本）
npm run release:prepare -- --bump minor --notes-file ./.superpowers/sdd/_notes-3.9.0.md

# 2) 生成带构建信息的 _release-notes-v3.9.0.md —— 不能省
npm run release:prepare -- --write-notes ./.superpowers/sdd/_notes-3.9.0.md

# 3) 上传到 aiec.fun（含自动公网回读校验）
npm run release:upload

# 4) 发布 / 更新 GitHub 兜底 release
npm run release:github

# 5) 只做公网回读校验，不改动任何东西
npm run release:verify
```

**为什么第 2 步不能省**：`release:github` 只**读**站点源目录里的 `_release-notes-v<版本>.md`，它自己从不生成这个文件（唯一生成入口就是 `prepare --write-notes`）。漏掉这步**不报任何错** —— `github()` 走 `if (fs.existsSync(notesPath))` 的假分支，`gh release create` 不带 `--notes-file`，于是安静地发出一个**正文为空**的 release。2026-10-14 发 3.1.0 时中招过一次。

**注意 `release:prepare` 的语义**：`release:prepare` = `node ./scripts/release-publish.mjs prepare`。它负责（a）bump `package.json` + `src-tauri/Cargo.toml` 两个版本号、（b）跑打包、（c）把安装包副本与 `version.json` 写进站点源目录。

**若 `release:prepare` 里的打包环节超时/失败**：先看站点源目录 `E:\办公文件\L1网站\alpha-file-manager\` 里有没有留下半成品（`version.json` 与安装包副本必须**成对**更新）。应用下载后会本地比 sha256，不匹配就删文件 —— 只换包不同步清单，会把正常用户挡在门外。

**若选择绕开 `release:prepare` 手动打包**（不推荐）：`npm run tauri:build` = `npm run sync-version && tauri build && node ./scripts/publish-bundle-to-release.mjs`。但**版本号与 `version.json` 仍需手工同步**，五处状态一致性（见 `release-workflow.md` 第二节）要自己保证。

### T7 · aiec 发布（= `release:upload`）

- **aiec.fun 就是本项目的主更新源**：`https://www.aiec.fun/alpha-file-manager/`。GitHub release 只是可用性兜底。
- 站点源目录（本地）：`E:\办公文件\L1网站\alpha-file-manager\`
- 服务器：`/var/www/wordpress/alpha-file-manager/`，`47.120.26.175:22`，用户 `kizemo`
- SFTP 凭据：`E:\办公文件\L1网站\.vscode\sftp.json`（**绝不外泄**）
- 上传用 python：`E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe`（含 paramiko）
- 环境变量可覆盖路径：`AFM_SITE_DIR` / `AFM_SITE_URL` / `AFM_SFTP_PYTHON` / `AFM_GH_REPO` / `AFM_TOOLCHAIN_BIN`
- 站点目录下的 `_upload_alpha_files.py` 与 `_release-notes-*.md` 是**本地工具，不在上传白名单内**，不会被传到服务器（nginx 下 `.htaccess` 无效，安全性依赖这个白名单）
- **缓存**：站点在 Cloudflare 之后，`version.json` 实测 `CF-Cache-Status: DYNAMIC`（边缘不缓存），但服务端没下发 `Cache-Control` ⇒ 客户端强制追加 cache-busting 参数是唯一且充分的保障，**不要删**。手工回读：
  ```powershell
  curl "https://www.aiec.fun/alpha-file-manager/version.json?_=$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
  ```
- **安装包要回读两次**（脚本已内建）：一次带 cache-busting（源站）、一次不带（用户实际下载路径）。源站大小 ≠ 本地 ⇒ **真的上传失败**，exit 1 需重传；源站一致但边缘 ≠ 本地 ⇒ 上传成功，只是 Cloudflare 边缘还拿着旧包，exit 0 + 警告，剩余窗口最长 `Cache-Control: max-age=7200`（2 小时），这段时间内用户下载会拿到旧包而清单报新 sha256 ⇒ 应用本地校验不通过会**直接删文件**，表现为「下载了却装不上」。规避：purge 该 URL / 换带 cache-busting 的链接 / 这段时间不推送更新提示。

### T8 · GitHub 发布（= `release:github`）

- 仓库：`kizemo/alpha-file-manager`（**不是** sigma 上游）
- 兜底源常量在 `src-tauri/src/app_updater.rs`：`UPDATE_MANIFEST_URL`（主源）、`GITHUB_RELEASES_API_URL`（兜底源）
- 改域名时别丢 `validate_download_url()` 里的 **HTTPS 强制 + localhost / 元数据服务 / 私有 IP 拒绝**（防 SSRF 安全网）

### T9 · 回滚预案（发版前先知道）

最廉价的回滚：**把站点 `version.json` 的 `version` 改回旧值**，用户立刻不再收到更新提示，无需回滚应用、无需重装。Rust 侧双源语义保证有效 —— 主源「解析成功但无更新」时**不会**去查 GitHub 兜底源取更高版本。

### T10 · 可选但推荐：winget 清单

- 清单路径：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\manifests\k\kizemo\AlphaFileManager\<version>\`
- 现存到 **3.6.5**（3.7.0 / 3.8.0 尚未补）
- 校验：`npm run check:winget` = `node ./docs/winget/check-manifest.mjs`
- 用户本次没点名 winget，**做完 T1–T8 后可问一句**或按 T11 的顺序顺带补上。

---

## 4. 门禁（同口径对照，别混口径）

```powershell
cd F:\soft\00selfmade\filemanager\sigma-file-manager
npm run verify-changelog     # CHANGELOG 校验（已挂进 npm run check）
npm run check:winget         # winget 清单校验
npx eslint .                 # 基线：188 errors / 0 warnings（HEAD，用 git stash 量）
npx vitest run               # 全量单测（默认 reporter；--reporter=basic 在本仓库这版不存在）
cargo test --lib             # 809 例量级
```

**基线口径铁律**

- `eslint .` 的基线数字必须用**干净检出**（`git stash push -u` 量 HEAD）拿。eslint **不读 `.gitignore`** ⇒ 工作副本里的 `.superpowers/`（150+ 个一次性脚本）会被一起 lint，工作副本里量到 **689 errors**，干净检出只有 **188**。已把 `.superpowers/**` 与 `graphify-out/**` 写进 `eslint.config.ts` 的 `globalIgnores`。
- ⚠️ **基线数字会随提交变** —— 2026-10-23 实测更新过一次（`177` → `188`）。量的时候一律现量，别拿旧数字对。
- 单文件 `eslint --fix` 通过 **≠** 回到基线（`--fix` 会把该文件里基线本来就有的违规一起修掉）。**禁 `--fix` / `--fix` stylelint**，只跑 `:check`。
- `$ErrorActionPreference='Stop'` 会把 npm/npx 的 stderr 变成 `NativeCommandError` ⇒ 调用 npm/npx 前后显式切回 `Continue`。
- `vitest` 红了先分清「真红」还是「超时」：断言失败才有内容可看；`Test timed out in 30000ms` 是另一回事（`single-instance-guard.test.ts` 要枚举全机进程，机器上 431 个进程时 WMI 一次 6.5 秒，42.5 秒 > 30 秒默认超时，脚本本身其实全过）。

**本轮已知的硬回归约束（改 UI 时别破坏）**

| 文件 | 约束 |
|---|---|
| `src/modules/settings/ui/__tests__/header.test.ts` | 英文界面显示 `kizemo` 且不含「段燚」；官网 href = `https://www.aiec.fun`；两张 logo 都在 DOM |
| `src/modules/themes/__tests__/afm-themes.test.ts` | 4 主题须定义全部 `REQUIRED_COLOR_TOKENS` 且为**裸 HSL 三元组**；`--toolbar-action-foreground` 对 `--window-toolbar` 与 `--background` 均 ≥3:1；4 个文件必须含字符串 `--toolbar-action-foreground`；`afm-themes.css` 中 `html[` 开头行须匹配 `^html\[data-afm-theme='afm-[a-z-]+'\]` 且不得含布局属性；`src/styles/index.css` 必须**最后** `@import "./afm-themes.css"` |
| `src/modules/themes/__tests__/afm-family-icons.test.ts` | tree/info 激活态仍用族色（8/8 通过） |

颜色 token 规则：必须**裸 HSL 三元组**（`--popover` / `--input` 例外）。

---

## 5. 已知踩坑（别重试）

### 5.1 发版链路

1. **`verify-changelog` 必须在 bump 之前跑** —— 顺序反了会因为「刚 bump、还没来得及写」而误报。
2. **`release:prepare --write-notes` 不能省** —— 省了会发出**正文为空**的 GitHub release 且不报错。
3. **`publish-bundle-to-release.mjs` 会遍历 `bundle/nsis/` 下所有历史安装包**，逐个 `rmSync` + 复制到 `release/`。只要 `release/` 里**任何一个**旧包被别的进程锁住（实测 3.4.0 被锁），整步 exit 1 —— 而 `tauri build` 本身早已成功。⚠️ **要分清「构建失败」和「复制失败」**：新包用 `node -e "fs.copyFileSync(a,b)"` 单独补进 `release/` 即可，不必重跑整个打包。
4. `release/` 里现在堆了 **46 个历史安装包**（2.2.0 → 3.8.0），每个 18–45 MB。
5. `release/SHA256SUMS.txt` 由脚本维护。

### 5.2 Windows / shell

6. **`.cmd` 的 `echo` 里不能出现 `>`** —— cmd.exe 把它当重定向符。症状是「构建日志少了一行 + git status 里冒出一个看不懂的文件」（曾生成名为 `3.6.0)` 的垃圾文件）。描述文字改用 `to` / `=>`，或整句加引号。
7. **`.cmd` 运行期间绝对不能编辑它** —— cmd.exe 按字节偏移逐行读，边跑边改会让后面几行错位、执行别处的片段。曾因此把版本从 3.6.0 顶到 3.7.0，十来分钟的 Rust 构建全丢。要改：**先 `task_stop` → 再改 → 再新建一个文件名重跑**，不要复用同名文件。
8. `.cmd` / `.ps1` **必须纯 ASCII**（cmd.exe 按系统 ANSI 代码页读，PowerShell 5.1 同理）。中文一律作命令行参数传入。
9. **长编译一律让 `.cmd` 内部用 `> logfile 2>&1` 重定向**，再单独读文件看进度。**后台跑不要挂 stdout 管道**（`Tee-Object` / `| Select-String`）—— 管道一断进程树跟着死，2026-10-23 因此报废过一次十分钟的 Rust 构建。日志用 `Get-Content` 读（cmd 重定向锁独占，`[System.IO.File]::ReadAllText` 会抛「文件正由另一进程使用」）。
10. **cargo 不在 PATH**：`C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin` —— ⚠️ toolchain 目录名里的 **`pc-` 不能省**，写成 `1.90.0-x86_64-windows-msvc` 时 `cargo metadata` 报 `program not found`，`tauri build` 3 秒内失败退出。MSVC 环境靠 `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`。
11. **`.NET API`（如 `[System.IO.File]::ReadAllText`）用进程 CWD，不受 `Set-Location` 影响 ⇒ 一律传绝对路径**。
12. **绝不要用 PowerShell 的 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件** —— PowerShell 5.1 按 ANSI 读无 BOM 的 UTF-8，中文注释读进来就是乱码，写回去就把文件毁了；`Set-Content -Encoding UTF8` 还会额外写 BOM。改 `.vue` / `.ts` / `.json` / `.css` / `.md` 一律用**编辑工具**，或 `node` 的 `fs.readFileSync/writeFileSync(...,'utf8')`。改完立刻验：`s.includes('<某个中文>')` 为真 **且** `s.match(/\uFFFD/g)` 为空。**保持原行尾**（`icon-direction-approved.md` 实测纯 LF）。
13. bash 里 `cd` / `Set-Location` 后写文件会被安全层拦 ⇒ 用绝对路径。

### 5.3 真机验收

14. **真机验收必须用 `tauri build` 的打包产物**，不能用 `tauri dev` —— dev 构建下所有路由页内容区全空白（`RouterView` 挂载却不渲染子组件；Vue DevTools 里搜 `navigator` / `settings` 只命中 Pinia store，搜 `sidebar` 能命中组件，可据此判别）。已归因：把 `src/` 全部回退到改动前仍复现，同机已安装的打包版完全正常 ⇒ dev 构建特有，**不是代码缺陷**。dev 构建上的界面验收结论**一律作废**。
15. **打包 / 装机前必须单独跑一次 `Get-Process alpha-file-manager` 确认为空**。
16. release 构建下 `SFM_E2E_DATA_DIR` 被**编译掉**（`src-tauri/src/app_dirs.rs` 的隔离覆盖是 `#[cfg(debug_assertions)]`）⇒ 只能「备份 → 灌夹具 → 验收 → 恢复」，且**先杀进程再 restore**。
17. **应用退出时会把内存里的统计状态写回真实 `user-stats.json`** ⇒ 夹具灌进去后只要进程还活着（哪怕被 `Stop-Process` 杀掉），磁盘上的真实文件就变成夹具残留。
18. `dash-accept-data.cjs backup` 的语义是「**备份文件不存在才建**」⇒ **不会**覆盖旧备份。跑 backup 后一定看它打印的字节数，若明显小于真实值，先 `inspect` 备份文件本身再决定信谁。
19. **`SendInput` 的 `INPUT` 结构必须显式 `[StructLayout(LayoutKind.Explicit, Size = 40)]`** —— 只声明 `KEYBDINPUT` 时 `Marshal.SizeOf` < 40，`SendInput` **静默失败**（返回 0、不报错）。照抄 `.superpowers\sdd\app-key.ps1` 的结构定义。
20. **输入文字的首选路径**：`Set-Clipboard` + `app-key.ps1 -Keys 'CTRL+V'`（中文/emoji 同样免疫 IME）。**别再试**：往 `Chrome_RenderWidgetHostHWND` 投 `WM_CHAR`（`0x0102`）会被静默丢弃；剪贴板 + 中键粘贴不会触发粘贴。
21. **注入的 Shift 到不了 webview 的 `shiftKey`** —— `Shift+Delete` 会变成「移至垃圾箱」。别再试：改 `VK_LSHIFT`、改扫描码（`KEYEVENTF_SCANCODE`）、塞进一次 `SendInput`、每次事件 + sleep 60ms，四条都试过、都不解决。
22. **本应用的工具栏按钮不暴露 UIA `InvokePattern`**；过滤框在 UIA 树里是空壳（只有 Chromium 的原生 IME 窗口 + `-∞x-∞` 的 Group，**没有 `[Edit]`、没有 `ValuePattern`**）。全局搜索的放大镜（标题栏约 `(1450,38)`）与右键菜单项（Chromium accessibility 的 `MenuItem` Name）**有** `InvokePattern`。
23. **Chromium 把列表行暴露成什么 `ControlType` 不稳定** —— 全局搜索结果行**不是** `ListItem`。稳定的是行内那个 **28×28 的图标**（面板其它图标是 19×19 / 25×25），按尺寸挑第一个即可定位整行。
24. **不要用 `Process.MainWindowHandle` 取本项目的主窗口** —— 它会落在 15×15 的托盘窗上。要自己 `EnumWindows` + `GetWindowThreadProcessId` 过滤，取 **class 为 `Tauri Window`、可见、面积最大**的那个。
25. **UIA 里从点往上找祖先会一路走到窗口级元素** —— 必须给祖先加硬边界（宽度上限 + 落在目标区域内）。
26. **`InvokePattern.Invoke()` 返回成功 ≠ 点击生效**（对禁用按钮是静默空操作）。「有没有真生效」只能靠落盘文件或像素可见变化判定。
27. **`PrintWindow` 会返回陈旧帧**（进度事件刷爆渲染线程时）—— 判后台任务运行状态要看**文件系统**（文件数是否停止增长），不要看屏幕。
28. **屏幕 BitBlt 只拍屏幕合成结果** —— 别的窗口盖在上面就会拍到别的窗口（曾拍到 Edge 的 GitHub 页面，产出两次假的「点击没反应」）。用 `WindowFromPoint` + `GetAncestor(h,2)` 校验实际绘制者，白名单**必须包含**应用自己的 `msedgewebview2`（其顶层窗口 owner pid 是 `msedgewebview2.exe`，只认主进程 pid 会把每一张干净截图都判成「被遮挡」）。
29. **`PrintWindow` 不渲染 hover / overlay 层**（悬停才出现的快捷面板、右键菜单都不在里面）。验这类交互只能走屏幕 DC BitBlt。

### 5.4 数据 / 存储

30. **给 `user-stats` 加顶层字段，「读白名单」和「写列表」要一起改** —— 读侧 `USER_STATS_TOP_LEVEL_KEYS = Object.keys(DEFAULT_USER_STATS)`，写侧 `saveStats()` 是**逐键显式 `set()`** 的固定五项。漏掉写侧 ⇒ 新建收藏分类界面出现、重启即丢，全程不报错。
31. **`cargo test --lib` 的隐形红线**：任何被测试链接到的代码都不能碰到 `tauri::AppHandle` 或 `tauri::async_runtime::*` —— 否则测试二进制在**加载阶段**以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉，一个测试都跑不了，报错里完全看不出跟你的模块有关。绕法：给业务模块加 `*_blocking` 同步内核入口（见 `src-tauri/src/dir_reader.rs`、`src-tauri/src/global_search.rs`），调用方自己选执行器。
32. **`#[tool_router]` 不等于接线完成** —— 必须给 `impl ServerHandler` 加 `#[tool_handler]`，否则 `tools/list` 返回 `{"tools":[]}`，一个工具都调不到。
33. **新增 i18n key 必须 17 个语言包全补齐**；中文 locale code 是 `ch` 不是 `zh`。

### 5.5 其它

34. **`net use` 的盘符参数必须带冒号** —— `net use Y: \\host\share` 才是映射盘符；不带冒号会被 `net.exe` 当成「连接名叫 Y 的远程资源」，报 **System error 67**，而 67 恰好也是「共享根本不存在」的错误码，两者一模一样。
35. **`net.exe` 的输出是控制台代码页（cp936），不是 UTF-8** —— 要用 `MultiByteToWideChar` + `GetConsoleOutputCP` 按代码页解。
36. **验收夹具不要建在 `%TEMP%` 下** —— `src-tauri/src/global_search/ignore.rs` 的 `builtin_ignored_paths()` 含 `/AppData/Local/Temp`，且 `matches_plain_ignored_path` 对 `/` 开头的模式做 `path.contains(...)` ⇒ 夹具放 `%TEMP%` 时整棵子树被剪掉，**目录内递归搜索命中恒为 0**，看起来像产品缺陷。放到 `C:\Users\<user>\` 下的普通目录即可。
37. **开关型控件要先探测再点击** —— 过滤按钮是 toggle，脚本里先查目标元素是否存在，不存在才点。
38. **PowerShell 函数只要有返回值，就不能在里面混用 `Write-Output`** —— 诊断输出会进管道成为返回值的一部分。函数内改用 `Write-Host`。
39. **中间产物一律写 `.superpowers\sdd\`**（已 gitignore），对话里只留指针 + 结论。

---

## 6. 交付标准（下一会话自检用）

- [ ] 8 张截图落盘（4 页 × EN/ZH），能看出**语言不同**（英文界面 / 中文界面），能看出**本轮 UI 改动**（图标、暖纸主题两枚红图标、目录树 tooltip）
- [ ] `README.md` 全部截图引用指向 `-en` 文件；`README.zh-CN.md` 全部指向 `-zh` 文件；两处都新增了 UI 改动介绍段落；英文版保持英文，中文版保持中文
- [ ] `docs/release-notes-3.9.0.md` 新建，格式与 3.8.0 一致，中文，无手写大小/sha256
- [ ] `CHANGELOG.md` 加 `## [3.9.0] - November 2026` 英文条目；`public/changelog/ch/changelog.md` 加中文条目；17 份 changelog 全部无 BOM / 无 U+FFFD
- [ ] `package.json` 与 `src-tauri/Cargo.toml` 版本号均为 **3.9.0**（两处必须一致）
- [ ] `npm run verify-changelog` EXIT=0
- [ ] `release:upload` 成功，aiec `version.json` 的 `version` = 3.9.0，sha256 与本地包一致，`npm run release:verify` 四项一致
- [ ] GitHub release `v3.9.0` 存在且**正文非空**（用 `gh release view v3.9.0 --repo kizemo/alpha-file-manager` 确认正文有内容）
- [ ] `npm run tauri:build` EXIT=0，`release/Alpha File Manager_3.9.0_x64-setup.exe` 存在
- [ ] 装机验证：静默 `/S` 覆盖安装，`FileVersion` = 3.9.0，图标三方指纹一致（对照 `src-tauri/icons/32x32.png`）
- [ ] 真实用户数据三项 sha256 与装机前一致（或已正确还原），无夹具残留
- [ ] 门禁：`eslint .` 与基线逐条相同（零新增）、`vitest` 全过、`cargo test --lib` 全过、`vue-tsc --build` 0 错误
- [ ] `git status --porcelain` 行数变化可解释（新增截图 + CHANGELOG + 版本号 + 升级说明）

---

## 7. 本轮（r15–r17）留下的证据文件（需要细节时直接读）

| 路径 | 内容 |
|---|---|
| `.superpowers\sdd\r17-acceptance-report.md` | r17 完整验收报告（8,415 B） |
| `.superpowers\sdd\design-demos\icon-direction-approved.md` | 图标方向拍板记录（§8 = r17 参数与实测） |
| `.superpowers\sdd\r17-icon-master.cjs` | r17 落地脚本（20,917 B；含 `glyphTransform` / `bboxOf` / `counterPixels` / 字节守卫） |
| `.superpowers\sdd\r17-icon-report.json` | r17 机器可读报告 |
| `.superpowers\sdd\r17-icon-proof.png` | r17 目视验收板（红线 = 长方形部分垂直中心） |
| `.superpowers\sdd\r17-install.ps1` / `r17-verify-installed.ps1` / `r17-build.cmd` | 装机 / 复验 / 打包脚本（**纯 ASCII**） |
| `.superpowers\sdd\r17-installed-icon-{16,32}.png` / `r17-installed-icon-zoom.png` | 已装 exe 的图标抠图 |
| `.superpowers\sdd\r17-userdata-backup\user-data\` | 真实数据备份 |
| `.superpowers\sdd\r17-icon-backup\assets\` | r15 原图标资产备份 |

**r17 已验证的关键数字**（下一会话可作为回归基线）

```
GEOM  bodyTop=262.31 bodyBot=897.88 bodyCenter=580.10 pxPerUnit=56.7467
SOLVE a0Center=536.50 target=580.10 dy=0.7683
ITER  0 dy=0.7683 center=580.00 residual=-0.10
REPORT glyph{565x455, 244..808, 353..807, centerY 580, residual -0.1, ratioW 56.5} overhang=0
       counters{base{6,45}, new{3,36}}
       files{light 31145, dark 30914, side64 1677}
三方指纹 fingerprint=3836172  alphaBBox 0,3-31,28  opaque 723  meanRgb 197,190,177
```

---

## 8. 给下一会话的提醒（4 条）

1. **代码在内层仓** `F:\soft\00selfmade\filemanager\sigma-file-manager`（remote `kizemo/alpha-file-manager`）；外层 `F:\soft\00selfmade\filemanager` 的 remote 是 `kizemo/focus-sync`，只放 `AGENTS.md` 与历史档，**不跟踪内层代码**。提交、推送、打 tag 一律在内层仓。
2. **本轮明确不 commit、不 push**；发版动作里只有 `release:github` 会对外发布（用户已授权）。其余改动留在工作树。
3. **顺序不能乱**：`verify-changelog` → `release:prepare --bump minor` → `release:prepare --write-notes` → `release:upload` → `release:github` → `release:verify`。少一步会安静地产生「看起来成功、实际不一致」的故障。
4. **版本号必须是 3.9.0**，不是 3.8.0（3.8.0 已发布，线上 sha256 与本地不同；见第 0.1 节的三条证据）。

---

*文档生成于 2026-11-01。本文档只写事实与路径，不重复 AGENTS.md 已有的通用铁律细节之外的新内容。*
