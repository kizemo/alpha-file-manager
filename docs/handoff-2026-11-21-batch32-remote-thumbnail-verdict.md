# 交接 · 批次 32 收尾 → 批次 33（远程缩略图真机定案 + 修复方案）

> 生成时间：2026-11-21 ｜ 分支 `main`，HEAD `26cbfb03`（与 origin 同步，工作树干净）
> 上一份交接：`docs/handoff-2026-11-21-batch31-aliyun-write-ops.md`（批次 31 阿里云写操作）
> 本份覆盖：批次 32 的真机验收**定案结论**、J1 根因、以及**已定但尚未开工**的修复方案。

---

## 1. 背景

用户诉求（批次 32 起点，原文）：

1. **云盘图片要能显示缩略图**；
2. **云盘文件左键单选会同时选中两个文件**，要修；
3. 去掉左侧导航「远程」悬停面板里网盘分区那一行添加按钮；
4. 暂时不执行上传；
5. 评估云盘文件（百度/夸克/阿里）能否建索引，为后续**文件查重**做准备。

批次 32 已提交：`bc48f7e3`（代码 + 单测）、`5018804f`（文档）、`29143eb7`（移除导航按钮）、`26cbfb03`（CHANGELOG 补记）。
门禁全绿：`npm run ts`=0、`npm run lint:check`=0、全量 vitest 0 fail、`npm run verify-changelog`=0。
打包产物 3.16.0（**含全部四项改动**）：

- 裸 exe `F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\alpha-file-manager.exe`
  85,103,616 B，sha256 前 16 位 `237689E29E8FE79B`
- 安装包 `...\release\local-builds\Alpha File Manager_3.16.0_x64-setup.exe`
  45,849,872 B，sha256 前 16 位 `39D6D0F7481C4B40`
- 构建日志 `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b32\build-b32c.log`（尾 `BUILD_EXIT=0`）

---

## 2. 真机验收结果（本轮定案，两个问题一好一坏）

完整报告：`.superpowers\sdd\b32\b32-accept-remote-ui.md`（含判据设计、三条自证、坑）
机器可读结论：`.superpowers\sdd\b32\b32-accept-remote-ui.json`

| 判据 | 结论 | 一句话依据 |
|---|---|---|
| **J1** 阿里云远程图片缩略图 | **FAIL** | 8 张抽样图片卡片的缩略图区全是平涂 `rgb(26,26,26)`（unique 3~4 / nonBgRatio 0.02 / stdLum 0.013），三次采样（立即 / +15s / +40s）逐字相同 ⇒ 终态就是无图 |
| **J2** 同名文件左键单选 | **PASS** | 同名对 `-1506381013.jpg` 单击一次：被点卡 `changedRatio=0.6254 / meanAbsDiff=36.799`，另一张 `0.0302 / 2.682`（均低于 0.05 / 3.0 阈值）；且应用自报「已选择 1 个项目」（右键菜单头，count=1） |

J1 的**对照自证**（证明量测不是瞎子）：同一套量测在视频目录通用图标 `VID_20230423_145252.mp4`
上得到 unique=44 / nonBgRatio=0.09 / stdLum=0.051 ⇒ 判据看得见细节。图片卡的 unique（3~4）
比通用图标（44）还贫瘠，不存在"真照片恰好长这样"。
J2 的**阳性自证**：同一对卡片的「另一张」，在**未点击前**的 t15→t40 基线段 diff 只有 0.0015 / 0.120。

关键证据文件（全部在 `.superpowers\sdd\b32\`）：

- 像素与几何：`b32-region-metrics.json`（J1 全部候选 + J2 前后对比）、`b32-region-metrics.ps1`（离线复算脚本）
- 截图：`b32-accept-*-0[5-9]*.png` / `-1[0-2]*.png`；目录级节点 dump：`b32-accept-nodes-<tag>.txt`（含 class/help，可 grep 中文）
- 运行日志：`b32-accept-run4.log`（终轮，退出码 0，`SANDBOX_CLEANED=true`）、`run3` / `run2` / `probe-run`

**验收工装本身已修好可用**（上一轮整轮废在导航步骤）：
`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b32\b32-accept-remote-ui.ps1`
静态解析 `PARSER_ERRORS=0`，调用
`& <script> -ExePath <exe> -TimeoutSec 240`。

---

## 3. J1 根因（已坐实，不要再重复调研）

**结论：批次 32 的「直链」方案在架构上不可能成功。不是 bug，是设计选错了。**

三条独立证据：

1. **Referer 约束（硬墙）** —— `.superpowers\sdd\b28-api-probe.md` §B 实测：
   阿里 OSS 预签名 URL 必须带 `Referer: https://www.alipan.com/` 才 200；
   少了回 400 `AuthorizationArgumentError`；加 `Authorization` 替代不了。
   而浏览器发 `<img src>` 请求时 **Referer 由页面自己决定**，必然是本地 Tauri 页的 origin ⇒ 不符。
2. **`thumbnail` 字段实测常为空串** —— 同上文档「仍未坐实的点」第 7 条。
   `file/list` 即使传了 `image_thumbnail_process`，响应里 `thumbnail` / `url` 仍可能为空。
3. **前端故障被放大成"平涂卡片"** ——
   `src/modules/navigator/components/file-browser/composables/use-image-thumbnails.ts:421`
   的 `shouldShowImageThumbnailFallback()` 对**远程条目返回 false**
   （单测 `composables/__tests__/use-image-thumbnails-remote.test.ts` 锁的就是这个行为），
   所以直链失败时**既不显示回退图标也不保留占位**，只剩卡片底色 —— 正是 J1 拍到的现象。
   另外 `getThumbnailSource()`（同文件 `:357`）远程分支不看 `entityType`，
   目录会被误判成有缩略图（`readRemoteEntityKind()` 在 `:341` 已有目录识别，但没接进去）。

代码路径确认无误（不是"没调用"）：
`remote_image_listing.rs:84 read_dir_once` → `read_dir_stream` → `provider.list_directory` →
`aliyun_provider.rs:1388 api_download_url` 的 `from_api_file_item` → `aliyun_api.rs:1606` 透传 `thumbnail_url` →
`remote_entries.rs:158 remote_entry_from_file_info` → `use-image-thumbnails.ts:64 getRemoteThumbnailUrl`。
前端也确实在发 `generate_image_thumbnail`（`remotePendingMap` 只收 `src === ''`，直链非空时远程条目不进该 map）。

---

## 4. 修复方案（已定稿，尚未开工）

**Rust 侧缩略图代理**：带 token + 正确 Referer 拉字节 → 本地解码缩放 → 落缓存 → 返回本地路径，
前端走与本地缩略图**完全相同**的渲染路径。

### 4.1 后端

新增命令 `remote_image_thumbnail(provider, connection_id, path, entry_id, thumbnail_url, file_size, max_dimension) -> Option<String>`

- 建议新建 `src-tauri/src/remote/thumbnails.rs`（`remote/mod.rs` 加 `pub mod thumbnails;`），
  或在 `src-tauri/src/image_thumbnails.rs:919` 旁边加；**不要**把 `tauri::` 写进
  `src-tauri/src/remote/aliyun_api.rs`（纯函数层铁律，见 `AGENTS.md` §2.8）。
- 在 `src-tauri/src/lib.rs:519` 附近的 `invoke_handler!` 注册。
- **降级链**：
  1. `thumbnail_url` 非空 ⇒ 直接 GET，请求头带 `Referer` = `src-tauri/src/remote/aliyun_api.rs:121`
     的 `ALIYUN_DOWNLOAD_REFERER`（**只取该常量，不要重新拼 URL**）；2xx 用，非 2xx 降级。
     其它 provider 无此头则不加。
  2. 调 `RemoteProvider::download_url`（`src-tauri/src/remote/provider.rs:172`），
     走现成带 401 刷新重试的实现 `aliyun_provider.rs:1388 api_download_url`，
     优先取 `download_url`，次选 `thumbnail_url`，再不行用 `file` 属性里的 `download_url`/`thumbnail`。
  3. 仍无 URL ⇒ 用 `RemoteProvider::download`（`provider.rs:176`）流式取字节到临时文件，设尺寸上限。
  4. 任一步失败 / 超限 / 解码失败 ⇒ 返回 `None`，不得 panic。
- 字节 → 缩放 → 写缓存**复用** `src-tauri/src/image_thumbnails.rs:737 generate_image_thumbnail_file`
  已有的解码、缩放与 `ThumbnailDecodeGate` / `MAX_*` 限制；缓存目录与本地缩略图**分开**。
- 缓存键 = `sha256(provider | connection_id | entry_id 或 path | modified_time | max_dimension)`，
  键冲突就换完整 64 位十六进制。

### 4.2 前端（同一批改完，别只改后端）

`src/modules/navigator/components/file-browser/composables/use-image-thumbnails.ts`

- `getThumbnailSource()`（`:357`）远程分支：有 `thumbnail_url` 也**不当作可直接渲染的成品**，
  改成走异步代理；并用 `readRemoteEntityKind()` 把目录排除掉。
- `shouldShowImageThumbnailFallback()`（`:421`）对远程条目返回 **true**（加载中/失败时显示通用图标，
  不要白屏平涂）；同步改写 `composables/__tests__/use-image-thumbnails-remote.test.ts` 里锁死旧行为的断言，
  并新增用例覆盖「远程条目加载中显示回退图标」。
- 远程 pending 分支加入代理调用（`:292` 的 `remotePendingMap` 逻辑已具备，只需换 invoke 目标）。

### 4.3 验收（必做，别只看代码）

1. 门禁：`npm run ts`、`npm run lint:check`、`npm run test:unit:run`（日志落文件）、
   后端 `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1`。
2. 打包：`npm run tauri:build`（**cargo 不在 PATH**，先设
   `$env:RUSTUP_TOOLCHAIN='1.90.0'` 并把
   `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;C:\Users\Duanyi\.cargo\bin`
   加进 PATH）。日志里 `Adding .sig` = 正常，不是失败。
3. 真机：跑 §2 那个工装，期望 **J1=PASS 且 J2 不回归**。
   工装已能列出远程目录，直接复用。

---

## 5. 本轮踩到的工装坑（下次写探针先看，省一轮）

1. **面包屑「含连接名就算已在根目录」是错的** ⇒ 深层目录被误判成根，导航根本没执行。
   正确判据：看面包屑链的**最后一段**是否等于连接名。
2. **候选过滤 `type -eq 'Button'` + 「y 在 band 之下 且 x ≥ 文件面板左边界」** ⇒
   左侧目录树条目（`Text` 类型、x≈40）被全灭，`OPEN_FOLDER_NO_CANDIDATE` 后 throw 退出。
   正确做法：目录导航走**左侧树**（`Text` 节点，按缩进 x 每级 +28px 定层级，点击后用完整面包屑校验）；
   面包屑件是 `Button`（class 含 `address-bar__part`）—— 这是区分两者的判据。
3. **`[B32X]::ExitDesktop` 里没有任何一个 P/Invoke 声明** ⇒ `TypeInitializationException`；
   而且 **`ExitDesktop` 和 `ExitWindowsDesktop` 都不存在于 user32.dll**，唯一正确释放方式是 `CloseDesktop`。
4. 网格渲染要等**节点数连续两次稳定**，固定 `Start-Sleep 2500` 会抓到半渲染态。
5. 卡片下半约 43% 是名称+大小的渐变遮罩带，**属 chrome 非图像内容**：量缩略图要取 `<img>` 上半部，
   否则 `nonBgRatio` 会被抬到 0.45 而假 PASS。
6. 布局状态不能靠 `Get-NodeByName 列表/详情/网格` 判（UIA 里根本没这两个 name）⇒ 用 grid-card 计数。
7. 中文节点名在 stdout 会乱码 ⇒ 落 UTF-8 文件再 `Select-String`。

---

## 6. 其它未闭合项

- **云盘索引 / 文件查重**：调研报告已出
  `.superpowers\sdd\b32\b32-cloud-index-feasibility.md`（22 KB，含 file:line 证据）。
  结论：唯一索引是 Tantivy 全文索引，schema 无 hash/provider/entry_id，远程条目从未入索引；
  首版应做「独立元数据镜像 + 元数据级查重」，内容级只有阿里能靠 `content_hash` 比对。
  **最小落地前置**：统一 `entryKey = provider + ':' + entry_id`（百度补 `fs_id`、夸克补 `fid`），
  并把远程写操作从按 `path` 改成按 `entry_id`（同名路径在阿里可重复）。
- **对外发布全部挂起**（用户明确"暂时不执行上传"）：站点 `www.aiec.fun` 未上传，GitHub release 未做。
- 验收报告里我自己登记的未判定项：J1 抽样 8 张全在同一目录同一批渲染，不覆盖"部分成功"；
  J2 只验了 1 组同名对；阿里深层目录（「照片备份」再往里）未重复验证。

---

## 7. 当前阻塞

派发给 worker 的 §4 实现任务在**开工前**就失败：

```
已达到 Token Plan 用量上限：请升级 Token Plan 套餐或购买积分补充用量。 (2056)
```

工作树干净（`git status --porcelain` 无输出），**没有任何半成品代码需要清理**。
额度恢复后，把本文件 §4 整段作为派工书即可直接开工。
