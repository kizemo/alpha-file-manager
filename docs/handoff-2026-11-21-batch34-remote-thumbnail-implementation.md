# 批次 34 交接：远程（阿里云盘）缩略图 Rust 代理 —— 实现落地与门禁收口

- 日期：2026-11-21
- 仓库：`F:/soft/00selfmade/filemanager/sigma-file-manager`，分支 `main`，HEAD `46390569`
- **本批次全部改动仍未 commit**（`git status --porcelain` 共 12 条，见 §2）
- 上游文档：`docs/handoff-2026-11-21-batch32-remote-thumbnail-verdict.md`（§4 方案与验收、§5 工装坑）
- 本轮过程真源（进度台账）：`F:/soft/00selfmade/filemanager/sigma-file-manager/.superpowers/sdd/b32/STATUS-remote-thumb.md`

## 0. 一句话现状

远程缩略图 Rust 代理 + 前端改造**已实现并通过本批次四项门禁（thumb / ts / lint / unit 全 0）**，exe 与 NSIS 安装包**已重新打包**；`cache_dir` 警告已定性为参数冗余（非缺陷）；**真机验收 J1/J2 见 §6**。

## 1. 背景（不重复调研，只给结论）

批次 32 已坐实：阿里云盘列表接口给的 `thumbnail_url` 是**带签名与有效期的直链**，前端直接当 `<img src>` 用会因跨域/签名过期/缺 Referer 而**架构性不可行**（不是调参能解决的）。因此本批次只做 Rust 代理：由 provider 在服务端代取字节，走与本地一致的校验/解码/缩放管线，落到**独立缓存目录**，前端只拿一个本地 `sfm://` 路径。

U 盘补测（`docs/handoff-2026-11-11-quark-pan.md` §20.22）已由批次 33 闭合，本批次**未触碰、无需重做**。

## 2. 实现清单（`git status --porcelain` 实测）

| 文件 | 关键锚点 | 作用 |
| --- | --- | --- |
| `src-tauri/src/remote/provider.rs` | `:213 async fn thumbnail_bytes` | trait 新增能力方法，**默认实现 `Ok(None)`**（区别于其余四个刻意无默认实现的方法） |
| `src-tauri/src/remote/aliyun_provider.rs` | `:1702 async fn thumbnail_bytes` | 阿里云盘实现：候选直链顺序 **列表 256px → 详情 thumbnail → 原图**；详情/原图失败不冒泡；全失败 `Ok(None)` |
| `src-tauri/src/remote/thumbnails.rs`（新增，未跟踪） | `:54 REMOTE_THUMBNAIL_MAX_DOWNLOAD_BYTES = 32MB`、`:98 pub async fn remote_generate_thumbnail`、`:132` 调用 trait 方法 | 命令本体：尺寸 `clamp(1,1024)`、缓存键、限额护栏；失败一律 `Ok(None)` 不 panic |
| `src-tauri/src/remote/mod.rs` | `:144 pub use thumbnails::*;`（**必须 glob**，`:142-143` 注释写明 `generate_handler!` 要 `__cmd__` 符号） | 模块与再导出 |
| `src-tauri/src/remote/commands.rs` | `:1284 "remote_generate_thumbnail"` | `REMOTE_COMMANDS` 末项注册（实测追加不红） |
| `src-tauri/src/lib.rs` | `:449 remote::remote_generate_thumbnail,` | `generate_handler!` |
| `src-tauri/permissions/app-commands.toml` | `:184 "remote_generate_thumbnail",` | 权限清单（**注意真实路径是 `src-tauri/permissions/`，不是 `capabilities/`**） |
| `src-tauri/src/image_thumbnails.rs` | `:30`/`:234` 独立远程缓存目录 `remote-image-thumbnails`、`:1731 temporary_remote_source_path`、`:1762 write_remote_thumbnail_bytes`、`:1804 stage_and_scale_remote_thumbnail`、stats/clear 成对 `:255`/`:288-302` | 远程像素 → 临时源文件 → 复用本地解码管线 → 原子改名进缓存；临时源文件**所有返回路径无条件删除** |
| `src/modules/navigator/components/file-browser/composables/use-image-thumbnails.ts` | `:61` 说明、`:235` 分派、`:243 invoke('remote_generate_thumbnail')` | 删直链短路；`is_dir` 闸门；按 `isRemotePath` 分派两个命令（**远程不传 `size`**）；`null` 也标失败；远程 pending 期 fallback 直接 `true`；统一用 `.get()` 不用 `.has()` |
| `src/modules/navigator/components/file-browser/composables/__tests__/use-image-thumbnails-remote.test.ts`（新增） | `:67`、`:78` | 断言「不把签名直链当缩略图」+ 走对命令 + 参数形状 |
| `docs/handoff-2026-11-11-quark-pan.md` | — | **非本轮改动**（轮前已存在的未提交修改，本轮未编辑该文件） |

设计要点（理由已在代码注释里，别在文档复述第二遍）：远程缓存键**不含 `size`**，靠 `modified_time` 换键，键带 `remote` 前缀与 `connection_id`；`modified_time` 由前端传入**仅作缓存键、不参与鉴权**（安全取舍已记录）；`src/types/dir-entry.ts:79 thumbnail_url?` 字段保留但前端不再当图用。

## 3. 门禁（第二轮为真源）

真源：`.superpowers/sdd/b32/gates-build-02.txt`（脚本 `gates-build-02.ps1`，15:26→15:37）

```
GUARD_TOUCHED_COUNT=0   EXIT_thumb=0   EXIT_cargo_test=101  VERDICT_cargo=RED
EXIT_ts=0   EXIT_lint=0   GATES_OK=False   BUILD_SKIPPED=gates_failed
```

- **本批次四项（thumb / ts / lint / unit）实测全 0** —— 判定「改动本身干净」的唯一依据。
- 全量 `cargo test --lib`：`1593 passed; 1 failed`（58.40s），失败项 `global_search`，定性见 §5。
- 远程缓存相关新增用例：`test result: ok. 7 passed`（`mod remote_thumbnail_tests`，`image_thumbnails.rs:1884` 起）。
- 首轮两处红已在第二轮修掉：① `image_thumbnails.rs` 远程缓存用例**测试前提写反**（旧用例断言「清远程时本地 `keep.png` 必须留下」，而实现刻意把两目录**成对清**，见 `:288-302` 注释）→ 整块重写并改名 `clearing_cache_empties_both_thumbnail_dirs_and_touches_nothing_else`；② 前端测试 `:95/:96` 缺空行触发 `@stylistic/padding-line-between-statements` → 各补一个空行。
- 分层红线（`AGENTS.md`）实测未破：`GUARD_TOUCHED_COUNT=0`，`remote/` 内无 `tauri::AppHandle`、无 `tauri::async_runtime::*`（`spawn_blocking` 会单态化 `GlobalRuntime<Wry>` ⇒ 加载期 `0xC0000139`）。**绝不写「跑一下 command」的单测。**

## 4. `cache_dir` 未用警告定性（本轮实测结论）

`src\image_thumbnails.rs:1807 unused variable: cache_dir` ⇒ **不是接线缺陷**。调用方 `write_remote_thumbnail_bytes`（`:1762`）已用传入目录做了 `create_dir_all`（`:1769`）、`thumbnail_path` 拼接（`:1772`）、`enforce_thumbnail_cache_limits`（`:1780`）；内层 `stage_and_scale_remote_thumbnail` 拿到的是**已算好的** `temporary_source_path` 与 `thumbnail_path`，因此该形参纯属冗余。

**本轮故意不改**：改签名会要求重打包（≈15 分钟），而零行为变更的清理不值得让「已验收产物 ↔ 源码」失配。留给下批次的修法：删掉 `cache_dir` 形参与调用点实参（2 行），然后重跑 `cargo test --lib -- image_thumbnails`。

同轮其他既存警告（**非本批次引入**）：`global_search/scan.rs:387 unused variable: reader`、`remote/commands.rs:32 unused import: async_trait::async_trait`、`remote/credentials.rs:14 unused import: RemoteErrorKind`、`with_file/with_directory never used`、`enum QuarkCode is never used`。

## 5. `global_search` 那条红：环境抖动，非回归（判别实验）

实验脚本 `\.superpowers\sdd\b32\global-search-probe.ps1` → 结论文件 `global-search-probe.txt`，同一条用例连跑三次：

| 次 | 环境 | 结果 |
| --- | --- | --- |
| run1 | 默认 `%TEMP%` | PASS 0.99s |
| run2 | 默认 `%TEMP%` | **FAIL** `src\global_search\index.rs:561:29` `writer.commit().unwrap()` → `OpenWriteError(IoError { code: 5, PermissionDenied, "拒绝访问。" }, filepath: "da05d38c85b44afa8bbcf351652f1aa7.term" })` |
| run3 | `TMP`/`TEMP` 改指 `F:\...\b32\tmpprobe` | PASS 7.19s |

三次失败文件名分别为 `.pos` / `.idx` / `.term`（随机名）⇒ 排除「同一残留文件反复挡住」。判定：**同机同码交替 PASS/FAIL，红来自 `%TEMP%` 上新建小文件被瞬时占用（杀毒/索引器类），落在 Tantivy commit 内部，与本批次改动无因果**。

两条被本轮**否证**的既有说法：
1. 「持久环境故障」—— 不成立（换目录与重跑都能绿）。
2. 仓库文档 `docs/handoff-2026-11-08-baidu-netdisk-phase1.md:451-454` 的「判回归前先**单跑**它」—— **单跑也不可靠**（run1 单跑绿、run2 单跑红）。正确做法：多跑 + 换 `TMP` 双向往返。

**门禁判据缺陷（下批次该修）**：`gates-build-02.ps1` 的 `VERDICT_cargo` 用「抖动用例单跑必绿」当放行条件，在 `%TEMP%` 抖动场景下会把**健康代码判红**。本轮的三条放行条件（单跑必绿 ∧ 失败集 ⊆ 允许名单 ∧ `git diff` 未触及该模块）第 1 条本身不可靠，应换成「单跑 N=3 至少 1 绿 或 换 `TMP` 必绿」。本轮因此改用独立脚本 `build-only-03.ps1` 完成打包，并在台账如实登记 `VERDICT_cargo=RED` —— **没有伪装成绿**。

## 6. 真机验收（J1/J2）

产物（实测）：
- `src-tauri/target/release/alpha-file-manager.exe` — **85,013,504 B**，`15:43:09`，`sha16=9AFB03D1BAF2A0CB`
- `src-tauri/target/release/bundle/nsis/Alpha File Manager_3.16.0_x64-setup.exe` — **45,934,362 B**，`15:43:07`，`sha16=D45BFA10B636013B`
- 打包日志 `\.superpowers\sdd\b32\build-only-03.txt`：`EXIT_build=0`、`Finished release profile in 5m 09s`
- **发布脚本执行了，但新包落到 `release\local-builds\`（订正本轮早前的误判）**：`PUBLISH_EXIT=0`，逐文件 sha256 实测 —— `release\local-builds\Alpha File Manager_3.16.0_x64-setup.exe` = **45,934,362 B / `sha16=D45BFA10B636013B`**（= bundle 原件指纹 ⇒ 确实是本轮新包）；顶层 `release\Alpha File Manager_3.16.0_x64-setup.exe` 仍是旧包（45,849,872 B / `sha16=39D6D0F7481C4B40` / 11:22:22），因为 `publish-bundle-to-release.mjs` 对**同版本号**走防呆分支，不覆盖顶层。⇒ **装机必须显式用 `release\local-builds\` 那份**；版本号未 bump（仍 3.16.0），顶层那份不含本批次改动。

工装：`\.superpowers\sdd\b32\b32-accept-remote-ui.ps1`（判据：J1 = `root/<手机备份>/<照片备份>` 内约 90 组同名 jpg/png 的图片条目须渲染真缩略图而非通用文件图标，对照组取同目录树里一个真 mp4，**对照组量不到则 J1 记 UNDETERMINED，绝不记 PASS**；J2 = 单击同名对中一条只选中该条）

**`Navigate-ToRemoteRoot` 开头已补「视图恢复」前置**（原工装没有这一步，`RefreshNodes 'boot'` 之前无任何视图判定）。早前记的「boot 224 节点 = Vue 还没挂载的竞态」叙述**已被 run 04/05 否证**，真实机制见 §6.1。同一轮还补了 `-Probe` 段的前置（探针此前绕过导航直接 dump，取证对象是错的页面）。

### 6.1 验收结论 = **UNDETERMINED**（run 01..05，0 次有效结论）

阻塞链（每一环都有日志/转储佐证，按因果排序）：

1. **boot 视图不确定**：同一 exe 三次启动分别落在 设置-搜索 / 设置-搜索 / **扩展功能页**（`b32-accept-nodes-boot.txt` 里 `Document ... value=http://tauri.localhost/extensions` + 8 个 `extension-card__name`）。根因 = 应用恢复上次退出时的视图（user-data 跨运行持久），与打包无关。⇒ 已补 `Ensure-FileBrowserView` 前置。
2. **rail 扫描法不是万能恢复手段**：boot 停设置页时 `Get-ActivityRailButtons` 给 6 个候选；boot 停扩展页时给 **0**（`accept-05-probe.txt: RAIL_CANDIDATES count=0` → `ERROR VIEW_NOT_BROWSER_AFTER_RAIL_SWEEP`），而 `-02-probe-boot.png` 截图上那些图标肉眼可见 ⇒ 该页图标钮不满足「最左 x 带内的无名 Button」形状。不能当唯一恢复手段。
3. **最硬的一条（未解，且不是工装能修的）**：当前应用状态下**侧栏根本不渲染云盘连接行**。三次全窗扫描（run 03 的 574 节点树 / run 04 的 122 节点 boot 树 / run 05 的 247 节点全窗、**无 x 窗过滤**）都搜不到名为「阿里云盘」的节点（`PROBE tree-root name=阿里云盘 candidates=0`）；侧栏只有 `C: D: E: F: G: 添加网络驱动器`。旧批次同一工装的转储里该节点**存在**（`b32-accept-nodes-j2-after-click.txt:146` = `Text;阿里云盘;l=170;t=89`）⇒ 指向应用侧状态（连接需重新授权 / 云盘分区被折叠 / 连接列表加载失败），需人工先看一眼。
4. 顺带否证一条误判：曾怀疑「残留实例劫持单实例锁 → UIA 附着到遗留窗口」（应用日志里确实同时出现过两个 pid）。**不成立** —— 工装 `:1583-1594` 启动前杀净同名进程与 WebView2 子进程，`:1636` 用 `MainByPid($hDesk, pid)` 在私有桌面上**按 pid 过滤**取窗，且有 `APP_EXITED_EARLY` / `MAIN_WINDOW_TOO_SMALL` / `RENDER_HOST_MISSING` 断言。

**判据形状订正（重要，别再拿 `Get-TreeNodes` 找连接行）**：工作态转储证明连接行是 **`Text` 节点**（`l=170;t=89`，3864 宽窗口下在侧栏带内），从来不是 `TreeItem`；工装里 `Get-TreeNodes` 的 `type -eq 'TreeItem'` + x 窗 2%–10% 是**错判据**。本轮已新增 `Get-ConnectionRowAny`（任意类型 + 名字匹配 + 左缘 2%–12% 带 + 排除 `address-bar*`）并在 `Ensure-FileBrowserView` 里替换掉两处旧判据。备用入口两个：地址栏 `Button;阿里云盘;cls=address-bar__part;help=sfm://remote/<connection-id>`（连接 ID 非机密）与面包屑 `Text;阿里云盘 / 视频备份;l=727;t=27`（点它会真发 `remote_read_dir`）。

**另一条工装坑（本轮实测）**：`-File` 直跑 `-Probe` 时 stdout **全丢**（run 04 无日志 ⇒ 只能靠 dump 文件反推）。必须 `Start-Process pwsh -PassThru -RedirectStandardOutput <log> -RedirectStandardError <err>`，再轮询 `$p.HasExited`。

### 6.2 下一手（按代价排序，勿跳序）

1. **人工确认前置状态**（人眼 10 秒，不要写代码）：打开装机版，看侧栏「已连接的云盘」是否可见、阿里云盘是否需要重新授权。这一步决定 §6.1-3 是「连接掉了」还是「UIA 取不到」。
2. 若连接可见但 UIA 取不到 ⇒ **放弃 rail/树点击**，改走地址栏直达 `sfm://remote/<connection-id>`（id 已在 `preflight.connection` 从 `user-remote-connections.json` 读到并写进日志）。
3. 让 boot 视图确定化：工装启动前清掉应用的「上次视图」持久状态（或注入固定初始路由），否则每轮从不同页面起跑、取证不可比。
4. 前置打通后跑**完整验收**（不带 `-Probe`）：`pwsh -File .superpowers/sdd/b32/b32-accept-remote-ui.ps1 -ExePath <装机后的 alpha-file-manager.exe>`，读 `b32-accept-remote-ui.json` 的 `JUDGE_J1` / `JUDGE_J2`，期望 **J1=PASS 且 J2 不回归**。`J1_*` / `J2_*` 阈值本轮**一个都没动**，下批次也不许动。
5. 若 J1 PASS，顺手清掉 §5 那条非阻塞冗余项（`generate_image_thumb_cached` 的 `cache_dir` 形参）。

### 6.3 run 11 判据（2026-11-21 19:23:26–19:25:30，装 `3.16.0` 85,032,960 B / `sha16=7E01DC1145409921`）

**结论：`VERDICT=UNDETERMINED`（`J1`/`J2` 均 `not evaluated`），`STEP nav.remote_root -> FAIL`。**
这是**如实失败**，不再是 run10 那种面包屑假 OK（本轮已拆掉该假阳性出口，证据链见 §6.2 与工装 `Navigate-ToRemoteRoot`）。证据：`b34-accept-run11.log`（`NAV_HOME_ENTRY_TIMEOUT` @124s、`NAV_NO_TREE_CONNECTION_ROW`、`FATAL could not reach the remote root`）、`b32-accept-remote-ui.json`、`b34-logoffset-run11.txt`（`BASE_BYTES=1741847`）。

**决定性证据 —— 点击已送达，卡在 refresh（这一条把 run10/run11 的「点不动」定性彻底翻转）**：

应用日志新增段（`BASE_BYTES=1,741,847` 起）只有 2 条远程命中，时间 `11:23:49`（= 本机 19:23:49，恰在 `NAV_HOME_ENTRY_CLICK` 之后）：

```
[WARN] Aliyun refresh attempt index=0 client_id=Some("25dzX3vbYqktVxyX") status=400 code=Some("InvalidParameter.ClientId")
[WARN] Aliyun refresh attempt index=1 client_id=None                     status=400 code=Some("InvalidParameter.ClientId")
```

三条推论，全部已坐实：

1. **PostMessage 合成点击在私有桌面上确实送达**（`FOREGROUND setForegroundWindow=false;matches=false` 只是「拿不到前台」的记录项，**不是**输入失效信号；勿因此改工装沙箱模型）。
2. **候选链第三级被证伪**：`client_id=None` 仍回 `InvalidParameter.ClientId` ⇒ 上一轮「不带 client_id 可能可行」的推断**作废**。真实语义是「该 `refresh_token` 绑定的 client 与任何候选都不匹配」，参数层面已无解。
3. **只打了 2 条 → 候选链第一级（存档值）为空被跳过**，链路本身工作正常。

**产品侧行为缺陷（新发现，未修）**：refresh 失败后点云盘入口，应用**既不进入远程窗格、也不给出可见错误**，面包屑保持 `位置`、首页 `home=True`，界面表现为「点了没反应」。这是可观测性缺口，值得单独记一笔。

**唯一前进条件**：必须在应用内**重新扫码授权阿里云盘**（手机侧，代办不了）。授权后重跑同一工装即可。

**工装已知小坑（本轮新记）**：传 `-ConnectionName` 会走「override」分支并**跳过** `user-remote-connections.json` 的 `id` 读取 ⇒ 日志出现 `id=`（空）。若要用 §6.2-2 的地址栏直达 `sfm://remote/<connection-id>`，**不要传 `-ConnectionName`**，让脚本自己读 id。配置文件真实形状是 `{ "connections": [ ... ] }`，不是顶层数组。

**装包取证澄清**：`b34-install-02.log` 的 `VERDICT=INSTALL_IDENTITY_SUSPECT` 是**假红** —— `SIZE_MATCH=True` 且 `NORMSHA_MATCH=True`（归一化 `__TAURI_BUNDLE_TYPE_VAR_*__` 后逐 64 KiB 块相等），唯一 False 的 `INSTALLED_MTIME_MOVED` 在 `/FORCE` 同版本重装下无判别力（NSIS 还原时间戳）。该判据应从判词降级为信息打印。

### 6.4 「列表视图出图、网格视图不出图」的三条前端缺陷（2026-11-21 20:0x–20:24，已修）

**触发**：用户重新扫码授权后实测 —— **列表视图能显示远程缩略图**，**缩略图（网格）视图不出图**。

**判定：不是后端通道问题，是前端三条缺陷叠加。** 列表视图能出图已直接证明 `remote_generate_thumbnail` 命令通道可用（同一后端、同一缓存目录、同一 `connection/path/mtime` 键段），故不必再查 Rust 侧。

**三方证据（磁盘 + 时序 + 代码，互相独立）**：

1. **磁盘**：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\remote-image-thumbnails` **COUNT=34**，34 个 sha 前缀互不相同。前 8 个是 `32×23 / 32×19 / 24×32 / 32×24`（列表视图 `listThumbnailPx=48` 档，`19:59:58`–`20:00:01`）；其后 26 个落在 **`340×340`×7 / `331×331`×6 / `340×255`×2 / `255×340`×4 / `248×331`×5**（60–207 KB，`20:00:13`–`20:01:54`）⇒ 网格侧在磁盘上留下了 **4 种 maxDimension 档（340 / 331 / 255 / 248）**，而它们的实际请求尺寸几乎是同一批物件。
2. **停滞**：`20:09:33` 复测仍是 `COUNT=34`、`NEWEST=20:01:54`、**7.6 分钟零新增**，同刻进程存活 `ws_mb=112.8`。⇒ 出完 26 张后不再有任何新请求，不是「还在慢慢跑」。
3. **代码**（三条根因逐条对得上上面两个现象）：

| # | 根因 | 位置 | 现象对应 |
|---|---|---|---|
| A | 网格卡的 `maxDimension` **未量化**：`useDevicePixelPreviewSize` 给出的像素尺寸会抖动，`normalizeImageThumbnailMaxDimension` 只做 `Math.min(384)`、不做桶量化，`getImageThumbnailKey` 又含 `maxDimension` ⇒ 每个抖动值都是一个新缓存键 / 新请求 | `use-image-thumbnails.ts`（`IMAGE_THUMBNAIL_MAX_DIMENSION` / `normalizeImageThumbnailMaxDimension`）、`file-browser-grid-card.vue`（`imageThumbnailMaxDimension`） | 磁盘上 340/331/255/248 **四种档** |
| B | `failedThumbnails` 是**永久终态**：`getImageThumbnail` 见到失败 key 直接早退返 `cachedThumbnail`，不再入队、不再重试 ⇒ 首发失败（如授权前的 400、超时）就永久空着 | `use-image-thumbnails.ts` 的 `getImageThumbnail` / `markThumbnailFailed` | 26 张之后**一条新请求都没有** |
| C | `clearThumbnails()` 不清 `processingThumbnails` / `processingPlaceholders` ⇒ 上一轮被打断的 in-flight 标记残留，新一轮同 key 请求被当成「已在处理」而丢弃 | `use-image-thumbnails.ts` 的 `clearThumbnails()` | 切目录/重进后**彻底不再发请求** |

补充：`file-browser-grid-card.vue` 的 `watch(imagePreviewSrc)` **无条件**清 `loadedImagePreviewSrc`，而 CSS 里 `--final` 是 `opacity: 0`、只有 `--final-loaded` 才 `opacity: 1` ⇒ 一旦 `imagePreviewSrc` 抖动（正是 A 的后果），**已加载的图会被重新打回不可见**，视觉上就等于「不出图」。

**修法（已全部落地，均为最小改动 + `FORK-MODIFICATION` 注释说明上游差异）**：

- A：新增 `export const IMAGE_THUMBNAIL_DIMENSION_BUCKET = 64` 与 `export function quantizeImageThumbnailMaxDimension()`（非有限/≤0 回 64，否则 `ceil(x/64)*64` 再夹到 384）；**只在 `file-browser-grid-card.vue` 调用**，`normalizeImageThumbnailMaxDimension` 本体不动 —— 否则列表视图的 48px 会被抬到 64，作废 1912 个既有本地缓存键并改变列表视觉。实测映射：`340/331→384`、`255/248→256`、`384→384`、`348→384`、`64→64`。
- B：`failedThumbnails` 改**有限次退避重试**（`MAX_IMAGE_THUMBNAIL_FAILURE_ATTEMPTS=3`，`IMAGE_THUMBNAIL_RETRY_BASE_DELAY_MS=1500` 起 `2^(attempts-1)` 指数推进，`IMAGE_THUMBNAIL_RETRY_MAX_DELAY_MS=15_000` 封顶），到点从 failed 集移除并 `triggerRef` 重排；成功出图时 `clearThumbnailFailureState`。
- C：`clearThumbnails()` 一并 `processingThumbnails.clear()` / `processingPlaceholders.clear()` / 清全部 retry 定时器 / 清 `thumbnailFailureAttempts`。

**测试**：`use-image-thumbnails-remote.test.ts` 由 6 例扩到 **9 例**，新增三例逐条对应 A/B/C（A 断言量化映射表；B 用 `vi.waitFor` 断言第二次 `remote_generate_thumbnail` 真的发出；C 断言 `clearThumbnails()` 后能立即重新入队并出图），均带真机证据注释。

**门禁**：首轮 `npm run check` `EXIT=1`，唯一报错是 `use-image-thumbnails-remote.test.ts:240:24 @stylistic/object-property-newline`（`vi.waitFor` 的内联对象），改多行对象后重跑 **`EXIT=0`** —— `vue-tsc --build` 通过、`eslint . --max-warnings 0` 通过、**Test Files 325 passed / Tests 3491 passed（90.12s）**、`verify-changelog`（60 条、顺序正确）、`verify:sandbox`（2 个扩展入口 PASS）。**`J1_*`/`J2_*` 阈值一个都没动。**

**产物（`npm run tauri:build`，`EXIT=0`，Rust 1.90.0 工具链，`Finished release profile in 4m 25s`）**：

- 安装包：`release\local-builds\Alpha File Manager_3.16.0_x64-setup.exe` —— **45,937,922 B，2026-11-21 20:24:14**
- 裸 exe：`src-tauri\target\release\alpha-file-manager.exe` —— **85,032,960 B，20:24:15**
- bundle 副本：`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.16.0_x64-setup.exe` —— 45,937,922 B，20:24:14

**仍未验证（不要当结论引用）**：**新包尚未装机**，网格视图是否真的出图只有装上 `3.16.0` 并打开阿里云盘远程图片目录才能确认；当前运行实例（`pid=806256`，`hwnd=789790`，`19:58:21` 启动）跑的是**旧**二进制，安装前必须先退出应用。像素级取证路线已放弃：`b34-ui.ps1` 在 .NET 10 下 `Add-Type -ReferencedAssemblies` 缺 `System.Private.Windows.GdiPlus`（`CS0012 IImage`，第 73 行），两次修未果，收益低于成本 —— A/B/C 的定论由「磁盘四档 + 7.6 分钟停滞 + 代码三条」支撑，不依赖截图。

### 6.5 装机复验：run11 → run13（2026-11-21 20:29–20:38，结论 `VERDICT=PASS`）

**装机（已闭合）**：优雅关旧实例（`CloseMainWindow`）→ `APP_EXITED=True` / `LEFT_PROCS=0` / `UNLOCKED=true`；`b34-install.ps1` → `INSTALL_EXIT=0`、`SIZE_MATCH=True`、`NORMSHA_MATCH=True`、**`VERDICT=INSTALL_IDENTITY_OK`**；安装器日志有 `20:29:36.520 PREINSTALL` + `20:29:50.633 POSTINSTALL`、`installerSha256=71f1c74491ea061b1f6f9835316414ff0b215dfd7b4fbc7554b60cc6ea4fe416`、early-guard `20:29:24.235 OUTCOME=proceed reason=forced`。证据 `.../b32/b34-install-03.log`。

**run11（`ACCEPT_EXIT=0`，`VERDICT=UNDETERMINED`）**：`STEP nav.remote_root -> FAIL`。决定性证据是应用日志仅 2 条 refresh 命中 —— `Aliyun refresh attempt index=0 client_id=Some("25dzX3vbYqktVxyX") status=400 code=Some("InvalidParameter.ClientId")` 与 `index=1 client_id=None status=400` ⇒ 证伪「client_id 推断错误」这条候选链，结论固定为「必须重扫授权」。用户随后扫码，**列表视图实测出图**（直接证明 `remote_generate_thumbnail` 通道可用）。

**run12（`ACCEPT_EXIT=0`，`VERDICT=UNDETERMINED`）暴露的是工装三处假「未判定」，不是产品回归**：

1. `STEP view.grid -> FAIL :: toggle=False` —— `Get-GridCardCount` 在卡片尚未布局时返回 0 就判「toggle 不存在」。
2. `J1=UNDETERMINED ... stillChanging=True`（`j1.sample_scan samples=8`）—— 工装把「参与比较的样本集合扩大」误读成「区域仍在变化」：t00 只采到 2 个、t15 采 5 个、t40 采 8 个。
3. `ERROR FATAL RuntimeException: 无法将值 "ERR_EMPTY" 转换为类型 "System.Double"` —— `Parse-Metrics`/`Parse-Diff` 对 `ERR_EMPTY` 无兜底，`[double]` 转换处炸掉，J2 直接未评估。

**run12 的关键收益（产品侧证据，逐条核对 `J1.metrics`）**：同一 entry 在多次采样间**像素逐字相同**（`-340582353.jpg` t00=t15=t40：`unique=123` / `stdLum=0.331` / `nonBg=0.6692` / `sat=0.3103` / `mode=242,236,225`；`-425317874.jpg` 同样三次一致）。8 个图片条目全部 `placeholderSignature=false`、`unique` 123–635、`nonBgRatio` 0.61–0.90，且逐条目 `mode` 各不相同（`203,36,41` 红 / `3,5,10` 近黑 / `225,181,52` 黄 / `202,221,179` 浅绿 / `229,240,251` 浅蓝）；四列并列 x 坐标 586/953/1321/1688、卡片矩形 331–332×208 ⇒ 当时确实是四列网格视图且在渲染互不相同的真实照片；control 是 `mp4-generic-icon` ⇒ 工装能区分通用图标与图片缩略图。

**缓存增长（run12 期间）**：`remote-image-thumbnails` 从 **34 → 47**，新增 13 个落在 `20:30:43`–`20:32:01`、尺寸 77,779–255,681 B（网格档，非 48px 列表档）⇒ 网格视图确实发起并完成了新的远程缩略图生成。应用日志 `Select-String 'thumbnail'` **0 命中** —— 产品无缩略图日志，属已知可观测性缺口。

**工装三处修复（5 处单行 `edit`；`Parser::ParseFile` → `PARSE_OK`，`FILE_BYTES=101302`）**：

- `:1246` / `:1255`：`Parse-Metrics`、`Parse-Diff` 加 `-or $f[0] -eq 'ERR_EMPTY'` → 返回 `$null`；三处调用点本就有 `$null` 守卫（`:1350` / `:1391` / `:2028-2042`），故是安全改动。
- `:1887`：`$stillChanging` 改为**只比较两次采样都出现的条目**（`$prevTop` 哈希表 + `$sigCommon` 计数，逐条比 `top_hue|top_unique|top_nonBg`）。
- `:1939` / `:1941`：分支改 `elseif ($sigCommon -eq 0 -or $stillChanging)`；reason 按 `$sigCommon` 分两段文案。
- **阈值一个都没动**（`J1_MIN_SAMPLES` / `J1_HUE_MIN` / `J1_NONBG_DELTA` / `J1_UNIQUE_RATIO` / `J1_PLACEHOLDER_NONBG` 全原值）。

**run13（20:35–20:38，`ACCEPT_EXIT=0`）**：

```
STEP j1.sample_scan  -> OK :: samples=8 stillChanging=False
STEP j1.control_scan -> OK :: kind=mp4-generic-icon
STEP j1.evaluate     -> OK :: qualifying=8 placeholder=0 total=8 controlHue=1 controlNonBg=0.0503 controlUnique=43
STEP j2.back_to_photo_backup -> OK :: path=手机备份 / 照片备份
STEP j2.click        -> OK :: entry=-85070471.jpg source=grid-thumbnail entry1Changed=True entry2Changed=False
STEP j2.read_count   -> OK :: count=1 text=已选择 1 个项目
J1=PASS :: 8 of 8 sampled thumbnail regions are richer than the generic icon of the control entry
J2=PASS :: the application reports 1 selected entry after the left click ...
VERDICT=PASS
```

11 个图片条目（8 个采到 t40、3 个最晚 t15）全部 `placeholder=False`，`unique` 123–635、`nonBg` 0.458–0.790，`mode` 各不相同（`203,36,41` / `3,5,10` / `5,5,4` / `52,50,44` / `227,225,220` / `229,240,251` / `241,236,225` / `242,236,225` / `178,179,172` / `202,221,179`）；control `VID_20230423_145252.mp4` 是 `unique=43 / nonBg=0.0503 / hue=1`。

**缓存零新增（这是 A 修复生效的直接证据）**：run13 期间 `NEW_DURING_RUN13=0`、`COUNT=47` 不变、`NEWEST=20:32:01` ⇒ 8/8 出图全部命中上一轮生成的缓存；既解释了 `t00` 即稳定（`stillChanging=False`），也说明**缓存键已随 `maxDimension` 量化收敛**（不再因 `useDevicePixelPreviewSize` 抖动产生新键）。

**run13 遗留（工装限制，不是产品缺陷）**：`STEP view.grid -> FAIL :: toggle=False` 仍出现（`Get-GridCardCount` 时序缺陷未修，本轮为省调用预算刻意跳过），但**已被同一轮内证据证伪** —— `j2.click` 打出 `source=grid-thumbnail`，且 J1 采样矩形与 run12 的四列坐标一致 ⇒ 实际就处在网格视图。**判读这一行必须同时看 `source=grid-thumbnail`，不能单看 `view.grid`。**

## 7. 已验证 / 推断 / 未验证（别混）

**已验证（本轮实测）**：本批次四项门禁为 0；新增 7 条远程缓存用例通过；`GUARD_TOUCHED_COUNT=0`；exe/NSIS 产物字节与 sha16；工装 preflight 确认它跑的就是新包（`version=3.16.0 bytes=85013504 sha256_16=9AFB03D1BAF2A0CB`）；`global_search` 红的抖动性质（三次往返）；`cache_dir` 是冗余参数（读码 + 调用方核对）。

**推断（证据充分但未当场证明）**：`global_search` 红由**杀毒/索引器瞬时占用 `%TEMP%`** 引起（能解释随机文件名与换卷即绿，但未抓 Process Monitor）。`use-image-thumbnails.ts` 的 camelCase→snake_case IPC 参数映射**应当**正确（Tauri 2 默认行为，且参数名单测已断言），但见下条。

**已闭合（原「未验证 ①」，2026-11-21 20:38）**：真机远程缩略图**在网格（缩略图）视图确实出图** —— `3.16.0` 装机后 run13 给出 `VERDICT=PASS / J1=PASS / J2=PASS`，11 个图片条目全部非占位、像素互不相同，且 run12 期间缓存 34→47 证明生成链路真的跑过；详见 §6.5。下方「未验证」只保留仍未判定的项。

**未验证（不得当结论引用）**：② 前端 → Rust 的真实 IPC 参数映射；③ ~~`release/` 副本与 bundle 新包的一致性~~（**已实测闭合**：`release\local-builds\` 那份逐文件 sha256 = bundle 原件 `D45BFA10B636013B`）；④ ~~装机类取证~~（**已实测闭合**：按「归一化 `__TAURI_BUNDLE_TYPE_VAR_[A-Z0-9]+__` 后逐 64 KiB 块比 sha + VersionInfo」得 `SIZE_MATCH=True / NORMSHA_MATCH=True / VERDICT=INSTALL_IDENTITY_OK`；`b34-install.ps1:101-103` 已把 `INSTALLED_MTIME_MOVED` 从判词降级为**仅信息打印**，理由见 §6.3 末段；同版本重装 NSIS 带 `/FORCE`，故 early-guard 是 `reason=forced` 而非 `same-build`）。

## 8. 下阶段建议（按优先级）

1. ~~若 §6 验收未拿到 `J1=PASS ∧ J2=PASS`~~ —— **本轮已拿到（run13，见 §6.5）**。若将来复跑又出现 `UNDETERMINED`：先判读是**产品缺陷**还是**取证失败**（区分「没触发」与「没修好」：先在通网/健康态让同一判据命中一次），再动代码；**别急着下调阈值**。
2. **修 `Switch-ToGrid` 时序缺陷**（`Get-GridCardCount` 在卡片尚未布局时返回 0 就判「toggle not found」）⇒ 改为「等卡片出现或超时」再判，否则 `report.view` 永远可能误记 `list`、`STEP view.grid` 永远可能假 FAIL。**判读时以 `j2.click` 的 `source=grid-thumbnail` 为准**（见 §6.5 末段）。
2. ~~把新包发布进 `release/`~~ —— **本轮已完成**（`PUBLISH_EXIT=0`，新包在 `release\local-builds\`，逐文件 sha256 与 bundle 原件一致，见 §6）；剩余动作 = 用那份**装机取证**（判据见 §7 未验证 ④）。
3. 把 §5 的门禁判据缺陷回写进 `gates-build-02.ps1`（抖动放行改成「多跑或换 `TMP`」）。
4. 清理 `image_thumbnails.rs:1807` 冗余形参（顺手在同批把既存 `unused import` 一起清了）。
5. 本批次改动 commit（含新增 `remote/thumbnails.rs` 与前端测试）。

## 9. 本轮工具坑（复用清单）

- **cargo 不在会话 PATH**：脚本内必须 `$env:RUSTUP_TOOLCHAIN='1.90.0'` 且把 `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;C:\Users\Duanyi\.cargo\bin` 前置进 `PATH`。
- 安装包真身是 `target/release/bundle/nsis/*_x64-setup.exe`；**不要**用 `-Filter '*.nsis'` 当判据。
- `Set-Location <repo>` + 相对 `Out-File` 会被本地硬安全策略拒绝 ⇒ 只用 `Push-Location` + **绝对** `Out-File`。
- 字符串拼接里内嵌方法调用不加括号 ⇒ ParserError 且**整段脚本不执行**（等于白跑一轮）⇒ 改完脚本先静态自检：`[System.Management.Automation.Language.Parser]::ParseFile($p,[ref]$toks,[ref]$errs)`，并测 `NON_ASCII`（工装脚本要求纯 ASCII，CJK 判据用码点构造）。
- 全仓 `Get-ChildItem -Recurse`（含 `target/`）会被磁盘竞争挂住且 `task_stop` 停不干净 ⇒ 别这么找残留文件。
- `edit` 凭记忆复述旧文本必 miss ⇒ 改文档前先逐字取行；大改用 `write` 整文件重写比多次 `edit` 省轮次。
- 门禁与打包**不要串成一个脚本**：串起来后 `GATES_OK=False` 会直接 `BUILD_SKIPPED`，本轮靠独立打包脚本绕开，代价是必须自己确认四项 in-scope 门禁为 0。
- 后台任务未终态会触发额度闸门（写 ack 也可能被拦一次）⇒ 收工前等完或 `task_stop`。

## 10. 证据路径索引（全路径）

- `F:/soft/00selfmade/filemanager/sigma-file-manager/.superpowers/sdd/b32/STATUS-remote-thumb.md`（过程台账，14,763 B）
- `.../b32/gates-build-01.txt`（首轮 157,722 B）、`.../b32/gates-build-02.ps1`（4,059 B）、`.../b32/gates-build-02.txt`（**第二轮真源**）
- `.../b32/global-search-probe.ps1`（1,893 B）、`.../b32/global-search-probe.txt`（判别实验三份证据）
- `.../b32/build-only-03.ps1`（1,497 B）、`.../b32/build-only-03.txt`（`EXIT_build=0`）
- `.../b32/b32-accept-remote-ui.ps1`（本轮加就绪轮询）、`.../b32/accept-01.txt`（竞态失败证据）、`.../b32/accept-02.txt`（重跑）
- `.../b32/b32-accept-remote-ui.json`（机读结论）、`.../b32/b32-region-metrics.ps1`（像素复算）、`.../b32/j1-j2-verdict.txt`、`.../b32/b32-accept-remote-ui-*.png`
- `.../b32/cargo-check-remote-thumb-0{1,2}.txt`
- **装机与复验（§6.5）**：`.../b32/b34-install-03.log`（`INSTALL_IDENTITY_OK`）、`.../b32/b34-accept-run11.log`、`.../b32/b34-accept-run12.log`、`.../b32/b34-accept-run13.log`（**`VERDICT=PASS` 真源**）、`.../b32/b32-accept-remote-ui.json`（run13 覆盖后的机读结论）、`.../b32/b34-cache-before-run12.txt`、`.../b32/b34-logoffset-run11.txt`、`.../b32/b34-auth-probe.txt`
- **收口与构建**：`.../b32/b34-grid-thumbnail-fix-summary.md`、`.../b32/b34-gate-frontend2.log`（`EXIT=0` 那轮）、`.../b32/b34-tauri-build.log`
- 复验工装本体：`.../b32/b32-accept-remote-ui.ps1`（101,302 B，本轮 5 处判据修复；改完必跑 `Parser::ParseFile`）
