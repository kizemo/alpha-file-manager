# 批次 35 交接：远程缩略图线收口 —— 缺陷修复 / 3.16.1 交付化 / 真机验收

> 上一份：`docs/handoff-2026-11-21-batch34-remote-thumbnail-implementation.md`
> 状态文件：`.superpowers/sdd/b32/STATUS-remote-thumb.md`
> 本轮全部结论以本文 §7「已验证 / 推断 / 未验证」为准，别把推断当实测。

## 0. 一句话现状

**远程缩略图线已收成可交付状态**：批次 34 遗留的最后一个真实产品缺陷「远程读取失败 → 点了没反应」已修复并真机坐实（同一探针在旧 3.16.0 上 FAIL、在新 3.16.1 上 PASS）；5 条既存 `unused import` 清干净；版本从 3.16.0 升到 **3.16.1**，根 `CHANGELOG.md` + 14 个本地化更新日志订正了被证伪的「直链缩略图」说法；四道门禁全绿；3.16.1 已打包、已装机。J1/J2 回归在 3.16.1 上 **run1 实测 PASS 且阈值逐字节未改**；其后四轮因**上游阿里云盘返回 HTTP 400** 记 UNDETERMINED（已坐实非本轮回归，且那几次的真实故障反过来又验证了「错误可见」）。**发布（推分支 / 推 tag / 上传站点 / 建 GitHub Release）已于同日 22:32–22:40 按用户拍板执行完毕**；两处陈旧资产（从未发布的 `v3.16.0` 标签、`release\` 顶层的 3.16.0 旧包）一并清理，详见 §12。

## 1. 背景（不重复调研，只给结论）

- 批次 34 落地了「远程（阿里云盘）缩略图 Rust 代理」：应用向网盘**要图、自己下载字节、按本地同样方式解码缩放**，远程缩略图独立缓存目录（`%APPDATA%\com.sigma-file-manager.app\remote-image-thumbnails`）。**只有阿里云盘支持，百度/夸克仍是普通文件图标。**
- 批次 34 的 J1/J2 真机验收在 run13 已 `VERDICT=PASS`；但当时仍有一处**真实产品缺陷**没修：远程目录读取失败时，UI 没有任何反馈（用户视角＝「点了没反应」）。
- 版本号 3.16.0 在本地**存在两份不同内容的构建**（顶层 `release/` 那份构建自 `5018804f`，含**直链**方案、缩略图不工作；`release/local-builds/` 那份才是批次 34 的新构建）⇒ 继续用 3.16.0 会踩「同号两份」的坑，**必须升号**。
- 线上 `https://www.aiec.fun/alpha-file-manager/version.json` 最新是 **3.14.0**；`v3.16.0` 标签指向 batch 29 提交 `0f55acf2`、**从未 push**、GitHub 上无 release ⇒ 3.15.0 / 3.16.0 都**未发布过**，本轮把 3.16.0 的内容并入 3.16.1 发布是安全的。

## 2. 本轮改动清单（`git status --porcelain` 实测，27 个文件）

### 2.1 缺陷修复（核心）

| 项 | 内容 |
| --- | --- |
| 文件 | `src/modules/navigator/components/file-browser/composables/use-file-browser-navigation.ts` |
| 位置 | `init()`（约 `:1117` 起） |
| 根因 | `readDir()` 开头（约 `:916`）**无条件** `error.value = null`；`init()` 的错误分支对**远程路径也**调 `navigateToDefaultDirectory()`，这会触发**第二次 `readDir`**，把刚写入的错误自己抹掉 ⇒ 面包屑落回「位置」、零可见反馈 |
| 修法 | `if (error.value)` 分支里，**远程路径不兜底**；仅本地路径保持原 `navigateToDefaultDirectory()` 行为 |
| 判定依据 | `isRemotePath`（`@/utils/remote-paths`，`sfm://remote` 前缀），已在 `:41` 导入 |
| 注释 | 代码里留了 `FORK-MODIFICATION 2026-11-21（批次 35）` 标记 |

**为什么这么修（取舍记录）**：走「不吞错误」而不是「加 toast」。既有 `FileBrowserError` 组件已经渲染 `error.value`，横幅是 DOM 文本、UIA 可读 ⇒ 不必新增 i18n key（新增 key 要同步 17 个语言包，收益不抵成本）；后端 `RemoteError` 已带 `kind`，前端 `describeRemoteError` 的**硬编码中文兜底文案合规**。改动面最小、零回归面，本地路径行为一字未动。

### 2.2 回归用例

- 文件：`src/modules/navigator/components/file-browser/composables/__tests__/use-file-browser-navigation.test.ts`
- 新增 1 条：`it('keeps the remote read failure visible instead of silently falling back to the default directory')`
  - `invokeMock` 对 `remote_read_dir` reject `{ kind: 'unreachable', message: '远端服务暂时不可用' }`
  - 断言 `error.value === '远端服务暂时不可用'`
  - 断言**未**调用 `path_exists`、**未**调用本地 `read_dir`（即确实没有兜底）
- 实测：**8 passed (8)**（该文件全绿）

### 2.3 清 5 条既存 `unused import`（批次 34 刻意留下的独立清理项）

| 文件 | 改动 |
| --- | --- |
| `src-tauri/src/compare/bench.rs` | 删 `use std::collections::BTreeMap;` |
| `src-tauri/src/image_edit.rs` | `use image::{...}` 去掉 `Rgb`，折成单行：`use image::{AnimationDecoder, DynamicImage, ImageFormat, ImageReader, RgbImage, Rgba, RgbaImage};` |
| `src-tauri/src/mcp/commands.rs` | 删 `use tauri::Manager;` |
| `src-tauri/src/remote/ftp_provider.rs` | `use std::net::{TcpListener, TcpStream};` → `use std::net::TcpListener;` |
| `src-tauri/src/remote/ftps_provider.rs` | 删 `use std::io::Read;` |

**只删 import，不动任何其它代码**；已由 `cargo test --lib` 全绿证明编译无误（见 §4）。

### 2.4 版本号 3.16.0 → 3.16.1

- `package.json`：`3.16.0` → `3.16.1`
- `node scripts/sync-version.js` ⇒ `Synced 3 Cargo manifests to 3.16.1`
  （`src-tauri/Cargo.toml`、`src-tauri/default-file-manager-common/Cargo.toml`、`src-tauri/default-file-manager-launcher/Cargo.toml`）
- `src-tauri/tauri.conf.json` 的 `"version": "../package.json"` ⇒ 无需另改
- `src-tauri/Cargo.lock` 由 cargo 自动同步

### 2.5 CHANGELOG 订正（根 + 14 本地化）

- 根 `CHANGELOG.md`：标题 `## [3.16.0] - November 2026` → `## [3.16.1] - November 2026`
- **缩略图那条整条重写**：删掉被证伪的「直接用网盘给出的图片地址」，改为「应用向网盘要图、自己下载字节、按本地同样方式解码缩放，远程缩略图独立缓存目录；目前只有阿里云盘支持，百度/夸克仍是普通文件图标」
- 摘要句尾补「缩略图视图与列表视图都出图 / 网盘读不出来会明说」
- 新增两条：「网格（缩略图）视图现在也出图」（尺寸量化、失败重试、处理中标记残留三处修复）、「网盘读不出来现在会明说而不是什么都不做」
- 14 个本地化更新日志（locale = `ch de es fa fr hi it ja pt ru sl tr ur vi`）全部同结构同步，**每节均为「摘要 + 11 条」**，行尾空格已清理
  - ⚠️ `public/changelog/` 下**没有 `en`**，英文只在根 `CHANGELOG.md`
  - ⚠️ `public/changelog/assets` 目录下**没有** `changelog.md`，遍历时要跳过

## 3. 门禁（本轮实测，全绿）

| 门禁 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test --lib -- --test-threads=1` | **1601 passed / 0 failed / 1 ignored**，EXIT=0（54.39 s） | `.superpowers/sdd/b35/gates-cargo-test.txt` |
| `npm run ts`（vue-tsc --build） | **EXIT_ts=0** | `.superpowers/sdd/b35/gates-ts.txt` |
| `npm run lint:check`（eslint + stylelint，`--max-warnings 0`） | **EXIT_lint=0** | `.superpowers/sdd/b35/gates-lint.txt` |
| `npm run test:unit:run` | **Test Files 325 passed / Tests 3492 passed**（126.79 s），EXIT=0（基线 3491 + 新增 1） | `.superpowers/sdd/b35/gates-unit.txt` |

- `global_search` 那条历史抖动本轮**未复现**。
- 首次 lint 曾红一次，唯一错误是新增用例第 397 行 `@stylistic/object-property-newline`（单行对象字面量），已用项目自带 eslint `--fix` 拆成多行对象后重跑整仓确认 0。

## 4. 打包与装机（3.16.1）

| 项 | 值 |
| --- | --- |
| 打包命令 | `npm run tauri:build`（Rust 1.90.0 工具链），EXIT_build=0 |
| 构建日志 | `.superpowers/sdd/b35/build.txt`（253,214 B） |
| 裸产物 exe | `src-tauri\target\release\alpha-file-manager.exe`，**85,031,424 B**，21:49:22 |
| NSIS 安装包 | `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.16.1_x64-setup.exe`，**45,941,645 B**，21:49:20 |
| 发布到 | `release\Alpha File Manager_3.16.1_x64-setup.exe`（**顶层**）。落位由 `scripts\publish-bundle-to-release.mjs` 的规则决定：目标不存在 → 落顶层；存在且内容相同 → 不动；存在但内容不同 → 改写 `local-builds/` 并告警。3.16.1 在顶层不存在同名包，故落顶层，sha16=`6FF25301DFECDCDA`，与 `bundle\nsis` 产物逐字节相同。反过来，**3.16.0 因顶层已有内容不同的同名包，被自动改写到 `local-builds/`（45,938,661 B），顶层那份 45,849,872 B 原样保留** |
| 装机 | `.superpowers/sdd/b32/b34-install.ps1`，`/FORCE /S`，INSTALL_EXIT=0 |
| 装机副本 | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，size=**85,031,424**，ver=**3.16.1**，normsha16=`d71883872919bd61` |
| 身份判据 | `SIZE_MATCH=True` / `NORMSHA_MATCH=True` / **`VERDICT=INSTALL_IDENTITY_OK`** |
| 安装器日志 | `C:\Users\Duanyi\AppData\Local\Temp\alpha-file-manager-install.log`（`installerVersion=3.16.1 detectedVersion=3.16.0 sameVersionReinstall=no`） |

## 5. 真机验收 A：新判据「远程读取失败必须可见」（本轮新增）

探针：`.superpowers/sdd/b35/b35-accept-remote-error.ps1`（纯 ASCII、自带 win32 shim、私有桌面 + UIA + 鼠标等价注入、防火墙规则写在 `try/finally`）。

**两阶段、单一受控变量**：

- **阶段 A（通网）**：远程根必须真的列出条目，且**不得**出现错误横幅 ⇒ 证明「判据看得见目标」。
- **阶段 B（按 exe 路径阻断出站）**：远程读取失败，**错误横幅必须可见**。
- **阴性对照**：候选串 `☃-B35-IMPOSSIBLE-CONTROL-☃` 在 A/B 两阶段都必须 **0 命中**，命中即判据本身坏了、全部判定作废。

**A/B 对照结果（同一套探针、同一判据）**：

| 构建 | 阶段 A | 阶段 B | 整体 VERDICT |
| --- | --- | --- | --- |
| 旧 **3.16.0**（含缺陷，装机版） | PASS（远程根列出 30 条、无横幅） | **FAIL**（远程读失败但**无横幅**，落回「位置」＝错误被吞） | **FAIL** |
| 新 **3.16.1**（本次修复，装机版） | PASS（远程根列出 30 条、无横幅） | **PASS**（横幅出现，标题「无法打开此文件夹」） | **PASS** |

- 阴性对照：两构建 A/B 两阶段**全部 0 命中** ⇒ 判据本身有效。
- 防火墙规则 `B35_AlphaFM_Outbound_Block` 在阶段 B 结束后已删除（`FIREWALL_RULE_REMOVED=true`、`stillPresentAfterRun=false`）。
- 旧构建对照证据另存为 `b35-accept-remote-error-OLD3160.json` / `-OLD3160.log`，未被覆盖。

## 6. 真机验收 B：J1/J2 回归（阈值未改）

探针：`.superpowers/sdd/b32/b32-accept-remote-ui.ps1`（`-ExePath` / `-OutDir` / `-TimeoutSec` / `-ConnectionName` / `-KeepOpen`）。

**本轮未改动任何阈值**，脚本内原值如下（引用即证明没降门槛）：

```
J1_HUE_MIN          = 5.0     J1_NONBG_DELTA    = 0.15
J1_UNIQUE_RATIO     = 3.0     J1_MIN_SAMPLES    = 3
J1_PLACEHOLDER_NONBG= 0.20
J2_CHANGED_RATIO    = 0.05    J2_CHANGED_MEANABS= 3.0
J2_UNCHANGED_RATIO  = 0.01    J2_UNCHANGED_MEANABS = 1.0
```

结果：**见 §6.1（由本轮实跑回填）**。

### 6.1 验收结论

**J1/J2 不降阈值回归 = PASS（run1 实测有效）**；随后**三轮**（run2/run3/run4）因**上游阿里云盘 HTTP 400** 记 UNDETERMINED（不是构建回归，理由见下）。四轮都跑在**同一份 3.16.1 二进制**上 —— 四次 `preflight.exe` 均为 `version=3.16.1 bytes=85031424 sha256_16=391134BF073F0D5C`。

| 运行 | 时间 | 结论 | 依据 |
| --- | --- | --- | --- |
| **run1** | 21:53–21:56 | **PASS** | J1：`qualifying=8 placeholder=0 total=8`（对照项是 mp4 通用图标 `hue=1 / unique=43 / nonBg=0.0503`，8 ≥ 阈值 3）；J2：`entry1Changed=True entry2Changed=False`、`count=1`（点一个只选中一个） |
| run2 | 22:02–22:04 | **UNDETERMINED** | `NAV_ROOT_TIMEOUT` / `FATAL could not reach the remote root`，J1/J2 均 `not evaluated`。**启动截图逐字显示「无法打开此文件夹 / 阿里云盘返回了 HTTP 400，响应内容无法识别。」** |
| run3 | 22:06–22:07 | **UNDETERMINED** | 同 run2，同一张截图、同一个报错 |
| **run4**（22:19–22:21 复测） | 22:19–22:21 | **UNDETERMINED** | 同 run2/run3：`NAV_ROOT_TIMEOUT` → `FATAL could not reach the remote root`，`errors=[VIEW_NOT_BROWSER_AFTER_RAIL_SWEEP, NAV_ROOT_TIMEOUT, FATAL …]`，J1/J2 均 `not evaluated`。**启动截图（`b32-accept-remote-ui-01-boot.png`）逐字仍是「无法打开此文件夹 / 阿里云盘返回了 HTTP 400，响应内容无法识别。」**，右栏连接 `阿里云盘` 路径 `sfm://remote/689681a8458d40979383d1ed8efbd948` 正常，面板 `0 个项目` + 错误横幅可见 |

**阈值逐字节未改**：run1 与 run2 报告的 `thresholds` 对象完全相同（脚本比对结果 `True`）；run4 报告的 `thresholds` 与之逐字节一致 —— `J1_hueMin=5`、`J1_hueRatio=2`、`J1_nonBgDelta=0.15`、`J1_uniqueRatio=3`、`J1_minSamples=3`、`J1_uniformStdLum=6`、`J1_uniformNonBg=0.05`、`J1_placeholderNonBg=0.2`、`J2_changedRatio=0.05`、`J2_changedMeanAbs=3`、`J2_unchangedRatio=0.01`、`J2_unchangedMeanAbs=1`。

**run2/run3/run4 不是构建回归**，三条理由：

1. 同一份二进制在 run1 刚刚 PASS；run4 的 `preflight.exe` 报出的 `sha256_16=391134BF073F0D5C` 与 run1 **逐字相同**，跑的是同一个安装包；
2. 失败原因是**上游返回 HTTP 400**，报错文案与截图都在现场文件里，不在我方代码路径上（run4 截图再次逐字复现同一句报错）；
3. 本轮改动只有「错误发生之后怎么办」，**不可能制造 HTTP 400**（网络代码一行未动；5 条 `unused import` 是纯删除；版本号只改字符串）。

**附带收获**：run2/run3/run4 这三次**真实上游故障**，反过来又证明修复在真实故障下生效 —— 错误留在原地可见（标题「无法打开此文件夹」＋原因「阿里云盘返回了 HTTP 400…」），而不是像 3.16.0 那样静默跳回「位置」零提示。

**上游活性复核（22:10，`.superpowers\sdd\b35\live-A\`）**：**通网**状态下只跑阶段 A ⇒ `PHASE_A=UNDETERMINED`，理由「remote content did not appear; **an error banner was visible although the network was open**」，远程条目数 0。⇒ **上游此刻仍坏着**，本轮末尾无法再做一次干净的 J1/J2。

**run4 复测（22:19–22:21，交付收口前）**：再次跑完整 J1/J2 脚本 ⇒ 仍 `NAV_ROOT_TIMEOUT`（`nav.remote_root` FAIL，`VIEW_NOT_BROWSER_AFTER_RAIL_SWEEP` 先行出现但脚本按既定回退继续），J1/J2 `not evaluated`，`VERDICT=UNDETERMINED`。**同一二进制、同一阈值、同一失败点、同一句上游 400 报错** ⇒ 上游自 20:55 起一直没恢复，**不是**本轮回归。

**排除本机/本轮因素的两条旁证**：

- **防火墙已彻底清理**：按 `ApplicationFilter.Program` 精确查该 exe 路径，只剩 2 条**入站 Allow**（安装器自带的旧规则），**没有任何出站 Block**；b35 探针自己也报 `stillPresentAfterRun=false`。run4 之后复查仍是这一条（同名规则只有那 2 条入站 Allow；出站 Block 名单里只有 Xbox / codex_sandbox / recuva 等无关项）。
- **远程缩略图缓存写入停在 20:55:50**（56 个文件，最新 mtime 20:55:50）——run1 的 J1 是**复用缓存里已有的图**；21:00 之后上游就不再供图，与 run2/run3/run4 的失败窗口吻合。run4 时点（22:21）复查仍是 **56 个文件 / 最新 mtime 20:55:50**，即上游三个多小时没再供过一张新图。

**已知非致命瞬时项**：`VIEW_NOT_BROWSER_AFTER_RAIL_SWEEP`（t≈28–30 s 时侧栏还没渲染完，`RAIL_CANDIDATES count=0`），脚本随后回退到已打开的浏览视图（`VIEW_ALREADY_BROWSER`）继续；**run1 与 run4 都出现过**，不影响 J1/J2 判定。

## 7. 已验证 / 推断 / 未验证（别混）

**已验证（实测）**

1. 缺陷根因与修法：`init()` 的兜底 + `readDir` 开头无条件清错 ⇒ 第二次 `readDir` 抹掉错误。修复后单测 8/8 通过。
2. 旧 3.16.0 装机版：同一探针判 **FAIL**（远程读失败无横幅）；新 3.16.1 装机版判 **PASS**（横幅可见）⇒ **修复在真机上成立**，且判据有判别力（阴性对照 0 命中、阶段 A 正常态可见）。
3. 四道门禁全绿：cargo 1601/0/1、ts=0、lint=0、unit 3492。
4. 3.16.1 打包成功、装机成功、装机副本与构建产物字节一致（`VERDICT=INSTALL_IDENTITY_OK`）。
5. 5 条 `unused import` 删除后 Rust 编译无误（cargo 全绿）。
6. 14 个本地化更新日志的 3.16.1 节结构一致（摘要 + 11 条）。

**推断（未直接实测，但有强证据）**

1. 3.16.0 从未发布过（线上 version.json 是 3.14.0；`v3.16.0` 标签未 push；GitHub 无 release）⇒ 把 3.16.0 的内容并入 3.16.1 发布，用户侧不会出现「同名两个版本」。
2. 百度/夸克仍是普通文件图标 —— 依据 `remote/thumbnails.rs` 里只有阿里云盘实现 `thumbnail_bytes`，其余 provider 走 trait 默认 `Ok(None)`；**未在真机上逐个盘符点验**。

**未验证**

1. 3.16.1 尚未推送、尚未上传站点、尚未打 tag。
2. `release/` 顶层那份**未发布的 3.16.0 旧包**（45,849,872 B，构建自 `5018804f`）如何处置 —— 未动，留用户决定。
3. 陈旧 `v3.16.0` 标签（指向 batch 29 提交 `0f55acf2`）是否清理 —— 未动，留用户决定。
4. 百度/夸克盘的缩略图表现未逐个真机点验。

## 8. 下阶段建议（按优先级）

1. **用户拍板发布**（本轮刻意不做，属对外动作）：推 `v3.16.1` tag + push + 上传安装包到站点 `version.json`。仓库自带发布脚本：`npm run release:status` / `release:prepare` / `release:upload` / `release:verify` / `release:github`。**注意 `main` 当前比 `origin/main` 领先 4 个提交**（批次 33、34、35 都还没 push），推 tag 前应先把分支推上去。
2. **用户拍板清理**：陈旧 `v3.16.0` 标签（指向未发布的 `0f55acf2`）是否 `git tag -f` 改指或删除；`release/` 顶层那份 3.16.0 旧包是否移入 `local-builds/`。
3. **上游恢复后补一次干净的 J1/J2**（不阻塞本轮交付）：run1 已是有效 PASS，但 run2/run3/run4/live-A **四次**都因阿里云盘 HTTP 400 未判定（run4 是 22:19–22:21 的复测）。等阿里云盘恢复（若是登录态失效，在应用里重新扫码登录一次）后，用同一条命令重跑 `.superpowers\sdd\b32\b32-accept-remote-ui.ps1` 取一份 `errors` 为空的报告即可。
4. 若要继续「远程缩略图」线：把 `thumbnail_bytes` 能力从阿里云盘扩展到百度/夸克（trait 层已有默认实现可覆写，成本主要在各家 API 的取图接口）。
5. 可选：把「远程读取失败必须可见」这条判据并入常驻回归脚本（它自带阴性对照，比人眼看截图可靠）。

## 9. 本轮工具坑（复用清单）

1. **eslint 对单行对象字面量报 `@stylistic/object-property-newline`** —— 新增用例里的 `Promise.reject({ ... })` 必须拆成多行对象再写。
2. **别用中英文关键词 grep 本地化 changelog 来判断「改没改」** —— 各语言措辞不同，会误判成「13 个语言没改」。正确做法是**数每节 `**` 行数**（本轮口径：摘要 + 11 条）。
3. **`public/changelog/assets` 下没有 `changelog.md`** —— 遍历本地化目录时要跳过，否则会误报「缺文件」。
4. **直跑 `pwsh -File` 时 stdout 全丢** —— 必须 `Start-Process pwsh -PassThru -RedirectStandardOutput/RedirectStandardError`。
5. **同版本重装 NSIS 要带 `/FORCE`**（本轮是新版本 3.16.1→3.16.1 之外的 3.16.0→3.16.1，其实无需 `/FORCE`，但脚本统一带上）；`INSTALLED_MTIME_MOVED` 在 `/FORCE` 下**无判别力**，别拿它当判据。
6. **装机副本与 `target\release` 裸产物的 sha256 必然不等** —— 差异只有 `__TAURI_BUNDLE_TYPE_VAR_*` 标记（等长 ⇒ 尺寸不变）。判据要用「大小相等 + 正则归一化标记后逐 64 KiB 块比 sha」。
7. **本机硬安全策略**：禁止 `Set-Location`/`cd` 后用相对写入路径（改用 `git -C` + 绝对路径）。
8. **验收探针必须自带正反对照**：先在通网状态让同一判据命中一次，再上故障态；否则 0 命中分不清「没触发」和「没修好」。
9. **云盘上游挂掉会让整轮验收 UNDETERMINED，别急着判回归**：本轮 run2/run3/live-A 三次全灭在「远程读失败」这一步，截图里是**上游返回 HTTP 400**。判回归之前先做三件事：① 读启动截图看报错文案是上游还是本地；② 查防火墙**按 exe 路径**有没有残留出站 Block（用 `Get-NetFirewallApplicationFilter` 按 `Program` 精查，别逐条 `Get-NetFirewallRule` 再查 filter —— 那会慢到分钟级）；③ 看远程缩略图缓存的**最新 mtime**，它能给出上游开始坏的时刻。
10. **同一轮里先跑成功的 PASS 和后跑的 UNDETERMINED 不矛盾**：关键是**二进制同一性**（比 `sha256_16`）＋**阈值对象逐字节相同**（比 JSON 里的 `thresholds`）＋**失败点在采样之前**。三条齐了，才敢判「不是回归」。

## 10. 证据路径索引（全路径）

仓库根：`F:\soft\00selfmade\filemanager\sigma-file-manager`

**本轮门禁**

- `.superpowers\sdd\b35\gates-cargo-test.txt`
- `.superpowers\sdd\b35\gates-ts.txt`
- `.superpowers\sdd\b35\gates-lint.txt`
- `.superpowers\sdd\b35\gates-unit.txt`

**打包 / 装机**

- `.superpowers\sdd\b35\build.txt`
- `.superpowers\sdd\b35\b35-install.log`
- `.superpowers\sdd\b35\b35-install.err.log`

**验收 A（错误可见）**

- 探针脚本：`.superpowers\sdd\b35\b35-accept-remote-error.ps1`
- 新构建（3.16.1）报告：`.superpowers\sdd\b35\new3161\b35-accept-remote-error.json`
- 新构建日志：`.superpowers\sdd\b35\new3161\b35-accept-remote-error.log`
- 新构建截图：`.superpowers\sdd\b35\new3161\b35-accept-remote-error-boot-phaseA.png`、`-phaseA.png`、`-boot-phaseB.png`、`b35-accept-remote-error.png`
- 节点快照：`.superpowers\sdd\b35\new3161\b35-accept-remote-error-nodes-phaseA.txt`、`-nodes-phaseB.txt`
- 旧构建（3.16.0）阴性对照：`.superpowers\sdd\b35\b35-accept-remote-error-OLD3160.json`、`-OLD3160.log`
- 运行日志：`.superpowers\sdd\b35\b35-run-new3161.stdout.log`、`b35-run-old-build.stdout.log`

**验收 B（J1/J2）**

- run1（**有效 PASS**）：`.superpowers\sdd\b35\j1j2-3161\b32-accept-remote-ui.json`、`b35-accept-remote-ui-01-boot.png` … `-12-j2-context-menu.png`、`.superpowers\sdd\b35\b35-j1j2.stdout.log`
- run2（UNDETERMINED，上游 400）：`.superpowers\sdd\b35\j1j2-3161-run2\b32-accept-remote-ui.json`、`b32-accept-remote-ui-01-boot.png`（**含 400 报错横幅**）、`.superpowers\sdd\b35\b35-j1j2-run2.stdout.log`
- run3（UNDETERMINED，上游 400）：`.superpowers\sdd\b35\j1j2-3161-run3\b32-accept-remote-ui.json`、`b32-accept-remote-ui-01-boot.png`、`.superpowers\sdd\b35\b35-j1j2-run3.stdout.log`
- run4（UNDETERMINED，上游 400，收口前复测）：`.superpowers\sdd\b35\j1j2-3161-run4\b32-accept-remote-ui.json`、`b32-accept-remote-ui-01-boot.png`（**含 400 报错横幅**）、`.superpowers\sdd\b35\b35-j1j2-run4.stdout.log`

**上一轮沿用**

- `.superpowers\sdd\b32\b32-accept-remote-ui.ps1`（J1/J2 主脚本，102 KB）
- `.superpowers\sdd\b32\b34-install.ps1`（装机 + 字节身份核对）
- `.superpowers\sdd\b32\STATUS-remote-thumb.md`
- `.superpowers\sdd\b32\b34-accept-run13.log`、`b34-accept-run14.log`
- `docs\handoff-2026-11-21-batch34-remote-thumbnail-implementation.md`

## 11. 批次 35 收尾记录（2026-11-21 21:30–22:21，含 run4 收口复测）

### 11.1 收尾判词（本轮实测）

| 项 | 判词 |
| --- | --- |
| 缺陷修复 | 根因坐实（兜底吞错 + 无条件清错），单测 8/8，真机 A/B 对照（旧 FAIL / 新 PASS） |
| 门禁 | cargo 1601 passed / 0 failed / 1 ignored；ts=0；lint=0；unit 3492 |
| 打包 | 3.16.1 exe 85,031,424 B + NSIS 45,941,645 B，EXIT_build=0 |
| 装机 | `VERDICT=INSTALL_IDENTITY_OK`，装机副本 ver=3.16.1 |
| 验收 A | 旧 3.16.0 FAIL → 新 3.16.1 **PASS**（横幅可见，阴性对照 0 命中） |
| 验收 B | **run1 PASS**（J1 `qualifying=8/8`、J2 `count=1`，阈值逐字节未改）；run2/run3/run4/live-A **UNDETERMINED** —— 根因坐实为**上游阿里云盘 HTTP 400**（run4 截图再次逐字复现），非本轮回归 |
| 发布 | **已执行**（同日 22:32–22:40 按用户拍板：推分支 + tag + 站点 + GitHub Release，并清理两处陈旧资产）—— 详见 §12 |

### 11.2 本轮新踩的工具坑

并入 §9 同类清单：第 1、2、3 条是本轮新增。

### 11.3 本轮提交范围

`git status --porcelain` 实测 27 个已改文件、**0 个未跟踪文件**：

```
 M CHANGELOG.md
 M package.json
 M public/changelog/{ch,de,es,fa,fr,hi,it,ja,pt,ru,sl,tr,ur,vi}/changelog.md
 M src-tauri/Cargo.lock
 M src-tauri/Cargo.toml
 M src-tauri/default-file-manager-common/Cargo.toml
 M src-tauri/default-file-manager-launcher/Cargo.toml
 M src-tauri/src/compare/bench.rs
 M src-tauri/src/image_edit.rs
 M src-tauri/src/mcp/commands.rs
 M src-tauri/src/remote/ftp_provider.rs
 M src-tauri/src/remote/ftps_provider.rs
 M src/modules/navigator/components/file-browser/composables/__tests__/use-file-browser-navigation.test.ts
 M src/modules/navigator/components/file-browser/composables/use-file-browser-navigation.ts
```

**不含** `.superpowers/` 下的探针、日志、截图（已被 gitignore），**不含** `release/` 下的安装包。

## 12. 3.16.1 发布记录（2026-11-21 22:32–22:40，用户拍板后执行）

> 完整执行日志：`.superpowers\sdd\b36\publish-log.md`（4,054 B）
> 中文用户面向发布说明：`.superpowers\sdd\b36\release-notes-v3.16.1.zh.md`（5,571 B）

### 12.1 拍板范围

用户问卷拍板：**① 全部发布**（推分支 + 推 tag + 上传站点 + 建 GitHub Release）；**② 都清理**（删 `v3.16.0` 标签 + 旧包移入 `local-builds/`）。两项均已执行完毕。

### 12.2 陈旧资产清理

| 项 | 处置 | 复核 |
| --- | --- | --- |
| 本地标签 `v3.16.0`（指向未发布的 `0f55acf2`） | `git tag -d v3.16.0` | 删除前 `git ls-remote --tags origin` 只有 `v3.14.0` ⇒ **该标签从未 push**，远端无需处理 |
| `release\Alpha File Manager_3.16.0_x64-setup.exe`（45,849,872 B） | 移入 `release\local-builds\`，改名 `...build-20261121-112222.exe` | `local-builds/` 已有一份同名 3.16.0（45,938,661 B），改名避免覆盖；两份都是从未发布过的 3.16.0 构建 |
| 站点源 `Alpha_File_Manager_3.16.0_x64-setup.exe`（45,847,038 B） | 改名 `...setup.unpublished`（**去掉 `.exe` 后缀**） | `_upload_alpha_files.py` 全量上传会收走目录里所有 `.exe`，不改名就会把含「直链缩略图」旧方案的包推到公网 |

清理后 `release/` 顶层最新 = 3.16.1（mtime 21:49:20）⇒ `latestInstaller()`（按 **mtime** 取最新，不是版本号）仍选中 3.16.1 ✓。

### 12.3 发布顺序（固定，不可调换）

```
commit → push origin main → git tag -a v3.16.1 → git push origin v3.16.1
       → prepare(--bump none --skip-build) → 先传安装包 → 再传清单 → verify → GitHub Release
```

| 步骤 | 实测结果 |
| --- | --- |
| `git push origin main` | `46390569..c8427ccf`（批次 33/34/35 五个提交全部上去了） |
| `git tag -a v3.16.1` + `push origin v3.16.1` | 远端 `v3.16.1^{}` = `c8427ccfcb5e267341f7afe49250d513072f929d` |
| `prepare --bump none --skip-build --notes-file …` | 版本 3.16.1 → 3.16.1；安装包 45,941,645 B；sha256 `6ff25301dfecdcda0e4f968a9b183221f90996d8392e33c225b9c53e4c4593ab`；已复制到站点源目录（sha 一致） |
| `_upload_alpha_files.py --only Alpha_File_Manager_3.16.1_x64-setup.exe` | 43.81 MB，服务器端 **md5 + size 双一致**（md5=`89230f26bde62c53550b53c65ada74af`） |
| `_upload_alpha_files.py --files-only` | index.html / .htaccess / version.json / download.php，4 个文件 md5 + size 均一致 |
| `release-publish.mjs verify` | 「版本号 / 大小 / sha256 三项均一致（源站与边缘都已生效）」，EXIT=0 |
| `release-publish.mjs github` | EXIT=0，新建 release |

**为什么先包后清单**：清单（`version.json`）会指向安装包；先传清单会让边缘节点在包还没到位时把用户指向 404。

### 12.4 线上最终回读（带 cache-busting）

| 项 | 实测值 |
| --- | --- |
| `https://www.aiec.fun/alpha-file-manager/version.json` | `version=3.16.1`、`pub_date=2026-11-21T14:32:34Z`、`sha256=6ff25301…` |
| `history` | `history_count=20`、`history[0]=3.14.0`、**`3.16.0 in history? False`** |
| 安装包直链 HEAD | `200`、`len=45,941,645`、`CF-Cache-Status=HIT` |
| 下载页 | `https://www.aiec.fun/alpha-file-manager/` |

### 12.5 GitHub Release

`https://github.com/kizemo/alpha-file-manager/releases/tag/v3.16.1` — `isDraft=false`、`isPrerelease=false`、`publishedAt=2026-11-21T14:40:15Z`；资产 `Alpha_File_Manager_3.16.1_x64-setup.exe`（45,941,645 B，`digest sha256:6ff25301…`）。

> `gh release create` 要求**远端已有该 tag**，否则会指向远端默认分支 HEAD —— 所以 push tag 必须排在建 release 之前。

### 12.6 站点清单的两个坑（发布期实测）

1. **站点源清单已被某次未上传的 `prepare` 写成 3.16.0**（线上真实版本是 3.14.0）。直接 prepare 3.16.1 会把**从未发布的 3.16.0**（含已证伪的「直链缩略图」说法）归档进公开 `history[]` ⇒ 先把站点源清单**重置为线上真实状态**（3.14.0，history[0]=3.13.5），再 prepare ⇒ 最终 `history[0]=3.14.0`，**3.16.0 不进历史**（线上实测查无此条目）。备份：`E:\办公文件\L1网站\alpha-file-manager\version.json.bak-20261121-223229`。
2. **`prepare` 默认 `--bump patch` 会再升一版**（3.16.1 → 3.16.2）⇒ 必须 `--bump none --skip-build`（版本已是 3.16.1、产物已打好）。

### 12.7 发布期新增的工具坑（并入 §9 同类清单）

| # | 坑 | 正确姿势 |
| --- | --- | --- |
| 1 | `release-publish.mjs` 的 `latestInstaller()` 按 **mtime** 取最新，不是版本号 | 清理旧包后必须复核 `release/` 顶层最新 mtime 仍是目标版本 |
| 2 | 站点源 `version.json` 会被**未上传的 prepare** 留成幻影版本 | 发布前把站点源清单重置为线上真实状态 |
| 3 | `_upload_alpha_files.py` 全量模式会收走目录里**所有 `.exe`** | 只传本次该传的：先 `--only <安装包>`，再 `--files-only` 传清单；未发布包改掉 `.exe` 后缀 |
| 4 | `npm run release:upload` 是全量上传 | 本次不走它（会重传 40 个历史包 + 连带推送未发布的 3.16.0） |
| 5 | `prepare` 默认会 bump 版本 | 用 `--bump none --skip-build` |

### 12.8 证据路径

- 发布执行日志：`.superpowers\sdd\b36\publish-log.md`
- 中文发布说明：`.superpowers\sdd\b36\release-notes-v3.16.1.zh.md`
- 带构建信息的版本说明：`E:\办公文件\L1网站\alpha-file-manager\_release-notes-v3.16.1.md`（5,780 B）
- 站点源清单备份：`E:\办公文件\L1网站\alpha-file-manager\version.json.bak-20261121-223229`
- 站点源目录：`E:\办公文件\L1网站\alpha-file-manager\`
- 发布产物：`release\Alpha File Manager_3.16.1_x64-setup.exe`（45,941,645 B，sha256 `6ff25301…`）

### 12.9 发布后状态

- 本地 `main` 与 `origin/main` 同步（发布后 HEAD = `c8427ccf`），远端已有 `v3.16.1` 标签。
- 工作树干净；无残留出站防火墙规则；无应用进程在跑。
- 唯一未闭合项仍是 §7 里的 J1/J2：上游阿里云盘 HTTP 400 恢复后补一次 `errors` 为空的干净报告（run1 已有效 PASS，不阻塞交付）。

